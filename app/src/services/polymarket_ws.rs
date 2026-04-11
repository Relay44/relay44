//! Polymarket WebSocket Orderbook Feed.
//!
//! Maintains real-time orderbook state for subscribed markets via the
//! Polymarket CLOB WebSocket. Used by the spread-capture strategy for
//! sub-second price updates on short-duration markets.

use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::AppState;

const WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";
const PING_INTERVAL_SECS: u64 = 10;
const RECONNECT_DELAY_SECS: u64 = 5;

// ── Types ──

#[derive(Debug, Clone)]
pub struct OrderbookState {
    pub token_id: String,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub last_trade_price: Option<f64>,
    pub last_trade_side: Option<String>,
    pub updated_at: std::time::Instant,
}

impl Default for OrderbookState {
    fn default() -> Self {
        Self {
            token_id: String::new(),
            bids: Vec::new(),
            asks: Vec::new(),
            last_trade_price: None,
            last_trade_side: None,
            updated_at: std::time::Instant::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: f64,
    pub size: f64,
}

impl OrderbookState {
    pub fn best_bid(&self) -> Option<f64> {
        self.bids.first().map(|l| l.price)
    }

    pub fn best_ask(&self) -> Option<f64> {
        self.asks.first().map(|l| l.price)
    }

    pub fn spread_bps(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) if bid > 0.0 => Some(((ask - bid) / bid) * 10_000.0),
            _ => None,
        }
    }

    pub fn mid_price(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid + ask) / 2.0),
            (Some(bid), None) => Some(bid),
            (None, Some(ask)) => Some(ask),
            _ => None,
        }
    }

    pub fn bid_depth_usdc(&self, levels: usize) -> f64 {
        self.bids
            .iter()
            .take(levels)
            .map(|l| l.price * l.size)
            .sum()
    }

    pub fn ask_depth_usdc(&self, levels: usize) -> f64 {
        self.asks
            .iter()
            .take(levels)
            .map(|l| l.price * l.size)
            .sum()
    }
}

/// Shared orderbook state, keyed by token_id.
pub type SharedOrderbooks = Arc<RwLock<HashMap<String, OrderbookState>>>;

pub fn new_shared_orderbooks() -> SharedOrderbooks {
    Arc::new(RwLock::new(HashMap::new()))
}

// ── WebSocket message types ──

#[derive(Debug, Deserialize)]
#[serde(tag = "event_type")]
enum WsMessage {
    #[serde(rename = "book")]
    Book {
        asset_id: String,
        market: Option<String>,
        #[serde(default)]
        bids: Vec<WsLevel>,
        #[serde(default)]
        asks: Vec<WsLevel>,
    },
    #[serde(rename = "price_change")]
    PriceChange {
        asset_id: String,
        #[serde(default)]
        changes: Vec<WsPriceChange>,
    },
    #[serde(rename = "last_trade_price")]
    LastTrade {
        asset_id: String,
        price: Option<String>,
        side: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct WsLevel {
    #[serde(default)]
    price: String,
    #[serde(default)]
    size: String,
}

#[derive(Debug, Deserialize)]
struct WsPriceChange {
    #[serde(default)]
    side: String,
    #[serde(default)]
    price: String,
    #[serde(default)]
    size: String,
}

fn parse_level(level: &WsLevel) -> Option<PriceLevel> {
    let price = level.price.parse::<f64>().ok()?;
    let size = level.size.parse::<f64>().ok()?;
    if price > 0.0 && size > 0.0 {
        Some(PriceLevel { price, size })
    } else if price > 0.0 {
        Some(PriceLevel { price, size: 0.0 }) // size=0 means level removed
    } else {
        None
    }
}

// ── Connection management ──

/// Subscribe to orderbook updates for a set of token IDs.
pub fn spawn_orderbook_feed(
    state: Arc<AppState>,
    orderbooks: SharedOrderbooks,
    token_ids: Vec<String>,
) {
    if token_ids.is_empty() {
        return;
    }

    let enabled = std::env::var("PM_WS_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    if !enabled {
        info!("Polymarket WS feed disabled (PM_WS_ENABLED=false)");
        return;
    }

    info!("Starting Polymarket WS feed for {} tokens", token_ids.len());

    tokio::spawn(async move {
        loop {
            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                info!("PM WS feed shutting down");
                break;
            }

            match run_ws_connection(&orderbooks, &token_ids).await {
                Ok(()) => {
                    info!("PM WS connection closed cleanly");
                }
                Err(e) => {
                    warn!("PM WS connection error: {}", e);
                }
            }

            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                break;
            }

            warn!("PM WS reconnecting in {}s...", RECONNECT_DELAY_SECS);
            tokio::time::sleep(Duration::from_secs(RECONNECT_DELAY_SECS)).await;
        }
    });
}

async fn run_ws_connection(
    orderbooks: &SharedOrderbooks,
    token_ids: &[String],
) -> Result<(), String> {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::connect_async;

    let (ws_stream, _) = connect_async(WS_URL)
        .await
        .map_err(|e| format!("WS connect failed: {}", e))?;

    let (mut write, mut read) = ws_stream.split();

    // Subscribe to markets
    let subscribe_msg = json!({
        "assets_ids": token_ids,
        "type": "market",
        "custom_feature_enabled": true
    });

    write
        .send(tokio_tungstenite::tungstenite::Message::Text(
            subscribe_msg.to_string(),
        ))
        .await
        .map_err(|e| format!("WS subscribe failed: {}", e))?;

    info!("PM WS connected, subscribed to {} tokens", token_ids.len());

    // Ping loop
    let write = Arc::new(tokio::sync::Mutex::new(write));
    let write_ping = write.clone();
    let ping_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(PING_INTERVAL_SECS));
        loop {
            interval.tick().await;
            let msg = tokio_tungstenite::tungstenite::Message::Text("PING".to_string());
            if write_ping.lock().await.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Read loop
    while let Some(msg) = read.next().await {
        let msg = msg.map_err(|e| format!("WS read error: {}", e))?;

        match msg {
            tokio_tungstenite::tungstenite::Message::Text(text) => {
                if text == "PONG" {
                    continue;
                }

                // Try to parse as array (Polymarket sends arrays of events)
                if let Ok(messages) = serde_json::from_str::<Vec<WsMessage>>(&text) {
                    for ws_msg in messages {
                        process_ws_message(orderbooks, ws_msg).await;
                    }
                } else if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                    process_ws_message(orderbooks, ws_msg).await;
                }
                // Silently ignore unparseable messages
            }
            tokio_tungstenite::tungstenite::Message::Close(_) => {
                info!("PM WS received close frame");
                break;
            }
            _ => {}
        }
    }

    ping_handle.abort();
    Ok(())
}

async fn process_ws_message(orderbooks: &SharedOrderbooks, msg: WsMessage) {
    match msg {
        WsMessage::Book {
            asset_id,
            bids,
            asks,
            ..
        } => {
            let mut parsed_bids: Vec<PriceLevel> = bids.iter().filter_map(parse_level).collect();
            let mut parsed_asks: Vec<PriceLevel> = asks.iter().filter_map(parse_level).collect();

            // Sort: bids descending, asks ascending
            parsed_bids.sort_by(|a, b| {
                b.price
                    .partial_cmp(&a.price)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            parsed_asks.sort_by(|a, b| {
                a.price
                    .partial_cmp(&b.price)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut books = orderbooks.write().await;
            let entry = books
                .entry(asset_id.clone())
                .or_insert_with(|| OrderbookState {
                    token_id: asset_id,
                    ..Default::default()
                });
            entry.bids = parsed_bids;
            entry.asks = parsed_asks;
            entry.updated_at = std::time::Instant::now();
        }

        WsMessage::PriceChange {
            asset_id, changes, ..
        } => {
            let mut books = orderbooks.write().await;
            if let Some(entry) = books.get_mut(&asset_id) {
                for change in changes {
                    if let (Ok(price), Ok(size)) =
                        (change.price.parse::<f64>(), change.size.parse::<f64>())
                    {
                        let levels = if change.side == "BUY" || change.side == "buy" {
                            &mut entry.bids
                        } else {
                            &mut entry.asks
                        };

                        // Remove existing level at this price
                        levels.retain(|l| (l.price - price).abs() > f64::EPSILON);

                        // Add back if size > 0
                        if size > 0.0 {
                            levels.push(PriceLevel { price, size });
                        }

                        // Re-sort
                        if change.side == "BUY" || change.side == "buy" {
                            levels.sort_by(|a, b| {
                                b.price
                                    .partial_cmp(&a.price)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            });
                        } else {
                            levels.sort_by(|a, b| {
                                a.price
                                    .partial_cmp(&b.price)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            });
                        }
                    }
                }
                entry.updated_at = std::time::Instant::now();
            }
        }

        WsMessage::LastTrade {
            asset_id,
            price,
            side,
            ..
        } => {
            let mut books = orderbooks.write().await;
            if let Some(entry) = books.get_mut(&asset_id) {
                entry.last_trade_price = price.and_then(|p| p.parse().ok());
                entry.last_trade_side = side;
                entry.updated_at = std::time::Instant::now();
            }
        }

        WsMessage::Unknown => {}
    }
}

/// Dynamically subscribe to additional token IDs on an active connection.
/// For now, this starts a new connection. Future: send subscribe message on existing WS.
pub async fn add_subscription(
    state: Arc<AppState>,
    orderbooks: SharedOrderbooks,
    new_token_ids: Vec<String>,
) {
    spawn_orderbook_feed(state, orderbooks, new_token_ids);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orderbook_state_best_bid_ask() {
        let state = OrderbookState {
            token_id: "tok-1".to_string(),
            bids: vec![
                PriceLevel {
                    price: 0.45,
                    size: 100.0,
                },
                PriceLevel {
                    price: 0.44,
                    size: 200.0,
                },
            ],
            asks: vec![
                PriceLevel {
                    price: 0.48,
                    size: 150.0,
                },
                PriceLevel {
                    price: 0.49,
                    size: 50.0,
                },
            ],
            last_trade_price: None,
            last_trade_side: None,
            updated_at: std::time::Instant::now(),
        };

        assert_eq!(state.best_bid(), Some(0.45));
        assert_eq!(state.best_ask(), Some(0.48));
        assert!((state.mid_price().unwrap() - 0.465).abs() < 0.001);
    }

    #[test]
    fn orderbook_spread_bps() {
        let state = OrderbookState {
            token_id: "tok-1".to_string(),
            bids: vec![PriceLevel {
                price: 0.45,
                size: 100.0,
            }],
            asks: vec![PriceLevel {
                price: 0.48,
                size: 100.0,
            }],
            last_trade_price: None,
            last_trade_side: None,
            updated_at: std::time::Instant::now(),
        };

        let spread = state.spread_bps().unwrap();
        assert!((spread - 666.67).abs() < 1.0); // (0.48-0.45)/0.45 * 10000
    }

    #[test]
    fn orderbook_depth() {
        let state = OrderbookState {
            token_id: "tok-1".to_string(),
            bids: vec![
                PriceLevel {
                    price: 0.45,
                    size: 100.0,
                },
                PriceLevel {
                    price: 0.44,
                    size: 200.0,
                },
            ],
            asks: vec![PriceLevel {
                price: 0.48,
                size: 150.0,
            }],
            last_trade_price: None,
            last_trade_side: None,
            updated_at: std::time::Instant::now(),
        };

        let bid_depth = state.bid_depth_usdc(5);
        assert!((bid_depth - (0.45 * 100.0 + 0.44 * 200.0)).abs() < 0.01);

        let ask_depth = state.ask_depth_usdc(5);
        assert!((ask_depth - (0.48 * 150.0)).abs() < 0.01);
    }

    #[test]
    fn parse_level_valid() {
        let level = WsLevel {
            price: "0.45".to_string(),
            size: "100".to_string(),
        };
        let parsed = parse_level(&level).unwrap();
        assert!((parsed.price - 0.45).abs() < f64::EPSILON);
        assert!((parsed.size - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_level_zero_size_removal() {
        let level = WsLevel {
            price: "0.45".to_string(),
            size: "0".to_string(),
        };
        let parsed = parse_level(&level).unwrap();
        assert!((parsed.size - 0.0).abs() < f64::EPSILON);
    }
}
