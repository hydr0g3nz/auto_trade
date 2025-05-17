// src/application/usecase/analysis_usecase.rs
// Technical analysis use cases

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::domain::service::TechnicalAnalysisService;
use crate::application::dto::ApplicationError;

/// Technical analysis use case
#[async_trait]
pub trait TechnicalAnalysisUseCase {
    async fn calculate_indicators(
        &self,
        prices: &[f64],
        rsi_period: usize,
        fast_ema_period: usize,
        slow_ema_period: usize,
    ) -> Result<IndicatorResults, ApplicationError>;
}

pub struct IndicatorResults {
    pub rsi: Option<f64>,
    pub fast_ema: Option<Vec<f64>>,
    pub slow_ema: Option<Vec<f64>>,
    pub macd_line: Option<Vec<f64>>,
    pub macd_signal: Option<Vec<f64>>,
}

pub struct TechnicalAnalysisProcessor {
    analysis_service: Arc<Mutex<dyn TechnicalAnalysisService + Send + Sync>>,
}

impl TechnicalAnalysisProcessor {
    pub fn new(analysis_service: Arc<Mutex<dyn TechnicalAnalysisService + Send + Sync>>) -> Self {
        Self { analysis_service }
    }
}

#[async_trait]
impl TechnicalAnalysisUseCase for TechnicalAnalysisProcessor {
    async fn calculate_indicators(
        &self,
        prices: &[f64],
        rsi_period: usize,
        fast_ema_period: usize,
        slow_ema_period: usize,
    ) -> Result<IndicatorResults, ApplicationError> {
        // Lock the analysis service once to reduce contention
        let analysis_service = self.analysis_service.lock().await;
        
        // Calculate RSI
        let rsi = analysis_service
            .calculate_rsi(prices, rsi_period)
            .await
            .map_err(|e| ApplicationError::DomainError(e.to_string()))?;
            
        // Calculate EMAs
        let fast_ema = if prices.len() >= fast_ema_period {
            Some(analysis_service
                .calculate_ema(prices, fast_ema_period)
                .await
                .map_err(|e| ApplicationError::DomainError(e.to_string()))?)
        } else {
            None
        };
        
        let slow_ema = if prices.len() >= slow_ema_period {
            Some(analysis_service
                .calculate_ema(prices, slow_ema_period)
                .await
                .map_err(|e| ApplicationError::DomainError(e.to_string()))?)
        } else {
            None
        };
        
        // Calculate MACD (if we have enough data)
        let (macd_line, macd_signal) = if prices.len() >= slow_ema_period + 9 {
            let (line, signal) = analysis_service
                .calculate_macd(prices, fast_ema_period, slow_ema_period, 9)
                .await
                .map_err(|e| ApplicationError::DomainError(e.to_string()))?;
                
            (Some(line), Some(signal))
        } else {
            (None, None)
        };
        
        Ok(IndicatorResults {
            rsi,
            fast_ema,
            slow_ema,
            macd_line,
            macd_signal,
        })
    }
}