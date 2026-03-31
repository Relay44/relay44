//! Provider adapter trait for pluggable DEX/venue integrations.
//!
//! Each provider implements this trait to standardize market data fetching,
//! order building, and order submission. The execution engine dispatches
//! through the trait rather than hand-written match arms.

use async_trait::async_trait;
use serde_json::Value;

use crate::api::ApiError;
use crate::services::external::types::{
    ExternalMarketSnapshot, ExternalOrderBookSnapshot, ExternalTradesSnapshot,
};

/// Credentials passed to the adapter for authenticated operations.
#[derive(Debug, Clone)]
pub struct ProviderCredentials {
    pub payload: Value,
}

/// Parameters for building a submit payload.
#[derive(Debug, Clone)]
pub struct BuildOrderParams {
    pub market_id: String,
    pub provider_market_ref: String,
    pub outcome: String,
    pub side: String,
    pub price: f64,
    pub quantity: Option<f64>,
    pub signed_order: Value,
}

/// Result of submitting an order to the provider.
#[derive(Debug, Clone)]
pub struct SubmitResult {
    pub provider_order_id: String,
    pub response: Value,
}

/// Trait that every external provider must implement.
///
/// This enables adding new DEXes/venues without modifying match arms
/// across the codebase.
#[async_trait]
pub trait ExternalProviderAdapter: Send + Sync {
    /// Human-readable provider name (e.g. "limitless", "polymarket", "aerodrome").
    fn name(&self) -> &'static str;

    /// Fetch active markets from this provider.
    async fn fetch_markets(
        &self,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<ExternalMarketSnapshot>, ApiError>;

    /// Fetch a single market by its provider-specific ID.
    async fn fetch_market(
        &self,
        market_ref: &str,
    ) -> Result<ExternalMarketSnapshot, ApiError>;

    /// Fetch the order book for a market+outcome.
    async fn fetch_orderbook(
        &self,
        market_ref: &str,
        outcome: &str,
        depth: u64,
    ) -> Result<ExternalOrderBookSnapshot, ApiError>;

    /// Fetch recent trades for a market.
    async fn fetch_trades(
        &self,
        market_ref: &str,
        outcome: Option<&str>,
        limit: u64,
        offset: u64,
    ) -> Result<ExternalTradesSnapshot, ApiError>;

    /// Build the provider-specific submit payload for an order.
    async fn build_order(
        &self,
        credentials: &ProviderCredentials,
        params: &BuildOrderParams,
    ) -> Result<Value, ApiError>;

    /// Submit a built order to the provider.
    async fn submit_order(
        &self,
        credentials: &ProviderCredentials,
        payload: &Value,
    ) -> Result<SubmitResult, ApiError>;

    /// Cancel an existing order (if supported). Returns Ok(()) on success.
    async fn cancel_order(
        &self,
        credentials: &ProviderCredentials,
        provider_order_id: &str,
    ) -> Result<(), ApiError> {
        let _ = (credentials, provider_order_id);
        Err(ApiError::bad_request(
            "CANCEL_NOT_SUPPORTED",
            &format!("{} does not support order cancellation", self.name()),
        ))
    }
}
