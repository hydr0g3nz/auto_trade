use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub symbol: String,
    pub rsi_period: usize,
    pub ema_fast_period: usize,
    pub ema_slow_period: usize,
    pub historical_window: usize,
    pub buy_threshold: f64,
    pub sell_threshold: f64,
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            symbol: "BTCUSDT".to_string(),
            rsi_period: 14,
            ema_fast_period: 5,
            ema_slow_period: 15,
            historical_window: 50,
            buy_threshold: -2.0,
            sell_threshold: 2.0,
        }
    }
}