// src/exchange/client.rs
use crate::domain::errors::{ExchangeError, ExchangeResult};
use crate::domain::models::{Order, OrderResponse, MarketData, PriceHistory};
use async_trait::async_trait;
use rust_decimal::Decimal;

/// Core trading client interface
#[async_trait]
pub trait ExchangeClient: Send + Sync {
    /// Connect to the exchange
    async fn connect(&mut self) -> ExchangeResult<()>;
    
    /// Disconnect from the exchange
    async fn disconnect(&mut self) -> ExchangeResult<()>;
    
    /// Get account balances
    async fn get_balances(&self) -> ExchangeResult<Vec<Balance>>;
    
    /// Get balance for a specific asset
    async fn get_balance(&self, asset: &str) -> ExchangeResult<Balance>;
    
    /// Place a new order
    async fn place_order(&self, order: &Order) -> ExchangeResult<OrderResponse>;
    
    /// Cancel an existing order
    async fn cancel_order(&self, order_id: &str) -> ExchangeResult<OrderResponse>;
    
    /// Get order status
    async fn get_order_status(&self, order_id: &str) -> ExchangeResult<OrderResponse>;
    
    /// Get open orders
    async fn get_open_orders(&self, symbol: Option<&str>) -> ExchangeResult<Vec<OrderResponse>>;
    
    /// Get historical klines (candlesticks)
    async fn get_klines(
        &self, 
        symbol: &str, 
        interval: &str, 
        limit: Option<u32>
    ) -> ExchangeResult<PriceHistory>;
    
    /// Get latest market data for a symbol
    async fn get_ticker(&self, symbol: &str) -> ExchangeResult<MarketData>;
    
    /// Subscribe to WebSocket market data stream
    async fn subscribe_to_market_data(&self, symbols: &[String], callback: Box<dyn MarketDataHandler>)
        -> ExchangeResult<()>;
}

#[derive(Debug, Clone)]
pub struct Balance {
    pub asset: String,
    pub free: Decimal,
    pub locked: Decimal,
    pub total: Decimal,
}

impl Balance {
    pub fn new(asset: &str, free: Decimal, locked: Decimal) -> Self {
        Self {
            asset: asset.to_string(),
            free,
            locked,
            total: free + locked,
        }
    }
}

/// Trait for handling market data updates
#[async_trait]
pub trait MarketDataHandler: Send + Sync {
    async fn on_kline_update(&mut self, kline: MarketData);
    async fn on_ticker_update(&mut self, ticker: MarketData);
    async fn on_error(&mut self, error: ExchangeError);
}

/// Market data subscription configuration
#[derive(Debug, Clone)]
pub struct MarketDataSubscription {
    pub symbol: String,
    pub channels: Vec<SubscriptionChannel>,
}

impl MarketDataSubscription {
    pub fn new(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            channels: Vec::new(),
        }
    }
    
    pub fn with_channel(mut self, channel: SubscriptionChannel) -> Self {
        self.channels.push(channel);
        self
    }
    
    pub fn with_klines(mut self, interval: &str) -> Self {
        self.channels.push(SubscriptionChannel::Kline(interval.to_string()));
        self
    }
    
    pub fn with_ticker(mut self) -> Self {
        self.channels.push(SubscriptionChannel::Ticker);
        self
    }
}

#[derive(Debug, Clone)]
pub enum SubscriptionChannel {
    Kline(String), // Interval
    Ticker,
    Trades,
    Depth,
    BookTicker,
}