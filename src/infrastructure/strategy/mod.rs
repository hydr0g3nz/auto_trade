// src/infrastructure/strategy/basic_strategy.rs
// Basic trading strategy implementation

use std::sync::Arc;
use std::collections::VecDeque;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::domain::model::{MarketData, TradingSignal, TradeAction, DomainError};
use crate::domain::service::{TradingStrategyService, TechnicalAnalysisService};

pub struct BasicTradingStrategy {
    analysis_service: Arc<Mutex<dyn TechnicalAnalysisService + Send + Sync>>,
    symbol: String,
    rsi_period: usize,
    rsi_overbought: f64,
    rsi_oversold: f64,
    ema_fast_period: usize,
    ema_slow_period: usize,
    price_history: VecDeque<f64>,
    max_history_size: usize,
}

impl BasicTradingStrategy {
    pub fn new(
        analysis_service: Arc<Mutex<dyn TechnicalAnalysisService + Send + Sync>>,
        symbol: String,
        rsi_period: usize,
        rsi_overbought: f64,
        rsi_oversold: f64,
        ema_fast_period: usize,
        ema_slow_period: usize,
    ) -> Self {
        // Calculate required history size based on indicator periods
        let max_period = rsi_period.max(ema_slow_period) + 10; // Add buffer
        
        Self {
            analysis_service,
            symbol,
            rsi_period,
            rsi_overbought,
            rsi_oversold,
            ema_fast_period,
            ema_slow_period,
            price_history: VecDeque::with_capacity(max_period),
            max_history_size: max_period,
        }
    }
    
    // Default strategy with common parameters
    pub fn default(analysis_service: Arc<Mutex<dyn TechnicalAnalysisService + Send + Sync>>, symbol: String) -> Self {
        Self::new(
            analysis_service,
            symbol,
            14, // RSI period
            70.0, // RSI overbought
            30.0, // RSI oversold
            5,   // Fast EMA period
            15   // Slow EMA period
        )
    }
}

#[async_trait]
impl TradingStrategyService for BasicTradingStrategy {
    async fn analyze(&self, data: &MarketData) -> Result<Option<TradingSignal>, DomainError> {
        // Check if we have enough data for analysis
        if self.price_history.len() < self.rsi_period {
            return Ok(None); // Not enough data yet
        }
        
        // Convert price history to vector for analysis
        let prices: Vec<f64> = self.price_history.iter().copied().collect();
        
        // Calculate indicators
        let rsi = self.analysis_service
            .lock()
            .await
            .calculate_rsi(&prices, self.rsi_period)
            .await?;
            
        let fast_ema = self.analysis_service
            .lock()
            .await
            .calculate_ema(&prices, self.ema_fast_period)
            .await?;
            
        let slow_ema = self.analysis_service
            .lock()
            .await
            .calculate_ema(&prices, self.ema_slow_period)
            .await?;
            
        // Determine trading action based on indicators
        let action = if let Some(rsi_value) = rsi {
            if rsi_value <= self.rsi_oversold && !fast_ema.is_empty() && !slow_ema.is_empty() 
               && fast_ema.last().unwrap() > slow_ema.last().unwrap() {
                // RSI oversold and fast EMA crossed above slow EMA - buy signal
                TradeAction::Buy
            } else if rsi_value >= self.rsi_overbought && !fast_ema.is_empty() && !slow_ema.is_empty() 
                    && fast_ema.last().unwrap() < slow_ema.last().unwrap() {
                // RSI overbought and fast EMA crossed below slow EMA - sell signal
                TradeAction::Sell
            } else {
                // No clear signal
                TradeAction::Hold
            }
        } else {
            TradeAction::Hold
        };

        // Log indicator values
        if let Some(rsi_value) = rsi {
            log::info!(
                "Strategy analysis - Symbol: {}, RSI: {:.2}, Fast EMA: {:.2}, Slow EMA: {:.2}, Action: {:?}",
                data.symbol,
                rsi_value,
                fast_ema.last().unwrap_or(&0.0),
                slow_ema.last().unwrap_or(&0.0),
                action
            );
        }

        // Generate signal
        let signal = TradingSignal {
            symbol: data.symbol.clone(),
            action,
            price: data.last_price,
            timestamp: chrono::Utc::now().timestamp(),
        };

        Ok(Some(signal))
    }
    
    async fn update_state(&mut self, data: &MarketData) -> Result<(), DomainError> {
        // Update price history with latest close price
        self.price_history.push_back(data.close_price);
        
        // Maintain max history size
        while self.price_history.len() > self.max_history_size {
            self.price_history.pop_front();
        }
        
        Ok(())
    }
}