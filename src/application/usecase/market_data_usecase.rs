// src/application/usecase/market_data_usecase.rs
// Market data processing use cases

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use std::collections::VecDeque;

use crate::domain::model::{MarketData, TradingSignal, TradeAction, DomainError};
use crate::domain::service::TradingStrategyService;
use crate::application::dto::ApplicationError;
use crate::application::service::TradingService;

/// Market data processing use case
#[async_trait]
pub trait MarketDataProcessingUseCase {
    async fn process_market_data(&self, market_data: MarketData) -> Result<(), ApplicationError>;
}

pub struct MarketDataProcessor {
    trading_strategy: Arc<Mutex<dyn TradingStrategyService + Send + Sync>>,
    trading_service: Arc<Mutex<dyn TradingService + Send + Sync>>,
    signal_sender: mpsc::Sender<TradingSignal>,
    price_history: Arc<Mutex<VecDeque<f64>>>,
    window_size: usize,
}

impl MarketDataProcessor {
    pub fn new(
        trading_strategy: Arc<Mutex<dyn TradingStrategyService + Send + Sync>>,
        trading_service: Arc<Mutex<dyn TradingService + Send + Sync>>,
        signal_sender: mpsc::Sender<TradingSignal>,
        window_size: usize,
    ) -> Self {
        Self {
            trading_strategy,
            trading_service,
            signal_sender,
            price_history: Arc::new(Mutex::new(VecDeque::with_capacity(window_size))),
            window_size,
        }
    }
    
    async fn update_price_history(&self, price: f64) -> Result<(), ApplicationError> {
        let mut history = self.price_history.lock().await;
        if history.len() >= self.window_size {
            history.pop_front();
        }
        history.push_back(price);
        Ok(())
    }
}

#[async_trait]
impl MarketDataProcessingUseCase for MarketDataProcessor {
    async fn process_market_data(&self, market_data: MarketData) -> Result<(), ApplicationError> {
        // Update strategy with new data
        self.trading_strategy
            .lock()
            .await
            .update_state(&market_data)
            .await?;
            
        // Update price history
        self.update_price_history(market_data.close_price).await?;
        
        // Generate trading signal based on latest data
        let signal_option = self.trading_strategy
            .lock()
            .await
            .analyze(&market_data)
            .await?;
        
        // If a signal was generated, send it to the signal processor
        if let Some(signal) = signal_option {
            if signal.action != TradeAction::Hold {
                self.signal_sender.send(signal).await
                    .map_err(|e| ApplicationError::DomainError(format!("Failed to send signal: {}", e)))?;
            }
        }
        
        Ok(())
    }
}