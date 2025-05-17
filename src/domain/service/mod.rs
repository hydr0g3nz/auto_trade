// src/domain/service/mod.rs
// Domain service interfaces

use async_trait::async_trait;
use crate::domain::model::{MarketData, TradingSignal, DomainError};

#[async_trait]
pub trait TradingStrategyService {
    /// Analyze market data and generate trading signals
    async fn analyze(&self, data: &MarketData) -> Result<Option<TradingSignal>, DomainError>;
    
    /// Update strategy internal state with new data
    async fn update_state(&mut self, data: &MarketData) -> Result<(), DomainError>;
}

#[async_trait]
pub trait RiskManagementService {
    /// Validate if an order meets risk criteria
    async fn validate_trade(&self, symbol: &str, quantity: f64, side: &str) -> Result<bool, DomainError>;
    
    /// Calculate maximum allowed position size
    async fn calculate_position_size(&self, symbol: &str, available_balance: f64) -> Result<f64, DomainError>;
}

#[async_trait]
pub trait TechnicalAnalysisService {
    /// Calculate RSI (Relative Strength Index)
    async fn calculate_rsi(&self, prices: &[f64], period: usize) -> Result<Option<f64>, DomainError>;
    
    /// Calculate EMA (Exponential Moving Average)
    async fn calculate_ema(&self, prices: &[f64], period: usize) -> Result<Vec<f64>, DomainError>;
    
    /// Calculate MACD (Moving Average Convergence Divergence)
    async fn calculate_macd(
        &self,
        prices: &[f64],
        fast_period: usize,
        slow_period: usize,
        signal_period: usize
    ) -> Result<(Vec<f64>, Vec<f64>), DomainError>;
}