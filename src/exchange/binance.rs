// src/exchange/binance.rs (continued)
async fn get_ticker(&self, symbol: &str) -> ExchangeResult<MarketData> {
    if !self.connected {
        return Err(ExchangeError::Connection("Not connected".to_string()));
    }

    // Send the request
    let response = self
        .http_client
        .send(market::ticker_price(symbol))
        .await
        .map_err(|e| ExchangeError::Api(format!("Failed to get ticker: {}", e)))?
        .into_body_str()
        .await
        .map_err(|e| ExchangeError::Api(format!("Failed to read response body: {}", e)))?;

    // Parse the response
    let ticker: Value = serde_json::from_str(&response)
        .map_err(|e| ExchangeError::Api(format!("Failed to parse ticker response: {}", e)))?;

    // Get price
    let price = ticker["price"]
        .as_str()
        .ok_or_else(|| ExchangeError::Api("Missing price in ticker response".to_string()))?;

    let price_decimal = Decimal::from_str(price)
        .map_err(|e| ExchangeError::Api(format!("Failed to parse price: {}", e)))?;

    // Get 24hr ticker for additional data
    let response_24hr = self
        .http_client
        .send(market::ticker_24hr(symbol))
        .await
        .map_err(|e| ExchangeError::Api(format!("Failed to get 24hr ticker: {}", e)))?
        .into_body_str()
        .await
        .map_err(|e| ExchangeError::Api(format!("Failed to read response body: {}", e)))?;

    // Parse the 24hr response
    let ticker_24hr: Value = serde_json::from_str(&response_24hr)
        .map_err(|e| ExchangeError::Api(format!("Failed to parse 24hr ticker: {}", e)))?;

    // Extract values with fallbacks to defaults
    let parse_decimal = |field: &str| -> Decimal {
        ticker_24hr[field]
            .as_str()
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or_default()
    };

    let timestamp = ticker_24hr["closeTime"]
        .as_i64()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    Ok(MarketData {
        symbol: symbol.to_string(),
        timestamp,
        last_price: price_decimal,
        open_price: parse_decimal("openPrice"),
        high_price: parse_decimal("highPrice"),
        low_price: parse_decimal("lowPrice"),
        close_price: price_decimal, // Use current price as close
        volume: parse_decimal("volume"),
        bid_price: Some(parse_decimal("bidPrice")),
        ask_price: Some(parse_decimal("askPrice")),
        interval: None,
    })
}

async fn subscribe_to_market_data(
    &self,
    symbols: &[String],
    callback: Box<dyn MarketDataHandler>,
) -> ExchangeResult<()> {
    if !self.connected {
        return Err(ExchangeError::Connection("Not connected".to_string()));
    }

    // Start the market data processor if not already started
    if self.market_data_tx.is_none() {
        self.start_market_data_processor(callback).await?;
    }

    let tx = self
        .market_data_tx
        .as_ref()
        .ok_or_else(|| ExchangeError::Api("Market data processor not initialized".to_string()))?;

    // Subscribe to ticker updates for each symbol
    for symbol in symbols {
        let symbol_clone = symbol.clone();
        let tx_clone = tx.clone();

        // Start a WebSocket connection for ticker data
        let ticker_handle = tokio::spawn(async move {
            if let Err(e) = Self::handle_ticker_websocket(symbol_clone.clone(), tx_clone).await {
                log::error!("Ticker WebSocket error: {:?}", e);
            }
        });

        self.websocket_handles.push(ticker_handle);

        // Subscribe to 1-minute klines for each symbol
        let symbol_clone = symbol.clone();
        let tx_clone = tx.clone();
        let interval = "1m".to_string();

        let kline_handle = tokio::spawn(async move {
            if let Err(e) = Self::handle_kline_websocket(symbol_clone, interval, tx_clone).await {
                log::error!("Kline WebSocket error: {:?}", e);
            }
        });

        self.websocket_handles.push(kline_handle);
    }

    Ok(())
}
