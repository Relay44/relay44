use super::Outcome;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub market_id: String,
    pub market_question: String,
    pub owner: String,
    pub yes_balance: u64,
    pub no_balance: u64,
    pub avg_yes_cost: f64,
    pub avg_no_cost: f64,
    pub current_yes_price: f64,
    pub current_no_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub total_deposited: u64,
    pub total_withdrawn: u64,
    pub open_order_count: u32,
    pub total_trades: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PositionListResponse {
    pub positions: Vec<Position>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimWinningsResponse {
    pub market_id: String,
    pub claimed_amount: u64,
    pub winning_outcome: Outcome,
    pub winning_tokens_burned: u64,
    pub tx_signature: String,
}
