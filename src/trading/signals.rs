// src/trading/signals.rs
use crate::domain::errors::{TradingError, TradingResult};
use crate::domain::models::{TradingSignal, TradeAction, PriceHistory, MarketData};
use crate::market_data::processor::MarketDataProcessor;
use crate::trading::strategies::TradingStrategy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::time::{Duration, Instant};

/// Signal processor that runs strategies and generates trading signals
pub struct SignalProcessor {
    // Market data source
    market_data: Arc<MarketDataProcessor>,
    
    // Trading strategies by ID
    strategies: Arc<Mutex<HashMap<String, Box<dyn TradingStrategy>>>>,
    
    // Signal broadcast channel
    signal_tx: broadcast::Sender<TradingSignal>,
    
    // Running flag
    running: Arc<Mutex<bool>>,
}

impl SignalProcessor {
    /// Create a new signal processor
    pub fn new(market_data: Arc<MarketDataProcessor>) -> Self {
        let (signal_tx, _) = broadcast::channel(100);
        
        Self {
            market_data,
            strategies: Arc::new(Mutex::new(HashMap::new())),
            signal_tx,
            running: Arc::new(Mutex::new(false)),
        }
    }
    
    /// Add a trading strategy
    pub fn add_strategy(&self, id: &str, strategy: Box<dyn TradingStrategy>) {
        let mut strategies = self.strategies.lock().unwrap();
        strategies.insert(id.to_string(), strategy);
    }
    
    /// Remove a trading strategy
    pub fn remove_strategy(&self, id: &str) -> Option<Box<dyn TradingStrategy>> {
        let mut strategies = self.strategies.lock().unwrap();
        strategies.remove(id)
    }
    
    /// Subscribe to trading signals
    pub fn subscribe(&self) -> broadcast::Receiver<TradingSignal> {
        self.signal_tx.subscribe()
    }
    
    /// Start the signal processor
    pub async fn start(&self, symbols: Vec<String>, interval: &str) -> TradingResult<()> {
        // Set running flag
        {
            let mut running = self.running.lock().unwrap();
            if *running {
                return Err(TradingError::Signal("Signal processor already running".to_string()));
            }
            *running = true;
        }
        
        // Clone necessary values for the task
        let market_data = self.market_data.clone();
        let strategies = self.strategies.clone();
        let signal_tx = self.signal_tx.clone();
        let running = self.running.clone();
        let interval_str = interval.to_string();
        
        // Start the processing loop
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(10));
            
            while *running.lock().unwrap() {
                timer.tick().await;
                
                for symbol in &symbols {
                    // Get the latest price history
                    if let Some(history) = market_data.get_price_history(symbol, &interval_str) {
                        // Run each strategy
                        let strategies = strategies.lock().unwrap();
                        for strategy in strategies.values() {
                            match strategy.analyze(&history).await {
                                Ok(Some(signal)) => {
                                    // Broadcast the signal
                                    log::info!("Generated signal: {:?}", signal);
                                    if let Err(e) = signal_tx.send(signal) {
                                        log::error!("Failed to broadcast signal: {}", e);
                                    }
                                }
                                Ok(None) => {
                                    // No signal generated
                                }
                                Err(e) => {
                                    log::error!("Strategy error: {:?}", e);
                                }
                            }
                        }
                    } else {
                        log::warn!("No price history for {}/{}", symbol, interval_str);
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Stop the signal processor
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }
    
    /// Check if the signal processor is running
    pub fn is_running(&self) -> bool {
        let running = self.running.lock().unwrap();
        *running
    }
    
    /// Generate a signal from market data (for manual signals)
    pub fn generate_signal(
        &self,
        symbol: &str,
        action: TradeAction,
        confidence: f64,
    ) -> TradingResult<()> {
        // Get the latest market data
        let market_data = self.market_data.get_latest_data(symbol)
            .ok_or_else(|| TradingError::Signal(format!("No market data available for {}", symbol)))?;
        
        // Create a signal
        let signal = TradingSignal {
            symbol: symbol.to_string(),
            action,
            price: market_data.last_price,
            confidence,
            timestamp: chrono::Utc::now().timestamp_millis(),
            indicators: Vec::new(),
        };
        
        // Broadcast the signal
        self.signal_tx.send(signal)
            .map_err(|e| TradingError::Signal(format!("Failed to broadcast signal: {}", e)))?;
        
        Ok(())
    }
    
    /// Get all active strategies
    pub fn get_strategies(&self) -> Vec<String> {
        let strategies = self.strategies.lock().unwrap();
        strategies.keys().cloned().collect()
    }
}