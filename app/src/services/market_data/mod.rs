//! Unified, versioned market-data event layer.
//!
//! Every producer (polymarket_ws, *_scanner, aerodrome_scanner, ...) emits
//! `L2Event`s through `MarketDataBus`. The `cache_writer` task subscribes,
//! maintains a top-of-book snapshot in Redis for lagged / cross-process
//! consumers, and fans out raw events on `r44:l2:{venue}:{market}` so the
//! Next.js frontend and future workers can stay in sync without polling.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod bus;
pub mod cache;
pub mod cache_writer;

pub use bus::MarketDataBus;
pub use cache::TopOfBook;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Venue {
    Polymarket,
    Limitless,
    Aerodrome,
    Internal,
}

impl Venue {
    pub fn as_str(self) -> &'static str {
        match self {
            Venue::Polymarket => "polymarket",
            Venue::Limitless => "limitless",
            Venue::Aerodrome => "aerodrome",
            Venue::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Level {
    pub price: f64,
    pub size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum L2Payload {
    /// Absolute book state. Self-sufficient; safe to receive without prior context.
    Snapshot {
        bids: Vec<L2Level>,
        asks: Vec<L2Level>,
        last_trade: Option<f64>,
    },
    /// Per-level changes. Consumers that miss one must reconcile from the cache.
    Delta {
        bid_updates: Vec<L2Level>,
        ask_updates: Vec<L2Level>,
        removed_bids: Vec<f64>,
        removed_asks: Vec<f64>,
    },
    /// Trade print. Does not mutate the book.
    Trade {
        price: f64,
        size: f64,
        side: Side,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Event {
    pub venue: Venue,
    pub market_key: String,
    pub seq: u64,
    pub observed_at: DateTime<Utc>,
    pub payload: L2Payload,
}

