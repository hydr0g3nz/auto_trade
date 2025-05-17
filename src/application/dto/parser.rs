// src/application/dto/parser.rs
// Parsers for DTOs

use super::{ApplicationError, WebSocketResponse, WebSocketMessage, KlineResponse};

/// Parse a WebSocket message into a KlineData response
pub fn parse_websocket_message(message: &str) -> Result<WebSocketResponse, ApplicationError> {
    serde_json::from_str(message).map_err(|e| ApplicationError::JsonError(e))
}

/// Parse a WebSocket message into a TickerData response
pub fn parse_websocket_message_ticker(message: &str) -> Result<WebSocketMessage, ApplicationError> {
    serde_json::from_str(message).map_err(|e| ApplicationError::JsonError(e))
}

impl KlineResponse {
    pub fn from_raw_data(data: &[serde_json::Value]) -> Result<Self, ApplicationError> {
        if data.len() < 11 {
            return Err(ApplicationError::ParseError(format!(
                "Invalid data length: expected 11 elements, got {}",
                data.len()
            )));
        }

        let parse_timestamp = |value: &serde_json::Value, field: &str| -> Result<chrono::DateTime<chrono::Utc>, ApplicationError> {
            value
                .as_i64()
                .ok_or_else(|| ApplicationError::ParseError(format!("Invalid {} format", field)))
                .and_then(|ts| {
                    chrono::DateTime::from_timestamp_millis(ts).ok_or_else(|| {
                        ApplicationError::ParseError(format!("Invalid timestamp for {}: {}", field, ts))
                    })
                })
        };

        let parse_float = |value: &serde_json::Value, field: &str| -> Result<f64, ApplicationError> {
            value
                .as_str()
                .ok_or_else(|| ApplicationError::ParseError(format!("Invalid {} format", field)))
                .and_then(|s| s.parse().map_err(ApplicationError::NumberParseError))
        };

        Ok(Self {
            open_time: parse_timestamp(&data[0], "open_time")?,
            open_price: parse_float(&data[1], "open_price")?,
            high_price: parse_float(&data[2], "high_price")?,
            low_price: parse_float(&data[3], "low_price")?,
            close_price: parse_float(&data[4], "close_price")?,
            volume: parse_float(&data[5], "volume")?,
            close_time: parse_timestamp(&data[6], "close_time")?,
            quote_asset_volume: parse_float(&data[7], "quote_asset_volume")?,
            number_of_trades: data[8]
                .as_u64()
                .ok_or_else(|| ApplicationError::ParseError("Invalid number_of_trades format".to_string()))?,
            taker_buy_base_volume: parse_float(&data[9], "taker_buy_base_volume")?,
            taker_buy_quote_volume: parse_float(&data[10], "taker_buy_quote_volume")?,
        })
    }
}