use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
mod domain;
use crate::domain::*;
mod dto;
use crate::dto::*;

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
}
impl BinanceExchangeClient {
    pub fn new(credentials: Credentials) -> Self {
        BinanceExchangeClient {
            connected: false,
            balance: 0.0,
            credentials: credentials.clone(),
            client: BinanceHttpClient::default().credentials(credentials),
            market_data: Arc::new(Mutex::new(MarketData::default())),
        }
    }
    pub async fn start(&mut self) -> Result<(), Error> {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            log::info!("Current market data: {:?}", self.market_data);
        }
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

        let market_data_kline = self.market_data.clone();
        let market_data_ticker = self.market_data.clone();
        let market_data_analysis = self.market_data.clone();

        let kline_handle = tokio::spawn(get_kline_data(kline_tx));
        let ticker_handle = tokio::spawn(get_ticker_data(ticker_tx));
        let analysis_handle = tokio::spawn(analyze_price_data(market_data_analysis, signal_tx));

        let kline_process = tokio::spawn(process_kline_data(kline_rx, market_data_kline));
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
    market_data: Arc<Mutex<MarketData>>,
) {
    while let Some(kline) = receiver.recv().await {
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

        // Log or do additional processing
        log::info!(
            "Kline Update - Symbol: {}, Open: {}, Close: {}",
            kline.symbol,
            kline.open_price,
            kline.close_price
        );
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
        log::info!(
            "Ticker Update - Symbol: {}, Last: {}",
            ticker.symbol,
            ticker.last_price
        );
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
                    Err(e) => log::error!("Failed to parse JSON: {} raw data: {}", e, data),
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
                    Err(e) => log::error!("Failed to parse JSON: {} raw data: {}", e, data),
                }
            }
            Err(_) => break,
        }
    }
    // Disconnect
    conn.close().await.expect("Failed to disconnect");
}
async fn analyze_price_data(
    market_data: Arc<Mutex<MarketData>>,
    signal_sender: mpsc::Sender<TradingSignal>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await; // Correctly await the tick without matching it to `()`
        let data = market_data.lock().unwrap().clone();

        // Simple example strategy - you can replace this with your own logic
        let signal = analyze_market_conditions(&data);

        if let Some(trading_signal) = signal {
            if let Err(e) = signal_sender.send(trading_signal).await {
                log::error!("Failed to send trading signal: {}", e);
            }
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
        if let Ok(data) = self.account_status().await {
            // Ok(())
        } else {
            return Err(TradingError::ConnectionError("Failed to connect".into()));
        }
        if let Ok(data) = self.api_trading_status().await {
            self.connected = true;
            log::info!("Connected to Binance");
            Ok(())
        } else {
            Err(TradingError::ConnectionError("Failed to connect".into()))
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

    client.connect().await.unwrap();
    // client.get_market_data().await;
    client.get_all_market_data().await;
    let order = Order {
        symbol: "BTC/USD".to_string(),
        quantity: 1.0,
        order_type: OrderType::Market,
        side: OrderSide::Buy,
    };

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
