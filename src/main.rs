// src/main.rs
use async_trait::async_trait;
use dotenv;
use env_logger::Builder;
use log;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{timeout, Duration};

// Import all modules
mod config;
mod domain;
mod dto;
mod market_data_manager;
mod signal_processor;
mod ta;
mod trading_strategy;
mod websocket_handler;

// Re-export commonly used items
use crate::config::TradingConfig;
use crate::domain::*;
use crate::dto::*;
use crate::market_data_manager::MarketDataManager;
use crate::signal_processor::SignalProcessor;
use crate::trading_strategy::TradingStrategy;
use crate::websocket_handler::WebSocketHandler;

use binance_spot_connector_rust::{
    http::Credentials,
    hyper::{BinanceHttpClient, Error as BinanceError},
    market::{self, klines::KlineInterval},
    trade::{self, order::Side},
    wallet,
};
use chrono::{DateTime, Utc};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;

// Error conversion for Binance API errors
impl From<BinanceError> for Error {
    fn from(err: BinanceError) -> Self {
        Error::RequestError(format!("Binance API error: {:?}", err))
    }
}

// Binance Exchange Client Implementation
#[derive(Clone)]
pub struct BinanceExchangeClient {
    connected: bool,
    balance: f64,
    credentials: Credentials,
    client: BinanceHttpClient<HttpsConnector<HttpConnector>>,
    symbol: String,
}

impl BinanceExchangeClient {
    pub fn new(credentials: Credentials) -> Self {
        BinanceExchangeClient {
            connected: false,
            balance: 0.0,
            symbol: String::new(),
            credentials: credentials.clone(),
            client: BinanceHttpClient::default().credentials(credentials),
        }
    }

    pub async fn set_symbol(&mut self, symbol: String) {
        self.symbol = symbol;
    }

    pub async fn get_historical_prices(
        &self,
        window_size: usize,
    ) -> Result<Vec<KlineResponse>, Error> {
        let request =
            market::klines(&self.symbol, KlineInterval::Minutes1).limit(window_size as u32);

        let response = self.client.send(request).await?;
        let data = response.into_body_str().await?;

        let raw_klines: Vec<Vec<serde_json::Value>> = serde_json::from_str(&data)?;

        let klines = raw_klines
            .iter()
            .map(|kline_data| KlineResponse::from_raw_data(kline_data))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(klines)
    }

    async fn account_status(&self) -> Result<String, BinanceError> {
        let data = self
            .client
            .send(wallet::account_status())
            .await?
            .into_body_str()
            .await?;
        Ok(data)
    }

    async fn api_trading_status(&self) -> Result<String, BinanceError> {
        let data = self
            .client
            .send(wallet::api_trading_status())
            .await?
            .into_body_str()
            .await?;
        Ok(data)
    }
}

#[async_trait]
impl ExchangeClient for BinanceExchangeClient {
    async fn connect(&mut self) -> Result<(), TradingError> {
        log::info!("Connecting to Binance...");

        match self.account_status().await {
            Ok(status) => {
                log::debug!("Account status: {}", status);
            }
            Err(e) => {
                log::error!("Failed to get account status: {:?}", e);
                return Err(TradingError::ConnectionError(
                    "Account status check failed".into(),
                ));
            }
        }

        match self.api_trading_status().await {
            Ok(status) => {
                log::debug!("API trading status: {}", status);
                self.connected = true;
                log::info!("Successfully connected to Binance");
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to get API trading status: {:?}", e);
                Err(TradingError::ConnectionError(
                    "API trading status check failed".into(),
                ))
            }
        }
    }

    async fn disconnect(&mut self) -> Result<(), TradingError> {
        self.connected = false;
        log::info!("Disconnected from Binance");
        Ok(())
    }

    async fn get_balance(&self) -> Result<f64, TradingError> {
        if self.connected {
            Ok(self.balance)
        } else {
            Err(TradingError::ConnectionError("Not connected".into()))
        }
    }

    async fn send_order(&mut self, order: &Order) -> Result<OrderResponse, TradingError> {
        if !self.connected {
            return Err(TradingError::ConnectionError("Not connected".into()));
        }

        log::info!("Sending order to Binance: {:?}", order);

        // For safety in demo, we'll simulate orders instead of real trading
        // Uncomment below for real trading:
        /*
        let side = match order.side {
            OrderSide::Buy => Side::Buy,
            OrderSide::Sell => Side::Sell,
        };

        let quantity = Decimal::from_f64(order.quantity)
            .ok_or_else(|| TradingError::OrderError("Invalid quantity".into()))?;

        let result = self
            .client
            .send(
                trade::new_order(&order.symbol, side, "MARKET")
                    .quantity(quantity),
            )
            .await
            .map_err(|e| TradingError::OrderError(format!("Order failed: {:?}", e)))?;

        let data = result.into_body_str().await
            .map_err(|e| TradingError::OrderError(format!("Response error: {:?}", e)))?;

        log::info!("Order response: {}", data);
        */

        // Simulated response for demo
        let timestamp = chrono::Utc::now().timestamp();
        let random_id = (timestamp % 100000) as u32;

        Ok(OrderResponse {
            order_id: format!("demo_{}_{}", timestamp, random_id),
            status: OrderStatus::Filled,
        })
    }

    async fn cancel_order(&mut self, order_id: &str) -> Result<(), TradingError> {
        log::info!("Canceling order: {}", order_id);
        // Mock implementation
        Ok(())
    }
}

// Enhanced Trading Bot with proper module usage
#[derive(Clone)]
pub struct EnhancedTradingBot {
    config: TradingConfig,
    exchange: BinanceExchangeClient,
    market_data_manager: MarketDataManager,
    strategy: TradingStrategy,
    websocket_handler: WebSocketHandler,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl EnhancedTradingBot {
    pub fn new(config: TradingConfig, exchange: BinanceExchangeClient) -> Self {
        let market_data_manager = MarketDataManager::new(config.historical_window);
        let strategy = TradingStrategy::new(config.clone());
        let websocket_handler = WebSocketHandler::new(config.symbol.clone());

        Self {
            config,
            exchange,
            market_data_manager,
            strategy,
            websocket_handler,
            shutdown_tx: None,
        }
    }

    pub async fn start(&mut self) -> Result<(), TradingError> {
        log::info!("Starting trading bot for symbol: {}", self.config.symbol);

        // Connect to exchange
        self.exchange.connect().await?;

        // Initialize historical data
        self.initialize_historical_data().await?;

        // Setup shutdown channel
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Start websocket streams
        let kline_rx = self.websocket_handler.start_kline_stream().await?;
        let ticker_rx = self.websocket_handler.start_ticker_stream().await?;

        // Start signal processing
        let (signal_tx, signal_rx) = mpsc::channel(100);

        // Clone necessary data for tasks
        let market_data_manager = self.market_data_manager.clone();
        let strategy = self.strategy.clone();

        // Create signal processor
        let mut processor = SignalProcessor::new(self.exchange.clone(), 20.0);

        // Create new exchange client for signal processor
        let api_key = dotenv::var("BINANCE_API_KEY").expect("BINANCE_API_KEY must be set");
        let api_secret = dotenv::var("BINANCE_API_SECRET").expect("BINANCE_API_SECRET must be set");
        let credentials_for_processor = Credentials::from_hmac(api_key, api_secret);
        let mut exchange_for_processor = BinanceExchangeClient::new(credentials_for_processor);
        exchange_for_processor
            .set_symbol(self.config.symbol.clone())
            .await;
        exchange_for_processor.connect().await?;

        // Spawn signal processor task
        let signal_task = tokio::spawn(async move {
            processor
                .start_processing(signal_rx)
                .await;
        });

        // Spawn data processing tasks
        let kline_task = {
            let mdm = market_data_manager.clone();
            let strat = strategy.clone();
            let sig_tx = signal_tx.clone();
            tokio::spawn(async move {
                Self::process_kline_data(kline_rx, mdm, strat, sig_tx).await;
            })
        };

        let ticker_task = {
            let mdm = market_data_manager.clone();
            tokio::spawn(async move {
                Self::process_ticker_data(ticker_rx, mdm).await;
            })
        };

        log::info!("Trading bot started successfully");

        // Wait for shutdown signal or task completion
        tokio::select! {
            _ = signal_task => {
                log::info!("Signal processor task completed");
            }
            _ = kline_task => {
                log::info!("Kline processing task completed");
            }
            _ = ticker_task => {
                log::info!("Ticker processing task completed");
            }
            _ = shutdown_rx.recv() => {
                log::info!("Shutdown signal received");
            }
            _ = signal::ctrl_c() => {
                log::info!("Ctrl+C received, shutting down...");
            }
        }

        self.shutdown().await?;
        Ok(())
    }

    async fn initialize_historical_data(&mut self) -> Result<(), TradingError> {
        log::info!("Initializing historical data...");

        let historical_data = self
            .exchange
            .get_historical_prices(self.config.historical_window)
            .await
            .map_err(|e| {
                TradingError::DataError(format!("Failed to get historical data: {:?}", e))
            })?;

        let prices: Vec<f64> = historical_data.iter().map(|k| k.close_price).collect();

        if prices.is_empty() {
            return Err(TradingError::DataError(
                "No historical data received".into(),
            ));
        }

        self.market_data_manager.initialize_history(prices).await?;

        log::info!(
            "Initialized with {} historical data points",
            historical_data.len()
        );
        Ok(())
    }

    async fn process_kline_data(
        mut kline_rx: mpsc::Receiver<Kline>,
        market_data_manager: MarketDataManager,
        strategy: TradingStrategy,
        signal_tx: mpsc::Sender<TradingSignal>,
    ) {
        log::info!("Started kline data processing");

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

            // Update market data
            if let Err(e) = market_data_manager
                .update_market_data(market_data.clone())
                .await
            {
                log::error!("Failed to update market data: {:?}", e);
                continue;
            }

            // Generate trading signals
            let price_history = market_data_manager.get_price_history().await;
            if let Some(signal) = strategy.analyze(&market_data, &price_history) {
                if let Err(e) = signal_tx.send(signal).await {
                    log::error!("Failed to send signal: {:?}", e);
                    break;
                }
            }

            // Log market update
            if kline.is_closed {
                log::info!(
                    "Kline closed - {}: O:{} H:{} L:{} C:{} V:{}",
                    market_data.symbol,
                    market_data.open_price,
                    market_data.high_price,
                    market_data.low_price,
                    market_data.close_price,
                    market_data.volume
                );
            }
        }

        log::info!("Kline data processing stopped");
    }

    async fn process_ticker_data(
        mut ticker_rx: mpsc::Receiver<TickerData>,
        market_data_manager: MarketDataManager,
    ) {
        log::info!("Started ticker data processing");

        while let Some(ticker) = ticker_rx.recv().await {
            let current_data = market_data_manager.get_current_data().await;
            let updated_data = MarketData {
                last_price: ticker.last_price.parse().unwrap_or(current_data.last_price),
                volume: ticker.volume.parse().unwrap_or(current_data.volume),
                ..current_data
            };

            if let Err(e) = market_data_manager.update_market_data(updated_data).await {
                log::error!("Failed to update ticker data: {:?}", e);
            }
        }

        log::info!("Ticker data processing stopped");
    }

    pub async fn shutdown(&mut self) -> Result<(), TradingError> {
        log::info!("Shutting down trading bot...");

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        self.exchange.disconnect().await?;
        log::info!("Trading bot shutdown complete");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Initialize logging
    Builder::from_default_env()
        .filter(None, log::LevelFilter::Info)
        .init();

    log::info!("Starting Binance Trading Bot");

    // Load configuration
    let config = TradingConfig::default();
    log::info!("Configuration: {:?}", config);

    // Load credentials from environment
    let api_key = dotenv::var("BINANCE_API_KEY")
        .expect("BINANCE_API_KEY must be set in environment or .env file");
    let api_secret = dotenv::var("BINANCE_API_SECRET")
        .expect("BINANCE_API_SECRET must be set in environment or .env file");

    let credentials = Credentials::from_hmac(api_key, api_secret);

    // Create exchange client
    let mut exchange_client = BinanceExchangeClient::new(credentials);
    exchange_client.set_symbol(config.symbol.clone()).await;

    // Create and start trading bot
    let mut trading_bot = EnhancedTradingBot::new(config, exchange_client);

    // Handle graceful shutdown
    trading_bot.start().await?;
    log::info!("Trading bot finished successfully");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_market_data_manager() {
        let manager = MarketDataManager::new(10);

        let test_data = MarketData {
            symbol: "BTCUSDT".to_string(),
            close_price: 50000.0,
            ..Default::default()
        };

        manager.update_market_data(test_data).await.unwrap();
        let history = manager.get_price_history().await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], 50000.0);
    }

    #[test]
    fn test_trading_strategy() {
        let config = TradingConfig::default();
        let strategy = TradingStrategy::new(config);

        let market_data = MarketData {
            symbol: "BTCUSDT".to_string(),
            open_price: 50000.0,
            last_price: 49000.0, // 2% drop
            ..Default::default()
        };

        let price_history = vec![50000.0; 20]; // Simple history
        let signal = strategy.analyze(&market_data, &price_history);

        assert!(signal.is_some());
        // Should generate buy signal due to 2% drop
        match signal.unwrap().action {
            TradeAction::Buy => {} // Expected
            _ => panic!("Expected buy signal"),
        }
    }
}
