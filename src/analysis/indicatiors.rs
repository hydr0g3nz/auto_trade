use crate::domain::errors::{AnalysisError, AnalysisResult};
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Simple Moving Average (SMA)
pub fn calculate_sma(prices: &[f64], period: usize) -> AnalysisResult<Vec<f64>> {
    if prices.len() < period {
        return Err(AnalysisError::InsufficientData(format!(
            "Not enough data for SMA calculation. Need at least {} points, got {}",
            period,
            prices.len()
        )));
    }

    let mut result = Vec::with_capacity(prices.len() - period + 1);
    let mut sum = prices.iter().take(period).sum::<f64>();
    
    // First SMA value
    result.push(sum / period as f64);
    
    // Calculate remaining values with sliding window
    for i in period..prices.len() {
        sum = sum - prices[i - period] + prices[i];
        result.push(sum / period as f64);
    }
    
    Ok(result)
}

/// Exponential Moving Average (EMA)
pub fn calculate_ema(prices: &[f64], period: usize) -> AnalysisResult<Vec<f64>> {
    if prices.len() < period {
        return Err(AnalysisError::InsufficientData(format!(
            "Not enough data for EMA calculation. Need at least {} points, got {}",
            period,
            prices.len()
        )));
    }

    let multiplier = 2.0 / (period + 1) as f64;
    let mut result = Vec::with_capacity(prices.len() - period + 1);
    
    // First EMA value is SMA
    let first_sma = prices.iter().take(period).sum::<f64>() / period as f64;
    result.push(first_sma);
    
    // Calculate remaining EMA values
    for i in period..prices.len() {
        let previous_ema = result[result.len() - 1];
        let new_ema = (prices[i] - previous_ema) * multiplier + previous_ema;
        result.push(new_ema);
    }
    
    Ok(result)
}

/// Relative Strength Index (RSI)
pub fn calculate_rsi(prices: &[f64], period: usize) -> AnalysisResult<f64> {
    if prices.len() <= period {
        return Err(AnalysisError::InsufficientData(format!(
            "Not enough data for RSI calculation. Need at least {} points, got {}",
            period + 1,
            prices.len()
        )));
    }

    let mut gains = Vec::with_capacity(prices.len() - 1);
    let mut losses = Vec::with_capacity(prices.len() - 1);
    
    // Calculate price changes
    for i in 1..prices.len() {
        let change = prices[i] - prices[i - 1];
        if change > 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }
    
    // Calculate initial averages
    let avg_gain = gains.iter().take(period).sum::<f64>() / period as f64;
    let avg_loss = losses.iter().take(period).sum::<f64>() / period as f64;
    
    // Smooth averages for the remaining periods
    let mut current_avg_gain = avg_gain;
    let mut current_avg_loss = avg_loss;
    
    for i in period..gains.len() {
        current_avg_gain = (current_avg_gain * (period - 1) as f64 + gains[i]) / period as f64;
        current_avg_loss = (current_avg_loss * (period - 1) as f64 + losses[i]) / period as f64;
    }
    
    // Calculate RSI
    if current_avg_loss.abs() < f64::EPSILON {
        return Ok(100.0);
    }
    
    let rs = current_avg_gain / current_avg_loss;
    Ok(100.0 - (100.0 / (1.0 + rs)))
}

/// MACD (Moving Average Convergence Divergence)
pub fn calculate_macd(
    prices: &[f64], 
    fast_period: usize, 
    slow_period: usize,
    signal_period: usize
) -> AnalysisResult<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    if prices.len() < slow_period + signal_period {
        return Err(AnalysisError::InsufficientData(format!(
            "Not enough data for MACD calculation. Need at least {} points, got {}",
            slow_period + signal_period,
            prices.len()
        )));
    }
    
    // Calculate EMAs
    let fast_ema = calculate_ema(prices, fast_period)?;
    let slow_ema = calculate_ema(prices, slow_period)?;
    
    // Align the EMAs (they may have different lengths)
    let offset = slow_period - fast_period;
    let aligned_fast_ema = if offset > 0 {
        fast_ema.iter().skip(offset).copied().collect::<Vec<f64>>()
    } else {
        fast_ema
    };
    
    // Calculate MACD line
    let mut macd_line = Vec::with_capacity(slow_ema.len());
    for i in 0..slow_ema.len() {
        macd_line.push(aligned_fast_ema[i] - slow_ema[i]);
    }
    
    // Calculate signal line
    let signal_line = calculate_ema(&macd_line, signal_period)?;
    
    // Calculate histogram
    let histogram: Vec<f64> = macd_line
        .iter()
        .zip(signal_line.iter())
        .map(|(macd, signal)| macd - signal)
        .collect();
    
    Ok((macd_line, signal_line, histogram))
}

/// Bollinger Bands
pub fn calculate_bollinger_bands(
    prices: &[f64],
    period: usize,
    std_dev_multiplier: f64
) -> AnalysisResult<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    if prices.len() < period {
        return Err(AnalysisError::InsufficientData(format!(
            "Not enough data for Bollinger Bands calculation. Need at least {} points, got {}",
            period,
            prices.len()
        )));
    }
    
    let sma = calculate_sma(prices, period)?;
    let mut upper_band = Vec::with_capacity(sma.len());
    let mut lower_band = Vec::with_capacity(sma.len());
    
    for (i, &middle) in sma.iter().enumerate() {
        // Calculate standard deviation for the window
        let window_start = i;
        let window_end = i + period;
        let window = &prices[window_start..window_end];
        
        let variance = window.iter()
            .map(|&x| (x - middle).powi(2))
            .sum::<f64>() / period as f64;
        
        let std_dev = variance.sqrt();
        
        // Calculate bands
        upper_band.push(middle + std_dev_multiplier * std_dev);
        lower_band.push(middle - std_dev_multiplier * std_dev);
    }
    
    Ok((upper_band, sma, lower_band))
}

/// Average True Range (ATR)
pub fn calculate_atr(
    high_prices: &[f64],
    low_prices: &[f64],
    close_prices: &[f64],
    period: usize
) -> AnalysisResult<Vec<f64>> {
    if high_prices.len() < period + 1 || low_prices.len() < period + 1 || close_prices.len() < period + 1 {
        return Err(AnalysisError::InsufficientData(format!(
            "Not enough data for ATR calculation. Need at least {} points, got {}",
            period + 1,
            high_prices.len().min(low_prices.len()).min(close_prices.len())
        )));
    }
    
    // Calculate true ranges
    let mut true_ranges = Vec::with_capacity(high_prices.len() - 1);
    for i in 1..high_prices.len() {
        let tr1 = high_prices[i] - low_prices[i];
        let tr2 = (high_prices[i] - close_prices[i-1]).abs();
        let tr3 = (low_prices[i] - close_prices[i-1]).abs();
        
        true_ranges.push(tr1.max(tr2).max(tr3));
    }
    
    // Calculate first ATR as simple average
    let first_atr = true_ranges.iter().take(period).sum::<f64>() / period as f64;
    
    // Calculate remaining ATRs using the smoothing formula
    let mut atr = Vec::with_capacity(true_ranges.len() - period + 1);
    atr.push(first_atr);
    
    for i in period..true_ranges.len() {
        let new_atr = (atr[atr.len() - 1] * (period - 1) as f64 + true_ranges[i]) / period as f64;
        atr.push(new_atr);
    }
    
    Ok(atr)
}

// Backward compatibility functions with original ta.rs
// These functions are simplified wrappers around the new error-handling functions

/// Simple Moving Average (SMA) - Legacy version
pub fn sma(prices: &[f64], period: usize) -> Vec<f64> {
    match calculate_sma(prices, period) {
        Ok(result) => result,
        Err(_) => Vec::new(),
    }
}

/// Exponential Moving Average (EMA) - Legacy version
pub fn ema(prices: &[f64], period: usize) -> Vec<f64> {
    match calculate_ema(prices, period) {
        Ok(result) => result,
        Err(_) => Vec::new(),
    }
}

/// Relative Strength Index (RSI) - Legacy version
pub fn rsi(prices: &[f64], period: usize) -> Option<f64> {
    calculate_rsi(prices, period).ok()
}

/// MACD (Moving Average Convergence Divergence) - Legacy version
pub fn macd(
    prices: &[f64], 
    fast_period: usize, 
    slow_period: usize,
    signal_period: usize
) -> (Vec<f64>, Vec<f64>) {
    match calculate_macd(prices, fast_period, slow_period, signal_period) {
        Ok((macd_line, signal_line, _)) => (macd_line, signal_line),
        Err(_) => (Vec::new(), Vec::new()),
    }
}