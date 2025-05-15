// src/trading/execution.rs
use crate::domain::errors::{TradingError, TradingResult};
use crate::domain::models::{
    Order, OrderResponse, OrderSide, OrderStatus, OrderType, TradingSignal,
};
use crate::exchange::client::ExchangeClient;
use crate::market_data::processor::MarketDataProcessor;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::time::{Duration, Instant};

/// Risk management parameters
pub struct RiskParameters {
    pub max_position_size: Decimal,
    pub max_order_size: Decimal,
    pub max_daily_loss: Decimal,
    pub stop_loss_percent: Decimal,
    pub take_profit_percent: Decimal,
    pub max_open_positions: usize,
    pub max_trades_per_day: usize,
}

impl Default for RiskParameters {
    fn default() -> Self {
        Self {
            max_position_size: Decimal::new(500, 0),   // $500
            max_order_size: Decimal::new(100, 0),      // $100
            max_daily_loss: Decimal::new(100, 0),      // $100
            stop_loss_percent: Decimal::new(5, 0),     // 5%
            take_profit_percent: Decimal::new(10, 0),  // 10%
            max_open_positions: 5,
            max_trades_per_day: 10,
        }
    }
}

/// Position information
#[derive(Debug, Clone)]
pub struct Position {
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub current_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
    pub open_time: i64,
    pub last_update: i64,
}

impl Position {
    pub fn new(
        symbol: &str,
        side: OrderSide,
        quantity: Decimal,
        price: Decimal,
        timestamp: i64,
    ) -> Self {
        Self {
            symbol: symbol.to_string(),
            side,
            quantity,
            entry_price: price,
            current_price: price,
            unrealized_pnl: Decimal::ZERO,
            stop_loss: None,
            take_profit: None,
            open_time: timestamp,
            last_update: timestamp,
        }
    }

    /// Calculate unrealized PnL based on current price
    pub fn calculate_pnl(&mut self, current_price: Decimal) {
        self.current_price = current_price;
        self.last_update = chrono::Utc::now().timestamp_millis();

        // Calculate PnL based on position side
        let price_diff = match self.side {
            OrderSide::Buy => current_price - self.entry_price,
            OrderSide::Sell => self.entry_price - current_price,
        };

        self.unrealized_pnl = price_diff * self.quantity;
    }

    /// Check if position should be closed based on stop loss or take profit
    pub fn should_close(&self) -> bool {
        match self.side {
            OrderSide::Buy => {
                // For long positions
                if let Some(stop_loss) = self.stop_loss {
                    if self.current_price <= stop_loss {
                        return true;
                    }
                }

                if let Some(take_profit) = self.take_profit {
                    if self.current_price >= take_profit {
                        return true;
                    }
                }
            }
            OrderSide::Sell => {
                // For short positions
                if let Some(stop_loss) = self.stop_loss {
                    if self.current_price >= stop_loss {
                        return true;
                    }
                }

                if let Some(take_profit) = self.take_profit {
                    if self.current_price <= take_profit {
                        return true;
                    }
                }
            }
        }

        false
    }
}

/// Trade record
#[derive(Debug, Clone)]
pub struct Trade {
    pub id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub price: Decimal,
    pub timestamp: i64,
    pub pnl: Option<Decimal>,
    pub entry_order_id: String,
    pub exit_order_id: Option<String>,
}

/// Trade execution service manages positions and executes orders
pub struct TradeExecutor<T: ExchangeClient> {
    exchange: Arc<T>,
    market_data: Arc<MarketDataProcessor>,
    positions: Arc<Mutex<HashMap<String, Position>>>,
    trades: Arc<Mutex<Vec<Trade>>>,
    risk_params: Arc<Mutex<RiskParameters>>,
    daily_pnl: Arc<Mutex<Decimal>>,
    trade_count: Arc<Mutex<usize>>,
    signal_tx: broadcast::Sender<TradingSignal>,
}

impl<T: ExchangeClient> TradeExecutor<T> {
    /// Create a new trade executor
    pub fn new(exchange: Arc<T>, market_data: Arc<MarketDataProcessor>) -> Self {
        let (signal_tx, _) = broadcast::channel(100);

        Self {
            exchange,
            market_data,
            positions: Arc::new(Mutex::new(HashMap::new())),
            trades: Arc::new(Mutex::new(Vec::new())),
            risk_params: Arc::new(Mutex::new(RiskParameters::default())),
            daily_pnl: Arc::new(Mutex::new(Decimal::ZERO)),
            trade_count: Arc::new(Mutex::new(0)),
            signal_tx,
        }
    }

    /// Subscribe to trading signals
    pub fn subscribe_to_signals(&self) -> broadcast::Receiver<TradingSignal> {
        self.signal_tx.subscribe()
    }

    /// Start the trade executor
    pub async fn start(&self) -> TradingResult<()> {
        // Start position monitoring task
        self.start_position_monitor();

        // Subscribe to market data updates
        let mut market_data_rx = self.market_data.subscribe();

        // Start market data handling loop
        let positions = self.positions.clone();
        let market_data = self.market_data.clone();

        tokio::spawn(async move {
            while let Ok(data) = market_data_rx.recv().await {
                // Update positions with latest prices
                let mut positions = positions.lock().unwrap();
                if let Some(position) = positions.get_mut(&data.symbol) {
                    position.calculate_pnl(data.last_price);
                }
            }
        });

        Ok(())
    }

    /// Start position monitoring task
    fn start_position_monitor(&self) {
        let positions = self.positions.clone();
        let exchange = self.exchange.clone();
        let risk_params = self.risk_params.clone();
        let trades = self.trades.clone();
        let daily_pnl = self.daily_pnl.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));

            loop {
                interval.tick().await;

                // Get positions that need to be closed
                let positions_to_close = {
                    let positions = positions.lock().unwrap();
                    positions
                        .values()
                        .filter(|p| p.should_close())
                        .map(|p| p.clone())
                        .collect::<Vec<_>>()
                };

                // Close positions
                for position in positions_to_close {
                    let close_side = match position.side {
                        OrderSide::Buy => OrderSide::Sell,
                        OrderSide::Sell => OrderSide::Buy,
                    };

                    let order = Order {
                        symbol: position.symbol.clone(),
                        quantity: position.quantity,
                        order_type: OrderType::Market,
                        side: close_side,
                        client_order_id: None,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                    };

                    match exchange.place_order(&order).await {
                        Ok(response) => {
                            log::info!(
                                "Closed position for {} at {}: {:?}",
                                position.symbol,
                                position.current_price,
                                response
                            );

                            // Remove position
                            {
                                let mut positions = positions.lock().unwrap();
                                positions.remove(&position.symbol);
                            }

                            // Record trade
                            {
                                let mut trades = trades.lock().unwrap();
                                if let Some(trade) = trades.iter_mut().rev().find(|t| {
                                    t.symbol == position.symbol
                                        && t.exit_order_id.is_none()
                                }) {
                                    trade.exit_order_id = Some(response.order_id.clone());
                                    trade.pnl = Some(position.unrealized_pnl);

                                    // Update daily PnL
                                    let mut daily_pnl = daily_pnl.lock().unwrap();
                                    *daily_pnl += position.unrealized_pnl;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to close position: {:?}", e);
                        }
                    }
                }
            }
        });
    }

    /// Execute a trading signal
    pub async fn execute_signal(&self, signal: TradingSignal) -> TradingResult<Option<OrderResponse>> {
        // Broadcast the signal to subscribers
        let _ = self.signal_tx.send(signal.clone());

        // Check if we should act on this signal
        if !self.should_execute_signal(&signal).await? {
            return Ok(None);
        }

        // Calculate order size based on risk parameters
        let order_size = self.calculate_order_size(&signal).await?;

        // Create and place the order
        let order = Order {
            symbol: signal.symbol.clone(),
            quantity: order_size,
            order_type: OrderType::Market,
            side: match signal.action {
                crate::domain::models::TradeAction::Buy => OrderSide::Buy,
                crate::domain::models::TradeAction::Sell => OrderSide::Sell,
                crate::domain::models::TradeAction::Hold => {
                    return Ok(None); // No action for Hold signals
                }
            },
            client_order_id: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        // Place the order
        let order_response = self.exchange.place_order(&order).await
            .map_err(|e| TradingError::OrderExecution(format!("Failed to place order: {:?}", e)))?;

        // Process the order response
        if order_response.status == OrderStatus::Filled || order_response.status == OrderStatus::PartiallyFilled {
            self.process_filled_order(&order, &order_response).await?;
        }

        Ok(Some(order_response))
    }

    /// Check if we should execute a trading signal
    async fn should_execute_signal(&self, signal: &TradingSignal) -> TradingResult<bool> {
        // Check if we already have a position for this symbol
        {
            let positions = self.positions.lock().unwrap();
            if positions.contains_key(&signal.symbol) {
                // We already have a position, so only execute if it's a closing signal
                let position = &positions[&signal.symbol];
                match (position.side, &signal.action) {
                    (OrderSide::Buy, crate::domain::models::TradeAction::Sell) => {
                        // We have a long position and this is a sell signal
                        return Ok(true);
                    }
                    (OrderSide::Sell, crate::domain::models::TradeAction::Buy) => {
                        // We have a short position and this is a buy signal
                        return Ok(true);
                    }
                    _ => {
                        // Signal is in the same direction as our position, ignore it
                        return Ok(false);
                    }
                }
            }
        }

        // Check risk parameters
        let risk_params = self.risk_params.lock().unwrap();

        // Check if we've hit the max number of open positions
        {
            let positions = self.positions.lock().unwrap();
            if positions.len() >= risk_params.max_open_positions {
                return Ok(false);
            }
        }

        // Check if we've hit the max number of trades per day
        {
            let trade_count = self.trade_count.lock().unwrap();
            if *trade_count >= risk_params.max_trades_per_day {
                return Ok(false);
            }
        }

        // Check if we've hit the max daily loss
        {
            let daily_pnl = self.daily_pnl.lock().unwrap();
            if *daily_pnl <= -risk_params.max_daily_loss {
                return Ok(false);
            }
        }

        // All checks passed, execute the signal
        Ok(true)
    }

    /// Calculate order size based on risk parameters
    async fn calculate_order_size(&self, signal: &TradingSignal) -> TradingResult<Decimal> {
        let risk_params = self.risk_params.lock().unwrap();

        // Default to max order size
        let mut order_size = risk_params.max_order_size;

        // Adjust order size based on confidence
        let confidence = Decimal::from_f64(signal.confidence).unwrap_or(Decimal::ONE);
        order_size = order_size * confidence;

        // Check if we have enough balance
        let base_asset = signal.symbol.split("USDT").next().unwrap_or("BTC");
        let quote_balance = match self.exchange.get_balance("USDT").await {
            Ok(balance) => balance.free,
            Err(_) => {
                // If we can't get the balance, use a conservative estimate
                risk_params.max_order_size / Decimal::new(2, 0)
            }
        };

        // Calculate how many units we can buy with our quote balance
        let max_units = quote_balance / signal.price;

        // Take the minimum of our calculated size and what we can afford
        order_size = order_size.min(max_units * signal.price);

        // Convert order size from quote currency (e.g., USDT) to base currency (e.g., BTC)
        let order_quantity = order_size / signal.price;

        // Round to appropriate precision (e.g., 5 decimal places for most crypto)
        let rounded_quantity = (order_quantity * Decimal::new(100000, 0)).trunc() / Decimal::new(100000, 0);

        Ok(rounded_quantity)
    }

    /// Process a filled order
    async fn process_filled_order(&self, order: &Order, response: &OrderResponse) -> TradingResult<()> {
        // Check if this is a new position or closing an existing one
        let is_new_position = {
            let positions = self.positions.lock().unwrap();
            !positions.contains_key(&order.symbol)
        };

        if is_new_position {
            // Create a new position
            let position = Position::new(
                &order.symbol,
                order.side.clone(),
                response.filled_quantity,
                response.average_price.unwrap_or_else(|| order.order_type.get_price().unwrap_or_default()),
                response.timestamp,
            );

            // Calculate stop loss and take profit levels
            let risk_params = self.risk_params.lock().unwrap();
            let mut position = position;

            match position.side {
                OrderSide::Buy => {
                    // Long position
                    position.stop_loss = Some(position.entry_price * (Decimal::ONE - risk_params.stop_loss_percent / Decimal::new(100, 0)));
                    position.take_profit = Some(position.entry_price * (Decimal::ONE + risk_params.take_profit_percent / Decimal::new(100, 0)));
                }
                OrderSide::Sell => {
                    // Short position
                    position.stop_loss = Some(position.entry_price * (Decimal::ONE + risk_params.stop_loss_percent / Decimal::new(100, 0)));
                    position.take_profit = Some(position.entry_price * (Decimal::ONE - risk_params.take_profit_percent / Decimal::new(100, 0)));
                }
            }

            // Add the position
            {
                let mut positions = self.positions.lock().unwrap();
                positions.insert(order.symbol.clone(), position);
            }

            // Record the trade
            {
                let mut trades = self.trades.lock().unwrap();
                trades.push(Trade {
                    id: format!("trade-{}", chrono::Utc::now().timestamp_millis()),
                    symbol: order.symbol.clone(),
                    side: order.side.clone(),
                    quantity: response.filled_quantity,
                    price: response.average_price.unwrap_or_else(|| order.order_type.get_price().unwrap_or_default()),
                    timestamp: response.timestamp,
                    pnl: None,
                    entry_order_id: response.order_id.clone(),
                    exit_order_id: None,
                });

                // Increment trade count
                let mut trade_count = self.trade_count.lock().unwrap();
                *trade_count += 1;
            }
        } else {
            // Closing an existing position
            let position = {
                let mut positions = self.positions.lock().unwrap();
                positions.remove(&order.symbol)
            };

            if let Some(position) = position {
                // Calculate realized PnL
                let exit_price = response.average_price.unwrap_or_else(|| order.order_type.get_price().unwrap_or_default());
                let price_diff = match position.side {
                    OrderSide::Buy => exit_price - position.entry_price,
                    OrderSide::Sell => position.entry_price - exit_price,
                };
                let realized_pnl = price_diff * position.quantity;

                // Record the trade exit
                {
                    let mut trades = self.trades.lock().unwrap();
                    if let Some(trade) = trades.iter_mut().rev().find(|t| {
                        t.symbol == order.symbol && t.exit_order_id.is_none()
                    }) {
                        trade.exit_order_id = Some(response.order_id.clone());
                        trade.pnl = Some(realized_pnl);
                    }
                }

                // Update daily PnL
                {
                    let mut daily_pnl = self.daily_pnl.lock().unwrap();
                    *daily_pnl += realized_pnl;
                }

                log::info!(
                    "Closed position for {} with PnL: {}",
                    order.symbol,
                    realized_pnl
                );
            }
        }

        Ok(())
    }

    /// Get all current positions
    pub fn get_positions(&self) -> Vec<Position> {
        let positions = self.positions.lock().unwrap();
        positions.values().cloned().collect()
    }

    /// Get all trades
    pub fn get_trades(&self) -> Vec<Trade> {
        let trades = self.trades.lock().unwrap();
        trades.clone()
    }

    /// Get daily PnL
    pub fn get_daily_pnl(&self) -> Decimal {
        let daily_pnl = self.daily_pnl.lock().unwrap();
        *daily_pnl
    }

    /// Update risk parameters
    pub fn update_risk_parameters(&self, params: RiskParameters) {
        let mut risk_params = self.risk_params.lock().unwrap();
        *risk_params = params;
    }

    /// Get current risk parameters
    pub fn get_risk_parameters(&self) -> RiskParameters {
        let risk_params = self.risk_params.lock().unwrap();
        risk_params.clone()
    }
}

/// Extension method for OrderType to get the price
trait OrderTypeExt {
    fn get_price(&self) -> Option<Decimal>;
}

impl OrderTypeExt for OrderType {
    fn get_price(&self) -> Option<Decimal> {
        match self {
            OrderType::Market => None,
            OrderType::Limit(price) => Some(*price),
            OrderType::Stop(price) => Some(*price),
            OrderType::StopLimit(_, limit_price) => Some(*limit_price),
        }
    }
}