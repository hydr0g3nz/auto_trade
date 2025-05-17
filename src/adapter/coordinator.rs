// src/interface/coordinator.rs
// Trading system coordinator

use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use std::collections::HashMap;

use crate::domain::model::{MarketData, TradingSignal};
use crate::domain::repository::{ExchangeRepository, MarketDataRepository};
use crate::domain::service::{TradingStrategyService, RiskManagementService, TechnicalAnalysisService};
use crate::application::service::{TradingService, TradingServiceImpl};
use crate::application::usecase::{MarketDataProcessingUseCase, MarketDataProcessor, SignalProcessingUseCase, SignalProcessor};
use crate::application::dto::ApplicationError;

pub struct TradingCoordinator {
    trading_service: Arc<Mutex<dyn TradingService + Send + Sync>>,
    market_data_processor: Arc<Mutex<dyn MarketDataProcessingUseCase + Send + Sync>>,
    signal_processor: Arc<Mutex<dyn SignalProcessingUseCase + Send + Sync>>,
    symbols: Vec<String>,
    market_data_receivers: HashMap<String, mpsc::Receiver<MarketData>>,
    signal_receiver: mpsc::Receiver<TradingSignal>,
    running: bool,
}

impl TradingCoordinator {
    pub fn new(
        exchange_repository: Arc<Mutex<dyn ExchangeRepository + Send + Sync>>,
        market_data_repository: Arc<Mutex<dyn MarketDataRepository + Send + Sync>>,
        trading_strategy: Arc<Mutex<dyn TradingStrategyService + Send + Sync>>,
        risk_management: Arc<Mutex<dyn RiskManagementService + Send + Sync>>,
        technical_analysis: Arc<Mutex<dyn TechnicalAnalysisService + Send + Sync>>,
        symbols: Vec<String>,
    ) -> Self {
        // Create channels
        let (signal_tx, signal_rx) = mpsc::channel::<TradingSignal>(100);
        
        // Create service and use case implementations
        let trading_service = Arc::new(Mutex::new(TradingServiceImpl::new(
            exchange_repository,
            market_data_repository.clone(),
            trading_strategy.clone(),
            risk_management,
        )));
        
        let market_data_processor = Arc::new(Mutex::new(MarketDataProcessor::new(
            trading_strategy,
            trading_service.clone(),
            signal_tx,
            30, // Window size
        )));
        
        let signal_processor = Arc::new(Mutex::new(SignalProcessor::new(
            trading_service.clone(),
        )));
        
        Self {
            trading_service,
            market_data_processor,
            signal_processor,
            symbols,
            market_data_receivers: HashMap::new(),
            signal_receiver: signal_rx,
            running: false,
        }
    }
    
    pub async fn start(&mut self) -> Result<(), ApplicationError> {
        // Initialize trading service
        let mut trading_service = self.trading_service.lock().await;
        
        // Add symbols to trading service
        for symbol in &self.symbols {
            trading_service.add_symbol(symbol.clone());
        }
        
        // Start trading service
        trading_service.start().await?;
        
        // Release lock
        drop(trading_service);
        
        // Start background tasks
        self.spawn_market_data_processor().await;
        self.spawn_signal_processor().await;
        
        self.running = true;
        log::info!("Trading coordinator started");
        
        Ok(())
    }
    
    pub async fn stop(&mut self) -> Result<(), ApplicationError> {
        if !self.running {
            return Ok(());
        }
        
        // Stop trading service
        self.trading_service.lock().await.stop().await?;
        
        self.running = false;
        log::info!("Trading coordinator stopped");
        
        Ok(())
    }
    
    async fn spawn_market_data_processor(&self) {
        let market_data_processor = self.market_data_processor.clone();
        
        tokio::spawn(async move {
            // In a complete implementation, this would process market data from receivers
            
            log::info!("Market data processor started");
        });
    }
    
    async fn spawn_signal_processor(&self) {
        let signal_processor = self.signal_processor.clone();
        let mut signal_receiver = self.signal_receiver.clone();
        
        tokio::spawn(async move {
            while let Some(signal) = signal_receiver.recv().await {
                if let Err(e) = signal_processor.lock().await.process_signal(signal).await {
                    log::error!("Error processing signal: {}", e);
                }
            }
            
            log::info!("Signal processor stopped");
        });
    }
}