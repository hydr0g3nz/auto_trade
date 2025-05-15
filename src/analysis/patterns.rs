// src/analysis/patterns.rs
use crate::domain::errors::{AnalysisError, AnalysisResult};
use crate::domain::models::Candlestick;

/// Detects potential chart patterns in price data
pub struct PatternDetector {
    /// Minimum number of candles required for pattern detection
    min_candles: usize,
    
    /// Tolerance for pattern detection (as a percentage)
    tolerance: f64,
}

impl PatternDetector {
    /// Create a new pattern detector with default settings
    pub fn new() -> Self {
        Self {
            min_candles: 20,
            tolerance: 0.03, // 3% tolerance
        }
    }
    
    /// Create a new pattern detector with custom settings
    pub fn with_settings(min_candles: usize, tolerance: f64) -> Self {
        Self {
            min_candles,
            tolerance,
        }
    }
    
    /// Detect head and shoulders pattern
    /// 
    /// A head and shoulders pattern consists of:
    /// 1. A peak (left shoulder)
    /// 2. A higher peak (head)
    /// 3. A lower peak similar to the first (right shoulder)
    /// 4. A neckline connecting the troughs between the peaks
    pub fn detect_head_and_shoulders(&self, candles: &[Candlestick]) -> AnalysisResult<Option<HeadAndShoulders>> {
        if candles.len() < self.min_candles {
            return Err(AnalysisError::InsufficientData(format!(
                "Need at least {} candles for pattern detection, got {}",
                self.min_candles,
                candles.len()
            )));
        }
        
        // Extract high prices for peak detection
        let high_prices: Vec<f64> = candles.iter()
            .map(|c| c.high.to_f64().unwrap_or_default())
            .collect();
            
        // Find local peaks (potential shoulders and head)
        let peaks = self.find_peaks(&high_prices, 3);
        
        // We need at least 3 peaks for a head and shoulders pattern
        if peaks.len() < 3 {
            return Ok(None);
        }
        
        // Analyze groups of 3 consecutive peaks to find potential head and shoulders patterns
        for i in 0..peaks.len() - 2 {
            let left_idx = peaks[i];
            let head_idx = peaks[i + 1];
            let right_idx = peaks[i + 2];
            
            // Skip if the peaks are too close together
            if head_idx - left_idx < 3 || right_idx - head_idx < 3 {
                continue;
            }
            
            let left_peak = high_prices[left_idx];
            let head_peak = high_prices[head_idx];
            let right_peak = high_prices[right_idx];
            
            // Check if the head is higher than both shoulders
            if head_peak > left_peak && head_peak > right_peak {
                // Check if shoulders are at similar heights (within tolerance)
                let height_diff = (left_peak - right_peak).abs() / left_peak;
                if height_diff <= self.tolerance {
                    // Find the neckline (connect troughs between peaks)
                    let left_trough_idx = self.find_trough(&high_prices, left_idx, head_idx);
                    let right_trough_idx = self.find_trough(&high_prices, head_idx, right_idx);
                    
                    // If we found valid troughs, we have a pattern
                    if let (Some(left_trough), Some(right_trough)) = (left_trough_idx, right_trough_idx) {
                        let left_trough_val = high_prices[left_trough];
                        let right_trough_val = high_prices[right_trough];
                        
                        return Ok(Some(HeadAndShoulders {
                            left_shoulder: candles[left_idx].clone(),
                            head: candles[head_idx].clone(),
                            right_shoulder: candles[right_idx].clone(),
                            left_trough: candles[left_trough].clone(),
                            right_trough: candles[right_trough].clone(),
                            neckline_slope: (right_trough_val - left_trough_val) / (right_trough as f64 - left_trough as f64),
                        }));
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    /// Detect double top pattern
    pub fn detect_double_top(&self, candles: &[Candlestick]) -> AnalysisResult<Option<DoubleTop>> {
        if candles.len() < self.min_candles {
            return Err(AnalysisError::InsufficientData(format!(
                "Need at least {} candles for pattern detection, got {}",
                self.min_candles,
                candles.len()
            )));
        }
        
        // Extract high prices for peak detection
        let high_prices: Vec<f64> = candles.iter()
            .map(|c| c.high.to_f64().unwrap_or_default())
            .collect();
            
        // Find local peaks
        let peaks = self.find_peaks(&high_prices, 2);
        
        // We need at least 2 peaks for a double top
        if peaks.len() < 2 {
            return Ok(None);
        }
        
        // Analyze pairs of peaks to find potential double tops
        for i in 0..peaks.len() - 1 {
            let first_idx = peaks[i];
            let second_idx = peaks[i + 1];
            
            // Peaks should be separated by some distance
            if second_idx - first_idx < 5 {
                continue;
            }
            
            let first_peak = high_prices[first_idx];
            let second_peak = high_prices[second_idx];
            
            // Both peaks should be at similar heights
            let height_diff = (first_peak - second_peak).abs() / first_peak;
            if height_diff <= self.tolerance {
                // Find the trough between peaks
                if let Some(trough_idx) = self.find_trough(&high_prices, first_idx, second_idx) {
                    let trough_val = high_prices[trough_idx];
                    
                    // Confirm that the trough is significantly lower than the peaks
                    let trough_diff_1 = (first_peak - trough_val) / first_peak;
                    let trough_diff_2 = (second_peak - trough_val) / second_peak;
                    
                    if trough_diff_1 > 0.03 && trough_diff_2 > 0.03 {
                        return Ok(Some(DoubleTop {
                            first_peak: candles[first_idx].clone(),
                            second_peak: candles[second_idx].clone(),
                            trough: candles[trough_idx].clone(),
                            height: (first_peak + second_peak) / 2.0,
                        }));
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    /// Detect double bottom pattern
    pub fn detect_double_bottom(&self, candles: &[Candlestick]) -> AnalysisResult<Option<DoubleBottom>> {
        if candles.len() < self.min_candles {
            return Err(AnalysisError::InsufficientData(format!(
                "Need at least {} candles for pattern detection, got {}",
                self.min_candles,
                candles.len()
            )));
        }
        
        // Extract low prices for trough detection
        let low_prices: Vec<f64> = candles.iter()
            .map(|c| c.low.to_f64().unwrap_or_default())
            .collect();
            
        // Find local troughs
        let troughs = self.find_troughs(&low_prices, 2);
        
        // We need at least 2 troughs for a double bottom
        if troughs.len() < 2 {
            return Ok(None);
        }
        
        // Analyze pairs of troughs to find potential double bottoms
        for i in 0..troughs.len() - 1 {
            let first_idx = troughs[i];
            let second_idx = troughs[i + 1];
            
            // Troughs should be separated by some distance
            if second_idx - first_idx < 5 {
                continue;
            }
            
            let first_trough = low_prices[first_idx];
            let second_trough = low_prices[second_idx];
            
            // Both troughs should be at similar heights
            let height_diff = (first_trough - second_trough).abs() / first_trough;
            if height_diff <= self.tolerance {
                // Find the peak between troughs
                if let Some(peak_idx) = self.find_peak(&low_prices, first_idx, second_idx) {
                    let peak_val = low_prices[peak_idx];
                    
                    // Confirm that the peak is significantly higher than the troughs
                    let peak_diff_1 = (peak_val - first_trough) / first_trough;
                    let peak_diff_2 = (peak_val - second_trough) / second_trough;
                    
                    if peak_diff_1 > 0.03 && peak_diff_2 > 0.03 {
                        return Ok(Some(DoubleBottom {
                            first_trough: candles[first_idx].clone(),
                            second_trough: candles[second_idx].clone(),
                            peak: candles[peak_idx].clone(),
                            depth: (first_trough + second_trough) / 2.0,
                        }));
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    /// Find local peaks in a price series
    fn find_peaks(&self, prices: &[f64], count: usize) -> Vec<usize> {
        let mut peaks = Vec::new();
        
        // We need at least 3 points to detect a peak
        if prices.len() < 3 {
            return peaks;
        }
        
        // Look for local maxima where a point is higher than its neighbors
        for i in 1..prices.len() - 1 {
            if prices[i] > prices[i - 1] && prices[i] > prices[i + 1] {
                peaks.push(i);
                
                // If we have enough peaks, return
                if peaks.len() >= count {
                    break;
                }
            }
        }
        
        peaks
    }
    
    /// Find local troughs in a price series
    fn find_troughs(&self, prices: &[f64], count: usize) -> Vec<usize> {
        let mut troughs = Vec::new();
        
        // We need at least 3 points to detect a trough
        if prices.len() < 3 {
            return troughs;
        }
        
        // Look for local minima where a point is lower than its neighbors
        for i in 1..prices.len() - 1 {
            if prices[i] < prices[i - 1] && prices[i] < prices[i + 1] {
                troughs.push(i);
                
                // If we have enough troughs, return
                if troughs.len() >= count {
                    break;
                }
            }
        }
        
        troughs
    }
    
    /// Find the trough (local minimum) between two indices
    fn find_trough(&self, prices: &[f64], start: usize, end: usize) -> Option<usize> {
        if start >= end || end >= prices.len() {
            return None;
        }
        
        let mut min_idx = start + 1;
        let mut min_val = prices[min_idx];
        
        for i in start + 2..end {
            if prices[i] < min_val {
                min_idx = i;
                min_val = prices[i];
            }
        }
        
        Some(min_idx)
    }
    
    /// Find the peak (local maximum) between two indices
    fn find_peak(&self, prices: &[f64], start: usize, end: usize) -> Option<usize> {
        if start >= end || end >= prices.len() {
            return None;
        }
        
        let mut max_idx = start + 1;
        let mut max_val = prices[max_idx];
        
        for i in start + 2..end {
            if prices[i] > max_val {
                max_idx = i;
                max_val = prices[i];
            }
        }
        
        Some(max_idx)
    }
}

/// Head and shoulders pattern
#[derive(Debug, Clone)]
pub struct HeadAndShoulders {
    pub left_shoulder: Candlestick,
    pub head: Candlestick,
    pub right_shoulder: Candlestick,
    pub left_trough: Candlestick,
    pub right_trough: Candlestick,
    pub neckline_slope: f64,
}

impl HeadAndShoulders {
    /// Calculate the target price after a breakout
    pub fn target_price(&self) -> f64 {
        let head_height = self.head.high.to_f64().unwrap_or_default() - 
            self.left_trough.low.to_f64().unwrap_or_default();
        
        let neckline_at_breakout = self.left_trough.low.to_f64().unwrap_or_default() +
            self.neckline_slope * (self.right_shoulder.close_time - self.left_trough.close_time) as f64;
            
        neckline_at_breakout - head_height
    }
}

/// Double top pattern
#[derive(Debug, Clone)]
pub struct DoubleTop {
    pub first_peak: Candlestick,
    pub second_peak: Candlestick,
    pub trough: Candlestick,
    pub height: f64,
}

impl DoubleTop {
    /// Calculate the target price after a breakout
    pub fn target_price(&self) -> f64 {
        let pattern_height = self.height - self.trough.low.to_f64().unwrap_or_default();
        self.trough.low.to_f64().unwrap_or_default() - pattern_height
    }
}

/// Double bottom pattern
#[derive(Debug, Clone)]
pub struct DoubleBottom {
    pub first_trough: Candlestick,
    pub second_trough: Candlestick,
    pub peak: Candlestick,
    pub depth: f64,
}

impl DoubleBottom {
    /// Calculate the target price after a breakout
    pub fn target_price(&self) -> f64 {
        let pattern_height = self.peak.high.to_f64().unwrap_or_default() - self.depth;
        self.peak.high.to_f64().unwrap_or_default() + pattern_height
    }
}