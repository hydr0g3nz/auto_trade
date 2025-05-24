use binance_spot_connector_rust::{
    market_stream::{kline::KlineStream, ticker::TickerStream},
    market::klines::KlineInterval,
    tokio_tungstenite::BinanceWebSocketClient,
};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use crate::dto::{parse_websocket_message, parse_websocket_message_ticker, Kline, TickerData};
use crate::domain::TradingError;
#[derive(Clone)]
pub struct WebSocketHandler {
    symbol: String,
}

impl WebSocketHandler {
    pub fn new(symbol: String) -> Self {
        Self { symbol }
    }

    pub async fn start_kline_stream(&self) -> Result<mpsc::Receiver<Kline>, TradingError> {
        let (tx, rx) = mpsc::channel(100);
        let symbol = self.symbol.clone();
        
        tokio::spawn(async move {
            if let Err(e) = Self::handle_kline_stream(symbol, tx).await {
                log::error!("Kline stream error: {:?}", e);
            }
        });
        
        Ok(rx)
    }

    pub async fn start_ticker_stream(&self) -> Result<mpsc::Receiver<TickerData>, TradingError> {
        let (tx, rx) = mpsc::channel(100);
        let symbol = self.symbol.clone();
        
        tokio::spawn(async move {
            if let Err(e) = Self::handle_ticker_stream(symbol, tx).await {
                log::error!("Ticker stream error: {:?}", e);
            }
        });
        
        Ok(rx)
    }

    async fn handle_kline_stream(
        symbol: String,
        sender: mpsc::Sender<Kline>
    ) -> Result<(), TradingError> {
        let (mut conn, _) = BinanceWebSocketClient::connect_async_default()
            .await
            .map_err(|e| TradingError::ConnectionError(e.to_string()))?;

        conn.subscribe(vec![
            &KlineStream::new(&symbol, KlineInterval::Minutes1).into()
        ]).await;

        while let Some(message) = conn.as_mut().next().await {
            match message {
                Ok(message) => {
                    let binary_data = message.into_data();
                    if let Ok(data) = std::str::from_utf8(&binary_data) {
                        if let Ok(response) = parse_websocket_message(data) {
                            let kline = response.data.kline;
                            if sender.send(kline).await.is_err() {
                                break; // Receiver dropped
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }

        conn.close().await.map_err(|e| TradingError::ConnectionError(e.to_string()))?;
        Ok(())
    }

    async fn handle_ticker_stream(
        symbol: String,
        sender: mpsc::Sender<TickerData>
    ) -> Result<(), TradingError> {
        let (mut conn, _) = BinanceWebSocketClient::connect_async_default()
            .await
            .map_err(|e| TradingError::ConnectionError(e.to_string()))?;

        conn.subscribe(vec![
            &TickerStream::from_symbol(&symbol).into()
        ]).await;

        while let Some(message) = conn.as_mut().next().await {
            match message {
                Ok(message) => {
                    let binary_data = message.into_data();
                    if let Ok(data) = std::str::from_utf8(&binary_data) {
                        if let Ok(response) = parse_websocket_message_ticker(data) {
                            if sender.send(response.data).await.is_err() {
                                break; // Receiver dropped
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }

        conn.close().await.map_err(|e| TradingError::ConnectionError(e.to_string()))?;
        Ok(())
    }
}