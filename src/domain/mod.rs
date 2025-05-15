// src/domain/mod.rs
pub mod errors;
pub mod models;

// Re-export common types for convenience
pub use errors::{AppError, AppResult, ExchangeError, ExchangeResult, TradingError, TradingResult};
pub use models::{
    Candlestick, MarketData, Order, OrderResponse, OrderSide, OrderStatus, OrderType, PriceHistory,
    TradeAction, TradingSignal,
};