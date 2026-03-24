use super::Outcome;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub market_id: String,
    pub buy_order_id: String,
    pub sell_order_id: String,
    pub outcome: Outcome,
    pub price: f64,
    pub price_bps: u16,
    pub quantity: u64,
    pub collateral_amount: u64,
    pub buyer: String,
    pub seller: String,
    pub tx_signature: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListTradesQuery {
    pub outcome: Option<String>,
    pub limit: Option<i64>,
    pub before: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TradeListResponse {
    pub trades: Vec<Trade>,
    pub cursor: Option<String>,
}

/// Internal representation of a matched trade before settlement
#[derive(Debug, Clone)]
pub struct MatchedTrade {
    pub buy_order_id: u64,
    pub sell_order_id: u64,
    pub market_id: String,
    pub outcome: Outcome,
    pub fill_price_bps: u16,
    pub fill_quantity: u64,
    pub buyer: String,
    pub seller: String,
}
