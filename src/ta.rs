use std::{collections::VecDeque, num::FpCategory, vec::Vec};

// Calculate Simple Moving Average (SMA)
pub fn calculate_sma(prices: &[f64], period: usize) -> Vec<f64> {
    if prices.len() < period {
        return Vec::new();
    }

    let mut sma = Vec::with_capacity(prices.len() - period + 1);
    let mut sum = prices[0..period].iter().sum::<f64>();
    sma.push(sum / period as f64);

    for i in period..prices.len() {
        sum = sum - prices[i - period] + prices[i];
        sma.push(sum / period as f64);
    }

    sma
}

// Calculate Relative Strength Index (RSI)
pub fn calculate_rsi(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period + 1 {
        return None; // ต้องมีข้อมูลเพียงพอ
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

    // คำนวณค่าเฉลี่ยแบบ SMA เป็นค่าตั้งต้นของ EMA
    let mut avg_gain = gains.iter().skip(1).take(period).sum::<f64>() / period as f64;
    let mut avg_loss = losses.iter().skip(1).take(period).sum::<f64>() / period as f64;

    let smoothing_factor = 2.0 / (period as f64 + 1.0);

    for i in (period + 1)..prices.len() {
        avg_gain = (gains[i] * smoothing_factor) + (avg_gain * (1.0 - smoothing_factor));
        avg_loss = (losses[i] * smoothing_factor) + (avg_loss * (1.0 - smoothing_factor));
    }

    if avg_loss == 0.0 {
        return Some(100.0);
    }

    let rs = avg_gain / avg_loss;
    Some(100.0 - (100.0 / (1.0 + rs)))
}



// Calculate MACD (Moving Average Convergence Divergence)
pub fn calculate_macd(
    prices: &[f64],
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> (Vec<f64>, Vec<f64>) {
    let fast_ema = calculate_ema(prices, fast_period);
    let slow_ema = calculate_ema(prices, slow_period);

    let mut macd_line = Vec::with_capacity(fast_ema.len());
    for i in 0..fast_ema.len() {
        macd_line.push(fast_ema[i] - slow_ema[i]);
    }

    let signal_line = calculate_ema(&macd_line, signal_period);

    (macd_line, signal_line)
}

// Calculate Exponential Moving Average (EMA)
pub fn calculate_ema(prices: &[f64], period: usize) -> Vec<f64> {
    if prices.len() < period {
        return Vec::new();
    }

    // ใช้เฉพาะ n ตัวสุดท้าย โดย n = period
    let start_idx = prices.len().saturating_sub(period);
    let prices_to_use = &prices[start_idx..];

    let multiplier = 2.0 / (period + 1) as f64;
    let mut ema = Vec::with_capacity(prices_to_use.len());

    // คำนวณ EMA แรกโดยใช้ SMA ของข้อมูลที่เลือก
    let first_sma = prices_to_use.iter().sum::<f64>() / prices_to_use.len() as f64;
    ema.push(first_sma);

    // คำนวณ EMA ถัดไป (ถ้ามีข้อมูลมากกว่า period)
    for i in 1..prices_to_use.len() {
        let new_ema = (prices_to_use[i] - ema[ema.len() - 1]) * multiplier + ema[ema.len() - 1];
        ema.push(new_ema);
    }

    ema
}