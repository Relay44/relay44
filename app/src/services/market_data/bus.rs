//! In-process broadcast bus for L2 market-data events.
//!
//! Producers emit here; the `cache_writer` task is the single Redis-side
//! subscriber, so producers stay ignorant of Redis. Late consumers that see
//! `RecvError::Lagged` should reconcile from `L2Cache::read_top`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use log::warn;
use tokio::sync::broadcast;

use super::{L2Event, Venue};

const MARKET_DATA_BUS_CAPACITY: usize = 4096;

pub struct MarketDataBus {
    tx: broadcast::Sender<Arc<L2Event>>,
    seq_polymarket: AtomicU64,
    seq_limitless: AtomicU64,
    seq_aerodrome: AtomicU64,
    seq_internal: AtomicU64,
}

impl MarketDataBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(MARKET_DATA_BUS_CAPACITY);
        Self {
            tx,
            seq_polymarket: AtomicU64::new(0),
            seq_limitless: AtomicU64::new(0),
            seq_aerodrome: AtomicU64::new(0),
            seq_internal: AtomicU64::new(0),
        }
    }

    /// Monotonic per-venue sequence number. Consumers use it to detect gaps
    /// on a single channel; it does not need to be per-market because the
    /// reconciliation substrate (snapshot cache) is keyed by market.
    pub fn next_seq(&self, venue: Venue) -> u64 {
        let counter = match venue {
            Venue::Polymarket => &self.seq_polymarket,
            Venue::Limitless => &self.seq_limitless,
            Venue::Aerodrome => &self.seq_aerodrome,
            Venue::Internal => &self.seq_internal,
        };
        counter.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn emit(&self, event: L2Event) {
        if self.tx.receiver_count() == 0 {
            return;
        }
        if let Err(e) = self.tx.send(Arc::new(event)) {
            warn!("market_data bus send failed: {}", e);
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<L2Event>> {
        self.tx.subscribe()
    }

    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for MarketDataBus {
    fn default() -> Self {
        Self::new()
    }
}
