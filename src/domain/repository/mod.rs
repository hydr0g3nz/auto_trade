// src/domain/repository/mod.rs
// Repository interfaces for domain entities

use async_trait::async_trait;
use crate::domain::model::{MarketData, Order, OrderResponse, DomainError};

/// Repository interface for exchange operations
#[async_trait]
pub trait ExchangeRepository {
    async fn connect(&mut self) -> Result<(), DomainError>;
    async fn disconnect(&mut self) -> Result<(), DomainError>;
    async fn get_balance(&self, asset: &str) -> Result<f64, DomainError>;
    async fn get_historical_prices(&self, symbol: &str, interval: &str, limit: usize) -> Result<Vec<f64>, DomainError>;
    async fn send_order(&self, order: &Order) -> Result<OrderResponse, DomainError>;
    async fn cancel_order(&self, order_id: &str) -> Result<(), DomainError>;
}

/// Repository interface for market data
#[async_trait]
pub trait MarketDataRepository {
    async fn get_latest_data(&self, symbol: &str) -> Result<MarketData, DomainError>;
    async fn subscribe_to_market_data(&self, symbol: &str) -> Result<(), DomainError>;
    async fn unsubscribe_from_market_data(&self, symbol: &str) -> Result<(), DomainError>;
}