use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use std::num::ParseFloatError;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketResponse {
    pub stream: String,
    pub data: KlineData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KlineData {
    #[serde(rename = "e")]
    pub event_type: String, // "kline"
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "k")]
    pub kline: Kline,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Kline {
    #[serde(rename = "t")]
    pub start_time: i64,
    #[serde(rename = "T")]
    pub end_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "i")]
    pub interval: String,
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    #[serde(rename = "o")]
    pub open_price: String,
    #[serde(rename = "c")]
    pub close_price: String,
    #[serde(rename = "h")]
    pub high_price: String,
    #[serde(rename = "l")]
    pub low_price: String,
    #[serde(rename = "v")]
    pub volume: String,
    #[serde(rename = "n")]
    pub number_of_trades: i64,
    #[serde(rename = "x")]
    pub is_closed: bool,
    #[serde(rename = "q")]
    pub quote_volume: String,
    #[serde(rename = "V")]
    pub taker_buy_volume: String,
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String,
    #[serde(rename = "B")]
    pub ignore: String,
}
pub fn parse_websocket_message(message: &str) -> Result<WebSocketResponse, serde_json::Error> {
    serde_json::from_str(message)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub stream: String,
    pub data: TickerData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TickerData {
    /// Event type
    #[serde(rename = "e")]
    pub event_type: String,

    /// Event time
    #[serde(rename = "E")]
    pub event_time: i64,

    /// Symbol
    #[serde(rename = "s")]
    pub symbol: String,

    /// Price change
    #[serde(rename = "p")]
    pub price_change: String,

    /// Price change percent
    #[serde(rename = "P")]
    pub price_change_percent: String,

    /// Weighted average price
    #[serde(rename = "w")]
    pub weighted_avg_price: String,

    /// First trade before 24h
    #[serde(rename = "x")]
    pub first_trade_price: String,

    /// Last price
    #[serde(rename = "c")]
    pub last_price: String,

    /// Last quantity
    #[serde(rename = "Q")]
    pub last_quantity: String,

    /// Best bid price
    #[serde(rename = "b")]
    pub bid_price: String,

    /// Best bid quantity
    #[serde(rename = "B")]
    pub bid_quantity: String,

    /// Best ask price
    #[serde(rename = "a")]
    pub ask_price: String,

    /// Best ask quantity
    #[serde(rename = "A")]
    pub ask_quantity: String,

    /// Open price
    #[serde(rename = "o")]
    pub open_price: String,

    /// High price
    #[serde(rename = "h")]
    pub high_price: String,

    /// Low price
    #[serde(rename = "l")]
    pub low_price: String,

    /// Total traded volume
    #[serde(rename = "v")]
    pub volume: String,

    /// Total traded quote asset volume
    #[serde(rename = "q")]
    pub quote_volume: String,

    /// Statistics open time
    #[serde(rename = "O")]
    pub open_time: i64,

    /// Statistics close time
    #[serde(rename = "C")]
    pub close_time: i64,

    /// First trade ID
    #[serde(rename = "F")]
    pub first_trade_id: i64,

    /// Last trade ID
    #[serde(rename = "L")]
    pub last_trade_id: i64,

    /// Total number of trades
    #[serde(rename = "n")]
    pub total_trades: i64,
}
pub fn parse_websocket_message_ticker(
    message: &str,
) -> Result<WebSocketMessage, serde_json::Error> {
    serde_json::from_str(message)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("API error: {0}")]
    ApiError(#[from] Box<dyn StdError + Send + Sync>),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Number parse error: {0}")]
    NumberParseError(#[from] ParseFloatError),

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Request error: {0}")]
    RequestError(String),

    #[error("HTTP error: {0}")]
    HttpError(String),
}

impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Error::RequestError(err.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KlineResponse {
    pub open_time: DateTime<Utc>,
    pub open_price: f64,
    pub high_price: f64,
    pub low_price: f64,
    pub close_price: f64,
    pub volume: f64,
    pub close_time: DateTime<Utc>,
    pub quote_asset_volume: f64,
    pub number_of_trades: u64,
    pub taker_buy_base_volume: f64,
    pub taker_buy_quote_volume: f64,
}

impl KlineResponse {
    pub fn from_raw_data(data: &[serde_json::Value]) -> Result<Self, Error> {
        if data.len() < 11 {
            return Err(Error::ParseError(format!(
                "Invalid data length: expected 11 elements, got {}",
                data.len()
            )));
        }

        let parse_timestamp =
            |value: &serde_json::Value, field: &str| -> Result<DateTime<Utc>, Error> {
                value
                    .as_i64()
                    .ok_or_else(|| Error::ParseError(format!("Invalid {} format", field)))
                    .and_then(|ts| {
                        DateTime::from_timestamp_millis(ts).ok_or_else(|| {
                            Error::ParseError(format!("Invalid timestamp for {}: {}", field, ts))
                        })
                    })
            };

        let parse_float = |value: &serde_json::Value, field: &str| -> Result<f64, Error> {
            value
                .as_str()
                .ok_or_else(|| Error::ParseError(format!("Invalid {} format", field)))
                .and_then(|s| s.parse().map_err(Error::NumberParseError))
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
                .ok_or_else(|| Error::ParseError("Invalid number_of_trades format".to_string()))?,
            taker_buy_base_volume: parse_float(&data[9], "taker_buy_base_volume")?,
            taker_buy_quote_volume: parse_float(&data[10], "taker_buy_quote_volume")?,
        })
    }
}
