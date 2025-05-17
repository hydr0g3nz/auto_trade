// src/infrastructure/exchange/binance.rs
// Binance exchange repository implementation

use std::sync::Arc;
use async_trait::async_trait;
use binance_spot_connector_rust::{
    http::Credentials,
    market::klines::KlineInterval,
    hyper::{BinanceHttpClient, Error as BinanceError},
    wallet, trade, market,
    trade::order::Side,
    hyper::hyper_tls::HttpsConnector,
    hyper::client::HttpConnector,
};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use tokio::sync::Mutex;

use crate::domain::model::{Order, OrderResponse, OrderStatus, OrderSide, OrderType, DomainError};
use crate::domain::repository::ExchangeRepository;
use crate::application::dto::{ApplicationError, KlineResponse};
use crate::application::dto::parser::*;

pub struct BinanceExchangeRepository {
    credentials: Credentials,
    client: BinanceHttpClient<HttpsConnector<HttpConnector>>,
    connected: bool,
}

impl BinanceExchangeRepository {
    pub fn new(api_key: String, api_secret: String) -> Self {
        let credentials = Credentials::from_hmac(api_key, api_secret);
        Self {
            credentials: credentials.clone(),
            client: BinanceHttpClient::default().credentials(credentials),
            connected: false,
        }
    }
    
    pub async fn account_status(&self) -> Result<String, BinanceError> {
        let data = self
            .client
            .send(wallet::account_status())
            .await?
            .into_body_str()
            .await?;
        log::info!("{}", data);
        Ok(data)
    }
    
    pub async fn api_trading_status(&self) -> Result<String, BinanceError> {
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
        symbol: &str,
        timeframe: KlineInterval,
        window_size: usize,
    ) -> Result<Vec<KlineResponse>, ApplicationError> {
        let request = market::klines(symbol, timeframe).limit(window_size as u32);
        let response = self
            .client
            .send(request)
            .await
            .map_err(|e| ApplicationError::RequestError(format!("{:?}", e)))?;
        let data = response
            .into_body_str()
            .await
            .map_err(|e| ApplicationError::HttpError(format!("{:?}", e)))?;

        let raw_klines: Vec<Vec<serde_json::Value>> = serde_json::from_str(&data)
            .map_err(|e| ApplicationError::JsonError(e))?;

        let klines = raw_klines
            .iter()
            .map(|kline_data| KlineResponse::from_raw_data(kline_data))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(klines)
    }
}

#[async_trait]
impl ExchangeRepository for BinanceExchangeRepository {
    async fn connect(&mut self) -> Result<(), DomainError> {
        // Check account status
        match self.account_status().await {
            Ok(_) => (),
            Err(e) => {
                log::error!("Failed to connect: {:?}", e);
                return Err(DomainError::ExchangeError("Failed to connect".into()));
            }
        }

        // Check API trading status
        match self.api_trading_status().await {
            Ok(_) => {
                self.connected = true;
                log::info!("Connected to Binance");
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to connect: {:?}", e);
                Err(DomainError::ExchangeError("Failed to connect".into()))
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), DomainError> {
        self.connected = false;
        Ok(())
    }

    async fn get_balance(&self, asset: &str) -> Result<f64, DomainError> {
        if !self.connected {
            return Err(DomainError::ExchangeError("Not connected".into()));
        }
        
        // This would fetch actual balance from the exchange
        // Unimplemented for now
        Ok(0.0)
    }
    
    async fn get_historical_prices(&self, symbol: &str, interval: &str, limit: usize) -> Result<Vec<f64>, DomainError> {
        if !self.connected {
            return Err(DomainError::ExchangeError("Not connected".into()));
        }
        
        let kline_interval = match interval {
            "1m" => KlineInterval::Minutes1,
            "5m" => KlineInterval::Minutes5,
            "15m" => KlineInterval::Minutes15,
            "1h" => KlineInterval::Hours1,
            "4h" => KlineInterval::Hours4,
            "1d" => KlineInterval::Days1,
            _ => return Err(DomainError::ExchangeError(format!("Invalid interval: {}", interval))),
        };
        
        let klines = self.get_klines(symbol, kline_interval, limit)
            .await
            .map_err(|e| DomainError::ExchangeError(e.to_string()))?;
            
        let prices = klines.iter().map(|k| k.close_price).collect();
        
        Ok(prices)
    }

    async fn send_order(&self, order: &Order) -> Result<OrderResponse, DomainError> {
        if !self.connected {
            return Err(DomainError::ExchangeError("Not connected".into()));
        }

        let side = match order.side {
            OrderSide::Buy => Side::Buy,
            OrderSide::Sell => Side::Sell,
        };
        
        let quantity = Decimal::from_f64(order.quantity)
            .ok_or_else(|| DomainError::InvalidOrder("Invalid quantity".into()))?;
            
        let result = self
            .client
            .send(
                trade::new_order(&order.symbol, side, order.order_type.to_string().as_str())
                    .quantity(quantity),
            )
            .await
            .map_err(|e| DomainError::ExchangeError(e.to_string()))?
            .into_body_str()
            .await
            .map_err(|e| DomainError::ExchangeError(e.to_string()))?;
            
        log::info!("Order result: {}", result);
        
        // Parse the response and extract order ID
        // This is simplified - a real implementation would parse the JSON response
        Ok(OrderResponse {
            order_id: "mock_id".to_string(), // Would be extracted from the response
            status: OrderStatus::Filled,      // Would be extracted from the response
        })
    }

    async fn cancel_order(&self, order_id: &str) -> Result<(), DomainError> {
        if !self.connected {
            return Err(DomainError::ExchangeError("Not connected".into()));
        }
        
        // Unimplemented for now
        Ok(())
    }
}