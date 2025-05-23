use tokio::sync::mpsc;
use crate::domain::{TradingSignal, TradeAction, Order, OrderType, OrderSide, ExchangeClient};

pub struct SignalProcessor<T: ExchangeClient> {
    exchange: T,
    position_size: f64,
}

impl<T: ExchangeClient> SignalProcessor<T> {
    pub fn new(exchange: T, position_size: f64) -> Self {
        Self {
            exchange,
            position_size,
        }
    }

    pub async fn start_processing(&mut self, mut signal_rx: mpsc::Receiver<TradingSignal>) {
        while let Some(signal) = signal_rx.recv().await {
            self.process_signal(signal).await;
        }
    }

    async fn process_signal(&mut self, signal: TradingSignal) {
        log::info!("Processing signal: {:?}", signal);

        match signal.action {
            TradeAction::Buy => {
                let order = Order {
                    symbol: signal.symbol.clone(),
                    quantity: self.position_size,
                    order_type: OrderType::Market,
                    side: OrderSide::Buy,
                };
                self.execute_order(order).await;
            }
            TradeAction::Sell => {
                let order = Order {
                    symbol: signal.symbol.clone(),
                    quantity: self.position_size,
                    order_type: OrderType::Market,
                    side: OrderSide::Sell,
                };
                self.execute_order(order).await;
            }
            TradeAction::Hold => {
                log::debug!("Holding position for {}", signal.symbol);
            }
        }
    }

    async fn execute_order(&mut self, order: Order) {
        match self.exchange.send_order(&order).await {
            Ok(response) => {
                log::info!("Order executed successfully: {:?}", response);
            }
            Err(e) => {
                log::error!("Failed to execute order: {:?}", e);
            }
        }
    }
}