use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
mod domain;
use crate::domain::*;
mod dto;
use crate::dto::Error as dtoError;
use crate::dto::*;
mod ta;
use binance_spot_connector_rust::market;
use binance_spot_connector_rust::market::time;
use binance_spot_connector_rust::market_stream::ticker;
use binance_spot_connector_rust::market_stream::ticker::TickerStream;
use binance_spot_connector_rust::trade;
use binance_spot_connector_rust::trade::order::Side;
use binance_spot_connector_rust::{
    http::Credentials,
    hyper::{BinanceHttpClient, Error},
    market::klines::KlineInterval,
    market_stream::kline::KlineStream,
    tokio_tungstenite::BinanceWebSocketClient,
    wallet::{self, account_status},
};
use ta::*;

use env_logger::Builder;
use futures_util::StreamExt;
use hyper::client::connect::Connect;
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use tokio::join;
use tokio::sync::mpsc;
pub struct BinanceExchangeClient {
    connected: bool,
    balance: f64,
    credentials: Credentials,
    client: BinanceHttpClient<HttpsConnector<HttpConnector>>,
    market_data: Arc<Mutex<MarketData>>,
    price_data: Arc<Mutex<VecDeque<f64>>>,
    symbol: String,
    current_timestamp: Arc<Mutex<i64>>,
}
impl BinanceExchangeClient {
    pub fn new(credentials: Credentials) -> Self {
        BinanceExchangeClient {
            connected: false,
            balance: 0.0,
            symbol: String::new(),
            credentials: credentials.clone(),
            client: BinanceHttpClient::default().credentials(credentials),
            market_data: Arc::new(Mutex::new(MarketData::default())),
            price_data: Arc::new(Mutex::new(VecDeque::new())),
            current_timestamp: Arc::new(Mutex::new(0)),
        }
    }
    pub async fn start(&mut self) -> Result<(), dtoError> {
        let prices = self
            .get_historical_prices(16)
            .await
            .map_err(|e| dtoError::HttpError(format!("{:?}", e)))?;
        // println!("Prices: {:?}", prices); //    self.update_prices()
        Ok(())
    }
    pub async fn set_symbol(&mut self, symbol: String) {
        self.symbol = symbol;
    }
    pub async fn get_historical_prices(
        &mut self,
        window_size: usize,
    ) -> Result<Vec<KlineResponse>, dtoError> {
        let data = self
            .get_klines(KlineInterval::Minutes1, window_size)
            .await
            .map_err(|e| dtoError::HttpError(format!("{:?}", e)))?;
        self.price_data = Arc::new(Mutex::new(data.iter().map(|k| k.close_price).collect()));
        {
            self.price_data.lock().unwrap().pop_back();
        }
        let rsi = {
            let history_vec = self
                .price_data
                .lock()
                .unwrap()
                .iter()
                // .rev()
                .copied()
                .collect::<Vec<f64>>();
            // calculate_rsi(&history_vec, 14)
            // let test = history_vec.clone()[history_vec.len() - 15..].to_vec();
            // println!("test: {:?}", test);
            println!("test: {:?}", &history_vec);
            rust_ti::momentum_indicators::bulk::relative_strength_index(
                &history_vec,
                &rust_ti::ConstantModelType::SimpleMovingAverage,&5,
            )
        };
        println!("rsi {:?}",rsi);
        Ok(data)
    }

    pub async fn account_status(&self) -> Result<String, Error> {
        let data = self
            .client
            .send(wallet::account_status())
            .await?
            .into_body_str()
            .await?;
        log::info!("{}", data);
        Ok(data)
    }
    pub async fn api_trading_status(&self) -> Result<String, Error> {
        let data = self
            .client
            .send(wallet::api_trading_status())
            .await?
            .into_body_str()
            .await?;
        log::info!("{}", data);
        Ok(data)
    }
    pub async fn get_klines(
        &self,
        timeframe: KlineInterval,
        window_size: usize,
    ) -> Result<Vec<KlineResponse>, dtoError> {
        let request = market::klines(&self.symbol, timeframe).limit(window_size as u32);
        let response = self
            .client
            .send(request)
            .await
            .map_err(|e| dtoError::RequestError(format!("{:?}", e)))?;
        let data = response
            .into_body_str()
            .await
            .map_err(|e| dtoError::HttpError(format!("{:?}", e)))?;

        let raw_klines: Vec<Vec<serde_json::Value>> = match serde_json::from_str(&data) {
            Ok(klines) => klines,
            Err(e) => return Err(dtoError::from(e)),
        };

        let klines = raw_klines
            .iter()
            .map(|kline_data| KlineResponse::from_raw_data(kline_data))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| dtoError::from(e))?;

        Ok(klines)
    }
    pub async fn send_order(&self, order: &Order) -> Result<String, Error> {
        let side = match order.side {
            OrderSide::Buy => Side::Buy,
            OrderSide::Sell => Side::Sell,
        };
        let quantity = Decimal::from_f64(order.quantity).unwrap();
        let data = self
            .client
            .send(
                trade::new_order(&order.symbol, side, order.order_type.to_string().as_str())
                    .quantity(quantity),
            )
            .await?
            .into_body_str()
            .await?;
        log::info!("{}", data);
        Ok(data)
    }

    pub async fn get_all_market_data(&mut self) {
        let (kline_tx, kline_rx) = mpsc::channel(100);
        let (ticker_tx, ticker_rx) = mpsc::channel(100);
        let (signal_tx, signal_rx) = mpsc::channel(100); // New channel for trading signals
        let (current_timestamp_tx, current_timestamp_rx) = mpsc::channel(100);
        let market_data_kline = self.market_data.clone();
        let market_data_ticker = self.market_data.clone();
        let market_data_analysis = self.market_data.clone();

        let kline_handle = tokio::spawn(get_kline_data(kline_tx));
        let ticker_handle = tokio::spawn(get_ticker_data(ticker_tx));
        let analysis_handle = tokio::spawn(analyze_price_data(
            market_data_analysis,
            signal_tx,
            current_timestamp_rx,
            self.current_timestamp.clone(),
            self.price_data.clone(),
        ));

        let kline_process = tokio::spawn(process_kline_data(
            kline_rx,
            current_timestamp_tx,
            market_data_kline,
        ));
        let ticker_process = tokio::spawn(process_ticker_data(ticker_rx, market_data_ticker));
        let signal_process = tokio::spawn(process_trading_signals(signal_rx));

        let _ = join!(
            kline_handle,
            ticker_handle,
            kline_process,
            ticker_process,
            analysis_handle,
            signal_process
        );
    }
}
async fn process_kline_data(
    mut receiver: mpsc::Receiver<Kline>,
    current_timestamp: mpsc::Sender<i64>,
    market_data: Arc<Mutex<MarketData>>,
) {
    while let Some(kline) = receiver.recv().await {
        {
            let mut data = market_data.lock().unwrap();
            // Update market data
            *data = MarketData {
                symbol: kline.symbol.clone(),
                open_price: kline.open_price.parse().unwrap_or_default(),
                close_price: kline.close_price.parse().unwrap_or_default(),
                high_price: kline.high_price.parse().unwrap_or_default(),
                low_price: kline.low_price.parse().unwrap_or_default(),
                ..*data
            };
        }
        let end_time = kline.end_time; // Copy the value
        if let Err(e) = current_timestamp.send(end_time).await {
            log::error!("Failed to send timestamp: {}", e);
        }
        // Log or do additional processing
        // log::info!(
        //     "Kline Update - Symbol: {}, Open: {}, Close: {}",
        //     kline.symbol,
        //     kline.open_price,
        //     kline.close_price
        // );
    }
}

// เช่นเดียวกันสำหรับ ticker data
async fn process_ticker_data(
    mut receiver: mpsc::Receiver<TickerData>,
    market_data: Arc<Mutex<MarketData>>,
) {
    while let Some(ticker) = receiver.recv().await {
        let mut data = market_data.lock().unwrap();
        // Update market data
        *data = MarketData {
            symbol: ticker.symbol.clone(),
            last_price: ticker.last_price.parse().unwrap_or_default(),
            ..*data
        };

        // Log or do additional processing
        // log::info!(
        //     "Ticker Update - Symbol: {}, Last: {}",
        //     ticker.symbol,
        //     ticker.last_price
        // );
    }
}
pub async fn get_kline_data(mut sender: mpsc::Sender<Kline>) {
    // Establish connection
    let (mut conn, _) = BinanceWebSocketClient::connect_async_default()
        .await
        .expect("Failed to connect");
    // Subscribe to streams
    conn.subscribe(vec![
        &KlineStream::new("BTCUSDT", KlineInterval::Minutes1).into()
    ])
    .await;
    // Start a timer for 10 seconds
    // let timer = tokio::time::Instant::now();
    // let duration = Duration::new(10, 0);
    // Read messages
    while let Some(message) = conn.as_mut().next().await {
        // if timer.elapsed() >= duration {
        //     log::info!("10 seconds elapsed, exiting loop.");
        //     break; // Exit the loop after 10 seconds
        // }
        match message {
            Ok(message) => {
                let binary_data = message.into_data();
                let data = std::str::from_utf8(&binary_data).expect("Failed to parse message");
                match parse_websocket_message(data) {
                    Ok(response) => {
                        let mut kline_data = Kline::default();
                        kline_data.symbol = response.data.symbol.clone();
                        kline_data.open_price = response.data.kline.open_price.clone();
                        kline_data.close_price = response.data.kline.close_price.clone();
                        kline_data.low_price = response.data.kline.low_price.clone();
                        kline_data.high_price = response.data.kline.high_price.clone();
                        kline_data.volume = response.data.kline.volume.clone();
                        kline_data.start_time = response.data.kline.start_time.clone();
                        kline_data.end_time = response.data.kline.end_time.clone();
                        if let Err(e) = sender.send(kline_data).await {
                            log::error!("Failed to send kline data: {}", e);
                        }
                        // log::info!(
                        //     "Received kline data for {}: Open: {}, Close: {}",
                        //     response.data.symbol,
                        //     response.data.kline.open_price,
                        //     response.data.kline.close_price,
                        // );
                    }
                    Err(e) => {
                        if let Ok(_) = data.trim().parse::<i64>() {
                            // Skip logging if it's an integer
                            // log::debug!("Received numeric data, skipping");
                            continue;
                        } else {
                            log::error!("Failed to parse JSON: {} raw data: {}", e, data);
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
    // Disconnect
    conn.close().await.expect("Failed to disconnect");
}
pub async fn get_ticker_data(mut sender: mpsc::Sender<TickerData>) {
    // Establish connection
    let (mut conn, _) = BinanceWebSocketClient::connect_async_default()
        .await
        .expect("Failed to connect");
    // Subscribe to streams
    conn.subscribe(vec![
        // &KlineStream::new("BTCUSDT", KlineInterval::Minutes1).into()
        &TickerStream::from_symbol("BTCUSDT").into(),
    ])
    .await;
    // Start a timer for 10 seconds
    // let timer = tokio::time::Instant::now();
    // let duration = Duration::new(10, 0);
    // Read messages
    while let Some(message) = conn.as_mut().next().await {
        // if timer.elapsed() >= duration {
        //     log::info!("10 seconds elapsed, exiting loop.");
        //     break; // Exit the loop after 10 seconds
        // }
        match message {
            Ok(message) => {
                let binary_data = message.into_data();
                let data = std::str::from_utf8(&binary_data).expect("Failed to parse message");
                match parse_websocket_message_ticker(data) {
                    Ok(response) => {
                        let mut ticker_data = TickerData::default();
                        ticker_data.symbol = response.data.symbol.clone();
                        ticker_data.last_price = response.data.last_price.clone();
                        if let Err(e) = sender.send(ticker_data).await {
                            log::error!("Failed to send kline data: {}", e);
                        }
                        // log::info!(
                        //     "Received ticker data for {}: last: {}, bid: {}, ask: {}",
                        //     response.data.symbol,
                        //     response.data.last_price,
                        //     response.data.bid_price,
                        //     response.data.ask_price,
                        // );
                    }
                    Err(e) => {
                        if let Ok(_) = data.trim().parse::<i64>() {
                            // Skip logging if it's an integer
                            // log::debug!("Received numeric data, skipping");
                            continue;
                        } else {
                            log::error!("Failed to parse JSON: {} raw data: {}", e, data);
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
    // Disconnect
    conn.close().await.expect("Failed to disconnect");
}
pub async fn update_prices(data: Arc<Mutex<VecDeque<f64>>>, prices: f64) {
    let mut data = data.lock().unwrap();
    data.push_back(prices);
    data.pop_front();
}
async fn analyze_price_data(
    market_data: Arc<Mutex<MarketData>>,
    signal_sender: mpsc::Sender<TradingSignal>,
    mut current_timestamp: mpsc::Receiver<i64>,
    current_timestamp_ud: Arc<Mutex<i64>>,
    history_data: Arc<Mutex<VecDeque<f64>>>,
) {
    while let Some(current_timestamp_closed) = current_timestamp.recv().await {
        // ตรวจสอบ 1: จัดการกรณี timestamp เริ่มต้น
        {
            let mut current_timestamp_ud_guard = current_timestamp_ud.lock().unwrap();
            if *current_timestamp_ud_guard == 0 {
                *current_timestamp_ud_guard = current_timestamp_closed;
                log::info!("current_timestamp_ud: {}", *current_timestamp_ud_guard);
                continue;
            }
        } // Guard ถูกปล่อยที่นี่

        // ตรวจสอบ 2: รับข้อมูลตลาด
        let data = market_data.lock().unwrap().clone();
        if data.timestamp as i64 == current_timestamp_closed {
            continue;
        }

        // ตรวจสอบ 3: อัพเดต timestamp ถ้าจำเป็น
        let should_update = {
            let current_ud = current_timestamp_ud.lock().unwrap();
            current_timestamp_closed > *current_ud
        }; // Guard ถูกปล่อยที่นี่

        if should_update {
            // อัพเดต timestamp
            {
                let mut current_ud = current_timestamp_ud.lock().unwrap();
                *current_ud = current_timestamp_closed;
            } // Guard ถูกปล่อยที่นี่
              // ตอนนี้อัพเดตราคาโดยไม่ถือล็อคใดๆ
            update_prices(history_data.clone(), data.close_price).await;
            // คำนวณตัวบ่งชี้หลังการอัพเดต
            let rsi = {
                let history_vec = history_data
                    .lock()
                    .unwrap()
                    .iter()
                    .copied()
                    .collect::<Vec<f64>>();
                // calculate_rsi(&history_vec, 14)
                rust_ti::momentum_indicators::bulk::relative_strength_index(
                    &history_vec,
                    &rust_ti::ConstantModelType::SimpleMovingAverage,
                    &14,
                )
            };
            log::info!("RSI: {:?}", rsi);

            let fast_ema = {
                let history_vec = history_data
                    .lock()
                    .unwrap()
                    .iter()
                    .copied()
                    .collect::<Vec<f64>>();
                calculate_ema(&history_vec, 5)
            };
            // log::info!("Fast EMA: {:?}", fast_ema);

            let slow_ema = {
                let history_vec = history_data
                    .lock()
                    .unwrap()
                    .iter()
                    .copied()
                    .collect::<Vec<f64>>();
                calculate_ema(&history_vec, 15)
            };
            // log::info!("Slow EMA: {:?}", slow_ema);
            // log::info!("close price: {}", data.close_price);
            {
                let history_vec = history_data
                    .lock()
                    .unwrap()
                    .iter()
                    .copied()
                    .collect::<Vec<f64>>();
                // log::info!("history_vec: {:?}", history_vec);
            }
            // ตรรกะสัญญาณของคุณ
            let signal = analyze_market_conditions(&data);
            // ส่วนส่งสัญญาณถูกคอมเมนต์ไว้
        }
    }
}

// Process trading signals
async fn process_trading_signals(mut receiver: mpsc::Receiver<TradingSignal>) {
    while let Some(signal) = receiver.recv().await {
        match signal.action {
            TradeAction::Buy => {
                log::info!(
                    "Buy Signal - Symbol: {}, Price: {}",
                    signal.symbol,
                    signal.price
                );
                // Add your order execution logic here
            }
            TradeAction::Sell => {
                log::info!(
                    "Sell Signal - Symbol: {}, Price: {}",
                    signal.symbol,
                    signal.price
                );
                // Add your order execution logic here
            }
            TradeAction::Hold => {
                log::debug!(
                    "Hold Position - Symbol: {}, Price: {}",
                    signal.symbol,
                    signal.price
                );
            }
        }
    }
}

// Example strategy function - replace with your own trading logic
fn analyze_market_conditions(data: &MarketData) -> Option<TradingSignal> {
    // Simple example: Generate buy signal if current price is lower than opening price by 2%
    let price_change_percentage = ((data.last_price - data.open_price) / data.open_price) * 100.0;

    let action = if price_change_percentage < -2.0 {
        TradeAction::Buy
    } else if price_change_percentage > 2.0 {
        TradeAction::Sell
    } else {
        TradeAction::Hold
    };

    Some(TradingSignal {
        symbol: data.symbol.clone(),
        action,
        price: data.last_price,
        timestamp: chrono::Utc::now().timestamp(),
    })
}
impl ExchangeClient for BinanceExchangeClient {
    async fn connect(&mut self) -> Result<(), TradingError> {
        match self.account_status().await {
            Ok(_) => (),
            Err(e) => {
                log::error!("Failed to connect: {:?}", e);
                return Err(TradingError::ConnectionError("Failed to connect".into()));
            }
        }

        match self.api_trading_status().await {
            Ok(_) => {
                self.connected = true;
                log::info!("Connected to Binance");
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to connect: {:?}", e);
                Err(TradingError::ConnectionError("Failed to connect".into()))
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), TradingError> {
        self.connected = false;
        Ok(())
    }

    async fn get_balance(&self) -> Result<f64, TradingError> {
        if self.connected {
            Ok(self.balance)
        } else {
            Err(TradingError::ConnectionError("Not connected".into()))
        }
    }

    async fn send_order(&mut self, order: &Order) -> Result<OrderResponse, TradingError> {
        if !self.connected {
            return Err(TradingError::ConnectionError("Not connected".into()));
        }

        // Mock implementation
        Ok(OrderResponse {
            order_id: "mock_order_123".to_string(),
            status: OrderStatus::Filled,
        })
    }

    async fn cancel_order(&mut self, _order_id: &str) -> Result<(), TradingError> {
        // Mock implementation
        Ok(())
    }
}
#[tokio::main]
async fn main() {
    Builder::from_default_env()
        .filter(None, log::LevelFilter::Debug)
        .init();
    let api_key = dotenv::var("BINANCE_API_KEY").expect("BINANCE_API_KEY must be set");
    let api_secret = dotenv::var("BINANCE_API_SECRET").expect("BINANCE_API_SECRET must be set");
    let credentials = Credentials::from_hmac(api_key, api_secret);
    let mut client = BinanceExchangeClient::new(credentials);
    client.set_symbol("BTCUSDT".to_string()).await;
    client.connect().await.unwrap();
    client.start().await;
    client.get_all_market_data().await;
    // client.get_market_data().await;

    // let response = client.send_order(&order).?await.unwrap();;
    // println!("Order response: {:?}", response);

    // let balance = client.get_balance().?await.unwrap();;
    // println!("Balance: {}", balance);
}
// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_mock_exchange() {
//         let mut client = BinanceExchangeClient :new();
//         client.connect().unwrap();

//         let order = Order {
//             symbol: "BTC/USD".to_string(),
//             quantity: 1.0,
//             order_type: OrderType::Market,
//             side: OrderSide::Buy,
//         };

//         let response = client.send_order(&order).unwrap();
//         assert_eq!(response.status, OrderStatus::Filled);

//         let balance = client.get_balance().unwrap();
//         assert!(balance < 100000.0);
//     }
// }
