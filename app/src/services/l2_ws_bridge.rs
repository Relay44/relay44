//! Bridges L2 market data bus events to the WebSocket hub for frontend clients.

use std::sync::Arc;

use log::{info, warn};
use tokio::sync::broadcast::error::RecvError;

use super::market_data::{L2Event, L2Payload, Venue};
use super::websocket::{MarketUpdate, OrderBookUpdate, TradeUpdate};
use crate::AppState;

pub fn spawn(state: Arc<AppState>) {
    let enabled = std::env::var("L2_WS_BRIDGE_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);

    if !enabled {
        info!("L2 WebSocket bridge disabled");
        return;
    }

    let mut rx = state.market_data.subscribe();

    info!("Starting L2 → WebSocket bridge");

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ev) => forward_event(&state, &ev).await,
                Err(RecvError::Lagged(n)) => {
                    warn!("l2_ws_bridge lagged by {} events", n);
                }
                Err(RecvError::Closed) => {
                    info!("market_data bus closed; l2_ws_bridge exiting");
                    return;
                }
            }
        }
    });
}

async fn forward_event(state: &AppState, ev: &L2Event) {
    let market_id = format_market_id(ev.venue, &ev.market_key);
    let ts = ev.observed_at.timestamp();

    match &ev.payload {
        L2Payload::Snapshot { bids, asks, .. } => {
            let (best_bid, best_ask) = (
                bids.first().map(|l| l.price).unwrap_or(0.0),
                asks.first().map(|l| l.price).unwrap_or(0.0),
            );

            state
                .ws_hub
                .broadcast_market(MarketUpdate {
                    market_id: market_id.clone(),
                    yes_price: best_bid,
                    no_price: 1.0 - best_ask,
                    status: "active".to_string(),
                    timestamp: ts,
                })
                .await;

            for bid in bids {
                state
                    .ws_hub
                    .broadcast_orderbook(OrderBookUpdate {
                        market_id: market_id.clone(),
                        outcome: "yes".to_string(),
                        side: "bid".to_string(),
                        price: bid.price,
                        quantity: bid.size as u64,
                        timestamp: ts,
                    })
                    .await;
            }
            for ask in asks {
                state
                    .ws_hub
                    .broadcast_orderbook(OrderBookUpdate {
                        market_id: market_id.clone(),
                        outcome: "yes".to_string(),
                        side: "ask".to_string(),
                        price: ask.price,
                        quantity: ask.size as u64,
                        timestamp: ts,
                    })
                    .await;
            }
        }
        L2Payload::Trade { price, size, side } => {
            let side_str = match side {
                super::market_data::Side::Buy => "buy",
                super::market_data::Side::Sell => "sell",
            };
            state
                .ws_hub
                .broadcast_trade(TradeUpdate {
                    market_id,
                    outcome: "yes".to_string(),
                    price: *price,
                    quantity: *size as u64,
                    buyer: side_str.to_string(),
                    seller: String::new(),
                    timestamp: ts,
                })
                .await;
        }
        L2Payload::Delta {
            bid_updates,
            ask_updates,
            ..
        } => {
            for bid in bid_updates {
                state
                    .ws_hub
                    .broadcast_orderbook(OrderBookUpdate {
                        market_id: market_id.clone(),
                        outcome: "yes".to_string(),
                        side: "bid".to_string(),
                        price: bid.price,
                        quantity: bid.size as u64,
                        timestamp: ts,
                    })
                    .await;
            }
            for ask in ask_updates {
                state
                    .ws_hub
                    .broadcast_orderbook(OrderBookUpdate {
                        market_id: market_id.clone(),
                        outcome: "yes".to_string(),
                        side: "ask".to_string(),
                        price: ask.price,
                        quantity: ask.size as u64,
                        timestamp: ts,
                    })
                    .await;
            }
        }
    }
}

fn format_market_id(venue: Venue, market_key: &str) -> String {
    format!("{}:{}", venue.as_str(), market_key)
}
