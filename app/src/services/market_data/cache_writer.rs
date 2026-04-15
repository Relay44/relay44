//! Single subscriber task that (1) folds L2 events into a top-of-book
//! snapshot in Redis and (2) re-publishes events on Redis pub/sub for
//! cross-process consumers (Next.js frontend, future workers).
//!
//! Producers stay Redis-free; this task owns the Redis side of the bus.

use std::sync::Arc;

use log::{debug, warn};
use tokio::sync::broadcast::error::RecvError;

use super::{cache, L2Event, L2Payload, TopOfBook};
use crate::AppState;

pub fn spawn(state: Arc<AppState>) -> tokio::task::JoinHandle<()> {
    let mut rx = state.market_data.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ev) => handle_event(&state, ev).await,
                Err(RecvError::Lagged(n)) => {
                    warn!("market_data cache_writer lagged by {} events", n);
                }
                Err(RecvError::Closed) => {
                    debug!("market_data bus closed; cache_writer exiting");
                    return;
                }
            }
        }
    })
}

async fn handle_event(state: &AppState, ev: Arc<L2Event>) {
    if let Some(top) = fold_top(&ev) {
        if let Err(e) =
            cache::write_top(&state.redis, ev.venue, &ev.market_key, &top).await
        {
            warn!("l2 cache write failed: {}", e);
        }
    }

    match serde_json::to_string(&*ev) {
        Ok(payload) => {
            let channel = format!("r44:l2:{}:{}", ev.venue.as_str(), ev.market_key);
            if let Err(e) = state.redis.publish(&channel, &payload).await {
                warn!("l2 pubsub publish failed ({}): {}", channel, e);
            }
            if let Err(e) = state.redis.publish("r44:l2:firehose", &payload).await {
                warn!("l2 pubsub firehose publish failed: {}", e);
            }
        }
        Err(e) => warn!("l2 event serialize failed: {}", e),
    }
}

fn fold_top(ev: &L2Event) -> Option<TopOfBook> {
    match &ev.payload {
        L2Payload::Snapshot {
            bids,
            asks,
            last_trade,
        } => Some(TopOfBook {
            best_bid: bids.first().cloned(),
            best_ask: asks.first().cloned(),
            last_trade: *last_trade,
            seq: ev.seq,
            observed_at: ev.observed_at,
        }),
        L2Payload::Delta { .. } | L2Payload::Trade { .. } => None,
    }
}
