// src/domain/model/mod.rs
// Core domain models

// Market data model
#[derive(Debug, Clone, Default)]
pub struct MarketData {
    pub symbol: String,
    pub timestamp: u64,
    pub volume: f64,
    pub last_price: f64,
    pub open_price: f64,
    pub close_price: f64,
    pub high_price: f64,
    pub low_price: f64,
}

// Order model representing a trade order
#[derive(Debug, Clone)]
pub struct Order {
    pub symbol: String,
    pub quantity: f64,
    pub order_type: OrderType,
    pub side: OrderSide,
}

#[derive(Debug, Clone)]
pub enum OrderType {
    Market,
    Limit(f64),
    Stop(f64),
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            OrderType::Market => write!(f, "MARKET"),
            OrderType::Limit(price) => write!(f, "LIMIT {}", price),
            OrderType::Stop(price) => write!(f, "STOP {}", price),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct OrderResponse {
    pub order_id: String,
    pub status: OrderStatus,
}

#[derive(Debug, Clone)]
pub enum OrderStatus {
    Filled,
    PartiallyFilled,
    Canceled,
    Rejected,
    Pending,
}

// Trading signal model
#[derive(Debug, Clone)]
pub struct TradingSignal {
    pub symbol: String,
    pub action: TradeAction,
    pub price: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub enum TradeAction {
    Buy,
    Sell,
    Hold,
}

// Domain-level errors
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Exchange error: {0}")]
    ExchangeError(String),
    
    #[error("Invalid order: {0}")]
    InvalidOrder(String),
    
    #[error("Market data error: {0}")]
    MarketDataError(String),
    
    #[error("Strategy error: {0}")]
    StrategyError(String),
}