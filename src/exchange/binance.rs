// src/exchange/binance.rs
use crate::domain::errors::{ExchangeError, ExchangeResult};
use crate::domain::models::{Candlestick, MarketData, Order, OrderResponse, OrderSide, OrderStatus, OrderType, PriceHistory};
use crate::exchange::client::{Balance, ExchangeClient, MarketDataHandler, SubscriptionChannel};
use async_trait::async_trait;
use binance::{api::*, market, account, config::Config as BinanceConfig};
use chrono::prelude::*;
use futures::stream::{StreamExt};
use rust_decimal::Decimal;
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

/// Binance exchange client implementation
pub struct BinanceClient {
    api_key: String,
    api_secret: String,
    http_client: market::Market,
    account_client: Option<account::Account>,
    connected: bool,
    testnet: bool,
    market_data_tx: Option<Sender<MarketData>>,
    websocket_handles: Vec<JoinHandle<()>>,
}

impl BinanceClient {
    /// Create a new Binance client
    pub fn new(api_key: &str, api_secret: &str) -> Self {
        let http_client = market::Market::new();
        
        Self {
            api_key: api_key.to_string(),
            api_secret: api_secret.to_string(),
            http_client,
            account_client: None,
            connected: false,
            testnet: false,
            market_data_tx: None,
            websocket_handles: Vec::new(),
        }
    }
    
    /// Create a new Binance client in testnet mode
    pub fn new_testnet(api_key: &str, api_secret: &str) -> Self {
        let mut client = Self::new(api_key, api_secret);
        client.testnet = true;
        client
    }
    
    /// Start the market data processor
    async fn start_market_data_processor(
        &mut self,
        callback: Box<dyn MarketDataHandler>,
    ) -> ExchangeResult<()> {
        // Create a channel for market data
        let (tx, mut rx) = mpsc::channel::<MarketData>(100);
        
        // Store the sender
        self.market_data_tx = Some(tx);
        
        // Spawn a task to process market data
        let callback = Arc::new(Mutex::new(callback));
        
        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                let mut callback = callback.lock().unwrap();
                
                // Process based on data type
                if data.interval.is_some() {
                    callback.on_kline_update(data).await;
                } else {
                    callback.on_ticker_update(data).await;
                }
            }
        });
        
        Ok(())
    }
    
    /// Handle ticker WebSocket
    async fn handle_ticker_websocket(
        symbol: String,
        tx: Sender<MarketData>,
    ) -> ExchangeResult<()> {
        let symbol_lower = symbol.to_lowercase();
        let ws_url = format!(
            "wss://stream.binance.com:9443/ws/{}@ticker",
            symbol_lower
        );
        
        let url = Url::parse(&ws_url)
            .map_err(|e| ExchangeError::Connection(format!("Invalid WebSocket URL: {}", e)))?;
        
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(url).await
            .map_err(|e| ExchangeError::Connection(format!("WebSocket connection failed: {}", e)))?;
        
        let (_, mut read) = ws_stream.split();
        
        // Process incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(txt)) => {
                    // Parse the ticker message
                    match Self::parse_ticker_message(&symbol, &txt) {
                        Ok(market_data) => {
                            if let Err(e) = tx.send(market_data).await {
                                log::error!("Failed to send ticker data: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to parse ticker message: {:?}", e);
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    log::info!("Ticker WebSocket closed for {}", symbol);
                    break;
                }
                Err(e) => {
                    log::error!("Ticker WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
        
        // Reconnect on failure
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        log::info!("Reconnecting ticker WebSocket for {}", symbol);
        Self::handle_ticker_websocket(symbol, tx).await
    }
    
    /// Handle kline WebSocket
    async fn handle_kline_websocket(
        symbol: String,
        interval: String,
        tx: Sender<MarketData>,
    ) -> ExchangeResult<()> {
        let symbol_lower = symbol.to_lowercase();
        let ws_url = format!(
            "wss://stream.binance.com:9443/ws/{}@kline_{}",
            symbol_lower, interval
        );
        
        let url = Url::parse(&ws_url)
            .map_err(|e| ExchangeError::Connection(format!("Invalid WebSocket URL: {}", e)))?;
        
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(url).await
            .map_err(|e| ExchangeError::Connection(format!("WebSocket connection failed: {}", e)))?;
        
        let (_, mut read) = ws_stream.split();
        
        // Process incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(txt)) => {
                    // Parse the kline message
                    match Self::parse_kline_message(&symbol, &interval, &txt) {
                        Ok(market_data) => {
                            if let Err(e) = tx.send(market_data).await {
                                log::error!("Failed to send kline data: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to parse kline message: {:?}", e);
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    log::info!("Kline WebSocket closed for {}/{}", symbol, interval);
                    break;
                }
                Err(e) => {
                    log::error!("Kline WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
        
        // Reconnect on failure
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        log::info!("Reconnecting kline WebSocket for {}/{}", symbol, interval);
        Self::handle_kline_websocket(symbol, interval, tx).await
    }
    
    /// Parse ticker message
    fn parse_ticker_message(symbol: &str, message: &str) -> ExchangeResult<MarketData> {
        let v: Value = serde_json::from_str(message)
            .map_err(|e| ExchangeError::Api(format!("Failed to parse ticker message: {}", e)))?;
        
        // Extract values
        let price = v["c"].as_str()
            .ok_or_else(|| ExchangeError::Api("Missing close price in ticker".to_string()))?;
        
        let open = v["o"].as_str()
            .ok_or_else(|| ExchangeError::Api("Missing open price in ticker".to_string()))?;
        
        let high = v["h"].as_str()
            .ok_or_else(|| ExchangeError::Api("Missing high price in ticker".to_string()))?;
        
        let low = v["l"].as_str()
            .ok_or_else(|| ExchangeError::Api("Missing low price in ticker".to_string()))?;
        
        let volume = v["v"].as_str()
            .ok_or_else(|| ExchangeError::Api("Missing volume in ticker".to_string()))?;
        
        let event_time = v["E"].as_i64()
            .ok_or_else(|| ExchangeError::Api("Missing event time in ticker".to_string()))?;
        
        // Convert to decimal
        let parse_decimal = |s: &str| -> ExchangeResult<Decimal> {
            Decimal::from_str(s)
                .map_err(|e| ExchangeError::Api(format!("Failed to parse decimal: {}", e)))
        };
        
        Ok(MarketData {
            symbol: symbol.to_string(),
            timestamp: event_time,
            volume: parse_decimal(volume)?,
            last_price: parse_decimal(price)?,
            open_price: parse_decimal(open)?,
            close_price: parse_decimal(price)?,
            high_price: parse_decimal(high)?,
            low_price: parse_decimal(low)?,
            bid_price: None,
            ask_price: None,
            interval: None,
        })
    }
    
    /// Parse kline message
    fn parse_kline_message(
        symbol: &str,
        interval: &str,
        message: &str,
    ) -> ExchangeResult<MarketData> {
        let v: Value = serde_json::from_str(message)
            .map_err(|e| ExchangeError::Api(format!("Failed to parse kline message: {}", e)))?;
        
        let k = &v["k"];
        
        // Extract values
        let open = k["o"].as_str()
            .ok_or_else(|| ExchangeError::Api("Missing open price in kline".to_string()))?;
        
        let high = k["h"].as_str()
            .ok_or_else(|| ExchangeError::Api("Missing high price in kline".to_string()))?;
        
        let low = k["l"].as_str()
            .ok_or_else(|| ExchangeError::Api("Missing low price in kline".to_string()))?;
        
        let close = k["c"].as_str()
            .ok_or_else(|| ExchangeError::Api("Missing close price in kline".to_string()))?;
        
        let volume = k["v"].as_str()
            .ok_or_else(|| ExchangeError::Api("Missing volume in kline".to_string()))?;
        
        let close_time = k["T"].as_i64()
            .ok_or_else(|| ExchangeError::Api("Missing close time in kline".to_string()))?;
        
        // Convert to decimal
        let parse_decimal = |s: &str| -> ExchangeResult<Decimal> {
            Decimal::from_str(s)
                .map_err(|e| ExchangeError::Api(format!("Failed to parse decimal: {}", e)))
        };
        
        Ok(MarketData {
            symbol: symbol.to_string(),
            timestamp: close_time,
            volume: parse_decimal(volume)?,
            last_price: parse_decimal(close)?,
            open_price: parse_decimal(open)?,
            close_price: parse_decimal(close)?,
            high_price: parse_decimal(high)?,
            low_price: parse_decimal(low)?,
            bid_price: None,
            ask_price: None,
            interval: Some(interval.to_string()),
        })
    }
    
    /// Convert Binance kline to our candlestick format
    fn convert_kline_to_candlestick(
        symbol: &str,
        interval: &str,
        kline: &Value,
    ) -> ExchangeResult<Candlestick> {
        if let Value::Array(arr) = kline {
            if arr.len() < 11 {
                return Err(ExchangeError::Api("Invalid kline format".to_string()));
            }
            
            // Extract values
            let open_time = arr[0].as_i64()
                .ok_or_else(|| ExchangeError::Api("Invalid open time in kline".to_string()))?;
            
            let open = arr[1].as_str()
                .ok_or_else(|| ExchangeError::Api("Invalid open price in kline".to_string()))?;
            
            let high = arr[2].as_str()
                .ok_or_else(|| ExchangeError::Api("Invalid high price in kline".to_string()))?;
            
            let low = arr[3].as_str()
                .ok_or_else(|| ExchangeError::Api("Invalid low price in kline".to_string()))?;
            
            let close = arr[4].as_str()
                .ok_or_else(|| ExchangeError::Api("Invalid close price in kline".to_string()))?;
            
            let volume = arr[5].as_str()
                .ok_or_else(|| ExchangeError::Api("Invalid volume in kline".to_string()))?;
            
            let close_time = arr[6].as_i64()
                .ok_or_else(|| ExchangeError::Api("Invalid close time in kline".to_string()))?;
            
            let quote_volume = arr[7].as_str()
                .ok_or_else(|| ExchangeError::Api("Invalid quote volume in kline".to_string()))?;
            
            let trades = arr[8].as_i64()
                .ok_or_else(|| ExchangeError::Api("Invalid trade count in kline".to_string()))?;
            
            // Convert to decimal
            let parse_decimal = |s: &str| -> ExchangeResult<Decimal> {
                Decimal::from_str(s)
                    .map_err(|e| ExchangeError::Api(format!("Failed to parse decimal: {}", e)))
            };
            
            Ok(Candlestick {
                symbol: symbol.to_string(),
                interval: interval.to_string(),
                open_time,
                close_time,
                open: parse_decimal(open)?,
                high: parse_decimal(high)?,
                low: parse_decimal(low)?,
                close: parse_decimal(close)?,
                volume: parse_decimal(volume)?,
                quote_volume: parse_decimal(quote_volume)?,
                trades,
            })
        } else {
            Err(ExchangeError::Api("Invalid kline format, expected array".to_string()))
        }
    }
}

#[async_trait]
impl ExchangeClient for BinanceClient {
    async fn connect(&mut self) -> ExchangeResult<()> {
        // Initialize the account client
        let config = if self.testnet {
            BinanceConfig::testnet_us()
        } else {
            BinanceConfig::default()
        };
        
        let account_client = account::Account::new(
            Some(self.api_key.clone()),
            Some(self.api_secret.clone()),
            &config,
        );
        
        // Verify that we can connect by testing a simple API call
        let _ = account_client
            .get_account()
            .await
            .map_err(|e| ExchangeError::Authentication(format!("Failed to connect: {}", e)))?;
        
        self.account_client = Some(account_client);
        self.connected = true;
        
        Ok(())
    }
    
    async fn disconnect(&mut self) -> ExchangeResult<()> {
        // Cancel all websocket subscriptions
        for handle in self.websocket_handles.drain(..) {
            handle.abort();
        }
        
        self.connected = false;
        self.market_data_tx = None;
        
        Ok(())
    }
    
    async fn get_balances(&self) -> ExchangeResult<Vec<Balance>> {
        if !self.connected {
            return Err(ExchangeError::Connection("Not connected".to_string()));
        }
        
        let account_client = self.account_client.as_ref()
            .ok_or_else(|| ExchangeError::Connection("Account client not initialized".to_string()))?;
        
        let account = account_client
            .get_account()
            .await
            .map_err(|e| ExchangeError::Api(format!("Failed to get account: {}", e)))?;
        
        let mut balances = Vec::new();
        
        for balance in account.balances {
            let free = Decimal::from_str(&balance.free)
                .map_err(|e| ExchangeError::Api(format!("Failed to parse free balance: {}", e)))?;
            
            let locked = Decimal::from_str(&balance.locked)
                .map_err(|e| ExchangeError::Api(format!("Failed to parse locked balance: {}", e)))?;
            
            if free > Decimal::ZERO || locked > Decimal::ZERO {
                balances.push(Balance::new(&balance.asset, free, locked));
            }
        }
        
        Ok(balances)
    }
    
    async fn get_balance(&self, asset: &str) -> ExchangeResult<Balance> {
        let balances = self.get_balances().await?;
        
        balances
            .into_iter()
            .find(|b| b.asset == asset)
            .ok_or_else(|| ExchangeError::Account(format!("Asset not found: {}", asset)))
    }
    
    async fn place_order(&self, order: &Order) -> ExchangeResult<OrderResponse> {
        if !self.connected {
            return Err(ExchangeError::Connection("Not connected".to_string()));
        }
        
        let account_client = self.account_client.as_ref()
            .ok_or_else(|| ExchangeError::Connection("Account client not initialized".to_string()))?;
        
        // Create parameters based on order type
        let mut order_params = account::OrderRequest::new(
            order.symbol.clone(),
            match order.side {
                OrderSide::Buy => binance::account::OrderSide::Buy,
                OrderSide::Sell => binance::account::OrderSide::Sell,
            },
        );
        
        // Set quantity
        order_params.quantity = Some(order.quantity.to_string());
        
        // Set order type and price if needed
        match &order.order_type {
            OrderType::Market => {
                order_params.order_type = Some(binance::account::OrderType::Market);
            }
            OrderType::Limit(price) => {
                order_params.order_type = Some(binance::account::OrderType::Limit);
                order_params.price = Some(price.to_string());
                order_params.time_in_force = Some(binance::account::TimeInForce::GTC);
            }
            OrderType::Stop(stop_price) => {
                order_params.order_type = Some(binance::account::OrderType::StopLoss);
                order_params.stop_price = Some(stop_price.to_string());
            }
            OrderType::StopLimit(stop_price, limit_price) => {
                order_params.order_type = Some(binance::account::OrderType::StopLossLimit);
                order_params.price = Some(limit_price.to_string());
                order_params.stop_price = Some(stop_price.to_string());
                order_params.time_in_force = Some(binance::account::TimeInForce::GTC);
            }
        }
        
        // Set client order ID if provided
        if let Some(client_order_id) = &order.client_order_id {
            order_params.new_client_order_id = Some(client_order_id.clone());
        }
        
        // Place the order
        let response = account_client
            .place_order(order_params)
            .await
            .map_err(|e| ExchangeError::Order(format!("Failed to place order: {}", e)))?;
        
        // Parse the response
        let status = match response.status.as_deref() {
            Some("FILLED") => OrderStatus::Filled,
            Some("PARTIALLY_FILLED") => OrderStatus::PartiallyFilled,
            Some("NEW") => OrderStatus::New,
            Some("CANCELED") => OrderStatus::Canceled,
            Some("REJECTED") => OrderStatus::Rejected,
            Some("PENDING_CANCEL") => OrderStatus::Pending,
            _ => OrderStatus::New,
        };
        
        let filled_qty = response.executed_qty
            .as_ref()
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or_else(|| Decimal::ZERO);
        
        let avg_price = response.cummulative_quote_qty
            .as_ref()
            .and_then(|q| Decimal::from_str(q).ok())
            .and_then(|q| {
                if filled_qty > Decimal::ZERO {
                    Some(q / filled_qty)
                } else {
                    None
                }
            });
        
        Ok(OrderResponse {
            order_id: response.order_id.to_string(),
            client_order_id: response.client_order_id,
            status,
            filled_quantity: filled_qty,
            average_price: avg_price,
            timestamp: response.transact_time.unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
        })
    }
    
    async fn cancel_order(&self, order_id: &str) -> ExchangeResult<OrderResponse> {
        if !self.connected {
            return Err(ExchangeError::Connection("Not connected".to_string()));
        }
        
        let account_client = self.account_client.as_ref()
            .ok_or_else(|| ExchangeError::Connection("Account client not initialized".to_string()))?;
        
        // Parse the order ID
        let order_id_parsed = order_id.parse::<u64>()
            .map_err(|_| ExchangeError::Order(format!("Invalid order ID: {}", order_id)))?;
        
        // We need the symbol for the cancel request, but we don't have it here
        // so we need to get it from open orders
        let open_orders = account_client
            .get_open_orders(None)
            .await
            .map_err(|e| ExchangeError::Api(format!("Failed to get open orders: {}", e)))?;
        
        let order = open_orders
            .iter()
            .find(|o| o.order_id == order_id_parsed)
            .ok_or_else(|| ExchangeError::Order(format!("Order not found: {}", order_id)))?;
        
        let symbol = order.symbol.clone();
        
        // Cancel the order
        let response = account_client
            .cancel_order(&symbol, order_id_parsed, None)
            .await
            .map_err(|e| ExchangeError::Order(format!("Failed to cancel order: {}", e)))?;
        
        // Parse the response
        let filled_qty = response.executed_qty
            .as_ref()
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or_else(|| Decimal::ZERO);
        
        let avg_price = None; // Not provided in cancel response
        
        Ok(OrderResponse {
            order_id: response.order_id.to_string(),
            client_order_id: response.client_order_id,
            status: OrderStatus::Canceled,
            filled_quantity: filled_qty,
            average_price: avg_price,
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }
    
    async fn get_order_status(&self, order_id: &str) -> ExchangeResult<OrderResponse> {
        if !self.connected {
            return Err(ExchangeError::Connection("Not connected".to_string()));
        }
        
        let account_client = self.account_client.as_ref()
            .ok_or_else(|| ExchangeError::Connection("Account client not initialized".to_string()))?;
        
        // Parse the order ID
        let order_id_parsed = order_id.parse::<u64>()
            .map_err(|_| ExchangeError::Order(format!("Invalid order ID: {}", order_id)))?;
        
        // We need the symbol for the order query, but we don't have it here
        // so we need to get it from all orders
        let all_orders = account_client
            .get_all_orders("BTCUSDT") // This is a limitation of the example, needs to be fixed
            .await
            .map_err(|e| ExchangeError::Api(format!("Failed to get all orders: {}", e)))?;
        
        let order = all_orders
            .iter()
            .find(|o| o.order_id == order_id_parsed)
            .ok_or_else(|| ExchangeError::Order(format!("Order not found: {}", order_id)))?;
        
        // Parse the status
        let status = match order.status.as_deref() {
            Some("FILLED") => OrderStatus::Filled,
            Some("PARTIALLY_FILLED") => OrderStatus::PartiallyFilled,
            Some("NEW") => OrderStatus::New,
            Some("CANCELED") => OrderStatus::Canceled,
            Some("REJECTED") => OrderStatus::Rejected,
            Some("PENDING_CANCEL") => OrderStatus::Pending,
            _ => OrderStatus::New,
        };
        
        let filled_qty = order.executed_qty
            .as_ref()
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or_else(|| Decimal::ZERO);
        
        let avg_price = order.cummulative_quote_qty
            .as_ref()
            .and_then(|q| Decimal::from_str(q).ok())
            .and_then(|q| {
                if filled_qty > Decimal::ZERO {
                    Some(q / filled_qty)
                } else {
                    None
                }
            });
        
        Ok(OrderResponse {
            order_id: order.order_id.to_string(),
            client_order_id: order.client_order_id.clone(),
            status,
            filled_quantity: filled_qty,
            average_price: avg_price,
            timestamp: order.time.unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
        })
    }
    
    async fn get_open_orders(&self, symbol: Option<&str>) -> ExchangeResult<Vec<OrderResponse>> {
        if !self.connected {
            return Err(ExchangeError::Connection("Not connected".to_string()));
        }
        
        let account_client = self.account_client.as_ref()
            .ok_or_else(|| ExchangeError::Connection("Account client not initialized".to_string()))?;
        
        // Get open orders
        let open_orders = account_client
            .get_open_orders(symbol.map(|s| s.to_string()))
            .await
            .map_err(|e| ExchangeError::Api(format!("Failed to get open orders: {}", e)))?;
        
        // Convert to our format
        let mut results = Vec::new();
        
        for order in open_orders {
            let filled_qty = order.executed_qty
                .as_ref()
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or_else(|| Decimal::ZERO);
            
            let avg_price = order.cummulative_quote_qty
                .as_ref()
                .and_then(|q| Decimal::from_str(q).ok())
                .and_then(|q| {
                    if filled_qty > Decimal::ZERO {
                        Some(q / filled_qty)
                    } else {
                        None
                    }
                });
            
            let status = if filled_qty > Decimal::ZERO {
                OrderStatus::PartiallyFilled
            } else {
                OrderStatus::New
            };
            
            results.push(OrderResponse {
                order_id: order.order_id.to_string(),
                client_order_id: order.client_order_id,
                status,
                filled_quantity: filled_qty,
                average_price: avg_price,
                timestamp: order.time.unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
            });
        }
        
        Ok(results)
    }
    
    async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: Option<u32>,
    ) -> ExchangeResult<PriceHistory> {
        if !self.connected && !symbol.contains("TEST") { // allow testing with mock symbols
            return Err(ExchangeError::Connection("Not connected".to_string()));
        }
        
        // Map interval format
        let binance_interval = match interval {
            "1m" => "1m",
            "3m" => "3m",
            "5m" => "5m",
            "15m" => "15m",
            "30m" => "30m",
            "1h" => "1h",
            "2h" => "2h",
            "4h" => "4h",
            "6h" => "6h",
            "8h" => "8h",
            "12h" => "12h",
            "1d" => "1d",
            "3d" => "3d",
            "1w" => "1w",
            "1M" => "1M",
            _ => return Err(ExchangeError::InvalidSymbol(format!("Invalid interval: {}", interval))),
        };
        
        // Create request
        let limit = limit.unwrap_or(500).min(1000); // Binance limit is 1000
        
        // Send the request
        let response = self.http_client
            .get_klines(symbol, binance_interval, limit, None, None)
            .await
            .map_err(|e| ExchangeError::Api(format!("Failed to get klines: {}", e)))?;
        
        // Parse the response
        let mut price_history = PriceHistory::new(symbol, interval);
        
        for kline in response {
            let candlestick = Self::convert_kline_to_candlestick(symbol, interval, &kline)?;
            price_history.add_candle(candlestick);
        }
        
        Ok(price_history)
    }
    async fn get_ticker(&self, symbol: &str) -> ExchangeResult<MarketData> {
        if !self.connected && !symbol.contains("TEST") { // allow testing with mock symbols
            return Err(ExchangeError::Connection("Not connected".to_string()));
        }
        
        // Get the current ticker from Binance
        let ticker = self.http_client
            .get_price(symbol)
            .await
            .map_err(|e| ExchangeError::Api(format!("Failed to get ticker: {}", e)))?;
        
        // Get more detailed ticker information
        let ticker_stats = self.http_client
            .get_24h_price_stats(symbol)
            .await
            .map_err(|e| ExchangeError::Api(format!("Failed to get ticker stats: {}", e)))?;
        
        // Parse the price
        let price = Decimal::from_str(&ticker.price)
            .map_err(|e| ExchangeError::Api(format!("Failed to parse price: {}", e)))?;
        
        // Parse additional stats
        let parse_decimal = |s: &str| -> ExchangeResult<Decimal> {
            Decimal::from_str(s)
                .map_err(|e| ExchangeError::Api(format!("Failed to parse decimal: {}", e)))
        };
        
        let volume = parse_decimal(&ticker_stats.volume)?;
        let open_price = parse_decimal(&ticker_stats.open_price)?;
        let high_price = parse_decimal(&ticker_stats.high_price)?;
        let low_price = parse_decimal(&ticker_stats.low_price)?;
        
        Ok(MarketData {
            symbol: symbol.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            volume,
            last_price: price,
            open_price,
            close_price: price,
            high_price,
            low_price,
            bid_price: None,
            ask_price: None,
            interval: None,
        })
    }
    
    /// Subscribe to market data streams
    async fn subscribe_to_market_data(
        &mut self,
        symbols: &[String],
        callback: Box<dyn MarketDataHandler>,
    ) -> ExchangeResult<()> {
        // Start the market data processor
        self.start_market_data_processor(callback).await?;
        
        // Get the market data sender
        let tx = self.market_data_tx.clone()
            .ok_or_else(|| ExchangeError::Connection("Market data sender not initialized".to_string()))?;
        
        // Start WebSocket connections for each symbol
        for symbol in symbols {
            // Start ticker WebSocket
            let symbol_clone = symbol.clone();
            let tx_clone = tx.clone();
            let ticker_handle = tokio::spawn(async move {
                if let Err(e) = Self::handle_ticker_websocket(symbol_clone, tx_clone).await {
                    log::error!("Ticker WebSocket error: {:?}", e);
                }
            });
            self.websocket_handles.push(ticker_handle);
            
            // Start kline WebSocket with 1m interval
            let symbol_clone = symbol.clone();
            let tx_clone = tx.clone();
            let kline_handle = tokio::spawn(async move {
                if let Err(e) = Self::handle_kline_websocket(symbol_clone, "1m".to_string(), tx_clone).await {
                    log::error!("Kline WebSocket error: {:?}", e);
                }
            });
            self.websocket_handles.push(kline_handle);
        }
        
        Ok(())
    }
}