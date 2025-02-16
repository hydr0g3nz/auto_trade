use std::vec::Vec;

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
pub fn calculate_rsi(prices: &[f64], period: usize) -> Vec<f64> {
    if prices.len() <= period {
        return Vec::new();
    }

    let mut gains = Vec::with_capacity(prices.len() - 1);
    let mut losses = Vec::with_capacity(prices.len() - 1);

    // Calculate price changes
    for i in 1..prices.len() {
        let change = prices[i] - prices[i - 1];
        gains.push(if change > 0.0 { change } else { 0.0 });
        losses.push(if change < 0.0 { -change } else { 0.0 });
    }

    let mut rsi = Vec::with_capacity(prices.len() - period);
    let mut avg_gain = gains[0..period].iter().sum::<f64>() / period as f64;
    let mut avg_loss = losses[0..period].iter().sum::<f64>() / period as f64;

    // Calculate initial RSI
    let mut rs = avg_gain / avg_loss;
    rsi.push(100.0 - (100.0 / (1.0 + rs)));

    // Calculate subsequent RSI values
    for i in period..gains.len() {
        avg_gain = (avg_gain * (period - 1) as f64 + gains[i]) / period as f64;
        avg_loss = (avg_loss * (period - 1) as f64 + losses[i]) / period as f64;
        rs = avg_gain / avg_loss;
        rsi.push(100.0 - (100.0 / (1.0 + rs)));
    }

    rsi
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

    let multiplier = 2.0 / (period + 1) as f64;
    let mut ema = Vec::with_capacity(prices.len());

    // Calculate first EMA using SMA
    let first_sma = prices[0..period].iter().sum::<f64>() / period as f64;
    ema.push(first_sma);

    // Calculate subsequent EMAs
    for i in period..prices.len() {
        let new_ema = (prices[i] - ema[ema.len() - 1]) * multiplier + ema[ema.len() - 1];
        ema.push(new_ema);
    }

    ema
}
