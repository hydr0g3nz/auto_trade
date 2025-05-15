// src/domain/errors.rs
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Exchange error: {0}")]
    Exchange(#[from] ExchangeError),
    
    #[error("Market data error: {0}")]
    MarketData(#[from] MarketDataError),
    
    #[error("Trading error: {0}")]
    Trading(#[from] TradingError),
    
    #[error("Analysis error: {0}")]
    Analysis(#[from] AnalysisError),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

// Implement From for common error types
impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Unknown(s)
    }
}

#[derive(Error, Debug)]
pub enum ExchangeError {
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Authentication error: {0}")]
    Authentication(String),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),
    
    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),
    
    #[error("Order error: {0}")]
    Order(String),
    
    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),
    
    #[error("Account error: {0}")]
    Account(String),
    
    #[error("API error: {0}")]
    Api(String),
    
    #[error("Request error: {0}")]
    Request(String),
}

#[derive(Error, Debug)]
pub enum MarketDataError {
    #[error("Websocket error: {0}")]
    WebSocket(String),
    
    #[error("Invalid data format: {0}")]
    InvalidFormat(String),
    
    #[error("Stream subscription error: {0}")]
    Subscription(String),
    
    #[error("Data parse error: {0}")]
    Parse(String),
    
    #[error("No data available for: {0}")]
    NoData(String),
}

#[derive(Error, Debug)]
pub enum TradingError {
    #[error("Strategy error: {0}")]
    Strategy(String),
    
    #[error("Risk management error: {0}")]
    RiskManagement(String),
    
    #[error("Order execution error: {0}")]
    OrderExecution(String),
    
    #[error("Signal error: {0}")]
    Signal(String),
    
    #[error("Position management error: {0}")]
    PositionManagement(String),
}

#[derive(Error, Debug)]
pub enum AnalysisError {
    #[error("Indicator calculation error: {0}")]
    IndicatorCalculation(String),
    
    #[error("Insufficient data for analysis: {0}")]
    InsufficientData(String),
    
    #[error("Pattern detection error: {0}")]
    PatternDetection(String),
}

// Result type alias for convenience
pub type AppResult<T> = Result<T, AppError>;
pub type ExchangeResult<T> = Result<T, ExchangeError>;
pub type MarketDataResult<T> = Result<T, MarketDataError>;
pub type TradingResult<T> = Result<T, TradingError>;
pub type AnalysisResult<T> = Result<T, AnalysisError>;