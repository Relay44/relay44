use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub wallet: String,
    pub username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub stats: UserStats,
    pub settings: UserSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    pub total_trades: u64,
    pub total_volume: f64,
    pub win_rate: f64,
    pub pnl_30d: f64,
    pub pnl_all_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    pub default_privacy_mode: String,
    pub notifications_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub owner: String,
    pub tx_type: TransactionType,
    pub market_id: Option<String>,
    pub amount: u64,
    pub fee: u64,
    pub tx_signature: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Deposit,
    Withdraw,
    Buy,
    Sell,
    Claim,
    Mint,
    Redeem,
}

impl From<u8> for TransactionType {
    fn from(val: u8) -> Self {
        match val {
            0 => TransactionType::Deposit,
            1 => TransactionType::Withdraw,
            2 => TransactionType::Buy,
            3 => TransactionType::Sell,
            4 => TransactionType::Claim,
            5 => TransactionType::Mint,
            6 => TransactionType::Redeem,
            _ => TransactionType::Deposit,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
}

#[derive(Debug, Deserialize)]
pub struct ListTransactionsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub tx_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TransactionListResponse {
    pub transactions: Vec<Transaction>,
    pub total: i64,
}
