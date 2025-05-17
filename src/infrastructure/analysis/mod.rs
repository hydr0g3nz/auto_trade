// src/infrastructure/analysis/technical_analysis.rs
// Implementation of technical analysis service

use async_trait::async_trait;
use crate::domain::model::DomainError;
use crate::domain::service::TechnicalAnalysisService;

pub struct TechnicalAnalysisImpl;

impl TechnicalAnalysisImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TechnicalAnalysisService for TechnicalAnalysisImpl {
    async fn calculate_rsi(&self, prices: &[f64], period: usize) -> Result<Option<f64>, DomainError> {
        if prices.len() < period + 1 {
            return Ok(None); // Not enough data
        }

        let mut gains = vec![0.0; prices.len()];
        let mut losses = vec![0.0; prices.len()];

        for i in 1..prices.len() {
            let change = prices[i] - prices[i - 1];
            if change > 0.0 {
                gains[i] = change;
            } else {
                losses[i] = -change;
            }
        }

        // Calculate initial SMA for gains and losses
        let mut avg_gain = gains.iter().skip(1).take(period).sum::<f64>() / period as f64;
        let mut avg_loss = losses.iter().skip(1).take(period).sum::<f64>() / period as f64;

        let smoothing_factor = 2.0 / (period as f64 + 1.0);

        // Apply smoothing for remaining periods (Wilder's RSI uses a modified EMA)
        for i in (period + 1)..prices.len() {
            avg_gain = (gains[i] * smoothing_factor) + (avg_gain * (1.0 - smoothing_factor));
            avg_loss = (losses[i] * smoothing_factor) + (avg_loss * (1.0 - smoothing_factor));
        }

        if avg_loss == 0.0 {
            return Ok(Some(100.0));
        }

        let rs = avg_gain / avg_loss;
        Ok(Some(100.0 - (100.0 / (1.0 + rs))))
    }
    
    async fn calculate_ema(&self, prices: &[f64], period: usize) -> Result<Vec<f64>, DomainError> {
        if prices.len() < period {
            return Ok(Vec::new());
        }

        // Use only the last n elements, where n = period
        let start_idx = prices.len().saturating_sub(period);
        let prices_to_use = &prices[start_idx..];

        let multiplier = 2.0 / (period + 1) as f64;
        let mut ema = Vec::with_capacity(prices_to_use.len());

        // First EMA uses SMA of the selected data
        let first_sma = prices_to_use.iter().sum::<f64>() / prices_to_use.len() as f64;
        ema.push(first_sma);

        // Calculate subsequent EMAs
        for i in 1..prices_to_use.len() {
            let new_ema = (prices_to_use[i] - ema[ema.len() - 1]) * multiplier + ema[ema.len() - 1];
            ema.push(new_ema);
        }

        Ok(ema)
    }
    
    async fn calculate_macd(
        &self,
        prices: &[f64],
        fast_period: usize,
        slow_period: usize,
        signal_period: usize
    ) -> Result<(Vec<f64>, Vec<f64>), DomainError> {
        let fast_ema = self.calculate_ema(prices, fast_period).await?;
        let slow_ema = self.calculate_ema(prices, slow_period).await?;

        // MACD line = Fast EMA - Slow EMA
        let mut macd_line = Vec::with_capacity(fast_ema.len());
        for i in 0..fast_ema.len().min(slow_ema.len()) {
            macd_line.push(fast_ema[i] - slow_ema[i]);
        }

        // Signal line = EMA of MACD line
        let signal_line = self.calculate_ema(&macd_line, signal_period).await?;

        Ok((macd_line, signal_line))
    }
}