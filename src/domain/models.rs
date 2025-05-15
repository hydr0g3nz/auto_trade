use rust_decimal::Decimal;
use std::fmt;
use chrono::{DateTime, Utc};

/// Core Trading Components
#[derive(Debug, Clone)]
pub struct Order {
    pub symbol: String,
    pub quantity: Decimal,
    pub order_type: OrderType,
    pub side: OrderSide,
    pub client_order_id: Option<String>,
    pub timestamp: i64,
}

impl Order {
    pub fn new_market_order(symbol: &str, quantity: Decimal, side: OrderSide) -> Self {
        Self {
            symbol: symbol.to_string(),
            quantity,
            order_type: OrderType::Market,
            side,
            client_order_id: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
    
    pub fn new_limit_order(symbol: &str, quantity: Decimal, price: Decimal, side: OrderSide) -> Self {
        Self {
            symbol: symbol.to_string(),
            quantity,
            order_type: OrderType::Limit(price),
            side,
            client_order_id: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum OrderType {
    Market,
    Limit(Decimal),
    Stop(Decimal),
    StopLimit(Decimal, Decimal),
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OrderType::Market => write!(f, "MARKET"),
            OrderType::Limit(price) => write!(f, "LIMIT {}", price),
            OrderType::Stop(price) => write!(f, "STOP {}", price),
            OrderType::StopLimit(stop, limit) => write!(f, "STOP_LIMIT stop={} limit={}", stop, limit),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        }
    }
}

impl From<String> for OrderSide {
    fn from(s: String) -> Self {
        match s.to_uppercase().as_str() {
            "BUY" => OrderSide::Buy,
            "SELL" => OrderSide::Sell,
            _ => OrderSide::Buy, // Default to Buy for unknown values
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderResponse {
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub status: OrderStatus,
    pub filled_quantity: Decimal,
    pub average_price: Option<Decimal>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderStatus {
    New,
    Filled,
    PartiallyFilled,
    Canceled,
    Rejected,
    Pending,
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OrderStatus::New => write!(f, "NEW"),
            OrderStatus::Filled => write!(f, "FILLED"),
            OrderStatus::PartiallyFilled => write!(f, "PARTIALLY_FILLED"),
            OrderStatus::Canceled => write!(f, "CANCELED"),
            OrderStatus::Rejected => write!(f, "REJECTED"),
            OrderStatus::Pending => write!(f, "PENDING"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TradingSignal {
    pub symbol: String,
    pub action: TradeAction,
    pub price: Decimal,
    pub confidence: f64,
    pub timestamp: i64,
    pub indicators: Vec<IndicatorValue>,
}

#[derive(Debug, Clone)]
pub enum TradeAction {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Clone)]
pub struct IndicatorValue {
    pub name: String,
    pub value: f64,
}

/// Market Data Structures
#[derive(Debug, Clone, Default)]
pub struct MarketData {
    pub symbol: String,
    pub timestamp: i64,
    pub volume: Decimal,
    pub last_price: Decimal,
    pub open_price: Decimal,
    pub close_price: Decimal,
    pub high_price: Decimal,
    pub low_price: Decimal,
    pub bid_price: Option<Decimal>,
    pub ask_price: Option<Decimal>,
    pub interval: Option<String>,
}

impl MarketData {
    // Convert from the older f64-based representation
    pub fn from_f64_based(md: &crate::domain::MarketData) -> Self {
        Self {
            symbol: md.symbol.clone(),
            timestamp: md.timestamp as i64,
            volume: Decimal::from_f64(md.volume),
            last_price: Decimal::from_f64(md.last_price),
            open_price: Decimal::from_f64(md.open_price),
            close_price: Decimal::from_f64(md.close_price),
            high_price: Decimal::from_f64(md.high_price),
            low_price: Decimal::from_f64(md.low_price),
            bid_price: None,
            ask_price: None,
            interval: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Candlestick {
    pub symbol: String,
    pub interval: String,
    pub open_time: i64,
    pub close_time: i64,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub quote_volume: Decimal,
    pub trades: i64,
}

// Generic price history container for technical analysis
#[derive(Debug, Clone)]
pub struct PriceHistory {
    pub symbol: String,
    pub interval: String,
    pub candles: Vec<Candlestick>,
}

impl PriceHistory {
    pub fn new(symbol: &str, interval: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            interval: interval.to_string(),
            candles: Vec::new(),
        }
    }

    pub fn add_candle(&mut self, candle: Candlestick) {
        self.candles.push(candle);
    }

    pub fn close_prices(&self) -> Vec<f64> {
        self.candles
            .iter()
            .map(|c| c.close.to_f64().unwrap_or_default())
            .collect()
    }

    pub fn high_prices(&self) -> Vec<f64> {
        self.candles
            .iter()
            .map(|c| c.high.to_f64().unwrap_or_default())
            .collect()
    }

    pub fn low_prices(&self) -> Vec<f64> {
        self.candles
            .iter()
            .map(|c| c.low.to_f64().unwrap_or_default())
            .collect()
    }

    pub fn volume(&self) -> Vec<f64> {
        self.candles
            .iter()
            .map(|c| c.volume.to_f64().unwrap_or_default())
            .collect()
    }
    
    pub fn timestamps(&self) -> Vec<i64> {
        self.candles.iter().map(|c| c.close_time).collect()
    }
}