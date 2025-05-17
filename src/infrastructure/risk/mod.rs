// src/infrastructure/risk/basic_risk_manager.rs
// Basic risk management implementation

use std::collections::HashMap;
use async_trait::async_trait;

use crate::domain::model::DomainError;
use crate::domain::service::RiskManagementService;

pub struct BasicRiskManager {
    max_position_size: f64,
    max_drawdown_percent: f64,
    max_positions: usize,
    active_positions: HashMap<String, f64>, // symbol -> size
}

impl BasicRiskManager {
    pub fn new(
        max_position_size: f64,
        max_drawdown_percent: f64,
        max_positions: usize,
    ) -> Self {
        Self {
            max_position_size,
            max_drawdown_percent,
            max_positions,
            active_positions: HashMap::new(),
        }
    }
    
    pub fn default() -> Self {
        Self::new(
            0.1, // Max 10% of portfolio in any position
            0.02, // Max 2% drawdown per trade
            5,   // Max 5 open positions at once
        )
    }
    
    pub fn add_position(&mut self, symbol: &str, size: f64) {
        self.active_positions.insert(symbol.to_string(), size);
    }
    
    pub fn remove_position(&mut self, symbol: &str) {
        self.active_positions.remove(symbol);
    }
}

#[async_trait]
impl RiskManagementService for BasicRiskManager {
    async fn validate_trade(&self, symbol: &str, quantity: f64, side: &str) -> Result<bool, DomainError> {
        // Check if we have too many positions
        if side.to_uppercase() == "BUY" && 
           self.active_positions.len() >= self.max_positions && 
           !self.active_positions.contains_key(symbol) {
            log::warn!("Risk check failed: maximum positions reached ({})", self.max_positions);
            return Ok(false);
        }
        
        // Check if position size exceeds maximum
        if quantity > self.max_position_size {
            log::warn!(
                "Risk check failed: position size ({}) exceeds maximum ({})",
                quantity,
                self.max_position_size
            );
            return Ok(false);
        }
        
        // All checks passed
        Ok(true)
    }
    
    async fn calculate_position_size(&self, symbol: &str, available_balance: f64) -> Result<f64, DomainError> {
        // Calculate position size based on risk parameters
        // This is simplified; a real implementation would consider volatility, etc.
        let max_risk_amount = available_balance * self.max_drawdown_percent;
        let position_size = (available_balance * self.max_position_size).min(max_risk_amount);
        
        log::info!(
            "Calculated position size for {}: {:.8} (from balance: {:.8})",
            symbol,
            position_size,
            available_balance
        );
        
        Ok(position_size)
    }
}