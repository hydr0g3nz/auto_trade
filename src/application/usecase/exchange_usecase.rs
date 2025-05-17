// src/application/usecase/exchange_usecase.rs
// Exchange use cases for order management

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::domain::model::{Order, OrderResponse, OrderType, OrderSide, DomainError};
use crate::domain::repository::ExchangeRepository;
use crate::domain::service::RiskManagementService;
use crate::application::dto::ApplicationError;

/// Order management use case
#[async_trait]
pub trait OrderManagementUseCase {
    async fn place_market_order(
        &self, 
        symbol: &str, 
        side: OrderSide, 
        quantity: f64
    ) -> Result<OrderResponse, ApplicationError>;
    
    async fn place_limit_order(
        &self, 
        symbol: &str, 
        side: OrderSide, 
        quantity: f64, 
        price: f64
    ) -> Result<OrderResponse, ApplicationError>;
    
    async fn cancel_order(&self, order_id: &str) -> Result<(), ApplicationError>;
}

pub struct OrderManager {
    exchange_repository: Arc<Mutex<dyn ExchangeRepository + Send + Sync>>,
    risk_service: Arc<Mutex<dyn RiskManagementService + Send + Sync>>,
}

impl OrderManager {
    pub fn new(
        exchange_repository: Arc<Mutex<dyn ExchangeRepository + Send + Sync>>,
        risk_service: Arc<Mutex<dyn RiskManagementService + Send + Sync>>,
    ) -> Self {
        Self {
            exchange_repository,
            risk_service,
        }
    }
    
    async fn validate_order(&self, symbol: &str, side: &OrderSide, quantity: f64) -> Result<bool, ApplicationError> {
        // Convert side to string for risk validation
        let side_str = match side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };
        
        // Check if trade passes risk validation
        let validation_result = self.risk_service
            .lock()
            .await
            .validate_trade(symbol, quantity, side_str)
            .await
            .map_err(|e| ApplicationError::DomainError(e.to_string()))?;
            
        Ok(validation_result)
    }
}

#[async_trait]
impl OrderManagementUseCase for OrderManager {
    async fn place_market_order(
        &self, 
        symbol: &str, 
        side: OrderSide, 
        quantity: f64
    ) -> Result<OrderResponse, ApplicationError> {
        // Validate order with risk management
        let is_valid = self.validate_order(symbol, &side, quantity).await?;
        
        if !is_valid {
            return Err(ApplicationError::DomainError("Order failed risk validation".into()));
        }
        
        // Create order
        let order = Order {
            symbol: symbol.to_string(),
            quantity,
            order_type: OrderType::Market,
            side: side.clone(),
        };
        
        // Send order to exchange
        let response = self.exchange_repository
            .lock()
            .await
            .send_order(&order)
            .await
            .map_err(|e| ApplicationError::DomainError(e.to_string()))?;
            
        Ok(response)
    }
    
    async fn place_limit_order(
        &self, 
        symbol: &str, 
        side: OrderSide, 
        quantity: f64, 
        price: f64
    ) -> Result<OrderResponse, ApplicationError> {
        // Validate order with risk management
        let is_valid = self.validate_order(symbol, &side, quantity).await?;
        
        if !is_valid {
            return Err(ApplicationError::DomainError("Order failed risk validation".into()));
        }
        
        // Create order
        let order = Order {
            symbol: symbol.to_string(),
            quantity,
            order_type: OrderType::Limit(price),
            side: side.clone(),
        };
        
        // Send order to exchange
        let response = self.exchange_repository
            .lock()
            .await
            .send_order(&order)
            .await
            .map_err(|e| ApplicationError::DomainError(e.to_string()))?;
            
        Ok(response)
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<(), ApplicationError> {
        self.exchange_repository
            .lock()
            .await
            .cancel_order(order_id)
            .await
            .map_err(|e| ApplicationError::DomainError(e.to_string()))
    }
}