use tokio::sync::mpsc;
use crate::{
    config::TradingConfig,
    domain::{ExchangeClient, MarketData, TradingError},
    market_data_manager::MarketDataManager,
    websocket_handler::WebSocketHandler,
    trading_strategy::TradingStrategy,
    signal_processor::SignalProcessor,
    dto::{Kline, TickerData},
};

pub struct TradingBot<T: ExchangeClient> {
    config: TradingConfig,
    exchange: T,
    market_data_manager: MarketDataManager,
    strategy: TradingStrategy,
    websocket_handler: WebSocketHandler,
}

impl<T: ExchangeClient> TradingBot<T> {
    pub fn new(config: TradingConfig, exchange: T) -> Self {
        let market_data_manager = MarketDataManager::new(config.historical_window);
        let strategy = TradingStrategy::new(config.clone());
        let websocket_handler = WebSocketHandler::new(config.symbol.clone());

        Self {
            config,
            exchange,
            market_data_manager,
            strategy,
            websocket_handler,
        }
    }

    pub async fn start(&mut self) -> Result<(), TradingError> {
        // Connect to exchange
        self.exchange.connect().await?;
        log::info!("Connected to exchange");

        // Initialize historical data
        self.initialize_historical_data().await?;

        // Start websocket streams
        let kline_rx = self.websocket_handler.start_kline_stream().await?;
        let ticker_rx = self.websocket_handler.start_ticker_stream().await?;

        // Start signal processing
        let (signal_tx, signal_rx) = mpsc::channel(100);
        let mut signal_processor = SignalProcessor::new(self.exchange, 0.001); // 0.001 BTC position size
        
        // Spawn tasks
        let market_data_manager = self.market_data_manager.clone();
        let strategy = self.strategy.clone();
        
        tokio::spawn(async move {
            signal_processor.start_processing(signal_rx).await;
        });

        let kline_task = tokio::spawn(self.process_kline_data(kline_rx, signal_tx.clone()));
        let ticker_task = tokio::spawn(self.process_ticker_data(ticker_rx));

        // Wait for tasks to complete
        let _ = tokio::join!(kline_task, ticker_task);

        Ok(())
    }

    async fn initialize_historical_data(&mut self) -> Result<(), TradingError> {
        // This would call your existing get_historical_prices method
        // For now, just initialize with empty data
        self.market_data_manager.initialize_history(vec![]).await?;
        Ok(())
    }

    async fn process_kline_data(
        &self,
        mut kline_rx: mpsc::Receiver<Kline>,
        signal_tx: mpsc::Sender<crate::domain::TradingSignal>,
    ) {
        while let Some(kline) = kline_rx.recv().await {
            let market_data = MarketData {
                symbol: kline.symbol.clone(),
                timestamp: kline.end_time as u64,
                open_price: kline.open_price.parse().unwrap_or_default(),
                close_price: kline.close_price.parse().unwrap_or_default(),
                high_price: kline.high_price.parse().unwrap_or_default(),
                low_price: kline.low_price.parse().unwrap_or_default(),
                volume: kline.volume.parse().unwrap_or_default(),
                last_price: kline.close_price.parse().unwrap_or_default(),
            };

            if let Err(e) = self.market_data_manager.update_market_data(market_data.clone()).await {
                log::error!("Failed to update market data: {:?}", e);
                continue;
            }

            // Generate trading signals
            let price_history = self.market_data_manager.get_price_history().await;
            if let Some(signal) = self.strategy.analyze(&market_data, &price_history) {
                if let Err(e) = signal_tx.send(signal).await {
                    log::error!("Failed to send signal: {:?}", e);
                    break;
                }
            }
        }
    }

    async fn process_ticker_data(&self, mut ticker_rx: mpsc::Receiver<TickerData>) {
        while let Some(ticker) = ticker_rx.recv().await {
            let current_data = self.market_data_manager.get_current_data().await;
            let updated_data = MarketData {
                last_price: ticker.last_price.parse().unwrap_or_default(),
                ..current_data
            };

            if let Err(e) = self.market_data_manager.update_market_data(updated_data).await {
                log::error!("Failed to update ticker data: {:?}", e);
            }
        }
    }
}