// src/config.rs
// Application configuration

use std::env;
use dotenv::dotenv;
use binance_spot_connector_rust::market::klines::KlineInterval;

#[derive(Debug, Clone)]
pub struct Config {
    pub api_key: String,
    pub api_secret: String,
    pub trading_enabled: bool,
    pub log_level: String,
    pub symbols: Vec<String>,
    pub kline_interval: KlineInterval,
    pub risk_max_position_size: f64,
    pub risk_max_drawdown_percent: f64,
    pub risk_max_positions: usize,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        // Load .env file if present
        dotenv().ok();
        
        let api_key = env::var("BINANCE_API_KEY")?;
        let api_secret = env::var("BINANCE_API_SECRET")?;
        
        // Parse optional values with defaults
        let trading_enabled = env::var("TRADING_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse::<bool>()
            .unwrap_or(false);
            
        let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        
        let symbols_str = env::var("TRADING_SYMBOLS").unwrap_or_else(|_| "BTCUSDT".to_string());
        let symbols = symbols_str.split(',')
            .map(|s| s.trim().to_string())
            .collect();
            
        let interval_str = env::var("KLINE_INTERVAL").unwrap_or_else(|_| "1m".to_string());
        let kline_interval = match interval_str.as_str() {
            "1m" => KlineInterval::Minutes1,
            "5m" => KlineInterval::Minutes5,
            "15m" => KlineInterval::Minutes15,
            "1h" => KlineInterval::Hours1,
            "4h" => KlineInterval::Hours4,
            "1d" => KlineInterval::Days1,
            _ => KlineInterval::Minutes1,
        };
        
        let risk_max_position_size = env::var("RISK_MAX_POSITION_SIZE")
            .unwrap_or_else(|_| "0.1".to_string())
            .parse::<f64>()
            .unwrap_or(0.1);
            
        let risk_max_drawdown_percent = env::var("RISK_MAX_DRAWDOWN_PERCENT")
            .unwrap_or_else(|_| "0.02".to_string())
            .parse::<f64>()
            .unwrap_or(0.02);
            
        let risk_max_positions = env::var("RISK_MAX_POSITIONS")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<usize>()
            .unwrap_or(5);
        
        Ok(Self {
            api_key,
            api_secret,
            trading_enabled,
            log_level,
            symbols,
            kline_interval,
            risk_max_position_size,
            risk_max_drawdown_percent,
            risk_max_positions,
        })
    }
}