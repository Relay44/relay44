//! Platform event bus for real-time agent lifecycle events.
//!
//! Broadcasts events via tokio channels. Consumers (WebSocket hub,
//! decision-cell automation, future webhooks) subscribe independently.

use chrono::{DateTime, Utc};
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;

/// Capacity of the event bus channel. Late receivers lose oldest events.
const EVENT_BUS_CAPACITY: usize = 2048;

/// Platform-wide events emitted by the agent execution engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum PlatformEvent {
    /// Agent completed an execution cycle (paper or live).
    #[serde(rename = "agent_executed")]
    AgentExecuted(AgentExecutedEvent),

    /// A paper position was opened.
    #[serde(rename = "position_opened")]
    PositionOpened(PositionEvent),

    /// A paper position was closed.
    #[serde(rename = "position_closed")]
    PositionClosed(PositionEvent),

    /// An agent was automatically deactivated (failures, market closed).
    #[serde(rename = "agent_deactivated")]
    AgentDeactivated(AgentLifecycleEvent),

    /// An agent execution failed.
    #[serde(rename = "agent_failed")]
    AgentFailed(AgentFailedEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutedEvent {
    pub agent_id: String,
    pub owner: String,
    pub provider: String,
    pub market_id: String,
    pub strategy: String,
    pub execution_mode: String,
    pub run_id: String,
    pub run_status: String,
    pub side: String,
    pub outcome: String,
    pub price: f64,
    pub metadata: Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionEvent {
    pub agent_id: String,
    pub owner: String,
    pub position_id: String,
    pub market_id: String,
    pub side: String,
    pub outcome: String,
    pub entry_price: f64,
    pub quantity: f64,
    pub realized_pnl: Option<f64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLifecycleEvent {
    pub agent_id: String,
    pub owner: String,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFailedEvent {
    pub agent_id: String,
    pub owner: String,
    pub provider: String,
    pub market_id: String,
    pub error_code: String,
    pub error_message: String,
    pub consecutive_failures: i32,
    pub timestamp: DateTime<Utc>,
}

/// Central event bus backed by a tokio broadcast channel.
pub struct EventBus {
    tx: broadcast::Sender<PlatformEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(EVENT_BUS_CAPACITY);
        Self { tx }
    }

    /// Publish an event. Silently drops if no subscribers.
    pub fn emit(&self, event: PlatformEvent) {
        if self.tx.receiver_count() > 0 {
            if let Err(e) = self.tx.send(event) {
                warn!("event bus send failed (lagged receivers): {}", e);
            }
        }
    }

    /// Subscribe to all platform events.
    pub fn subscribe(&self) -> broadcast::Receiver<PlatformEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
