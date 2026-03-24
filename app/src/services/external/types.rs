use serde::{Deserialize, Serialize};

use crate::api::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalProvider {
    Limitless,
    Polymarket,
}

impl ExternalProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Limitless => "limitless",
            Self::Polymarket => "polymarket",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "limitless" => Some(Self::Limitless),
            "polymarket" => Some(Self::Polymarket),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMarketId {
    pub provider: ExternalProvider,
    pub value: String,
}

impl ExternalMarketId {
    pub fn parse(raw: &str) -> Result<Self, ApiError> {
        let trimmed = raw.trim();
        let (provider_raw, value) = trimmed.split_once(':').ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_MARKET_ID",
                "external market id must be namespaced like limitless:<slug> or polymarket:<id>",
            )
        })?;

        let provider = ExternalProvider::from_str(provider_raw).ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_MARKET_SOURCE",
                "market source must be one of: limitless, polymarket",
            )
        })?;

        let value = value.trim();
        if value.is_empty() {
            return Err(ApiError::bad_request(
                "INVALID_MARKET_ID",
                "market id value cannot be empty",
            ));
        }

        Ok(Self {
            provider,
            value: value.to_string(),
        })
    }

    pub fn full_id(&self) -> String {
        format!("{}:{}", self.provider.as_str(), self.value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalOutcome {
    pub label: String,
    pub probability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalMarketSnapshot {
    pub id: String,
    pub question: String,
    pub description: String,
    pub category: String,
    pub status: String,
    pub close_time: u64,
    pub resolved: bool,
    pub outcome: Option<String>,
    pub yes_price: f64,
    pub no_price: f64,
    pub volume: f64,
    pub source: String,
    pub provider: String,
    pub is_external: bool,
    pub external_url: String,
    pub chain_id: u64,
    pub requires_credentials: bool,
    pub execution_users: bool,
    pub execution_agents: bool,
    pub outcomes: Vec<ExternalOutcome>,
    pub provider_market_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalOrderBookLevel {
    pub price: f64,
    pub quantity: f64,
    pub orders: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalOrderBookSnapshot {
    pub market_id: String,
    pub outcome: String,
    pub bids: Vec<ExternalOrderBookLevel>,
    pub asks: Vec<ExternalOrderBookLevel>,
    pub last_updated: String,
    pub source: String,
    pub provider: String,
    pub chain_id: u64,
    pub provider_market_ref: String,
    pub is_synthetic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTradeSnapshot {
    pub id: String,
    pub market_id: String,
    pub outcome: String,
    pub price: f64,
    pub price_bps: u64,
    pub quantity: u64,
    pub tx_hash: String,
    pub block_number: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTradesSnapshot {
    pub trades: Vec<ExternalTradeSnapshot>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
    pub has_more: bool,
    pub source: String,
    pub provider: String,
    pub chain_id: u64,
    pub provider_market_ref: String,
    pub is_synthetic: bool,
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn clamp_probability(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.5;
    }
    value.clamp(0.0, 1.0)
}

pub fn price_to_bps(value: f64) -> u64 {
    (clamp_probability(value) * 10_000.0)
        .round()
        .clamp(0.0, 10_000.0) as u64
}

pub fn is_binary_yes_no(outcomes: &[ExternalOutcome]) -> bool {
    if outcomes.len() != 2 {
        return false;
    }

    let mut labels = outcomes
        .iter()
        .map(|entry| entry.label.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    labels.sort();

    labels == ["no".to_string(), "yes".to_string()]
}
