//! WebSocket service for real-time updates
//!
//! Order book updates, trade notifications, position changes.

use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};

/// Message types sent to WebSocket clients
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    #[serde(rename = "orderbook")]
    OrderBook(OrderBookUpdate),
    #[serde(rename = "trade")]
    Trade(TradeUpdate),
    #[serde(rename = "position")]
    Position(PositionUpdate),
    #[serde(rename = "market")]
    Market(MarketUpdate),
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderBookUpdate {
    pub market_id: String,
    pub outcome: String,
    pub side: String,
    pub price: f64,
    pub quantity: u64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeUpdate {
    pub market_id: String,
    pub outcome: String,
    pub price: f64,
    pub quantity: u64,
    pub buyer: String,
    pub seller: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PositionUpdate {
    pub market_id: String,
    pub owner: String,
    pub yes_balance: u64,
    pub no_balance: u64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarketUpdate {
    pub market_id: String,
    pub yes_price: f64,
    pub no_price: f64,
    pub status: String,
    pub timestamp: i64,
}

/// Subscription request from client
#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub channel: String,
    pub market_id: Option<String>,
}

/// WebSocket hub managing all connections and subscriptions
pub struct WebSocketHub {
    /// Broadcast channels per market for order book updates
    market_channels: RwLock<HashMap<String, broadcast::Sender<WsMessage>>>,
    /// Global channel for all-market updates
    global_tx: broadcast::Sender<WsMessage>,
}

impl WebSocketHub {
    pub fn new() -> Self {
        let (global_tx, _) = broadcast::channel(1024);
        Self {
            market_channels: RwLock::new(HashMap::new()),
            global_tx,
        }
    }

    /// Get or create a channel for a specific market
    pub async fn get_market_channel(&self, market_id: &str) -> broadcast::Sender<WsMessage> {
        let mut channels = self.market_channels.write().await;

        if let Some(tx) = channels.get(market_id) {
            tx.clone()
        } else {
            let (tx, _) = broadcast::channel(256);
            channels.insert(market_id.to_string(), tx.clone());
            tx
        }
    }

    /// Subscribe to a market's updates
    pub async fn subscribe_market(&self, market_id: &str) -> broadcast::Receiver<WsMessage> {
        let tx = self.get_market_channel(market_id).await;
        tx.subscribe()
    }

    /// Subscribe to global updates
    pub fn subscribe_global(&self) -> broadcast::Receiver<WsMessage> {
        self.global_tx.subscribe()
    }

    /// Broadcast order book update
    pub async fn broadcast_orderbook(&self, update: OrderBookUpdate) {
        let market_id = update.market_id.clone();
        let msg = WsMessage::OrderBook(update);

        // Send to market-specific channel
        if let Some(tx) = self.market_channels.read().await.get(&market_id) {
            if tx.send(msg.clone()).is_err() {
                // No receivers - this is fine
            }
        }

        // Also send to global channel
        let _ = self.global_tx.send(msg);
    }

    /// Broadcast trade update
    pub async fn broadcast_trade(&self, update: TradeUpdate) {
        let market_id = update.market_id.clone();
        let msg = WsMessage::Trade(update);

        if let Some(tx) = self.market_channels.read().await.get(&market_id) {
            let _ = tx.send(msg.clone());
        }

        let _ = self.global_tx.send(msg);
    }

    /// Broadcast position update (targeted to specific user)
    pub async fn broadcast_position(&self, update: PositionUpdate) {
        let market_id = update.market_id.clone();
        let msg = WsMessage::Position(update);

        if let Some(tx) = self.market_channels.read().await.get(&market_id) {
            let _ = tx.send(msg.clone());
        }
    }

    /// Broadcast market status update
    pub async fn broadcast_market(&self, update: MarketUpdate) {
        let market_id = update.market_id.clone();
        let msg = WsMessage::Market(update);

        if let Some(tx) = self.market_channels.read().await.get(&market_id) {
            let _ = tx.send(msg.clone());
        }

        let _ = self.global_tx.send(msg);
    }

    /// Clean up channels with no subscribers
    pub async fn cleanup_empty_channels(&self) {
        let mut channels = self.market_channels.write().await;
        channels.retain(|market_id, tx| {
            let has_receivers = tx.receiver_count() > 0;
            if !has_receivers {
                info!("Cleaning up empty channel for market: {}", market_id);
            }
            has_receivers
        });
    }
}

impl Default for WebSocketHub {
    fn default() -> Self {
        Self::new()
    }
}
