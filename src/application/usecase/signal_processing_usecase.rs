// src/application/usecase/signal_processing_usecase.rs
// Signal processing use cases

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::domain::model::{TradingSignal, TradeAction};
use crate::application::dto::ApplicationError;
use crate::application::service::TradingService;

/// Signal processing use case
#[async_trait]
pub trait SignalProcessingUseCase {
    async fn process_signal(&self, signal: TradingSignal) -> Result<(), ApplicationError>;
}

pub struct SignalProcessor {
    trading_service: Arc<Mutex<dyn TradingService + Send + Sync>>,
}

impl SignalProcessor {
    pub fn new(trading_service: Arc<Mutex<dyn TradingService + Send + Sync>>) -> Self {
        Self { trading_service }
    }
}

#[async_trait]
impl SignalProcessingUseCase for SignalProcessor {
    async fn process_signal(&self, signal: TradingSignal) -> Result<(), ApplicationError> {
        match signal.action {
            TradeAction::Buy | TradeAction::Sell => {
                log::info!(
                    "{} Signal - Symbol: {}, Price: {}",
                    match signal.action {
                        TradeAction::Buy => "Buy",
                        TradeAction::Sell => "Sell",
                        _ => unreachable!(),
                    },
                    signal.symbol,
                    signal.price
                );
                
                // Execute the trade via trading service
                let response = self.trading_service
                    .lock()
                    .await
                    .execute_trade(signal)
                    .await?;
                    
                log::info!("Order executed with ID: {}", response.order_id);
            },
            TradeAction::Hold => {
                log::debug!(
                    "Hold Position - Symbol: {}, Price: {}",
                    signal.symbol,
                    signal.price
                );
                // No action needed for hold signals
            }
        }
        
        Ok(())
    }
}