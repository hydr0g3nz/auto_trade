// src/application/service/mod.rs
// Application services

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::domain::model::{MarketData, Order, OrderResponse, TradingSignal, DomainError};
use crate::domain::repository::{ExchangeRepository, MarketDataRepository};
use crate::domain::service::{TradingStrategyService, RiskManagementService};
use crate::application::dto::ApplicationError;

// Convert between domain and application errors
impl From<DomainError> for ApplicationError {
    fn from(error: DomainError) -> Self {
        ApplicationError::DomainError(error.to_string())
    }
}

#[async_trait]
pub trait TradingService {
    /// Start the trading service
    async fn start(&mut self) -> Result<(), ApplicationError>;
    
    /// Stop the trading service
    async fn stop(&mut self) -> Result<(), ApplicationError>;
    
    /// Execute a trade based on a signal
    async fn execute_trade(&self, signal: TradingSignal) -> Result<OrderResponse, ApplicationError>;
    
    /// Get the current market data for a symbol
    async fn get_market_data(&self, symbol: &str) -> Result<MarketData, ApplicationError>;
    
    /// Get the historical market data for a symbol
    async fn get_historical_data(&self, symbol: &str, interval: &str, limit: usize) -> Result<Vec<MarketData>, ApplicationError>;
}

pub struct TradingServiceImpl {
    exchange_repository: Arc<Mutex<dyn ExchangeRepository + Send + Sync>>,
    market_data_repository: Arc<Mutex<dyn MarketDataRepository + Send + Sync>>,
    trading_strategy: Arc<Mutex<dyn TradingStrategyService + Send + Sync>>,
    risk_management: Arc<Mutex<dyn RiskManagementService + Send + Sync>>,
    active_symbols: Vec<String>,
    is_running: bool,
}

impl TradingServiceImpl {
    pub fn new(
        exchange_repository: Arc<Mutex<dyn ExchangeRepository + Send + Sync>>,
        market_data_repository: Arc<Mutex<dyn MarketDataRepository + Send + Sync>>,
        trading_strategy: Arc<Mutex<dyn TradingStrategyService + Send + Sync>>,
        risk_management: Arc<Mutex<dyn RiskManagementService + Send + Sync>>,
    ) -> Self {
        Self {
            exchange_repository,
            market_data_repository,
            trading_strategy,
            risk_management,
            active_symbols: Vec::new(),
            is_running: false,
        }
    }
    
    pub fn add_symbol(&mut self, symbol: String) {
        if !self.active_symbols.contains(&symbol) {
            self.active_symbols.push(symbol);
        }
    }
}

#[async_trait]
impl TradingService for TradingServiceImpl {
    async fn start(&mut self) -> Result<(), ApplicationError> {
        // Connect to exchange
        self.exchange_repository.lock().await.connect().await?;
        
        // Subscribe to market data for all active symbols
        for symbol in &self.active_symbols {
            self.market_data_repository
                .lock()
                .await
                .subscribe_to_market_data(symbol)
                .await?;
        }
        
        self.is_running = true;
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<(), ApplicationError> {
        // Unsubscribe from market data
        for symbol in &self.active_symbols {
            self.market_data_repository
                .lock()
                .await
                .unsubscribe_from_market_data(symbol)
                .await?;
        }
        
        // Disconnect from exchange
        self.exchange_repository.lock().await.disconnect().await?;
        
        self.is_running = false;
        Ok(())
    }
    
    async fn execute_trade(&self, signal: TradingSignal) -> Result<OrderResponse, ApplicationError> {
        // Check if trade meets risk criteria
        let quantity = 0.01; // Would be calculated based on risk profile
        
        let risk_validated = self.risk_management
            .lock()
            .await
            .validate_trade(&signal.symbol, quantity, match signal.action {
                crate::domain::model::TradeAction::Buy => "BUY",
                crate::domain::model::TradeAction::Sell => "SELL",
                _ => return Err(ApplicationError::DomainError("Cannot execute HOLD action".into())),
            })
            .await?;
            
        if !risk_validated {
            return Err(ApplicationError::DomainError("Trade failed risk validation".into()));
        }
        
        // Create order
        let order = Order {
            symbol: signal.symbol.clone(),
            quantity,
            order_type: crate::domain::model::OrderType::Market,
            side: match signal.action {
                crate::domain::model::TradeAction::Buy => crate::domain::model::OrderSide::Buy,
                crate::domain::model::TradeAction::Sell => crate::domain::model::OrderSide::Sell,
                _ => return Err(ApplicationError::DomainError("Cannot execute HOLD action".into())),
            },
        };
        
        // Send order to exchange
        let response = self.exchange_repository.lock().await.send_order(&order).await?;
        
        Ok(response)
    }
    
    async fn get_market_data(&self, symbol: &str) -> Result<MarketData, ApplicationError> {
        self.market_data_repository
            .lock()
            .await
            .get_latest_data(symbol)
            .await
            .map_err(ApplicationError::from)
    }
    
    async fn get_historical_data(&self, symbol: &str, interval: &str, limit: usize) -> Result<Vec<MarketData>, ApplicationError> {
        // This would typically query historical data and convert to MarketData format
        // For now, just return an empty vector as it's unimplemented
        Ok(Vec::new())
    }
    
    /// Get historical prices as a vector of floats
    async fn get_historical_prices(&self, symbol: &str, interval: &str, limit: usize) -> Result<Vec<f64>, ApplicationError> {
        let prices = self.exchange_repository
            .lock()
            .await
            .get_historical_prices(symbol, interval, limit)
            .await?;
        
        Ok(prices)
    }
}