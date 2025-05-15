
pub mod errors;
pub mod models;

// Re-export core types for backward compatibility
pub use errors::{AppError, AppResult, ExchangeError, ExchangeResult, TradingError, TradingResult};
pub use models::{
    Candlestick, MarketData, Order, OrderResponse, OrderSide, OrderStatus, OrderType, PriceHistory,
    TradeAction, TradingSignal,
};

// For backward compatibility with existing code
pub use self::models::OrderSide as OrderSideCompat;
pub use self::models::OrderStatus as OrderStatusCompat;
pub use self::models::OrderType as OrderTypeCompat;
pub use self::models::TradeAction as TradeActionCompat;

// Convert between decimal and f64 for compatibility with existing code
pub trait DecimalCompatExt {
    fn to_f64(&self) -> f64;
    fn from_f64(value: f64) -> Self;
}

impl DecimalCompatExt for rust_decimal::Decimal {
    fn to_f64(&self) -> f64 {
        self.to_f64().unwrap_or_default()
    }

    fn from_f64(value: f64) -> Self {
        rust_decimal::Decimal::from_f64(value).unwrap_or_default()
    }
}