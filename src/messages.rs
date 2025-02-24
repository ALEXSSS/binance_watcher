use serde::{Deserialize, Serialize};

/// web socket Subscription entity [documentation]
///
/// [documentation]: [https://developers.binance.com/docs/derivatives/usds-margined-futures/websocket-market-streams/Live-Subscribing-Unsubscribing-to-streams]
#[derive(Serialize, Deserialize)]
pub struct Subscription {
    pub method: String,
    pub params: Vec<String>,
    pub id: String,
}

/// web socket BookDepthUpdate entity [documentation]
///
/// [documentation]: [https://developers.binance.com/docs/derivatives/usds-margined-futures/websocket-market-streams/Diff-Book-Depth-Streams]
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct BookDepthUpdate {
    pub e: String,        // Event type
    pub E: u64,           // Event time
    pub T: u64,           // Transaction time
    pub s: String,        // Symbol
    pub U: u64,           // First update ID in event
    pub u: u64,           // Final update ID in event
    pub pu: u64,          // Final update ID in last stream(ie `u` in last stream)
    pub b: Vec<LevelApi>, // bids
    pub a: Vec<LevelApi>, // asks
}

/// http api full book response body entity
///
/// [documentation]: [https://developers.binance.com/docs/derivatives/usds-margined-futures/websocket-market-streams/How-to-manage-a-local-order-book-correctly]
#[derive(Serialize, Deserialize)]
pub struct FullBook {
    // tbd: warning could've been ignored as above but it has a long name?
    #[serde(rename(deserialize = "lastUpdateId"))]
    pub last_update_id: u64,
    pub bids: Vec<LevelApi>,
    pub asks: Vec<LevelApi>,
}

/// Book level sent by binance via ws and http, the order matters
#[derive(Serialize, Deserialize)]
pub struct LevelApi {
    pub price: String,
    pub quantity: String,
}
