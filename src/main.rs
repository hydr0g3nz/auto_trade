// src/main.rs
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{timeout, Duration};
use tokio::signal;
use serde::{Deserialize, Serialize};
use dotenv;

// Modules
mod domain;
mod dto;
mod ta;

use crate::domain::*;
use crate::dto::{Error as DtoError, *};
use crate::ta::*;

use binance_spot_connector_rust::{
    http::Credentials,
    hyper::{BinanceHttpClient, Error as BinanceError},
    market::{self, klines::KlineInterval},
    market_stream::{kline::KlineStream, ticker::TickerStream},
    tokio_tungstenite::BinanceWebSocketClient,
    trade::{self, order::Side},
    wallet,
};

// Error conversion for Binance API errors
impl From<BinanceError> for DtoError {
    fn from(err: BinanceError) -> Self {
        DtoError::RequestError(format!("Binance API error: {:?}", err))
    }
}
use env_logger::Builder;
use futures_util::StreamExt;
use hyper::client::{HttpConnector};
use hyper_tls::HttpsConnector;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

// Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub symbol: String,
    pub rsi_period: usize,
    pub ema_fast_period: usize,
    pub ema_slow_period: usize,
    pub historical_window: usize,
    pub buy_threshold: f64,
    pub sell_threshold: f64,
    pub position_size: f64,
    pub max_retries: usize,
    pub websocket_timeout_secs: u64,
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            symbol: "BTCUSDT".to_string(),
            rsi_period: 14,
            ema_fast_period: 5,
            ema_slow_period: 15,
            historical_window: 50,
            buy_threshold: -2.0,
            sell_threshold: 2.0,
            position_size: 0.001,
            max_retries: 3,
            websocket_timeout_secs: 30,
        }
    }
}

// Market Data Manager
#[derive(Debug, Clone)]
pub struct MarketDataManager {
    current_data: Arc<RwLock<MarketData>>,
    price_history: Arc<RwLock<VecDeque<f64>>>,
    max_history_size: usize,
}

impl MarketDataManager {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            current_data: Arc::new(RwLock::new(MarketData::default())),
            price_history: Arc::new(RwLock::new(VecDeque::with_capacity(max_history_size))),
            max_history_size,
        }
    }

    pub async fn update_market_data(&self, data: MarketData) -> Result<(), TradingError> {
        // Update current data
        {
            let mut current = self.current_data.write().await;
            *current = data.clone();
        }
        
        // Update price history
        {
            let mut history = self.price_history.write().await;
            if history.len() >= self.max_history_size {
                history.pop_front();
            }
            history.push_back(data.close_price);
        }
        
        Ok(())
    }

    pub async fn get_current_data(&self) -> MarketData {
        self.current_data.read().await.clone()
    }

    pub async fn get_price_history(&self) -> Vec<f64> {
        self.price_history.read().await.iter().copied().collect()
    }

    pub async fn initialize_history(&self, prices: Vec<f64>) -> Result<(), TradingError> {
        let mut history = self.price_history.write().await;
        history.clear();
        for price in prices.into_iter().take(self.max_history_size) {
            history.push_back(price);
        }
        log::info!("Initialized price history with {} entries", history.len());
        Ok(())
    }

    pub async fn get_history_size(&self) -> usize {
        self.price_history.read().await.len()
    }
}

// WebSocket Handler
pub struct WebSocketHandler {
    symbol: String,
    timeout_duration: Duration,
}

impl WebSocketHandler {
    pub fn new(symbol: String, timeout_secs: u64) -> Self {
        Self {
            symbol,
            timeout_duration: Duration::from_secs(timeout_secs),
        }
    }

    pub async fn start_kline_stream(&self) -> Result<mpsc::Receiver<Kline>, TradingError> {
        let (tx, rx) = mpsc::channel(100);
        let symbol = self.symbol.clone();
        let timeout_duration = self.timeout_duration;
        
        tokio::spawn(async move {
            let mut retry_count = 0;
            let max_retries = 5;
            
            while retry_count < max_retries {
                match Self::handle_kline_stream(symbol.clone(), tx.clone(), timeout_duration).await {
                    Ok(_) => {
                        log::info!("Kline stream completed normally");
                        break;
                    }
                    Err(e) => {
                        retry_count += 1;
                        log::error!("Kline stream error (attempt {}/{}): {:?}", retry_count, max_retries, e);
                        if retry_count < max_retries {
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        });
        
        Ok(rx)
    }

    pub async fn start_ticker_stream(&self) -> Result<mpsc::Receiver<TickerData>, TradingError> {
        let (tx, rx) = mpsc::channel(100);
        let symbol = self.symbol.clone();
        let timeout_duration = self.timeout_duration;
        
        tokio::spawn(async move {
            let mut retry_count = 0;
            let max_retries = 5;
            
            while retry_count < max_retries {
                match Self::handle_ticker_stream(symbol.clone(), tx.clone(), timeout_duration).await {
                    Ok(_) => {
                        log::info!("Ticker stream completed normally");
                        break;
                    }
                    Err(e) => {
                        retry_count += 1;
                        log::error!("Ticker stream error (attempt {}/{}): {:?}", retry_count, max_retries, e);
                        if retry_count < max_retries {
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        });
        
        Ok(rx)
    }

    async fn handle_kline_stream(
        symbol: String,
        sender: mpsc::Sender<Kline>,
        timeout_duration: Duration,
    ) -> Result<(), TradingError> {
        let (mut conn, _) = BinanceWebSocketClient::connect_async_default()
            .await
            .map_err(|e| TradingError::ConnectionError(e.to_string()))?;

        conn.subscribe(vec![
            &KlineStream::new(&symbol, KlineInterval::Minutes1).into()
        ]).await;

        log::info!("Started kline stream for {}", symbol);

        loop {
            match timeout(timeout_duration, conn.as_mut().next()).await {
                Ok(Some(message_result)) => {
                    match message_result {
                        Ok(message) => {
                            let binary_data = message.into_data();
                            if let Ok(data) = std::str::from_utf8(&binary_data) {
                                match parse_websocket_message(data) {
                                    Ok(response) => {
                                        if sender.send(response.data.kline).await.is_err() {
                                            log::warn!("Kline receiver dropped, stopping stream");
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        if !data.trim().chars().all(char::is_numeric) {
                                            log::debug!("Failed to parse kline message: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("WebSocket message error: {:?}", e);
                            break;
                        }
                    }
                }
                Ok(None) => {
                    log::warn!("WebSocket stream ended");
                    break;
                }
                Err(_) => {
                    log::warn!("WebSocket timeout, continuing...");
                    continue;
                }
            }
        }

        conn.close().await.map_err(|e| TradingError::ConnectionError(e.to_string()))?;
        Ok(())
    }

    async fn handle_ticker_stream(
        symbol: String,
        sender: mpsc::Sender<TickerData>,
        timeout_duration: Duration,
    ) -> Result<(), TradingError> {
        let (mut conn, _) = BinanceWebSocketClient::connect_async_default()
            .await
            .map_err(|e| TradingError::ConnectionError(e.to_string()))?;

        conn.subscribe(vec![
            &TickerStream::from_symbol(&symbol).into()
        ]).await;

        log::info!("Started ticker stream for {}", symbol);

        loop {
            match timeout(timeout_duration, conn.as_mut().next()).await {
                Ok(Some(message_result)) => {
                    match message_result {
                        Ok(message) => {
                            let binary_data = message.into_data();
                            if let Ok(data) = std::str::from_utf8(&binary_data) {
                                match parse_websocket_message_ticker(data) {
                                    Ok(response) => {
                                        if sender.send(response.data).await.is_err() {
                                            log::warn!("Ticker receiver dropped, stopping stream");
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        if !data.trim().chars().all(char::is_numeric) {
                                            log::debug!("Failed to parse ticker message: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("WebSocket message error: {:?}", e);
                            break;
                        }
                    }
                }
                Ok(None) => {
                    log::warn!("WebSocket stream ended");
                    break;
                }
                Err(_) => {
                    log::warn!("WebSocket timeout, continuing...");
                    continue;
                }
            }
        }

        conn.close().await.map_err(|e| TradingError::ConnectionError(e.to_string()))?;
        Ok(())
    }
}

// Trading Strategy
#[derive(Debug)]
struct TradingIndicators {
    rsi: Option<f64>,
    fast_ema: Option<f64>,
    slow_ema: Option<f64>,
}

#[derive(Clone)]
pub struct TradingStrategy {
    config: TradingConfig,
}

impl TradingStrategy {
    pub fn new(config: TradingConfig) -> Self {
        Self { config }
    }

    pub fn analyze(&self, market_data: &MarketData, price_history: &[f64]) -> Option<TradingSignal> {
        if price_history.len() < self.config.rsi_period.max(self.config.ema_slow_period) {
            log::debug!("Insufficient price history: {} < {}", 
                price_history.len(), 
                self.config.rsi_period.max(self.config.ema_slow_period)
            );
            return None;
        }

        let indicators = self.calculate_indicators(price_history);
        let action = self.determine_action(market_data, &indicators);

        log::debug!("Market analysis - RSI: {:?}, Fast EMA: {:?}, Slow EMA: {:?}, Action: {:?}",
            indicators.rsi, indicators.fast_ema, indicators.slow_ema, action);

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

        // Enhanced strategy combining multiple indicators
        match (indicators.rsi, indicators.fast_ema, indicators.slow_ema) {
            (Some(rsi), Some(fast), Some(slow)) => {
                // Buy conditions: oversold RSI + upward EMA trend + price dip
                if rsi < 30.0 && fast > slow && price_change_pct < self.config.buy_threshold {
                    TradeAction::Buy
                }
                // Sell conditions: overbought RSI + downward EMA trend + price spike  
                else if rsi > 70.0 && fast < slow && price_change_pct > self.config.sell_threshold {
                    TradeAction::Sell
                }
                else {
                    TradeAction::Hold
                }
            }
            _ => {
                // Fallback to simple price action
                if price_change_pct < self.config.buy_threshold {
                    TradeAction::Buy
                } else if price_change_pct > self.config.sell_threshold {
                    TradeAction::Sell
                } else {
                    TradeAction::Hold
                }
            }
        }
    }
}

// Signal Processor
pub struct SignalProcessor {
    position_size: f64,
    last_signal_time: i64,
    signal_cooldown: i64, // seconds
}

impl SignalProcessor {
    pub fn new(position_size: f64) -> Self {
        Self {
            position_size,
            last_signal_time: 0,
            signal_cooldown: 300, // 5 minutes
        }
    }

    pub async fn start_processing<T: ExchangeClient + Send+Sync + 'static>(
        &mut self,
        mut signal_rx: mpsc::Receiver<TradingSignal>,
        mut exchange: T,
    ) {
        log::info!("Started signal processing");
        
        while let Some(signal) = signal_rx.recv().await {
            if self.should_process_signal(&signal) {
                self.process_signal(signal, &mut exchange).await;
            } else {
                log::debug!("Skipping signal due to cooldown");
            }
        }
        
        log::info!("Signal processing stopped");
    }

    fn should_process_signal(&mut self, signal: &TradingSignal) -> bool {
        let now = chrono::Utc::now().timestamp();
        let time_since_last = now - self.last_signal_time;
        
        if time_since_last < self.signal_cooldown && matches!(signal.action, TradeAction::Hold) {
            return false;
        }
        
        if !matches!(signal.action, TradeAction::Hold) {
            self.last_signal_time = now;
        }
        
        true
    }

    async fn process_signal<T: ExchangeClient + Send + Sync>(&mut self, signal: TradingSignal, exchange: &mut T) {
        log::info!("Processing signal: {:?}", signal);

        match signal.action {
            TradeAction::Buy => {
                let order = Order {
                    symbol: signal.symbol.clone(),
                    quantity: self.position_size,
                    order_type: OrderType::Market,
                    side: OrderSide::Buy,
                };
                self.execute_order(order, exchange).await;
            }
            TradeAction::Sell => {
                let order = Order {
                    symbol: signal.symbol.clone(),
                    quantity: self.position_size,
                    order_type: OrderType::Market,
                    side: OrderSide::Sell,
                };
                self.execute_order(order, exchange).await;
            }
            TradeAction::Hold => {
                log::debug!("Holding position for {}", signal.symbol);
            }
        }
    }

    async fn execute_order<T: ExchangeClient + Send + Sync>(&mut self, order: Order, exchange: &mut T) {
        log::info!("Attempting to execute order: {:?}", order);
        
        match exchange.send_order(&order).await {
            Ok(response) => {
                log::info!("Order executed successfully: {:?}", response);
            }
            Err(e) => {
                log::error!("Failed to execute order: {:?}", e);
            }
        }
    }
}

// Binance Exchange Client Implementation
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

    pub async fn get_historical_prices(&self, window_size: usize) -> Result<Vec<KlineResponse>, DtoError> {
        let request = market::klines(&self.symbol, KlineInterval::Minutes1)
            .limit(window_size as u32);
            
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

impl ExchangeClient for BinanceExchangeClient {
    async fn connect(&mut self) -> Result<(), TradingError> {
        log::info!("Connecting to Binance...");
        
        match self.account_status().await {
            Ok(status) => {
                log::debug!("Account status: {}", status);
            }
            Err(e) => {
                log::error!("Failed to get account status: {:?}", e);
                return Err(TradingError::ConnectionError("Account status check failed".into()));
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
                Err(TradingError::ConnectionError("API trading status check failed".into()))
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

// Trading Bot Main Orchestrator
pub struct TradingBot {
    config: TradingConfig,
    exchange: BinanceExchangeClient,
    market_data_manager: MarketDataManager,
    strategy: TradingStrategy,
    websocket_handler: WebSocketHandler,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl TradingBot {
    pub fn new(config: TradingConfig, exchange: BinanceExchangeClient) -> Self {
        let market_data_manager = MarketDataManager::new(config.historical_window);
        let strategy = TradingStrategy::new(config.clone());
        let websocket_handler = WebSocketHandler::new(
            config.symbol.clone(), 
            config.websocket_timeout_secs
        );

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
        
        // Create new exchange client for signal processor
        // Note: In production, you might want to share the client or use a connection pool
        let api_key = dotenv::var("BINANCE_API_KEY").expect("BINANCE_API_KEY must be set");
        let api_secret = dotenv::var("BINANCE_API_SECRET").expect("BINANCE_API_SECRET must be set");
        let credentials_for_processor = Credentials::from_hmac(api_key, api_secret);
        let mut exchange_for_processor = BinanceExchangeClient::new(credentials_for_processor);
        exchange_for_processor.set_symbol(self.config.symbol.clone()).await;
        
        // Spawn signal processor task
        let position_size = self.config.position_size;
        let signal_task = tokio::spawn(async move {
            let mut processor = SignalProcessor::new(position_size);
            processor.start_processing(signal_rx, exchange_for_processor).await;
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
        
        let historical_data = self.exchange
            .get_historical_prices(self.config.historical_window)
            .await
            .map_err(|e| TradingError::DataError(format!("Failed to get historical data: {:?}", e)))?;

        let prices: Vec<f64> = historical_data.iter().map(|k| k.close_price).collect();
        
        if prices.is_empty() {
            return Err(TradingError::DataError("No historical data received".into()));
        }
        
        self.market_data_manager.initialize_history(prices).await?;
        
        log::info!("Initialized with {} historical data points", historical_data.len());
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
            if let Err(e) = market_data_manager.update_market_data(market_data.clone()).await {
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
                log::info!("Kline closed - {}: O:{} H:{} L:{} C:{} V:{}", 
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
    let mut trading_bot = TradingBot::new(config, exchange_client);

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