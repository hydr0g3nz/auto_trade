use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::domain::{MarketData, TradingError};

#[derive(Debug, Clone)]
pub struct MarketDataManager {
    current_data: Arc<RwLock<MarketData>>,
    price_history: Arc<RwLock<VecDeque<f64>>>,
    max_history_size: usize,
}

impl MarketDataManager {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            current_data: Arc::new(RwLock::new(MarketData::default())),
            price_history: Arc::new(RwLock::new(VecDeque::with_capacity(max_history_size))),
            max_history_size,
        }
    }

    pub async fn update_market_data(&self, data: MarketData) -> Result<(), TradingError> {
        {
            let mut current = self.current_data.write().await;
            *current = data.clone();
        }
        
        {
            let mut history = self.price_history.write().await;
            history.push_back(data.close_price);
            if history.len() > self.max_history_size {
                history.pop_front();
            }
        }
        
        Ok(())
    }

    pub async fn get_current_data(&self) -> MarketData {
        self.current_data.read().await.clone()
    }

    pub async fn get_price_history(&self) -> Vec<f64> {
        self.price_history.read().await.iter().copied().collect()
    }

    pub async fn initialize_history(&self, prices: Vec<f64>) -> Result<(), TradingError> {
        let mut history = self.price_history.write().await;
        history.clear();
        for price in prices.into_iter().take(self.max_history_size) {
            history.push_back(price);
        }
        Ok(())
    }
}