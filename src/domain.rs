use std::{error::Error, fmt};

/// Core Trading Components
#[derive(Debug, Clone)]
pub struct Order {
    pub symbol: String,
    pub quantity: f64,
    pub order_type: OrderType,
    pub side: OrderSide,
    // Add more fields as needed
}

#[derive(Debug, Clone)]
pub enum OrderType {
    Market,
    Limit(f64),
    Stop(f64),
    // Add more order types
}
impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    // Add more response fields
}

#[derive(Debug, Clone)]
pub enum OrderStatus {
    Filled,
    PartiallyFilled,
    Canceled,
    Rejected,
    Pending,
}
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
/// Market Data Structures
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

/// Error Handling
#[derive(Debug)]
pub enum TradingError {
    ConnectionError(String),
    AuthenticationError(String),
    OrderError(String),
    DataError(String),
    NetworkError(String),
    // Add more error variants
}

impl fmt::Display for TradingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TradingError::ConnectionError(msg) => write!(f, "Connection Error: {}", msg),
            // Implement other variants
            _ => write!(f, "Generic trading error"),
        }
    }
}

impl Error for TradingError {}

/// Core Trading Traits
pub trait ExchangeClient {
    async fn connect(&mut self) -> Result<(), TradingError>;
    async fn disconnect(&mut self) -> Result<(), TradingError>;
    async fn get_balance(&self) -> Result<f64, TradingError>;
    async fn send_order(&mut self, order: &Order) -> Result<OrderResponse, TradingError>;
    async fn cancel_order(&mut self, order_id: &str) -> Result<(), TradingError>;
    // Add more exchange methods
}

pub trait MarketDataHandler {
    fn subscribe_to_symbol(&mut self, symbol: &str) -> Result<(), TradingError>;
    fn get_latest_data(&self, symbol: &str) -> Option<MarketData>;
    // Add more market data methods
}

pub trait RiskManager {
    fn pre_trade_check(&self, order: &Order) -> Result<(), TradingError>;
    fn validate_order(&self, order: &Order) -> Result<(), TradingError>;
    // Add risk management methods
}
