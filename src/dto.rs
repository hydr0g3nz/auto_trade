use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketResponse {
    pub stream: String,
    pub data: KlineData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KlineData {
    #[serde(rename = "e")]
    pub event_type: String,  // "kline"
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "k")]
    pub kline: Kline,
}

#[derive(Debug, Serialize, Deserialize)]
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