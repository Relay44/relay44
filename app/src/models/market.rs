use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub id: String,
    pub address: String,
    pub question: String,
    pub description: String,
    pub category: String,
    pub status: MarketStatus,
    pub yes_price: f64,
    pub no_price: f64,
    pub yes_supply: u64,
    pub no_supply: u64,
    pub volume_24h: f64,
    pub total_volume: f64,
    pub total_collateral: u64,
    pub fee_bps: u16,
    pub oracle: String,
    pub collateral_mint: String,
    pub yes_mint: String,
    pub no_mint: String,
    pub resolution_deadline: DateTime<Utc>,
    pub trading_end: DateTime<Utc>,
    pub resolved_outcome: Option<Outcome>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MarketStatus {
    Active,
    Paused,
    Closed,
    Resolved,
    Cancelled,
}

impl From<u8> for MarketStatus {
    fn from(value: u8) -> Self {
        match value {
            0 => MarketStatus::Active,
            1 => MarketStatus::Paused,
            2 => MarketStatus::Closed,
            3 => MarketStatus::Resolved,
            4 => MarketStatus::Cancelled,
            _ => MarketStatus::Active,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Yes,
    No,
}

impl From<u8> for Outcome {
    fn from(value: u8) -> Self {
        match value {
            1 => Outcome::Yes,
            2 => Outcome::No,
            _ => Outcome::Yes,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateMarketRequest {
    pub market_id: String,
    pub question: String,
    pub description: String,
    pub category: String,
    pub oracle: String,
    pub resolution_deadline: DateTime<Utc>,
    pub trading_end: DateTime<Utc>,
    pub fee_bps: u16,
    /// Collateral token mint address (e.g., USDC)
    /// If not provided, defaults to USDC on mainnet
    #[serde(default = "default_collateral_mint")]
    pub collateral_mint: String,
}

/// Default collateral mint is USDC on Solana mainnet
fn default_collateral_mint() -> String {
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ListMarketsQuery {
    pub status: Option<String>,
    pub category: Option<String>,
    #[allow(dead_code)]
    pub sort: Option<String>,
    #[allow(dead_code)]
    pub order: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct MarketListResponse {
    pub markets: Vec<Market>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct OrderBookResponse {
    pub market_id: String,
    pub outcome: String,
    pub timestamp: DateTime<Utc>,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub spread: f64,
    pub mid_price: f64,
}

#[derive(Debug, Serialize)]
pub struct OrderBookLevel {
    pub price: f64,
    pub quantity: u64,
    pub orders: u32,
}
