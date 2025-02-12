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

#[derive(Debug, Serialize, Deserialize,Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize,Default)]
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
pub fn parse_websocket_message_ticker(message: &str) -> Result<WebSocketMessage, serde_json::Error> {
    serde_json::from_str(message)
}