// src/trading/strategies.rs
use crate::analysis::indicators;
use crate::domain::errors::{TradingError, TradingResult};
use crate::domain::models::{MarketData, PriceHistory, TradeAction, TradingSignal};
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::sync::Arc;

/// Trading strategy trait that all strategies must implement
#[async_trait]
pub trait TradingStrategy: Send + Sync {
    /// Get the name of the strategy
    fn name(&self) -> &str;
    
    /// Get the description of the strategy
    fn description(&self) -> &str;
    
    /// Analyze market data and generate trading signals
    async fn analyze(&self, data: &PriceHistory) -> TradingResult<Option<TradingSignal>>;
    
    /// Get strategy parameters
    fn parameters(&self) -> Vec<StrategyParameter>;
    
    /// Update strategy parameters
    fn update_parameter(&mut self, name: &str, value: ParameterValue) -> TradingResult<()>;
}

/// Strategy parameter value
#[derive(Debug, Clone)]
pub enum ParameterValue {
    Integer(i64),
    Decimal(Decimal),
    Float(f64),
    Boolean(bool),
    String(String),
}

/// Strategy parameter
#[derive(Debug, Clone)]
pub struct StrategyParameter {
    pub name: String,
    pub description: String,
    pub value: ParameterValue,
    pub range: Option<ParameterRange>,
}

/// Parameter value range
#[derive(Debug, Clone)]
pub enum ParameterRange {
    Integer(i64, i64),
    Decimal(Decimal, Decimal),
    Float(f64, f64),
}

/// Simple Moving Average Crossover Strategy
pub struct SMACrossoverStrategy {
    name: String,
    description: String,
    fast_period: usize,
    slow_period: usize,
    symbol: String,
}

impl SMACrossoverStrategy {
    pub fn new(symbol: &str, fast_period: usize, slow_period: usize) -> Self {
        Self {
            name: "SMA Crossover".to_string(),
            description: "Generates buy/sell signals based on fast and slow SMA crossovers".to_string(),
            fast_period,
            slow_period,
            symbol: symbol.to_string(),
        }
    }
}

#[async_trait]
impl TradingStrategy for SMACrossoverStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn description(&self) -> &str {
        &self.description
    }
    
    async fn analyze(&self, data: &PriceHistory) -> TradingResult<Option<TradingSignal>> {
        if data.candles.len() < self.slow_period + 2 {
            return Err(TradingError::Strategy(format!(
                "Not enough data for SMA Crossover analysis. Need at least {} candles",
                self.slow_period + 2
            )));
        }
        
        // Get close prices
        let prices = data.close_prices();
        
        // Calculate SMAs
        let fast_sma = indicators::calculate_sma(&prices, self.fast_period)
            .map_err(|e| TradingError::Strategy(format!("Failed to calculate fast SMA: {}", e)))?;
            
        let slow_sma = indicators::calculate_sma(&prices, self.slow_period)
            .map_err(|e| TradingError::Strategy(format!("Failed to calculate slow SMA: {}", e)))?;
        
        // We need at least 2 values in each SMA to detect a crossover
        if fast_sma.len() < 2 || slow_sma.len() < 2 {
            return Ok(None);
        }
        
        // Align the SMAs (they might have different lengths due to the calculation)
        let fast_offset = if fast_sma.len() > slow_sma.len() {
            fast_sma.len() - slow_sma.len()
        } else {
            0
        };
        
        let slow_offset = if slow_sma.len() > fast_sma.len() {
            slow_sma.len() - fast_sma.len()
        } else {
            0
        };
        
        // Get the last two values for comparison
        let fast_current = fast_sma[fast_sma.len() - 1];
        let fast_previous = fast_sma[fast_sma.len() - 2];
        let slow_current = slow_sma[slow_sma.len() - 1];
        let slow_previous = slow_sma[slow_sma.len() - 2];
        
        // Check for crossover
        let was_above = fast_previous > slow_previous;
        let is_above = fast_current > slow_current;
        
        // Get the latest price and timestamp
        let latest_candle = &data.candles[data.candles.len() - 1];
        let price = latest_candle.close;
        let timestamp = latest_candle.close_time;
        
        // Generate signals based on crossover
        if !was_above && is_above {
            // Bullish crossover (fast crosses above slow)
            let signal = TradingSignal {
                symbol: self.symbol.clone(),
                action: TradeAction::Buy,
                price,
                confidence: 0.8, // Example confidence value
                timestamp,
                indicators: vec![],
            };
            Ok(Some(signal))
        } else if was_above && !is_above {
            // Bearish crossover (fast crosses below slow)
            let signal = TradingSignal {
                symbol: self.symbol.clone(),
                action: TradeAction::Sell,
                price,
                confidence: 0.8, // Example confidence value
                timestamp,
                indicators: vec![],
            };
            Ok(Some(signal))
        } else {
            // No crossover
            Ok(None)
        }
    }
    
    fn parameters(&self) -> Vec<StrategyParameter> {
        vec![
            StrategyParameter {
                name: "fast_period".to_string(),
                description: "Fast SMA period".to_string(),
                value: ParameterValue::Integer(self.fast_period as i64),
                range: Some(ParameterRange::Integer(2, 50)),
            },
            StrategyParameter {
                name: "slow_period".to_string(),
                description: "Slow SMA period".to_string(),
                value: ParameterValue::Integer(self.slow_period as i64),
                range: Some(ParameterRange::Integer(5, 200)),
            },
        ]
    }
    
    fn update_parameter(&mut self, name: &str, value: ParameterValue) -> TradingResult<()> {
        match (name, value) {
            ("fast_period", ParameterValue::Integer(period)) => {
                if period < 2 || period >= self.slow_period as i64 {
                    return Err(TradingError::Strategy(format!(
                        "Fast period must be >= 2 and < slow period ({})",
                        self.slow_period
                    )));
                }
                self.fast_period = period as usize;
                Ok(())
            },
            ("slow_period", ParameterValue::Integer(period)) => {
                if period <= self.fast_period as i64 {
                    return Err(TradingError::Strategy(format!(
                        "Slow period must be > fast period ({})",
                        self.fast_period
                    )));
                }
                self.slow_period = period as usize;
                Ok(())
            },
            _ => Err(TradingError::Strategy(format!("Unknown parameter: {}", name))),
        }
    }
}

/// RSI Strategy
pub struct RSIStrategy {
    name: String,
    description: String,
    period: usize,
    overbought_threshold: f64,
    oversold_threshold: f64,
    symbol: String,
}

impl RSIStrategy {
    pub fn new(
        symbol: &str,
        period: usize,
        overbought_threshold: f64,
        oversold_threshold: f64,
    ) -> Self {
        Self {
            name: "RSI Strategy".to_string(),
            description: "Generates signals based on RSI overbought/oversold conditions".to_string(),
            period,
            overbought_threshold,
            oversold_threshold,
            symbol: symbol.to_string(),
        }
    }
}

#[async_trait]
impl TradingStrategy for RSIStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn description(&self) -> &str {
        &self.description
    }
    
    async fn analyze(&self, data: &PriceHistory) -> TradingResult<Option<TradingSignal>> {
        if data.candles.len() < self.period + 1 {
            return Err(TradingError::Strategy(format!(
                "Not enough data for RSI analysis. Need at least {} candles",
                self.period + 1
            )));
        }
        
        // Get close prices
        let prices = data.close_prices();
        
        // Calculate RSI
        let rsi = indicators::calculate_rsi(&prices, self.period)
            .map_err(|e| TradingError::Strategy(format!("Failed to calculate RSI: {}", e)))?;
        
        // Get the latest price and timestamp
        let latest_candle = &data.candles[data.candles.len() - 1];
        let price = latest_candle.close;
        let timestamp = latest_candle.close_time;
        
        // Generate signals based on RSI values
        if rsi <= self.oversold_threshold {
            // Oversold condition (potential buy)
            let signal = TradingSignal {
                symbol: self.symbol.clone(),
                action: TradeAction::Buy,
                price,
                confidence: (self.oversold_threshold - rsi) / self.oversold_threshold,
                timestamp,
                indicators: vec![],
            };
            Ok(Some(signal))
        } else if rsi >= self.overbought_threshold {
            // Overbought condition (potential sell)
            let signal = TradingSignal {
                symbol: self.symbol.clone(),
                action: TradeAction::Sell,
                price,
                confidence: (rsi - self.overbought_threshold) / (100.0 - self.overbought_threshold),
                timestamp,
                indicators: vec![],
            };
            Ok(Some(signal))
        } else {
            // No signal
            Ok(None)
        }
    }
    
    fn parameters(&self) -> Vec<StrategyParameter> {
        vec![
            StrategyParameter {
                name: "period".to_string(),
                description: "RSI period".to_string(),
                value: ParameterValue::Integer(self.period as i64),
                range: Some(ParameterRange::Integer(2, 30)),
            },
            StrategyParameter {
                name: "overbought_threshold".to_string(),
                description: "RSI overbought threshold".to_string(),
                value: ParameterValue::Float(self.overbought_threshold),
                range: Some(ParameterRange::Float(60.0, 90.0)),
            },
            StrategyParameter {
                name: "oversold_threshold".to_string(),
                description: "RSI oversold threshold".to_string(),
                value: ParameterValue::Float(self.oversold_threshold),
                range: Some(ParameterRange::Float(10.0, 40.0)),
            },
        ]
    }
    
    fn update_parameter(&mut self, name: &str, value: ParameterValue) -> TradingResult<()> {
        match (name, value) {
            ("period", ParameterValue::Integer(period)) => {
                if period < 2 {
                    return Err(TradingError::Strategy("RSI period must be >= 2".to_string()));
                }
                self.period = period as usize;
                Ok(())
            },
            ("overbought_threshold", ParameterValue::Float(threshold)) => {
                if threshold <= self.oversold_threshold || threshold > 100.0 {
                    return Err(TradingError::Strategy(format!(
                        "Overbought threshold must be > oversold threshold ({}) and <= 100",
                        self.oversold_threshold
                    )));
                }
                self.overbought_threshold = threshold;
                Ok(())
            },
            ("oversold_threshold", ParameterValue::Float(threshold)) => {
                if threshold >= self.overbought_threshold || threshold < 0.0 {
                    return Err(TradingError::Strategy(format!(
                        "Oversold threshold must be < overbought threshold ({}) and >= 0",
                        self.overbought_threshold
                    )));
                }
                self.oversold_threshold = threshold;
                Ok(())
            },
            _ => Err(TradingError::Strategy(format!("Unknown parameter: {}", name))),
        }
    }
}

/// MACD Strategy
pub struct MACDStrategy {
    name: String,
    description: String,
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
    symbol: String,
}

impl MACDStrategy {
    pub fn new(
        symbol: &str,
        fast_period: usize,
        slow_period: usize,
        signal_period: usize,
    ) -> Self {
        Self {
            name: "MACD Strategy".to_string(),
            description: "Generates signals based on MACD line and signal line crossovers".to_string(),
            fast_period,
            slow_period,
            signal_period,
            symbol: symbol.to_string(),
        }
    }
}

#[async_trait]
impl TradingStrategy for MACDStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn description(&self) -> &str {
        &self.description
    }
    
    async fn analyze(&self, data: &PriceHistory) -> TradingResult<Option<TradingSignal>> {
        if data.candles.len() < self.slow_period + self.signal_period + 2 {
            return Err(TradingError::Strategy(format!(
                "Not enough data for MACD analysis. Need at least {} candles",
                self.slow_period + self.signal_period + 2
            )));
        }
        
        // Get close prices
        let prices = data.close_prices();
        
        // Calculate MACD
        let (macd_line, signal_line, _) = indicators::calculate_macd(
            &prices,
            self.fast_period,
            self.slow_period,
            self.signal_period,
        )
        .map_err(|e| TradingError::Strategy(format!("Failed to calculate MACD: {}", e)))?;
        
        // Check if we have enough data points
        if macd_line.len() < 2 || signal_line.len() < 2 {
            return Ok(None);
        }
        
        // Get the last two values for comparison
        let macd_current = macd_line[macd_line.len() - 1];
        let macd_previous = macd_line[macd_line.len() - 2];
        let signal_current = signal_line[signal_line.len() - 1];
        let signal_previous = signal_line[signal_line.len() - 2];
        
        // Check for crossover
        let was_above = macd_previous > signal_previous;
        let is_above = macd_current > signal_current;
        
        // Get the latest price and timestamp
        let latest_candle = &data.candles[data.candles.len() - 1];
        let price = latest_candle.close;
        let timestamp = latest_candle.close_time;
        
        // Generate signals based on crossover
        if !was_above && is_above {
            // Bullish crossover (MACD crosses above signal)
            let signal = TradingSignal {
                symbol: self.symbol.clone(),
                action: TradeAction::Buy,
                price,
                confidence: 0.8, // Example confidence value
                timestamp,
                indicators: vec![],
            };
            Ok(Some(signal))
        } else if was_above && !is_above {
            // Bearish crossover (MACD crosses below signal)
            let signal = TradingSignal {
                symbol: self.symbol.clone(),
                action: TradeAction::Sell,
                price,
                confidence: 0.8, // Example confidence value
                timestamp,
                indicators: vec![],
            };
            Ok(Some(signal))
        } else {
            // No crossover
            Ok(None)
        }
    }
    
    fn parameters(&self) -> Vec<StrategyParameter> {
        vec![
            StrategyParameter {
                name: "fast_period".to_string(),
                description: "Fast EMA period".to_string(),
                value: ParameterValue::Integer(self.fast_period as i64),
                range: Some(ParameterRange::Integer(5, 20)),
            },
            StrategyParameter {
                name: "slow_period".to_string(),
                description: "Slow EMA period".to_string(),
                value: ParameterValue::Integer(self.slow_period as i64),
                range: Some(ParameterRange::Integer(10, 40)),
            },
            StrategyParameter {
                name: "signal_period".to_string(),
                description: "Signal line period".to_string(),
                value: ParameterValue::Integer(self.signal_period as i64),
                range: Some(ParameterRange::Integer(5, 15)),
            },
        ]
    }
    
    fn update_parameter(&mut self, name: &str, value: ParameterValue) -> TradingResult<()> {
        match (name, value) {
            ("fast_period", ParameterValue::Integer(period)) => {
                if period < 2 || period >= self.slow_period as i64 {
                    return Err(TradingError::Strategy(format!(
                        "Fast period must be >= 2 and < slow period ({})",
                        self.slow_period
                    )));
                }
                self.fast_period = period as usize;
                Ok(())
            },
            ("slow_period", ParameterValue::Integer(period)) => {
                if period <= self.fast_period as i64 {
                    return Err(TradingError::Strategy(format!(
                        "Slow period must be > fast period ({})",
                        self.fast_period
                    )));
                }
                self.slow_period = period as usize;
                Ok(())
            },
            ("signal_period", ParameterValue::Integer(period)) => {
                if period < 2 {
                    return Err(TradingError::Strategy(
                        "Signal period must be >= 2".to_string()
                    ));
                }
                self.signal_period = period as usize;
                Ok(())
            },
            _ => Err(TradingError::Strategy(format!("Unknown parameter: {}", name))),
        }
    }
}