// src/infrastructure/market/binance_market.rs
// Binance market data repository implementation

use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use binance_spot_connector_rust::{
    market_stream::kline::KlineStream,
    market_stream::ticker::TickerStream,
    tokio_tungstenite::BinanceWebSocketClient,
    market::klines::KlineInterval,
};
use futures_util::StreamExt;

use crate::domain::model::{MarketData, DomainError};
use crate::domain::repository::MarketDataRepository;
use crate::application::dto::{ApplicationError, Kline, TickerData};
use crate::application::dto::parser::{parse_websocket_message, parse_websocket_message_ticker};

pub struct BinanceMarketRepository {
    market_data: Arc<Mutex<HashMap<String, MarketData>>>,
    active_connections: HashMap<String, (mpsc::Sender<Kline>, mpsc::Sender<TickerData>)>,
    kline_interval: KlineInterval,
}

impl BinanceMarketRepository {
    pub fn new(kline_interval: KlineInterval) -> Self {
        Self {
            market_data: Arc::new(Mutex::new(HashMap::new())),
            active_connections: HashMap::new(),
            kline_interval,
        }
    }
    
    pub fn default() -> Self {
        Self::new(KlineInterval::Minutes1)
    }
}

#[async_trait]
impl MarketDataRepository for BinanceMarketRepository {
    async fn get_latest_data(&self, symbol: &str) -> Result<MarketData, DomainError> {
        let market_data = self.market_data.lock().await;
        market_data.get(symbol)
            .cloned()
            .ok_or_else(|| DomainError::MarketDataError(format!("No data available for {}", symbol)))
    }
    
    async fn subscribe_to_market_data(&self, symbol: &str) -> Result<(), DomainError> {
        if self.active_connections.contains_key(symbol) {
            return Ok(()); // Already subscribed
        }
        
        let (kline_tx, mut kline_rx) = mpsc::channel::<Kline>(100);
        let (ticker_tx, mut ticker_rx) = mpsc::channel::<TickerData>(100);
        
        // Start WebSocket connections for klines and ticker
        let symbol_clone = symbol.to_string();
        let market_data_clone = self.market_data.clone();
        let kline_interval = self.kline_interval.clone();
        
        // Spawn task for kline processing
        tokio::spawn(async move {
            Self::run_kline_stream(&symbol_clone, kline_interval, kline_tx).await;
        });
        
        let symbol_clone = symbol.to_string();
        tokio::spawn(async move {
            Self::run_ticker_stream(&symbol_clone, ticker_tx).await;
        });
        
        // Spawn task for processing received klines
        let symbol_clone = symbol.to_string();
        let market_data_clone = self.market_data.clone();
        tokio::spawn(async move {
            Self::process_kline_data(&symbol_clone, &mut kline_rx, market_data_clone).await;
        });
        
        // Spawn task for processing received tickers
        let symbol_clone = symbol.to_string();
        let market_data_clone = self.market_data.clone();
        tokio::spawn(async move {
            Self::process_ticker_data(&symbol_clone, &mut ticker_rx, market_data_clone).await;
        });
        
        // Store the channels for later stopping
        // (In a real implementation, we would store task handles as well)
        let mut active_conns = self.active_connections.clone();
        active_conns.insert(symbol.to_string(), (kline_tx, ticker_tx));
        
        Ok(())
    }
    
    async fn unsubscribe_from_market_data(&self, _symbol: &str) -> Result<(), DomainError> {
        // Unimplemented for now
        // Would close WebSocket connections and stop tasks
        Ok(())
    }
}

impl BinanceMarketRepository {
    async fn run_kline_stream(symbol: &str, interval: KlineInterval, sender: mpsc::Sender<Kline>) {
        // Establish connection
        let (mut conn, _) = match BinanceWebSocketClient::connect_async_default().await {
            Ok(conn) => conn,
            Err(e) => {
                log::error!("Failed to connect to WebSocket: {:?}", e);
                return;
            }
        };
        
        // Subscribe to streams
        if let Err(e) = conn.subscribe(vec![&KlineStream::new(symbol, interval).into()]).await {
            log::error!("Failed to subscribe to kline stream: {:?}", e);
            return;
        }
        
        // Process messages
        while let Some(message) = conn.as_mut().next().await {
            match message {
                Ok(message) => {
                    let binary_data = message.into_data();
                    match std::str::from_utf8(&binary_data) {
                        Ok(data) => {
                            // Skip numeric data (ping/pong)
                            if let Ok(_) = data.trim().parse::<i64>() {
                                continue;
                            }
                            
                            match parse_websocket_message(data) {
                                Ok(response) => {
                                    let mut kline_data = Kline::default();
                                    kline_data.symbol = response.data.symbol.clone();
                                    kline_data.open_price = response.data.kline.open_price.clone();
                                    kline_data.close_price = response.data.kline.close_price.clone();
                                    kline_data.low_price = response.data.kline.low_price.clone();
                                    kline_data.high_price = response.data.kline.high_price.clone();
                                    kline_data.volume = response.data.kline.volume.clone();
                                    kline_data.start_time = response.data.kline.start_time.clone();
                                    kline_data.end_time = response.data.kline.end_time.clone();
                                    
                                    if let Err(e) = sender.send(kline_data).await {
                                        log::error!("Failed to send kline data: {}", e);
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to parse kline JSON: {} raw data: {}", e, data);
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to convert binary data to string: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("WebSocket error: {:?}", e);
                    break;
                }
            }
        }
    }
    
    async fn run_ticker_stream(symbol: &str, sender: mpsc::Sender<TickerData>) {
        // Establish connection
        let (mut conn, _) = match BinanceWebSocketClient::connect_async_default().await {
            Ok(conn) => conn,
            Err(e) => {
                log::error!("Failed to connect to WebSocket: {:?}", e);
                return;
            }
        };
        
        // Subscribe to streams
        if let Err(e) = conn.subscribe(vec![&TickerStream::from_symbol(symbol).into()]).await {
            log::error!("Failed to subscribe to ticker stream: {:?}", e);
            return;
        }
        
        // Process messages
        while let Some(message) = conn.as_mut().next().await {
            match message {
                Ok(message) => {
                    let binary_data = message.into_data();
                    match std::str::from_utf8(&binary_data) {
                        Ok(data) => {
                            // Skip numeric data (ping/pong)
                            if let Ok(_) = data.trim().parse::<i64>() {
                                continue;
                            }
                            
                            match parse_websocket_message_ticker(data) {
                                Ok(response) => {
                                    let mut ticker_data = TickerData::default();
                                    ticker_data.symbol = response.data.symbol.clone();
                                    ticker_data.last_price = response.data.last_price.clone();
                                    
                                    if let Err(e) = sender.send(ticker_data).await {
                                        log::error!("Failed to send ticker data: {}", e);
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to parse ticker JSON: {} raw data: {}", e, data);
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to convert binary data to string: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("WebSocket error: {:?}", e);
                    break;
                }
            }
        }
    }
    
    async fn process_kline_data(
        symbol: &str,
        receiver: &mut mpsc::Receiver<Kline>,
        market_data: Arc<Mutex<HashMap<String, MarketData>>>,
    ) {
        while let Some(kline) = receiver.recv().await {
            let mut data_map = market_data.lock().await;
            
            // Get or create market data entry
            let data = data_map.entry(symbol.to_string()).or_insert_with(|| MarketData {
                symbol: symbol.to_string(),
                ..Default::default()
            });
            
            // Update market data
            data.open_price = kline.open_price.parse().unwrap_or_default();
            data.close_price = kline.close_price.parse().unwrap_or_default();
            data.high_price = kline.high_price.parse().unwrap_or_default();
            data.low_price = kline.low_price.parse().unwrap_or_default();
            
            // Log market data update
            log::debug!(
                "Kline Update - Symbol: {}, Open: {}, Close: {}",
                kline.symbol,
                kline.open_price,
                kline.close_price
            );
        }
    }
    
    async fn process_ticker_data(
        symbol: &str,
        receiver: &mut mpsc::Receiver<TickerData>,
        market_data: Arc<Mutex<HashMap<String, MarketData>>>,
    ) {
        while let Some(ticker) = receiver.recv().await {
            let mut data_map = market_data.lock().await;
            
            // Get or create market data entry
            let data = data_map.entry(symbol.to_string()).or_insert_with(|| MarketData {
                symbol: symbol.to_string(),
                ..Default::default()
            });
            
            // Update market data
            data.last_price = ticker.last_price.parse().unwrap_or_default();
            
            // Log market data update
            log::debug!(
                "Ticker Update - Symbol: {}, Last: {}",
                ticker.symbol,
                ticker.last_price
            );
        }
    }
}