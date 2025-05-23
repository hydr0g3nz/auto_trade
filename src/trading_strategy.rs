use crate::domain::{MarketData, TradingSignal, TradeAction};
use crate::config::TradingConfig;
use crate::ta::{calculate_rsi, calculate_ema};

pub struct TradingStrategy {
    config: TradingConfig,
}

impl TradingStrategy {
    pub fn new(config: TradingConfig) -> Self {
        Self { config }
    }

    pub fn analyze(&self, market_data: &MarketData, price_history: &[f64]) -> Option<TradingSignal> {
        if price_history.len() < self.config.rsi_period.max(self.config.ema_slow_period) {
            return None;
        }

        let indicators = self.calculate_indicators(price_history);
        let action = self.determine_action(market_data, &indicators);

        Some(TradingSignal {
            symbol: market_data.symbol.clone(),
            action,
            price: market_data.last_price,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    fn calculate_indicators(&self, prices: &[f64]) -> TradingIndicators {
        let rsi = calculate_rsi(prices, self.config.rsi_period);
        let fast_ema = calculate_ema(prices, self.config.ema_fast_period);
        let slow_ema = calculate_ema(prices, self.config.ema_slow_period);

        TradingIndicators {
            rsi,
            fast_ema: fast_ema.last().copied(),
            slow_ema: slow_ema.last().copied(),
        }
    }

    fn determine_action(&self, market_data: &MarketData, indicators: &TradingIndicators) -> TradeAction {
        let price_change_pct = if market_data.open_price != 0.0 {
            ((market_data.last_price - market_data.open_price) / market_data.open_price) * 100.0
        } else {
            0.0
        };

        // Simple strategy combining price action with RSI
        match (indicators.rsi, indicators.fast_ema, indicators.slow_ema) {
            (Some(rsi), Some(fast), Some(slow)) => {
                if price_change_pct < self.config.buy_threshold && rsi < 30.0 && fast > slow {
                    TradeAction::Buy
                } else if price_change_pct > self.config.sell_threshold && rsi > 70.0 && fast < slow {
                    TradeAction::Sell
                } else {
                    TradeAction::Hold
                }
            }
            _ => TradeAction::Hold,
        }
    }
}

#[derive(Debug)]
struct TradingIndicators {
    rsi: Option<f64>,
    fast_ema: Option<f64>,
    slow_ema: Option<f64>,
}