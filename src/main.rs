use std::time::Duration;

mod domain;
use crate::domain::*;
mod dto;
use crate::dto::*;

use binance_spot_connector_rust::market_stream::ticker::TickerStream;
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
use tokio::join;
pub struct BinanceExchangeClient {
    connected: bool,
    balance: f64,
    credentials: Credentials,
    client: BinanceHttpClient<HttpsConnector<HttpConnector>>,
}
impl BinanceExchangeClient {
    pub fn new(credentials: Credentials) -> Self {
        BinanceExchangeClient {
            connected: false,
            balance: 0.0,
            credentials: credentials.clone(),
            client: BinanceHttpClient::default().credentials(credentials),
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
        let client = BinanceHttpClient::default().credentials(self.credentials.clone());
        let data = self
            .client
            .send(wallet::api_trading_status())
            .await?
            .into_body_str()
            .await?;
        log::info!("{}", data);
        Ok(data)
    }

    pub async fn get_all_market_data(&self) {
        // Create two separate tasks for kline and ticker data
        let kline_handle = tokio::spawn(get_kline_data());
        let ticker_handle = tokio::spawn(get_ticker_data());

        // Wait for both tasks to complete
        let _ = join!(kline_handle, ticker_handle);
    }
}
pub async fn get_kline_data() {
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
                        log::info!(
                            "Received kline data for {}: Open: {}, Close: {}",
                            response.data.symbol,
                            response.data.kline.open_price,
                            response.data.kline.close_price,
                        );
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
pub async fn get_ticker_data() {
    // Establish connection
    let (mut conn, _) = BinanceWebSocketClient::connect_async_default()
        .await
        .expect("Failed to connect");
    // Subscribe to streams
    conn.subscribe(vec![
        // &KlineStream::new("BTCUSDT", KlineInterval::Minutes1).into()
        &TickerStream::from_symbol("BTCUSDT").into()
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
                        log::info!(
                            "Received ticker data for {}: last: {}, bid: {}, ask: {}",
                            response.data.symbol,
                            response.data.last_price,
                            response.data.bid_price,
                            response.data.ask_price,
                        );
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
