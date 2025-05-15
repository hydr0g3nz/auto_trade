// src/market_data/processor.rs
use crate::domain::errors::{MarketDataError, MarketDataResult};
use crate::domain::models::{Candlestick, MarketData, PriceHistory};
use crate::exchange::client::MarketDataHandler;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

const MAX_CANDLES: usize = 1000;

/// Market data processor that stores and manages market data
pub struct MarketDataProcessor {
    // Store price histories by symbol and interval
    price_histories: Arc<Mutex<HashMap<String, HashMap<String, PriceHistory>>>>,
    
    // Store the latest market data by symbol
    latest_data: Arc<Mutex<HashMap<String, MarketData>>>,
    
    // Signal channel for new data
    data_tx: broadcast::Sender<MarketData>,
}

impl MarketDataProcessor {
    /// Create a new market data processor
    pub fn new() -> Self {
        // Create broadcast channel with buffer size of 100
        let (data_tx, _) = broadcast::channel(100);
        
        Self {
            price_histories: Arc::new(Mutex::new(HashMap::new())),
            latest_data: Arc::new(Mutex::new(HashMap::new())),
            data_tx,
        }
    }
    
    /// Subscribe to market data updates
    pub fn subscribe(&self) -> broadcast::Receiver<MarketData> {
        self.data_tx.subscribe()
    }
    
    /// Get the latest market data for a symbol
    pub fn get_latest_data(&self, symbol: &str) -> Option<MarketData> {
        self.latest_data
            .lock()
            .unwrap()
            .get(symbol)
            .cloned()
    }
    
    /// Get price history for a symbol and interval
    pub fn get_price_history(&self, symbol: &str, interval: &str) -> Option<PriceHistory> {
        self.price_histories
            .lock()
            .unwrap()
            .get(symbol)
            .and_then(|intervals| intervals.get(interval))
            .cloned()
    }
    
    /// Add a candlestick to price history
    pub fn add_candlestick(&self, symbol: &str, interval: &str, candle: Candlestick) {
        let mut histories = self.price_histories.lock().unwrap();
        let symbol_histories = histories
            .entry(symbol.to_string())
            .or_insert_with(HashMap::new);
        
        let history = symbol_histories
            .entry(interval.to_string())
            .or_insert_with(|| PriceHistory::new(symbol, interval));
        
        // Add the candle and limit the history size
        history.add_candle(candle);
        
        if history.candles.len() > MAX_CANDLES {
            // Remove oldest candles if we've exceeded the maximum
            let excess = history.candles.len() - MAX_CANDLES;
            history.candles.drain(0..excess);
        }
    }
    
    /// Add a complete price history
    pub fn add_price_history(&self, history: PriceHistory) {
        let mut histories = self.price_histories.lock().unwrap();
        let symbol_histories = histories
            .entry(history.symbol.clone())
            .or_insert_with(HashMap::new);
        
        symbol_histories.insert(history.interval.clone(), history);
    }
    
    /// Update market data and notify subscribers
    fn update_market_data(&self, data: MarketData) -> MarketDataResult<()> {
        // Update latest data
        {
            let mut latest = self.latest_data.lock().unwrap();
            latest.insert(data.symbol.clone(), data.clone());
        }
        
        // Notify subscribers
        if let Err(e) = self.data_tx.send(data) {
            log::warn!("Failed to broadcast market data: {}", e);
            // This is not a fatal error as it just means there are no subscribers
        }
        
        Ok(())
    }
}

#[async_trait]
impl MarketDataHandler for MarketDataProcessor {
    async fn on_kline_update(&mut self, kline: MarketData) {
        // Create a candlestick from the kline data
        if let Some(interval) = &kline.interval {
            let candle = Candlestick {
                symbol: kline.symbol.clone(),
                interval: interval.clone(),
                open_time: kline.timestamp - 60000, // Assuming 1-minute candle
                close_time: kline.timestamp,
                open: kline.open_price,
                high: kline.high_price,
                low: kline.low_price,
                close: kline.close_price,
                volume: kline.volume,
                quote_volume: Decimal::ZERO, // Not available in MarketData
                trades: 0, // Not available in MarketData
            };
            
            // Add the candlestick to history
            self.add_candlestick(&kline.symbol, interval, candle);
        }
        
        // Update latest data and notify subscribers
        if let Err(e) = self.update_market_data(kline) {
            log::error!("Failed to update market data: {:?}", e);
        }
    }
    
    async fn on_ticker_update(&mut self, ticker: MarketData) {
        // Update latest data and notify subscribers
        if let Err(e) = self.update_market_data(ticker) {
            log::error!("Failed to update ticker data: {:?}", e);
        }
    }
    
    async fn on_error(&mut self, error: crate::domain::errors::ExchangeError) {
        log::error!("Exchange error in market data handler: {:?}", error);
    }
}