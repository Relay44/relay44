//! Smoke tests for the market-data bus envelope + seq counters.
//!
//! No Redis required — the bus itself is pure in-process broadcast. Cache
//! and pub/sub fanout are exercised via the `cache_writer` task, which is
//! tested separately when `TEST_REDIS_URL` is set.

use std::sync::Arc;

use relay44_backend::services::market_data::{
    L2Event, L2Level, L2Payload, MarketDataBus, Venue,
};

fn snapshot(market: &str, seq: u64, price: f64) -> L2Event {
    L2Event {
        venue: Venue::Polymarket,
        market_key: market.to_string(),
        seq,
        observed_at: chrono::Utc::now(),
        payload: L2Payload::Snapshot {
            bids: vec![L2Level { price, size: 1.0 }],
            asks: vec![],
            last_trade: None,
        },
    }
}

#[tokio::test]
async fn bus_delivers_to_subscriber() {
    let bus = Arc::new(MarketDataBus::new());
    let mut rx = bus.subscribe();

    bus.emit(snapshot("token-1", 1, 0.42));

    let ev = rx.recv().await.expect("receive");
    assert_eq!(ev.market_key, "token-1");
    assert_eq!(ev.seq, 1);
}

#[tokio::test]
async fn emit_without_subscribers_is_noop() {
    let bus = MarketDataBus::new();
    bus.emit(snapshot("token-x", 1, 0.5));
    assert_eq!(bus.receiver_count(), 0);
}

#[tokio::test]
async fn seq_is_monotonic_per_venue() {
    let bus = MarketDataBus::new();
    let a = bus.next_seq(Venue::Polymarket);
    let b = bus.next_seq(Venue::Polymarket);
    let c = bus.next_seq(Venue::Polymarket);
    assert_eq!((a, b, c), (1, 2, 3));

    let lim_a = bus.next_seq(Venue::Limitless);
    let lim_b = bus.next_seq(Venue::Limitless);
    assert_eq!((lim_a, lim_b), (1, 2));
    assert_eq!(bus.next_seq(Venue::Polymarket), 4);
}

#[tokio::test]
async fn multiple_subscribers_each_receive_events() {
    let bus = Arc::new(MarketDataBus::new());
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();

    bus.emit(snapshot("m", 1, 0.1));
    bus.emit(snapshot("m", 2, 0.2));

    for rx in [&mut rx1, &mut rx2] {
        let e1 = rx.recv().await.unwrap();
        let e2 = rx.recv().await.unwrap();
        assert_eq!(e1.seq, 1);
        assert_eq!(e2.seq, 2);
    }
}
