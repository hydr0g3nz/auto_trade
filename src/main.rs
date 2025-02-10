mod domain;
use crate::domain::*;

use binance_spot_connector_rust::{
    http::Credentials,
    hyper::{BinanceHttpClient, Error},
    wallet::{self, account_status},
};

use hyper::client::connect::Connect;

use env_logger::Builder;

pub struct BinanceExchangeClient<T>
where
    T: Connect + Clone + Send + Sync + 'static,
{
    connected: bool,
    balance: f64,
    credentials: Credentials,
    client: BinanceHttpClient<T>,
}
impl<T> BinanceExchangeClient<T>
where
    T: Connect + Clone + Send + Sync + 'static,
{
    pub fn new(credentials: Credentials) -> Self {
        BinanceExchangeClient {
            connected: false,
            balance: 0.0,
            credentials,
            client: BinanceHttpClient::default().credentials(credentials.clone()),
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
}

impl ExchangeClient for BinanceExchangeClient<T>
where
    T: Connect + Clone + Send + Sync + 'static,
{
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
