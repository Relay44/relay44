//! Shared in-memory queue for alerter signals.
//!
//! Individual alerters (probability_alert, volume_spike_alert, future
//! additions) publish `Signal`s to this bus instead of sending to Telegram
//! directly. The digest_scheduler drains the bus on a fixed cadence, ranks
//! the signals, and emits a single aggregated Telegram message — which keeps
//! the supergroup from being flooded when many markets move at once.
//!
//! The bus is bounded (oldest entries dropped on overflow) so a misbehaving
//! alerter can't leak unbounded memory if the digest_scheduler is disabled.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::RwLock;

const DEFAULT_CAPACITY: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalKind {
    ProbabilityShift,
    VolumeSpike,
    NewMarket,
}

impl SignalKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SignalKind::ProbabilityShift => "probability_shift",
            SignalKind::VolumeSpike => "volume_spike",
            SignalKind::NewMarket => "new_market",
        }
    }
}

/// A single alerter signal. The `body` is pre-formatted by the producing
/// alerter (e.g. "↑ 10.0¢ → 18.0¢ (+80.0%)") so the digest_scheduler can
/// emit one without reaching back for venue-specific data.
#[derive(Debug, Clone)]
pub struct Signal {
    pub kind: SignalKind,
    pub venue: String,
    pub market_key: String,
    pub slug: Option<String>,
    pub question: String,
    pub liquidity_usd: Option<f64>,
    pub volume_24h_usd: Option<f64>,
    pub category: Option<String>,
    /// Magnitude of the triggering move. For ProbabilityShift this is |Δ%|;
    /// for VolumeSpike this is the multiplier over baseline. Used for scoring.
    pub move_size: f64,
    pub body: String,
    /// Unix seconds. Used for recency decay in the scorer.
    pub timestamp_secs: u64,
}

impl Signal {
    /// Stable identity across alerter kinds for cooldown keying. Different
    /// signal kinds on the same market share a cooldown so we don't emit two
    /// digest entries for the same market in one digest run.
    pub fn dedup_key(&self) -> String {
        format!("{}:{}", self.venue, self.market_key)
    }
}

pub struct AlertBus {
    capacity: usize,
    signals: RwLock<VecDeque<Signal>>,
}

impl Default for AlertBus {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }
}

impl AlertBus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            signals: RwLock::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub async fn publish(&self, signal: Signal) {
        let mut q = self.signals.write().await;
        if q.len() >= self.capacity {
            q.pop_front();
        }
        q.push_back(signal);
    }

    /// Empties the bus and returns every signal queued since the previous
    /// drain. Callers (the digest_scheduler) are responsible for scoring and
    /// filtering — the bus itself doesn't know what's "important".
    pub async fn drain(&self) -> Vec<Signal> {
        let mut q = self.signals.write().await;
        let out: Vec<Signal> = q.drain(..).collect();
        out
    }

    pub async fn len(&self) -> usize {
        self.signals.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.signals.read().await.is_empty()
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_signal(market: &str) -> Signal {
        Signal {
            kind: SignalKind::ProbabilityShift,
            venue: "polymarket".to_string(),
            market_key: market.to_string(),
            slug: Some("slug".to_string()),
            question: "Q?".to_string(),
            liquidity_usd: Some(1_000.0),
            volume_24h_usd: Some(2_000.0),
            category: None,
            move_size: 10.0,
            body: "body".to_string(),
            timestamp_secs: now_secs(),
        }
    }

    #[tokio::test]
    async fn publish_then_drain_returns_all_in_order() {
        let bus = AlertBus::with_capacity(16);
        bus.publish(mk_signal("a")).await;
        bus.publish(mk_signal("b")).await;
        bus.publish(mk_signal("c")).await;
        let drained = bus.drain().await;
        assert_eq!(drained.len(), 3);
        assert_eq!(drained[0].market_key, "a");
        assert_eq!(drained[2].market_key, "c");
    }

    #[tokio::test]
    async fn drain_empties_the_bus() {
        let bus = AlertBus::with_capacity(8);
        bus.publish(mk_signal("a")).await;
        let _ = bus.drain().await;
        assert_eq!(bus.len().await, 0);
    }

    #[tokio::test]
    async fn capacity_bounded_drops_oldest() {
        let bus = AlertBus::with_capacity(2);
        bus.publish(mk_signal("a")).await;
        bus.publish(mk_signal("b")).await;
        bus.publish(mk_signal("c")).await;
        let drained = bus.drain().await;
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].market_key, "b");
        assert_eq!(drained[1].market_key, "c");
    }

    #[test]
    fn dedup_key_combines_venue_and_market() {
        let s = mk_signal("tok1");
        assert_eq!(s.dedup_key(), "polymarket:tok1");
    }
}
