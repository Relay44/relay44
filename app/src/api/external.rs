use actix_web::{web, HttpRequest, HttpResponse, Responder};
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine as _;
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use sha3::{Digest, Keccak256};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::auth::{extract_authenticated_user, extract_jwt_user, AuthenticatedUserWithRole};
use crate::api::jwt::{check_role, UserRole};
use crate::api::ApiError;
use crate::config::{AppConfig, ExternalExecutionMode};
use crate::services::external;
use crate::services::external::credentials::{decrypt_json, encrypt_json, mask_secret};
use crate::services::external::ledger::{self, PerformanceLedgerKind};
use crate::services::external::paper::{realized_pnl, simulate_fill, unrealized_pnl};
use crate::services::external::types::{
    ExternalMarketId, ExternalMarketSnapshot, ExternalProvider,
};
use crate::services::provider_rails::{evaluate_provider_access, ProviderRailAction, RailProvider};
use crate::AppState;
use sqlx::{Postgres, QueryBuilder};

const MAX_PAGE_SIZE: i64 = 200;
const MAX_EXTERNAL_STATE_IMPORT_BATCH_SIZE: usize = 2_000;
const LIMITLESS_SIGNING_NAME: &str = "Limitless CTF Exchange";
const LIMITLESS_SIGNING_VERSION: &str = "1";
const LIMITLESS_ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";
const LIMITLESS_SCALE: u128 = 1_000_000;
const LIMITLESS_PRICE_TICK_INT: u128 = 1_000;
const POLYMARKET_SIGNING_NAME: &str = "Polymarket CTF Exchange";
const POLYMARKET_SIGNING_VERSION: &str = "1";
const POLYMARKET_CHAIN_ID: u64 = 137;
const POLYMARKET_PRICE_SCALE: u128 = 1_000_000;
const POLYMARKET_LOT_STEP_INT: u128 = 10_000;
const POLYMARKET_EXCHANGE: &str = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E";
const POLYMARKET_NEG_RISK_EXCHANGE: &str = "0xC5d563A36AE78145C45a50134d48A1215220f80a";
const POLYMARKET_RELAYER_API_BASE: &str = "https://relayer-v2.polymarket.com";

struct PolymarketCredentials {
    api_key: String,
    api_secret: String,
    api_passphrase: String,
    funder: String,
    signature_type: u8,
}

struct PolymarketBuilderCredentials<'a> {
    api_key: &'a str,
    api_secret: &'a str,
    api_passphrase: &'a str,
}

#[derive(Debug)]
struct PolymarketBuilderHeaders {
    api_key: String,
    api_passphrase: String,
    signature: String,
    timestamp: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolymarketForwardRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: String,
}

struct PolymarketForwarder<'a> {
    url: &'a str,
    shared_secret: &'a str,
}

struct PolymarketOrderContext {
    token_id: String,
    fee_rate_bps: u64,
    minimum_tick_size: f64,
    neg_risk: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListExternalCredentialsQuery {
    pub provider: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCredentialStatusQuery {
    pub provider: String,
    pub credential_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertExternalCredentialRequest {
    pub provider: String,
    pub label: Option<String>,
    pub credentials: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindLimitlessWalletRequest {
    pub credential_id: String,
    pub base_wallet: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExternalStateBatchRequest {
    pub rows: Vec<Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCredentialResponse {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub key_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub credentials: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCredentialsListResponse {
    pub credentials: Vec<ExternalCredentialResponse>,
    pub total: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExternalStateResponse {
    pub ok: bool,
    pub table: String,
    pub imported: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCredentialCheck {
    pub code: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCredentialStatusResponse {
    pub provider: String,
    pub credential_id: Option<String>,
    pub ready: bool,
    pub base_wallet: Option<String>,
    pub profile_status: Option<String>,
    pub checks: Vec<ExternalCredentialCheck>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateExternalOrderIntentRequest {
    pub provider: String,
    pub market_id: String,
    pub outcome: String,
    pub side: String,
    pub price: f64,
    pub quantity: f64,
    pub credential_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalOrderIntentResponse {
    pub id: String,
    pub provider: String,
    pub market_id: String,
    pub preflight: Value,
    pub typed_data: Value,
    pub status: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareExternalOrderSubmitRequest {
    pub intent_id: String,
    pub signed_order: Value,
    pub credential_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedExternalProviderRequestResponse {
    pub provider: String,
    pub url: String,
    pub method: String,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitExternalOrderRequest {
    pub intent_id: String,
    pub signed_order: Value,
    pub credential_id: Option<String>,
    pub provider_response: Option<Value>,
    pub provider_status: Option<u16>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelExternalOrderRequest {
    pub provider: String,
    pub provider_order_id: String,
    pub credential_id: Option<String>,
    pub payload: Option<Value>,
    pub provider_response: Option<Value>,
    pub provider_status: Option<u16>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalOrderResponse {
    pub id: String,
    pub provider: String,
    pub market_id: String,
    pub provider_order_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub response_payload: Value,
    pub error_message: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListExternalOrdersQuery {
    pub provider: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalOrdersListResponse {
    pub orders: Vec<ExternalOrderResponse>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateExternalAgentRequest {
    pub name: String,
    pub provider: String,
    pub market_id: String,
    pub outcome: String,
    pub side: String,
    pub price: f64,
    pub quantity: f64,
    pub cadence_seconds: u64,
    pub strategy: String,
    pub strategy_params: Option<Value>,
    pub credential_id: Option<String>,
    pub execution_mode: Option<String>,
    pub cohort: Option<String>,
    pub active: Option<bool>,
    pub max_notional_per_execution: Option<f64>,
    pub max_daily_spend_usdc: Option<f64>,
    pub max_slippage_bps: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExternalAgentRequest {
    pub name: Option<String>,
    pub outcome: Option<String>,
    pub side: Option<String>,
    pub price: Option<f64>,
    pub quantity: Option<f64>,
    pub cadence_seconds: Option<u64>,
    pub strategy: Option<String>,
    pub strategy_params: Option<Value>,
    pub credential_id: Option<String>,
    pub execution_mode: Option<String>,
    pub cohort: Option<String>,
    pub active: Option<bool>,
    pub max_notional_per_execution: Option<f64>,
    pub max_daily_spend_usdc: Option<f64>,
    pub max_slippage_bps: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteExternalAgentRequest {
    pub force: Option<bool>,
    pub signed_order: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListExternalAgentsQuery {
    pub provider: Option<String>,
    pub active: Option<bool>,
    pub scope: Option<String>,
    pub owner: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicExternalAgentsQuery {
    pub provider: Option<String>,
    pub active: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketPublicTradesQuery {
    pub market_id: Option<String>,
    pub wallet: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketOrderbookHistoryQuery {
    pub market_id: Option<String>,
    pub outcome: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchWalletsQuery {
    pub market_category: Option<String>,
    pub window: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReplayRequest {
    pub strategy: String,
    pub baseline: Option<String>,
    pub market_id: Option<String>,
    pub market_category: Option<String>,
    pub target_wallet: Option<String>,
    pub delay_ms: Option<u64>,
    pub window_hours: Option<u64>,
    pub follow_ratio: Option<f64>,
    pub markout_minutes: Option<u64>,
    pub max_trades: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentResponse {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub provider: String,
    pub market_id: String,
    pub outcome: String,
    pub side: String,
    pub price: f64,
    pub quantity: f64,
    pub cadence_seconds: u64,
    pub strategy: String,
    pub strategy_label: String,
    pub strategy_params: Value,
    pub execution_mode: String,
    pub cohort: String,
    pub credential_id: Option<String>,
    pub max_notional_per_execution: Option<f64>,
    pub max_daily_spend_usdc: Option<f64>,
    pub max_slippage_bps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_performance: Option<ExternalAgentPaperPerformance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub active: bool,
    pub last_executed_at: Option<String>,
    pub next_execution_at: String,
    pub consecutive_failures: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LimitlessProfileRank {
    #[serde(default)]
    fee_rate_bps: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LimitlessProfile {
    id: u64,
    #[serde(rename = "account")]
    _account: String,
    #[serde(default)]
    rank: Option<LimitlessProfileRank>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentsListResponse {
    pub agents: Vec<ExternalAgentResponse>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunnerTickRequest {
    pub limit: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunnerTickResponse {
    pub executed: bool,
    pub agents_scanned: u64,
    pub agents_executed: u64,
    pub skips_by_reason: BTreeMap<String, u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentPerformanceQuery {
    pub owner: Option<String>,
    pub scope: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentPerformanceResponse {
    pub scope: String,
    pub owner: Option<String>,
    pub totals: ExternalAgentPerformanceTotals,
    pub strategies: Vec<ExternalAgentStrategyPerformance>,
    pub timeline: Vec<ExternalAgentPerformancePoint>,
    pub updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyReplayResponse {
    pub run: external::polymarket_index::StrategyReplayRunRecord,
    pub fills: Vec<external::polymarket_index::StrategyReplayFillRecord>,
}

const PUBLIC_PAPER_AGENT_SOURCE: &str = "relay44-paper";
const PUBLIC_RESEARCH_COHORT: &str = "public_research";
const PRIVATE_ALPHA_COHORT: &str = "private_alpha";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentPerformanceTotals {
    pub agents: u64,
    pub active_agents: u64,
    pub open_positions: u64,
    pub closed_positions: u64,
    pub fills: u64,
    pub volume_usdc: f64,
    pub fees_usdc: f64,
    pub realized_pnl_usdc: f64,
    pub unrealized_pnl_usdc: f64,
    pub net_pnl_usdc: f64,
    pub max_drawdown_usdc: f64,
    pub runner_reliability: f64,
    pub p50_detection_to_order_ms: Option<f64>,
    pub p50_slippage_ticks: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentStrategyPerformance {
    pub strategy: String,
    pub agents: u64,
    pub active_agents: u64,
    pub open_positions: u64,
    pub closed_positions: u64,
    pub fills: u64,
    pub volume_usdc: f64,
    pub fees_usdc: f64,
    pub realized_pnl_usdc: f64,
    pub unrealized_pnl_usdc: f64,
    pub net_pnl_usdc: f64,
    pub win_rate: f64,
    pub max_drawdown_usdc: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentPerformancePoint {
    pub bucket: String,
    pub volume_usdc: f64,
    pub realized_pnl_usdc: f64,
    pub unrealized_pnl_usdc: f64,
    pub net_pnl_usdc: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAgentPaperPerformance {
    pub open_positions: u64,
    pub closed_positions: u64,
    pub fills: u64,
    pub volume_usdc: f64,
    pub fees_usdc: f64,
    pub realized_pnl_usdc: f64,
    pub unrealized_pnl_usdc: f64,
    pub net_pnl_usdc: f64,
    pub max_drawdown_usdc: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateExternalSignalRequest {
    pub publisher: Option<String>,
    pub provider: String,
    pub market_id: String,
    pub direction: String,
    pub confidence_bps: i32,
    pub fair_value_low: f64,
    pub fair_value_high: f64,
    pub midpoint_delta_bps: i32,
    pub catalyst_summary: String,
    pub invalidators: Vec<String>,
    pub rationale: Option<String>,
    pub expires_at: String,
    pub signal_type: Option<String>,
    pub metadata: Option<Value>,
    pub agent_id: Option<String>,
    pub memo_mode: Option<String>,
    pub sources: Option<Vec<String>>,
    pub resolution_rules_read: Option<bool>,
    pub resolution_criteria: Option<String>,
    pub resolution_hazards: Option<Vec<String>>,
    pub has_live_reference: Option<bool>,
    pub repricing_half_life_minutes: Option<i32>,
    pub confidence_reasoning: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListExternalSignalsQuery {
    pub provider: Option<String>,
    pub market_id: Option<String>,
    pub active_only: Option<bool>,
    pub publisher: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSignalResponse {
    pub id: String,
    pub publisher: String,
    pub provider: String,
    pub market_id: String,
    pub signal_type: String,
    pub direction: String,
    pub confidence_bps: i32,
    pub fair_value_low: f64,
    pub fair_value_high: f64,
    pub midpoint_delta_bps: i32,
    pub catalyst_summary: String,
    pub invalidators: Vec<String>,
    pub rationale: Option<String>,
    pub metadata: Value,
    pub memo_mode: Option<String>,
    pub sources: Vec<String>,
    pub resolution_rules_read: bool,
    pub resolution_criteria: Option<String>,
    pub resolution_hazards: Vec<String>,
    pub has_live_reference: bool,
    pub repricing_half_life_minutes: Option<i32>,
    pub confidence_reasoning: Option<String>,
    pub active: bool,
    pub expires_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub agent_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSignalsListResponse {
    pub signals: Vec<ExternalSignalResponse>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalMarketOrderbookQuery {
    pub outcome: String,
    pub depth: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalMarketTradesQuery {
    pub outcome: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketIndexerBackfillRequest {
    pub market_id: Option<String>,
    pub days: Option<u64>,
    pub public_tape: Option<bool>,
    pub user_fills: Option<bool>,
    pub max_markets: Option<u64>,
    pub max_pages_per_market: Option<u64>,
    #[serde(default)]
    pub user_events: Vec<Value>,
    #[serde(default)]
    pub relayer_transactions: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketIndexerTrackedMarketResponse {
    pub market_id: String,
    pub provider_market_ref: String,
    pub condition_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketIndexerLaneHealth {
    pub lane: String,
    pub status: String,
    pub tracked_markets: u64,
    pub indexed_markets: u64,
    pub indexed_from: Option<String>,
    pub indexed_through: Option<String>,
    pub is_partial_backfill: bool,
    pub last_error: Option<String>,
    pub updated_at: Option<String>,
    pub builder_configured: Option<bool>,
    pub matched_events: Option<u64>,
    pub mined_events: Option<u64>,
    pub confirmed_events: Option<u64>,
    pub retrying_events: Option<u64>,
    pub failed_events: Option<u64>,
    pub last_event_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketIndexerHealthResponse {
    pub ok: bool,
    pub tracked_markets: u64,
    pub tracked_market_details: Vec<PolymarketIndexerTrackedMarketResponse>,
    pub public_tape: PolymarketIndexerLaneHealth,
    pub user_fills: PolymarketIndexerLaneHealth,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketIndexerBackfillResponse {
    pub ok: bool,
    pub tracked_markets: u64,
    pub tracked_market_details: Vec<PolymarketIndexerTrackedMarketResponse>,
    pub public_trades_ingested: u64,
    pub user_fill_events_ingested: u64,
    pub user_lifecycle_events_reconciled: u64,
    pub public_tape: PolymarketIndexerLaneHealth,
    pub user_fills: PolymarketIndexerLaneHealth,
}
#[derive(Debug, Clone)]
struct StoredCredential {
    id: String,
    owner: String,
    payload: Value,
}

#[derive(Debug)]
struct ExternalOrderIntentRecord {
    provider: ExternalProvider,
    market_id: String,
    provider_market_ref: String,
    credential_id: Option<String>,
    price: f64,
    typed_data: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct ExternalAgentRecord {
    pub(crate) id: String,
    pub(crate) owner: String,
    pub(crate) cohort: String,
    pub(crate) name: String,
    pub(crate) provider: ExternalProvider,
    pub(crate) market_id: String,
    pub(crate) outcome: String,
    pub(crate) side: String,
    pub(crate) price: f64,
    pub(crate) quantity: f64,
    pub(crate) cadence_seconds: i64,
    pub(crate) strategy: String,
    pub(crate) strategy_params: Value,
    pub(crate) execution_mode: ExternalExecutionMode,
    pub(crate) credential_id: Option<String>,
    pub(crate) active: bool,
    pub(crate) next_execution_at: chrono::DateTime<Utc>,
    pub(crate) consecutive_failures: i32,
    pub(crate) last_error_code: Option<String>,
    // Execution guardrails
    pub(crate) max_notional_per_execution: Option<f64>,
    pub(crate) max_daily_spend_usdc: Option<f64>,
    pub(crate) max_slippage_bps: Option<i32>,
}

#[derive(Debug, Clone)]
struct PaperPositionRecord {
    id: String,
    entry_price: f64,
    filled_quantity: f64,
    fees_paid_usdc: f64,
    hold_until: chrono::DateTime<Utc>,
    opened_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct LivePositionRecord {
    id: String,
    entry_price: f64,
    filled_quantity: f64,
    fees_paid_usdc: f64,
    hold_until: chrono::DateTime<Utc>,
    opened_at: chrono::DateTime<Utc>,
    status: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentExecutionOutcome {
    pub(crate) executed: bool,
    pub(crate) skip_reason: Option<String>,
    pub(crate) run_status: String,
    pub(crate) run_id: String,
    pub(crate) external_order_id: Option<String>,
    pub(crate) provider_order_id: Option<String>,
    pub(crate) next_execution_at: chrono::DateTime<Utc>,
    pub(crate) response: Value,
}

fn normalize_provider(raw: &str) -> Result<ExternalProvider, ApiError> {
    ExternalProvider::from_str(raw).ok_or_else(|| {
        ApiError::bad_request(
            "INVALID_PROVIDER",
            "provider must be one of: limitless, polymarket",
        )
    })
}

fn parse_external_execution_mode(raw: &str) -> Result<ExternalExecutionMode, ApiError> {
    ExternalExecutionMode::from_str(raw).ok_or_else(|| {
        ApiError::bad_request(
            "INVALID_EXECUTION_MODE",
            "executionMode must be one of: live, paper",
        )
    })
}

fn normalize_agent_cohort(raw: &str) -> Result<String, ApiError> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        PUBLIC_RESEARCH_COHORT => Ok(PUBLIC_RESEARCH_COHORT.to_string()),
        PRIVATE_ALPHA_COHORT => Ok(PRIVATE_ALPHA_COHORT.to_string()),
        _ => Err(ApiError::bad_request(
            "INVALID_AGENT_COHORT",
            "cohort must be one of: public_research, private_alpha",
        )),
    }
}

fn resolve_agent_cohort(
    role: UserRole,
    owner: &str,
    execution_mode: ExternalExecutionMode,
    requested: Option<&str>,
    public_owner: Option<&str>,
) -> Result<String, ApiError> {
    match requested {
        Some(_raw) if !matches!(role, UserRole::Admin) => Err(ApiError::forbidden(
            "Only admins can override external agent cohort",
        )),
        Some(raw) => normalize_agent_cohort(raw),
        None => {
            let owner = owner.trim().to_ascii_lowercase();
            let public_owner = public_owner
                .map(|value| value.trim().to_ascii_lowercase())
                .unwrap_or_default();
            if execution_mode == ExternalExecutionMode::Paper
                && !public_owner.is_empty()
                && owner == public_owner
            {
                Ok(PUBLIC_RESEARCH_COHORT.to_string())
            } else {
                Ok(PRIVATE_ALPHA_COHORT.to_string())
            }
        }
    }
}

fn strategy_label(strategy: &str) -> String {
    match strategy.trim().to_ascii_lowercase().as_str() {
        "momentum" => "proving".to_string(),
        "mean-revert" => "research".to_string(),
        "market-maker" => "optimization".to_string(),
        "maker-reward" | "maker_reward" => "rebates".to_string(),
        "event-repricing" | "event_repricing" | "event-repricing-v2" | "event_repricing_v2" => {
            "scenario".to_string()
        }
        "wallet-follow" | "wallet_follow" | "wallet-follow-v2" | "wallet_follow_v2" => {
            "mirror".to_string()
        }
        _ => strategy.trim().to_string(),
    }
}

fn public_paper_cohort_owner(state: &AppState) -> Option<&str> {
    if !state.config.paper_cohort_public_enabled {
        return None;
    }

    let owner = state.config.paper_cohort_public_owner.trim();
    if owner.is_empty() {
        None
    } else {
        Some(owner)
    }
}

fn requested_execution_mode(
    default_mode: ExternalExecutionMode,
    role: UserRole,
    requested: Option<&str>,
) -> Result<ExternalExecutionMode, ApiError> {
    match requested {
        Some(raw) if matches!(role, UserRole::Admin) => parse_external_execution_mode(raw),
        Some(_) => Err(ApiError::forbidden(
            "Only admins can override external agent execution mode",
        )),
        None => Ok(default_mode),
    }
}

fn run_skip_status_for_mode(mode: ExternalExecutionMode) -> &'static str {
    match mode {
        ExternalExecutionMode::Paper => "paper_skipped",
        ExternalExecutionMode::Live => "skipped",
    }
}

fn to_rail_provider(provider: ExternalProvider) -> RailProvider {
    match provider {
        ExternalProvider::Limitless => RailProvider::Limitless,
        ExternalProvider::Polymarket => RailProvider::Polymarket,
        ExternalProvider::Aerodrome => RailProvider::Limitless, // Aerodrome uses same Base chain rails
    }
}

fn ensure_provider_action_allowed(
    req: &HttpRequest,
    provider: ExternalProvider,
    action: ProviderRailAction,
) -> Result<(), ApiError> {
    let rail_provider = to_rail_provider(provider);
    let decision = evaluate_provider_access(req, rail_provider, action);
    if decision.allowed {
        return Ok(());
    }

    Err(ApiError::legal_restricted(
        "REGION_PROVIDER_RESTRICTED",
        "provider unavailable in your region for this action",
        Some(json!({
            "provider": rail_provider.as_str(),
            "action": action.as_str(),
            "country": decision.country,
            "regionClass": decision.region_class.as_str(),
            "routingMode": decision.mode.as_str(),
            "legacyCloseOnly": decision.legacy_close_only,
            "safeFallbackRestriction": decision.safe_fallback_restriction,
            "detail": decision.reason
        })),
    ))
}

fn normalize_side(raw: &str) -> Result<String, ApiError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "buy" | "sell" => Ok(raw.trim().to_ascii_lowercase()),
        _ => Err(ApiError::bad_request(
            "INVALID_SIDE",
            "side must be one of: buy, sell",
        )),
    }
}

fn normalize_outcome(raw: &str) -> Result<String, ApiError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "yes" | "no" => Ok(raw.trim().to_ascii_lowercase()),
        _ => Err(ApiError::bad_request(
            "INVALID_OUTCOME",
            "outcome must be one of: yes, no",
        )),
    }
}

fn normalize_namespaced_market_id(provider: ExternalProvider, market_id: &str) -> String {
    if market_id.contains(':') {
        return market_id.trim().to_string();
    }
    format!("{}:{}", provider.as_str(), market_id.trim())
}

fn normalize_direction(raw: &str) -> Result<String, ApiError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "yes" | "no" | "neutral" => Ok(raw.trim().to_ascii_lowercase()),
        _ => Err(ApiError::bad_request(
            "INVALID_DIRECTION",
            "direction must be one of: yes, no, neutral",
        )),
    }
}

fn normalize_strategy_params(strategy: &str, value: Option<&Value>) -> Result<Value, ApiError> {
    let empty = json!({});
    crate::services::external::strategy::validate_strategy_params(strategy, value.unwrap_or(&empty))
}

fn validate_agent_risk_fields(
    max_notional_per_execution: Option<f64>,
    max_daily_spend_usdc: Option<f64>,
    max_slippage_bps: Option<i32>,
) -> Result<(), ApiError> {
    if let Some(value) = max_notional_per_execution {
        if value <= 0.0 {
            return Err(ApiError::bad_request(
                "INVALID_MAX_NOTIONAL",
                "maxNotionalPerExecution must be greater than zero",
            ));
        }
    }
    if let Some(value) = max_daily_spend_usdc {
        if value <= 0.0 {
            return Err(ApiError::bad_request(
                "INVALID_MAX_DAILY_SPEND",
                "maxDailySpendUsdc must be greater than zero",
            ));
        }
    }
    if let Some(value) = max_slippage_bps {
        if value < 0 {
            return Err(ApiError::bad_request(
                "INVALID_MAX_SLIPPAGE",
                "maxSlippageBps must be zero or positive",
            ));
        }
    }
    Ok(())
}

fn ensure_live_strategy_allowed(
    strategy: &str,
    execution_mode: ExternalExecutionMode,
) -> Result<(), ApiError> {
    if matches!(execution_mode, ExternalExecutionMode::Live) {
        let normalized = strategy.trim().to_ascii_lowercase().replace('_', "-");
        if normalized != "wallet-follow-v2" {
            return Err(ApiError::bad_request(
                "LIVE_STRATEGY_RESTRICTED",
                "only wallet_follow_v2 agents can run in live mode",
            ));
        }
    }
    Ok(())
}

fn normalized_strategy_key(strategy: &str) -> String {
    strategy.trim().to_ascii_lowercase().replace('_', "-")
}

fn is_event_repricing_v2_strategy(strategy: &str) -> bool {
    normalized_strategy_key(strategy) == "event-repricing-v2"
}

fn event_repricing_v2_candidate_ineligibility(
    signal: &ExternalSignalResponse,
    market: &ExternalMarketSnapshot,
    requirements: &crate::services::external::strategy::EventRepricingV2Requirements,
    now: DateTime<Utc>,
) -> Option<String> {
    if signal.signal_type != "scenario_lab" {
        return Some("event_repricing_v2 requires an active scenario_lab signal".to_string());
    }
    if requirements.require_resolution_rules && !signal.resolution_rules_read {
        return Some("event_repricing_v2: resolution rules not confirmed".to_string());
    }
    if signal.sources.len() < requirements.min_signal_sources as usize {
        return Some(format!(
            "event_repricing_v2: only {} sources, need {}",
            signal.sources.len(),
            requirements.min_signal_sources
        ));
    }
    if requirements.require_live_reference && !signal.has_live_reference {
        return Some("event_repricing_v2: no canonical live reference attached".to_string());
    }
    if signal.resolution_hazards.len() as u64 > requirements.max_resolution_hazards {
        return Some(format!(
            "event_repricing_v2: {} unresolved resolution hazards",
            signal.resolution_hazards.len()
        ));
    }

    let close_time = i64::try_from(market.close_time).ok();
    let Some(close_time) = close_time.filter(|value| *value > 0) else {
        return Some("event_repricing_v2: market close window unavailable".to_string());
    };

    let time_to_resolution_seconds = close_time - now.timestamp();
    if time_to_resolution_seconds < (requirements.min_hours_to_resolution as i64 * 3600) {
        return Some(format!(
            "event_repricing_v2: market resolves too soon ({}h required)",
            requirements.min_hours_to_resolution
        ));
    }

    None
}

async fn ensure_event_repricing_v2_candidate_eligible(
    state: &AppState,
    market: &ExternalMarketSnapshot,
    strategy: &str,
    strategy_params: &Value,
    active: bool,
) -> Result<(), ApiError> {
    if !active || !is_event_repricing_v2_strategy(strategy) {
        return Ok(());
    }

    let requirements =
        crate::services::external::strategy::event_repricing_v2_requirements(strategy_params)?;
    let signal = load_active_market_signal(state, market.id.as_str())
        .await?
        .ok_or_else(|| {
            ApiError::bad_request(
                "EVENT_REPRICING_SIGNAL_REQUIRED",
                "event_repricing_v2 requires an active scenario_lab signal for the selected market",
            )
        })?;

    if let Some(reason) =
        event_repricing_v2_candidate_ineligibility(&signal, market, &requirements, Utc::now())
    {
        return Err(ApiError::bad_request(
            "EVENT_REPRICING_MARKET_INELIGIBLE",
            reason.as_str(),
        ));
    }

    Ok(())
}

fn parse_query_datetime(
    raw: Option<&str>,
    field: &str,
) -> Result<Option<chrono::DateTime<Utc>>, ApiError> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|_| {
            ApiError::bad_request(
                "INVALID_TIMESTAMP",
                format!("{field} must be an RFC3339 timestamp").as_str(),
            )
        })
}

fn mask_credentials(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut next = serde_json::Map::new();
            for (key, raw) in map {
                if matches!(
                    key.as_str(),
                    "baseWallet" | "base_wallet" | "funder" | "signatureType" | "signature_type"
                ) {
                    next.insert(key.clone(), raw.clone());
                } else if raw.is_string() {
                    let masked = mask_secret(raw.as_str().unwrap_or_default());
                    next.insert(key.clone(), Value::String(masked));
                } else {
                    next.insert(key.clone(), mask_credentials(raw));
                }
            }
            Value::Object(next)
        }
        Value::Array(entries) => Value::Array(entries.iter().map(mask_credentials).collect()),
        _ => value.clone(),
    }
}

fn ensure_external_features_enabled(state: &AppState) -> Result<(), ApiError> {
    if !state.config.external_markets_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_MARKETS_DISABLED",
            "external market integration is disabled",
        ));
    }
    Ok(())
}

fn execution_mode(state: &AppState) -> ExternalExecutionMode {
    state.config.external_execution_mode
}

fn normalize_agent_owner(value: Option<&str>) -> Option<String> {
    value
        .map(|entry| entry.trim().to_ascii_lowercase())
        .filter(|entry| !entry.is_empty())
}

fn resolve_external_agent_owner_scope(
    user: &AuthenticatedUserWithRole,
    scope: Option<&str>,
    owner: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let requested_owner = normalize_agent_owner(owner);
    let wallet = user.wallet_address.trim().to_ascii_lowercase();

    if !matches!(user.role, UserRole::Admin) {
        if let Some(requested_owner) = requested_owner.as_ref() {
            if requested_owner != &wallet {
                return Err(ApiError::forbidden("Insufficient permissions"));
            }
        }
        return Ok(Some(wallet));
    }

    match scope
        .unwrap_or_else(|| {
            if requested_owner.is_some() {
                "owner"
            } else {
                "self"
            }
        })
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "all" => Ok(None),
        "owner" => Ok(Some(requested_owner.unwrap_or(wallet))),
        _ => Ok(Some(requested_owner.unwrap_or(wallet))),
    }
}

fn ensure_live_write_mode(state: &AppState) -> Result<(), ApiError> {
    if execution_mode(state).is_paper() {
        return Err(ApiError::conflict(
            "EXTERNAL_PAPER_MODE_ONLY",
            "live external venue writes are disabled while EXTERNAL_EXECUTION_MODE=paper",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ExternalStateImportTable {
    Credentials,
    OrderIntents,
    Orders,
    Agents,
    AgentRuns,
}

impl ExternalStateImportTable {
    fn parse(raw: &str) -> Result<Self, ApiError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "external_credentials" => Ok(Self::Credentials),
            "external_order_intents" => Ok(Self::OrderIntents),
            "external_orders" => Ok(Self::Orders),
            "external_agents" => Ok(Self::Agents),
            "external_agent_runs" => Ok(Self::AgentRuns),
            _ => Err(ApiError::bad_request(
                "INVALID_EXTERNAL_STATE_TABLE",
                "unsupported external state table",
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Credentials => "external_credentials",
            Self::OrderIntents => "external_order_intents",
            Self::Orders => "external_orders",
            Self::Agents => "external_agents",
            Self::AgentRuns => "external_agent_runs",
        }
    }
}

async fn ensure_external_state_import_admin(
    req: &HttpRequest,
    state: &web::Data<Arc<AppState>>,
) -> Result<(), ApiError> {
    if let Some(expected) = (!state.config.admin_control_key.trim().is_empty())
        .then_some(state.config.admin_control_key.trim())
    {
        let provided = req
            .headers()
            .get("x-admin-key")
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .unwrap_or("");
        if provided == expected {
            return Ok(());
        }
    }

    let user = extract_jwt_user(req, state)?;
    check_role(user.role, UserRole::Admin)?;
    Ok(())
}

fn parse_string_value(raw: Option<&Value>) -> String {
    raw.and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn parse_f64_value(raw: Option<&Value>) -> f64 {
    match raw {
        Some(Value::Number(value)) => value.as_f64().unwrap_or(0.0),
        Some(Value::String(value)) => value.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn parse_i64_value(raw: Option<&Value>) -> i64 {
    match raw {
        Some(Value::Number(value)) => value.as_i64().unwrap_or(0),
        Some(Value::String(value)) => value.parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

fn normalize_polymarket_market_ref(raw: &str) -> String {
    raw.trim()
        .strip_prefix("polymarket:")
        .unwrap_or(raw.trim())
        .to_string()
}

async fn import_external_state_rows(
    state: &web::Data<Arc<AppState>>,
    table: ExternalStateImportTable,
    rows: Vec<Value>,
) -> Result<usize, ApiError> {
    if rows.is_empty() {
        return Ok(0);
    }

    if rows.len() > MAX_EXTERNAL_STATE_IMPORT_BATCH_SIZE {
        return Err(ApiError::bad_request(
            "EXTERNAL_STATE_BATCH_TOO_LARGE",
            "external state import batch exceeds maximum size",
        ));
    }

    let imported = rows.len();
    let payload = Value::Array(rows);

    let query = match table {
        ExternalStateImportTable::Credentials => {
            r#"
            INSERT INTO external_credentials (
                id,
                owner,
                provider,
                label,
                encrypted_payload,
                key_id,
                created_at,
                updated_at,
                revoked_at
            )
            SELECT
                entry.id,
                LOWER(entry.owner),
                entry.provider,
                entry.label,
                entry.encrypted_payload,
                entry.key_id,
                entry.created_at,
                entry.updated_at,
                entry.revoked_at
            FROM jsonb_to_recordset($1::jsonb) AS entry(
                id TEXT,
                owner TEXT,
                provider TEXT,
                label TEXT,
                encrypted_payload TEXT,
                key_id TEXT,
                created_at TIMESTAMPTZ,
                updated_at TIMESTAMPTZ,
                revoked_at TIMESTAMPTZ
            )
            ON CONFLICT (id) DO UPDATE
            SET owner = EXCLUDED.owner,
                provider = EXCLUDED.provider,
                label = EXCLUDED.label,
                encrypted_payload = EXCLUDED.encrypted_payload,
                key_id = EXCLUDED.key_id,
                created_at = EXCLUDED.created_at,
                updated_at = EXCLUDED.updated_at,
                revoked_at = EXCLUDED.revoked_at
            "#
        }
        ExternalStateImportTable::OrderIntents => {
            r#"
            INSERT INTO external_order_intents (
                id,
                owner,
                provider,
                market_id,
                provider_market_ref,
                outcome,
                side,
                price,
                quantity,
                preflight,
                typed_data,
                status,
                credential_id,
                created_at,
                updated_at
            )
            SELECT
                entry.id,
                LOWER(entry.owner),
                entry.provider,
                entry.market_id,
                entry.provider_market_ref,
                entry.outcome,
                entry.side,
                entry.price,
                entry.quantity,
                COALESCE(entry.preflight, '{}'::jsonb),
                COALESCE(entry.typed_data, '{}'::jsonb),
                entry.status,
                entry.credential_id,
                entry.created_at,
                entry.updated_at
            FROM jsonb_to_recordset($1::jsonb) AS entry(
                id TEXT,
                owner TEXT,
                provider TEXT,
                market_id TEXT,
                provider_market_ref TEXT,
                outcome TEXT,
                side TEXT,
                price DOUBLE PRECISION,
                quantity DOUBLE PRECISION,
                preflight JSONB,
                typed_data JSONB,
                status TEXT,
                credential_id TEXT,
                created_at TIMESTAMPTZ,
                updated_at TIMESTAMPTZ
            )
            ON CONFLICT (id) DO UPDATE
            SET owner = EXCLUDED.owner,
                provider = EXCLUDED.provider,
                market_id = EXCLUDED.market_id,
                provider_market_ref = EXCLUDED.provider_market_ref,
                outcome = EXCLUDED.outcome,
                side = EXCLUDED.side,
                price = EXCLUDED.price,
                quantity = EXCLUDED.quantity,
                preflight = EXCLUDED.preflight,
                typed_data = EXCLUDED.typed_data,
                status = EXCLUDED.status,
                credential_id = EXCLUDED.credential_id,
                created_at = EXCLUDED.created_at,
                updated_at = EXCLUDED.updated_at
            "#
        }
        ExternalStateImportTable::Orders => {
            r#"
            INSERT INTO external_orders (
                id,
                owner,
                provider,
                intent_id,
                market_id,
                provider_order_id,
                status,
                request_payload,
                response_payload,
                error_message,
                created_at,
                updated_at
            )
            SELECT
                entry.id,
                LOWER(entry.owner),
                entry.provider,
                entry.intent_id,
                entry.market_id,
                entry.provider_order_id,
                entry.status,
                COALESCE(entry.request_payload, '{}'::jsonb),
                COALESCE(entry.response_payload, '{}'::jsonb),
                entry.error_message,
                entry.created_at,
                entry.updated_at
            FROM jsonb_to_recordset($1::jsonb) AS entry(
                id TEXT,
                owner TEXT,
                provider TEXT,
                intent_id TEXT,
                market_id TEXT,
                provider_order_id TEXT,
                status TEXT,
                request_payload JSONB,
                response_payload JSONB,
                error_message TEXT,
                created_at TIMESTAMPTZ,
                updated_at TIMESTAMPTZ
            )
            ON CONFLICT (id) DO UPDATE
            SET owner = EXCLUDED.owner,
                provider = EXCLUDED.provider,
                intent_id = EXCLUDED.intent_id,
                market_id = EXCLUDED.market_id,
                provider_order_id = EXCLUDED.provider_order_id,
                status = EXCLUDED.status,
                request_payload = EXCLUDED.request_payload,
                response_payload = EXCLUDED.response_payload,
                error_message = EXCLUDED.error_message,
                created_at = EXCLUDED.created_at,
                updated_at = EXCLUDED.updated_at
            "#
        }
        ExternalStateImportTable::Agents => {
            r#"
            INSERT INTO external_agents (
                id,
                owner,
                name,
                provider,
                market_id,
                provider_market_ref,
                outcome,
                side,
                price,
                quantity,
                cadence_seconds,
                strategy,
                strategy_params,
                execution_mode,
                credential_id,
                active,
                max_notional_per_execution,
                max_daily_spend_usdc,
                max_slippage_bps,
                last_executed_at,
                next_execution_at,
                created_at,
                updated_at
            )
            SELECT
                entry.id,
                LOWER(entry.owner),
                entry.name,
                entry.provider,
                entry.market_id,
                entry.provider_market_ref,
                entry.outcome,
                entry.side,
                entry.price,
                entry.quantity,
                entry.cadence_seconds,
                entry.strategy,
                COALESCE(entry.strategy_params, '{}'::jsonb),
                COALESCE(NULLIF(LOWER(entry.execution_mode), ''), 'live'),
                entry.credential_id,
                entry.active,
                entry.max_notional_per_execution,
                entry.max_daily_spend_usdc,
                entry.max_slippage_bps,
                entry.last_executed_at,
                entry.next_execution_at,
                entry.created_at,
                entry.updated_at
            FROM jsonb_to_recordset($1::jsonb) AS entry(
                id TEXT,
                owner TEXT,
                name TEXT,
                provider TEXT,
                market_id TEXT,
                provider_market_ref TEXT,
                outcome TEXT,
                side TEXT,
                price DOUBLE PRECISION,
                quantity DOUBLE PRECISION,
                cadence_seconds BIGINT,
                strategy TEXT,
                strategy_params JSONB,
                execution_mode TEXT,
                credential_id TEXT,
                active BOOLEAN,
                max_notional_per_execution DOUBLE PRECISION,
                max_daily_spend_usdc DOUBLE PRECISION,
                max_slippage_bps INTEGER,
                last_executed_at TIMESTAMPTZ,
                next_execution_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ,
                updated_at TIMESTAMPTZ
            )
            ON CONFLICT (id) DO UPDATE
            SET owner = EXCLUDED.owner,
                name = EXCLUDED.name,
                provider = EXCLUDED.provider,
                market_id = EXCLUDED.market_id,
                provider_market_ref = EXCLUDED.provider_market_ref,
                outcome = EXCLUDED.outcome,
                side = EXCLUDED.side,
                price = EXCLUDED.price,
                quantity = EXCLUDED.quantity,
                cadence_seconds = EXCLUDED.cadence_seconds,
                strategy = EXCLUDED.strategy,
                strategy_params = EXCLUDED.strategy_params,
                execution_mode = EXCLUDED.execution_mode,
                credential_id = EXCLUDED.credential_id,
                active = EXCLUDED.active,
                max_notional_per_execution = EXCLUDED.max_notional_per_execution,
                max_daily_spend_usdc = EXCLUDED.max_daily_spend_usdc,
                max_slippage_bps = EXCLUDED.max_slippage_bps,
                last_executed_at = EXCLUDED.last_executed_at,
                next_execution_at = EXCLUDED.next_execution_at,
                created_at = EXCLUDED.created_at,
                updated_at = EXCLUDED.updated_at
            "#
        }
        ExternalStateImportTable::AgentRuns => {
            r#"
            INSERT INTO external_agent_runs (
                id,
                agent_id,
                owner,
                status,
                intent_id,
                external_order_id,
                error_message,
                metadata,
                created_at
            )
            SELECT
                entry.id,
                entry.agent_id,
                LOWER(entry.owner),
                entry.status,
                entry.intent_id,
                entry.external_order_id,
                entry.error_message,
                COALESCE(entry.metadata, '{}'::jsonb),
                entry.created_at
            FROM jsonb_to_recordset($1::jsonb) AS entry(
                id TEXT,
                agent_id TEXT,
                owner TEXT,
                status TEXT,
                intent_id TEXT,
                external_order_id TEXT,
                error_message TEXT,
                metadata JSONB,
                created_at TIMESTAMPTZ
            )
            ON CONFLICT (id) DO UPDATE
            SET agent_id = EXCLUDED.agent_id,
                owner = EXCLUDED.owner,
                status = EXCLUDED.status,
                intent_id = EXCLUDED.intent_id,
                external_order_id = EXCLUDED.external_order_id,
                error_message = EXCLUDED.error_message,
                metadata = EXCLUDED.metadata,
                created_at = EXCLUDED.created_at
            "#
        }
    };

    sqlx::query(query)
        .bind(payload)
        .execute(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(imported)
}

async fn reset_external_state(state: &web::Data<Arc<AppState>>) -> Result<(), ApiError> {
    let mut tx = state
        .db
        .pool()
        .begin()
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    for statement in [
        "DELETE FROM external_agent_runs",
        "DELETE FROM external_orders",
        "DELETE FROM external_order_intents",
        "DELETE FROM external_agents",
        "DELETE FROM external_credentials",
        "DELETE FROM external_market_cache",
    ] {
        sqlx::query(statement)
            .execute(&mut *tx)
            .await
            .map_err(|err| ApiError::internal(&err.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(())
}

fn increment_skip_reason(skips: &mut BTreeMap<String, u64>, reason: &str) {
    let entry = skips.entry(reason.to_string()).or_insert(0);
    *entry += 1;
}

fn provider_order_id_from_payload(payload: &Value) -> String {
    provider_order_id(payload)
}

pub(crate) fn skip_reason_from_error(err: &ApiError) -> String {
    match err.code.trim() {
        "CREDENTIAL_NOT_READY" => "credential_not_ready".to_string(),
        "MARKET_NOT_EXECUTABLE" => "market_not_executable".to_string(),
        "POLYMARKET_EXECUTION_NOT_IMPLEMENTED" => "provider_not_ready".to_string(),
        code => code.to_ascii_lowercase(),
    }
}

pub(crate) fn run_status_from_error(err: &ApiError) -> &'static str {
    match err.code.trim() {
        "CREDENTIAL_NOT_READY"
        | "MARKET_NOT_EXECUTABLE"
        | "POLYMARKET_EXECUTION_NOT_IMPLEMENTED" => "skipped",
        _ => "failed",
    }
}

fn parse_external_agent_record(
    row: sqlx::postgres::PgRow,
) -> Result<ExternalAgentRecord, ApiError> {
    let provider_raw: String = row
        .try_get("provider")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let execution_mode_raw: String = row
        .try_get("execution_mode")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(ExternalAgentRecord {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        owner: row
            .try_get("owner")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        cohort: row
            .try_get("cohort")
            .unwrap_or_else(|_| PRIVATE_ALPHA_COHORT.to_string()),
        name: row
            .try_get("name")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider: normalize_provider(provider_raw.as_str())?,
        market_id: row
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        outcome: row
            .try_get("outcome")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        side: row
            .try_get("side")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        price: row
            .try_get("price")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        quantity: row
            .try_get("quantity")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        cadence_seconds: row
            .try_get("cadence_seconds")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        strategy: row
            .try_get("strategy")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        strategy_params: row.try_get("strategy_params").unwrap_or_else(|_| json!({})),
        execution_mode: parse_external_execution_mode(execution_mode_raw.as_str())?,
        credential_id: row.try_get("credential_id").ok(),
        active: row
            .try_get("active")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        next_execution_at: row
            .try_get("next_execution_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        consecutive_failures: row.try_get::<i32, _>("consecutive_failures").unwrap_or(0),
        last_error_code: row.try_get("last_error_code").ok(),
        max_notional_per_execution: row.try_get("max_notional_per_execution").ok().flatten(),
        max_daily_spend_usdc: row.try_get("max_daily_spend_usdc").ok().flatten(),
        max_slippage_bps: row.try_get("max_slippage_bps").ok().flatten(),
    })
}

fn parse_paper_position(row: sqlx::postgres::PgRow) -> Result<PaperPositionRecord, ApiError> {
    Ok(PaperPositionRecord {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        entry_price: row
            .try_get("entry_price")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        filled_quantity: row
            .try_get("filled_quantity")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        fees_paid_usdc: row
            .try_get("fees_paid_usdc")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        hold_until: row
            .try_get("hold_until")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        opened_at: row
            .try_get("opened_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn parse_live_position(row: sqlx::postgres::PgRow) -> Result<LivePositionRecord, ApiError> {
    Ok(LivePositionRecord {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        entry_price: row
            .try_get("entry_price")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        filled_quantity: row
            .try_get("filled_quantity")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        fees_paid_usdc: row
            .try_get("fees_paid_usdc")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        hold_until: row
            .try_get("hold_until")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        opened_at: row
            .try_get("opened_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        status: row
            .try_get("status")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

pub(crate) async fn load_external_agent_for_owner(
    state: &AppState,
    agent_id: &str,
    owner: &str,
) -> Result<ExternalAgentRecord, ApiError> {
    let row = sqlx::query(
        "SELECT id, owner, cohort, name, provider, market_id, outcome, side, price, quantity,
                cadence_seconds, strategy, strategy_params, execution_mode, credential_id, active, last_executed_at, next_execution_at,
                consecutive_failures, last_error_code,
                max_notional_per_execution, max_daily_spend_usdc, max_slippage_bps
         FROM external_agents
         WHERE id = $1 AND owner = $2",
    )
    .bind(agent_id)
    .bind(owner)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .ok_or_else(|| ApiError::not_found("External agent"))?;

    parse_external_agent_record(row)
}

async fn load_external_agent_by_id(
    state: &AppState,
    agent_id: &str,
) -> Result<ExternalAgentRecord, ApiError> {
    let row = sqlx::query(
        "SELECT id, owner, cohort, name, provider, market_id, outcome, side, price, quantity,
                cadence_seconds, strategy, strategy_params, execution_mode, credential_id, active, last_executed_at, next_execution_at,
                consecutive_failures, last_error_code,
                max_notional_per_execution, max_daily_spend_usdc, max_slippage_bps
         FROM external_agents
         WHERE id = $1",
    )
    .bind(agent_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .ok_or_else(|| ApiError::not_found("External agent"))?;

    parse_external_agent_record(row)
}

async fn load_due_external_agents(
    state: &AppState,
    limit: i64,
) -> Result<Vec<ExternalAgentRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, owner, cohort, name, provider, market_id, outcome, side, price, quantity,
                cadence_seconds, strategy, strategy_params, execution_mode, credential_id, active, last_executed_at, next_execution_at,
                consecutive_failures, last_error_code,
                max_notional_per_execution, max_daily_spend_usdc, max_slippage_bps
         FROM external_agents
         WHERE active = TRUE
         ORDER BY next_execution_at ASC, id ASC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    rows.into_iter().map(parse_external_agent_record).collect()
}

async fn load_open_paper_position(
    state: &AppState,
    agent_id: &str,
) -> Result<Option<PaperPositionRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT id, entry_price, filled_quantity, fees_paid_usdc, hold_until, opened_at
         FROM paper_positions
         WHERE agent_id = $1 AND status = 'open'
         LIMIT 1",
    )
    .bind(agent_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    row.map(parse_paper_position).transpose()
}

async fn load_open_live_position(
    state: &AppState,
    agent_id: &str,
) -> Result<Option<LivePositionRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT id, entry_price, filled_quantity, fees_paid_usdc, hold_until, opened_at, status
         FROM external_positions
         WHERE agent_id = $1 AND status = 'open'
         LIMIT 1",
    )
    .bind(agent_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    row.map(parse_live_position).transpose()
}

async fn insert_external_agent_run(
    state: &AppState,
    run_id: &str,
    agent: &ExternalAgentRecord,
    status: &str,
    external_order_id: Option<&str>,
    error_message: Option<&str>,
    metadata: &Value,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO external_agent_runs (
            id, agent_id, owner, status, intent_id, external_order_id, error_message, metadata, created_at
        ) VALUES ($1,$2,$3,$4,NULL,$5,$6,$7,NOW())",
    )
    .bind(run_id)
    .bind(agent.id.as_str())
    .bind(agent.owner.as_str())
    .bind(status)
    .bind(external_order_id)
    .bind(error_message)
    .bind(metadata)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(())
}

async fn update_external_agent_schedule(
    state: &AppState,
    agent_id: &str,
    executed_at: chrono::DateTime<Utc>,
    next_execution_at: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE external_agents
         SET last_executed_at = $2, next_execution_at = $3, updated_at = NOW()
         WHERE id = $1",
    )
    .bind(agent_id)
    .bind(executed_at)
    .bind(next_execution_at)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(())
}

fn failure_backoff_multiplier(consecutive_failures: i32) -> i64 {
    match consecutive_failures {
        0..=2 => 1,
        3..=5 => 4,
        6..=10 => 16,
        _ => 64,
    }
}

const MAX_CONSECUTIVE_FAILURES_BEFORE_DEACTIVATE: i32 = 20;

async fn record_agent_failure(
    state: &AppState,
    agent_id: &str,
    error_code: &str,
    cadence_seconds: i64,
    current_failures: i32,
    now: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    let next_failures = current_failures.saturating_add(1);
    let backoff = failure_backoff_multiplier(next_failures);
    let next_execution_at = now + Duration::seconds(cadence_seconds.max(1).saturating_mul(backoff));
    sqlx::query(
        "UPDATE external_agents
         SET consecutive_failures = $2,
             last_error_code = $3,
             last_executed_at = $4,
             next_execution_at = $5,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(agent_id)
    .bind(next_failures)
    .bind(error_code)
    .bind(now)
    .bind(next_execution_at)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;
    Ok(())
}

async fn reset_agent_failures(state: &AppState, agent_id: &str) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE external_agents
         SET consecutive_failures = 0, last_error_code = NULL
         WHERE id = $1 AND consecutive_failures > 0",
    )
    .bind(agent_id)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;
    Ok(())
}

async fn deactivate_external_agent(
    state: &AppState,
    agent_id: &str,
    executed_at: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE external_agents
         SET active = FALSE,
             last_executed_at = $2,
             next_execution_at = $2,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(agent_id)
    .bind(executed_at)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(())
}

async fn load_credential(
    state: &AppState,
    owner: &str,
    provider: ExternalProvider,
    credential_id: Option<&str>,
) -> Result<StoredCredential, ApiError> {
    let row = if let Some(id) = credential_id {
        sqlx::query(
            "SELECT id, provider, label, encrypted_payload, key_id
             FROM external_credentials
             WHERE id = $1 AND owner = $2 AND revoked_at IS NULL",
        )
        .bind(id)
        .bind(owner)
        .fetch_optional(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
    } else {
        sqlx::query(
            "SELECT id, provider, label, encrypted_payload, key_id
             FROM external_credentials
             WHERE owner = $1 AND provider = $2 AND revoked_at IS NULL
             ORDER BY updated_at DESC
             LIMIT 1",
        )
        .bind(owner)
        .bind(provider.as_str())
        .fetch_optional(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
    };

    let row = row.ok_or_else(|| {
        ApiError::bad_request(
            "CREDENTIAL_NOT_FOUND",
            "no active credential found for provider",
        )
    })?;

    let encrypted_payload: String = row
        .try_get("encrypted_payload")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let key_id: String = row
        .try_get("key_id")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let payload = decrypt_json(
        state.config.external_credentials_master_key.as_str(),
        key_id.as_str(),
        encrypted_payload.as_str(),
    )?;

    Ok(StoredCredential {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        owner: owner.to_string(),
        payload,
    })
}

async fn load_external_order_intent_record(
    state: &AppState,
    owner: &str,
    intent_id: &str,
) -> Result<ExternalOrderIntentRecord, ApiError> {
    let row = sqlx::query(
        "SELECT provider, market_id, provider_market_ref, credential_id, price, typed_data
         FROM external_order_intents
         WHERE id = $1 AND owner = $2",
    )
    .bind(intent_id)
    .bind(owner)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .ok_or_else(|| ApiError::not_found("External order intent"))?;

    let provider_raw: String = row
        .try_get("provider")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(ExternalOrderIntentRecord {
        provider: normalize_provider(provider_raw.as_str())?,
        market_id: row
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider_market_ref: row
            .try_get("provider_market_ref")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        credential_id: row.try_get("credential_id").ok(),
        price: row
            .try_get("price")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        typed_data: row
            .try_get("typed_data")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn api_key_from_payload(payload: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = payload.get(*key).and_then(|entry| entry.as_str()) {
            if !value.trim().is_empty() {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

fn payload_string(payload: &Value, keys: &[&str]) -> Option<String> {
    api_key_from_payload(payload, keys)
}

fn polymarket_live_execution_message() -> &'static str {
    "Polymarket execution requires a wallet-backed account with valid CLOB credentials, a funder wallet, and a supported signing path."
}

fn polymarket_signature_type_from_payload(payload: &Value) -> Result<u8, ApiError> {
    let raw = payload
        .get("signatureType")
        .or_else(|| payload.get("signature_type"))
        .ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_CREDENTIALS",
                "polymarket credential must include signatureType",
            )
        })?;

    let value = if let Some(number) = raw.as_u64() {
        number
    } else if let Some(text) = raw.as_str() {
        text.trim().parse::<u64>().map_err(|_| {
            ApiError::bad_request(
                "INVALID_CREDENTIALS",
                "polymarket signatureType must be 0, 1, or 2",
            )
        })?
    } else {
        return Err(ApiError::bad_request(
            "INVALID_CREDENTIALS",
            "polymarket signatureType must be 0, 1, or 2",
        ));
    };

    match value {
        0..=2 => Ok(value as u8),
        _ => Err(ApiError::bad_request(
            "INVALID_CREDENTIALS",
            "polymarket signatureType must be 0, 1, or 2",
        )),
    }
}

fn polymarket_signature_type_label(signature_type: u8) -> &'static str {
    match signature_type {
        0 => "EOA",
        1 => "proxy",
        2 => "gnosis_safe",
        _ => "unknown",
    }
}

fn polymarket_authenticated_message(auth_status: &str) -> String {
    match auth_status {
        "ready" => "Polymarket CLOB credentials authenticated.".to_string(),
        "invalid_credentials" => "Polymarket rejected the stored CLOB credentials.".to_string(),
        "invalid_owner" => "Credential owner wallet is invalid for Polymarket auth.".to_string(),
        _ => "Polymarket auth check is unavailable right now.".to_string(),
    }
}

fn parse_string_list(value: Option<&Value>) -> Vec<String> {
    let Some(raw) = value else {
        return Vec::new();
    };

    if let Some(items) = raw.as_array() {
        return items
            .iter()
            .filter_map(|item| item.as_str())
            .map(ToOwned::to_owned)
            .collect();
    }

    if let Some(text) = raw.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(text) {
            return parsed;
        }
    }

    Vec::new()
}

fn normalized_string_list(values: Option<&[String]>) -> Vec<String> {
    values
        .into_iter()
        .flatten()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn metadata_string_list(value: &Value, key: &str) -> Vec<String> {
    parse_string_list(value.get(key))
}

fn metadata_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
}

fn metadata_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn metadata_i32(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|entry| i32::try_from(entry).ok())
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn push_unique_string(values: &mut Vec<String>, candidate: &str) {
    let trimmed = candidate.trim();
    if trimmed.is_empty()
        || values
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(trimmed))
    {
        return;
    }

    values.push(trimmed.to_string());
}

fn looks_like_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn supports_resolution_inference(market: &ExternalMarketSnapshot) -> bool {
    market
        .provider
        .eq_ignore_ascii_case(ExternalProvider::Polymarket.as_str())
}

fn inferred_resolution_criteria(market: &ExternalMarketSnapshot) -> Option<String> {
    if !supports_resolution_inference(market) {
        return None;
    }

    let description = normalize_whitespace(market.description.as_str());
    if description.is_empty() {
        return None;
    }

    Some(description)
}

fn inferred_resolution_hazards(
    market: &ExternalMarketSnapshot,
    criteria: Option<&str>,
) -> Vec<String> {
    let Some(criteria) = criteria else {
        return vec!["missing_resolution_criteria".to_string()];
    };

    let mut hazards = Vec::new();
    let normalized_criteria = criteria.to_ascii_lowercase();
    let normalized_question = normalize_whitespace(market.question.as_str()).to_ascii_lowercase();

    if normalized_criteria.contains("50-50") || normalized_criteria.contains("50/50") {
        hazards.push("fallback_split_resolution".to_string());
    }
    if normalized_question.contains(" before ")
        || normalized_question.contains(" after ")
        || normalized_criteria.contains("if neither occurs")
    {
        hazards.push("relative_event_dependency".to_string());
    }
    if normalized_criteria.contains("will not count")
        || normalized_criteria.contains("will not qualify")
        || (normalized_criteria.contains("only") && normalized_criteria.contains("qualify"))
    {
        hazards.push("narrow_qualification_clauses".to_string());
    }
    if normalized_criteria.contains("credible reporting")
        || normalized_criteria.contains("credible media reporting")
        || normalized_criteria.contains("consensus of credible")
    {
        hazards.push("credible_reporting_discretion".to_string());
    }
    if normalized_criteria.contains("official information from")
        || normalized_criteria.contains("official announcement")
        || normalized_criteria.contains("official announcements")
    {
        hazards.push("official_source_dependency".to_string());
    }

    hazards.sort();
    hazards.dedup();
    hazards
}

fn inferred_canonical_live_sources(market: &ExternalMarketSnapshot) -> Vec<String> {
    if !supports_resolution_inference(market) {
        return Vec::new();
    }

    let category = normalize_whitespace(market.category.as_str()).to_ascii_lowercase();
    let question = normalize_whitespace(market.question.as_str()).to_ascii_lowercase();
    let criteria = normalize_whitespace(market.description.as_str()).to_ascii_lowercase();
    let haystack = format!("{question} {criteria} {category}");
    let mut sources = Vec::new();

    if haystack.contains("gta vi") || haystack.contains("grand theft auto vi") {
        push_unique_string(&mut sources, "https://www.rockstargames.com/newswire");
    }
    if category == "crypto" || haystack.contains("bitcoin") || haystack.contains("btc") {
        push_unique_string(&mut sources, "https://www.coinbase.com/price/bitcoin");
    }
    if haystack.contains("ukraine") || haystack.contains("russia") || haystack.contains("ceasefire")
    {
        push_unique_string(&mut sources, "https://apnews.com/hub/russia-ukraine");
    }
    if haystack.contains("taiwan") || haystack.contains("china") {
        push_unique_string(&mut sources, "https://apnews.com/hub/china");
    }
    if category == "sports"
        || haystack.contains("nba")
        || haystack.contains("finals")
        || haystack.contains("playoff")
        || haystack.contains("thunder")
    {
        push_unique_string(&mut sources, "https://www.nba.com/standings");
    }

    sources
}

fn inferred_signal_sources(
    body: &CreateExternalSignalRequest,
    market: &ExternalMarketSnapshot,
) -> Vec<String> {
    let mut sources = normalized_string_list(body.sources.as_deref());
    let should_infer_live_sources = sources.is_empty();
    if !market.external_url.trim().is_empty() {
        push_unique_string(&mut sources, market.external_url.as_str());
    }
    if should_infer_live_sources {
        for source in inferred_canonical_live_sources(market) {
            push_unique_string(&mut sources, source.as_str());
        }
    }
    sources
}

fn inferred_has_live_reference(
    body: &CreateExternalSignalRequest,
    market: &ExternalMarketSnapshot,
    sources: &[String],
) -> bool {
    if let Some(value) = body.has_live_reference {
        return value;
    }

    sources.iter().any(|source| {
        looks_like_url(source) && !source.eq_ignore_ascii_case(market.external_url.as_str())
    })
}

fn merge_signal_metadata(
    body: &CreateExternalSignalRequest,
    market: &ExternalMarketSnapshot,
) -> Value {
    let mut metadata = match body.metadata.clone() {
        Some(Value::Object(map)) => Value::Object(map),
        Some(other) => json!({ "payload": other }),
        None => json!({}),
    };
    if let Some(memo_mode) = body
        .memo_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        metadata.as_object_mut().expect("metadata object").insert(
            "memoMode".to_string(),
            json!(memo_mode.to_ascii_lowercase()),
        );
    }

    let sources = inferred_signal_sources(body, market);
    let has_live_reference = inferred_has_live_reference(body, market, &sources);
    if !sources.is_empty() {
        metadata
            .as_object_mut()
            .expect("metadata object")
            .insert("sources".to_string(), json!(sources));
    }
    let inferred_criteria = inferred_resolution_criteria(market);
    if let Some(value) = body
        .resolution_rules_read
        .or_else(|| inferred_criteria.as_ref().map(|_| true))
    {
        metadata
            .as_object_mut()
            .expect("metadata object")
            .insert("resolutionRulesRead".to_string(), json!(value));
    }
    if let Some(value) = body
        .resolution_criteria
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .or(inferred_criteria)
    {
        metadata
            .as_object_mut()
            .expect("metadata object")
            .insert("resolutionCriteria".to_string(), json!(value));
    }
    let resolution_criteria = metadata_string(&metadata, "resolutionCriteria");
    let hazards = if body.resolution_hazards.is_some() {
        normalized_string_list(body.resolution_hazards.as_deref())
    } else {
        inferred_resolution_hazards(market, resolution_criteria.as_deref())
    };
    if !hazards.is_empty() {
        metadata
            .as_object_mut()
            .expect("metadata object")
            .insert("resolutionHazards".to_string(), json!(hazards));
    }
    metadata
        .as_object_mut()
        .expect("metadata object")
        .insert("hasLiveReference".to_string(), json!(has_live_reference));
    if let Some(value) = body.repricing_half_life_minutes.filter(|value| *value > 0) {
        metadata
            .as_object_mut()
            .expect("metadata object")
            .insert("repricingHalfLifeMinutes".to_string(), json!(value));
    }
    if let Some(value) = body
        .confidence_reasoning
        .as_deref()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        metadata
            .as_object_mut()
            .expect("metadata object")
            .insert("confidenceReasoning".to_string(), json!(value));
    }

    metadata
}

fn parse_json_f64(value: Option<&Value>) -> Option<f64> {
    let raw = value?;
    if let Some(number) = raw.as_f64() {
        return Some(number);
    }
    raw.as_str()
        .and_then(|text| text.trim().parse::<f64>().ok())
}

fn parse_json_u64(value: Option<&Value>) -> Option<u64> {
    let raw = value?;
    if let Some(number) = raw.as_u64() {
        return Some(number);
    }
    raw.as_str()
        .and_then(|text| text.trim().parse::<u64>().ok())
}

fn polymarket_side_value(side: &str) -> Result<u8, ApiError> {
    match side {
        "buy" => Ok(0),
        "sell" => Ok(1),
        _ => Err(ApiError::bad_request(
            "INVALID_SIDE",
            "side must be one of: buy, sell",
        )),
    }
}

fn polymarket_side_label(side: u8) -> Result<&'static str, ApiError> {
    match side {
        0 => Ok("BUY"),
        1 => Ok("SELL"),
        _ => Err(ApiError::bad_request(
            "INVALID_SIDE",
            "side must be one of: buy, sell",
        )),
    }
}

fn polymarket_price_step_int(minimum_tick_size: f64) -> Result<u128, ApiError> {
    let step = scale_limitless_decimal(minimum_tick_size, "minimumTickSize")?;
    match step {
        100_000 | 10_000 | 1_000 | 100 => Ok(step),
        _ => Err(ApiError::internal("unsupported polymarket tick size")),
    }
}

fn masked_polymarket_salt() -> u64 {
    (Uuid::new_v4().as_u128() as u64) & ((1_u64 << 53) - 1)
}

fn polymarket_exchange_contract(neg_risk: bool) -> &'static str {
    if neg_risk {
        POLYMARKET_NEG_RISK_EXCHANGE
    } else {
        POLYMARKET_EXCHANGE
    }
}

fn extract_polymarket_token_id(market: &Value, outcome: &str) -> Result<String, ApiError> {
    let outcomes = parse_string_list(market.get("outcomes"));
    let token_ids = parse_string_list(market.get("clobTokenIds"));

    if outcomes.is_empty() || token_ids.is_empty() {
        return Err(ApiError::bad_request(
            "POLYMARKET_TOKEN_NOT_FOUND",
            "polymarket market payload did not include outcome token ids",
        ));
    }

    for (index, label) in outcomes.iter().enumerate() {
        if label.eq_ignore_ascii_case(outcome) {
            if let Some(token_id) = token_ids.get(index) {
                return Ok(token_id.clone());
            }
        }
    }

    let fallback = if outcome.eq_ignore_ascii_case("yes") {
        token_ids.first()
    } else {
        token_ids.get(1)
    };

    fallback.cloned().ok_or_else(|| {
        ApiError::bad_request(
            "POLYMARKET_TOKEN_NOT_FOUND",
            "unable to map outcome to a polymarket token id",
        )
    })
}

fn polymarket_credentials(
    credential: &StoredCredential,
) -> Result<PolymarketCredentials, ApiError> {
    let api_key = payload_string(&credential.payload, &["apiKey", "api_key"]).ok_or_else(|| {
        ApiError::bad_request(
            "INVALID_CREDENTIALS",
            "polymarket credential must include apiKey",
        )
    })?;
    let api_secret =
        payload_string(&credential.payload, &["apiSecret", "api_secret"]).ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_CREDENTIALS",
                "polymarket credential must include apiSecret",
            )
        })?;
    let api_passphrase = payload_string(&credential.payload, &["apiPassphrase", "api_passphrase"])
        .ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_CREDENTIALS",
                "polymarket credential must include apiPassphrase",
            )
        })?;
    let funder = payload_string(&credential.payload, &["funder"]).ok_or_else(|| {
        ApiError::bad_request(
            "INVALID_CREDENTIALS",
            "polymarket credential must include funder",
        )
    })?;
    let funder = normalize_evm_wallet(funder.as_str())?;
    let signature_type = polymarket_signature_type_from_payload(&credential.payload)?;

    Ok(PolymarketCredentials {
        api_key,
        api_secret,
        api_passphrase,
        funder,
        signature_type,
    })
}

async fn fetch_polymarket_order_context(
    state: &AppState,
    market: &Value,
    outcome: &str,
) -> Result<PolymarketOrderContext, ApiError> {
    let token_id = extract_polymarket_token_id(market, outcome)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let tick_payload = client
        .get(format!(
            "{}/tick-size",
            state.config.polymarket_clob_api_base.trim_end_matches('/')
        ))
        .query(&[("token_id", token_id.as_str())])
        .send()
        .await
        .map_err(|err| ApiError::internal(&format!("polymarket tick-size failed: {}", err)))?
        .error_for_status()
        .map_err(|err| ApiError::internal(&format!("polymarket tick-size failed: {}", err)))?
        .json::<Value>()
        .await
        .map_err(|err| {
            ApiError::internal(&format!("polymarket tick-size payload invalid: {}", err))
        })?;

    let minimum_tick_size = parse_json_f64(
        tick_payload
            .get("minimum_tick_size")
            .or_else(|| tick_payload.get("minimumTickSize")),
    )
    .ok_or_else(|| ApiError::internal("polymarket tick-size payload missing minimum tick size"))?;

    let fee_payload = client
        .get(format!(
            "{}/fee-rate",
            state.config.polymarket_clob_api_base.trim_end_matches('/')
        ))
        .query(&[("token_id", token_id.as_str())])
        .send()
        .await
        .map_err(|err| ApiError::internal(&format!("polymarket fee-rate failed: {}", err)))?
        .error_for_status()
        .map_err(|err| ApiError::internal(&format!("polymarket fee-rate failed: {}", err)))?
        .json::<Value>()
        .await
        .map_err(|err| {
            ApiError::internal(&format!("polymarket fee-rate payload invalid: {}", err))
        })?;

    let fee_rate_bps = parse_json_u64(
        fee_payload
            .get("base_fee")
            .or_else(|| fee_payload.get("baseFee")),
    )
    .ok_or_else(|| ApiError::internal("polymarket fee-rate payload missing base fee"))?;

    let neg_risk_payload = client
        .get(format!(
            "{}/neg-risk",
            state.config.polymarket_clob_api_base.trim_end_matches('/')
        ))
        .query(&[("token_id", token_id.as_str())])
        .send()
        .await
        .map_err(|err| ApiError::internal(&format!("polymarket neg-risk failed: {}", err)))?
        .error_for_status()
        .map_err(|err| ApiError::internal(&format!("polymarket neg-risk failed: {}", err)))?
        .json::<Value>()
        .await
        .map_err(|err| {
            ApiError::internal(&format!("polymarket neg-risk payload invalid: {}", err))
        })?;

    let neg_risk = neg_risk_payload
        .get("neg_risk")
        .or_else(|| neg_risk_payload.get("negRisk"))
        .and_then(|value| value.as_bool())
        .ok_or_else(|| ApiError::internal("polymarket neg-risk payload missing neg_risk"))?;

    Ok(PolymarketOrderContext {
        token_id,
        fee_rate_bps,
        minimum_tick_size,
        neg_risk,
    })
}

fn build_polymarket_order_message(
    owner: &str,
    credentials: &PolymarketCredentials,
    side: &str,
    price: f64,
    quantity: f64,
    context: &PolymarketOrderContext,
) -> Result<Value, ApiError> {
    let signer = normalize_evm_wallet(owner)?;
    let maker = if credentials.signature_type == 0 {
        signer.clone()
    } else {
        credentials.funder.clone()
    };
    let side_value = polymarket_side_value(side)?;
    let tick_step_int = polymarket_price_step_int(context.minimum_tick_size)?;
    let price_int = scale_limitless_decimal(price, "price")?;
    if price_int < tick_step_int || price_int > POLYMARKET_PRICE_SCALE - tick_step_int {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "price is outside the supported polymarket tick range",
        ));
    }
    if price_int % tick_step_int != 0 {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "price must align to the venue tick size for Polymarket",
        ));
    }

    let shares_int = scale_limitless_decimal(quantity, "quantity")?;
    if shares_int % POLYMARKET_LOT_STEP_INT != 0 {
        return Err(ApiError::bad_request(
            "INVALID_QUANTITY",
            "quantity must align to 0.01 share increments for Polymarket",
        ));
    }

    let notional_int = shares_int
        .checked_mul(price_int)
        .ok_or_else(|| ApiError::internal("polymarket order amount overflow"))?
        / POLYMARKET_PRICE_SCALE;

    let (maker_amount, taker_amount) = if side_value == 0 {
        (notional_int, shares_int)
    } else {
        (shares_int, notional_int)
    };

    Ok(json!({
        "salt": masked_polymarket_salt(),
        "maker": maker,
        "signer": signer,
        "taker": LIMITLESS_ZERO_ADDRESS,
        "tokenId": context.token_id,
        "makerAmount": maker_amount.to_string(),
        "takerAmount": taker_amount.to_string(),
        "expiration": "0",
        "nonce": "0",
        "feeRateBps": context.fee_rate_bps.to_string(),
        "side": side_value,
        "signatureType": credentials.signature_type,
    }))
}

async fn build_polymarket_typed_data(
    state: &AppState,
    owner: &str,
    credential: &StoredCredential,
    request: &CreateExternalOrderIntentRequest,
    provider_market_payload: &Value,
) -> Result<Value, ApiError> {
    let credentials = polymarket_credentials(credential)?;
    let context =
        fetch_polymarket_order_context(state, provider_market_payload, request.outcome.as_str())
            .await?;
    let message = build_polymarket_order_message(
        owner,
        &credentials,
        request.side.as_str(),
        request.price,
        request.quantity,
        &context,
    )?;

    Ok(json!({
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "version", "type": "string" },
                { "name": "chainId", "type": "uint256" },
                { "name": "verifyingContract", "type": "address" }
            ],
            "Order": [
                { "name": "salt", "type": "uint256" },
                { "name": "maker", "type": "address" },
                { "name": "signer", "type": "address" },
                { "name": "taker", "type": "address" },
                { "name": "tokenId", "type": "uint256" },
                { "name": "makerAmount", "type": "uint256" },
                { "name": "takerAmount", "type": "uint256" },
                { "name": "expiration", "type": "uint256" },
                { "name": "nonce", "type": "uint256" },
                { "name": "feeRateBps", "type": "uint256" },
                { "name": "side", "type": "uint8" },
                { "name": "signatureType", "type": "uint8" }
            ]
        },
        "domain": {
            "name": POLYMARKET_SIGNING_NAME,
            "version": POLYMARKET_SIGNING_VERSION,
            "chainId": POLYMARKET_CHAIN_ID,
            "verifyingContract": polymarket_exchange_contract(context.neg_risk),
        },
        "primaryType": "Order",
        "message": message,
    }))
}

fn signed_order_signature(signed_order: &Value) -> Result<String, ApiError> {
    signed_order
        .get("signature")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_SIGNED_ORDER",
                "signed order must include a signature",
            )
        })
}

fn verify_submitted_typed_data(
    intent_typed_data: &Value,
    signed_order: &Value,
) -> Result<(), ApiError> {
    let Some(submitted) = signed_order
        .get("typedData")
        .or_else(|| signed_order.get("typed_data"))
    else {
        return Ok(());
    };

    if submitted != intent_typed_data {
        return Err(ApiError::bad_request(
            "INVALID_SIGNED_ORDER",
            "submitted typed data does not match the prepared intent",
        ));
    }

    Ok(())
}

fn submitted_typed_data<'a>(signed_order: &'a Value) -> Option<&'a Value> {
    signed_order
        .get("typedData")
        .or_else(|| signed_order.get("typed_data"))
}

fn required_signed_order_typed_data<'a>(signed_order: &'a Value) -> Result<&'a Value, ApiError> {
    submitted_typed_data(signed_order).ok_or_else(|| {
        ApiError::bad_request(
            "INVALID_SIGNED_ORDER",
            "signed order must include typedData unless it is already provider-formatted",
        )
    })
}

fn build_polymarket_submit_payload(
    credential: &StoredCredential,
    typed_data: &Value,
    signed_order: &Value,
) -> Result<Value, ApiError> {
    verify_submitted_typed_data(typed_data, signed_order)?;
    let credentials = polymarket_credentials(credential)?;
    let message = typed_data
        .get("message")
        .and_then(|value| value.as_object())
        .ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_SIGNED_ORDER",
                "order intent is missing the typed-data message payload",
            )
        })?;
    let side_value = message
        .get("side")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| {
            ApiError::bad_request("INVALID_SIGNED_ORDER", "typed data message is missing side")
        })? as u8;
    let side = polymarket_side_label(side_value)?;
    let signature = signed_order_signature(signed_order)?;

    Ok(json!({
        "order": {
            "salt": message.get("salt").cloned().unwrap_or_else(|| json!(0)),
            "maker": message.get("maker").cloned().unwrap_or(Value::Null),
            "signer": message.get("signer").cloned().unwrap_or(Value::Null),
            "taker": message.get("taker").cloned().unwrap_or(Value::Null),
            "tokenId": message.get("tokenId").cloned().unwrap_or(Value::Null),
            "makerAmount": message.get("makerAmount").cloned().unwrap_or(Value::Null),
            "takerAmount": message.get("takerAmount").cloned().unwrap_or(Value::Null),
            "expiration": message.get("expiration").cloned().unwrap_or_else(|| json!("0")),
            "nonce": message.get("nonce").cloned().unwrap_or_else(|| json!("0")),
            "feeRateBps": message.get("feeRateBps").cloned().unwrap_or_else(|| json!("0")),
            "side": side,
            "signatureType": message.get("signatureType").cloned().unwrap_or_else(|| json!(credentials.signature_type)),
            "signature": signature,
        },
        "owner": credentials.api_key,
        "orderType": "GTC",
    }))
}

async fn build_provider_submit_payload(
    state: &AppState,
    provider: ExternalProvider,
    credential: &StoredCredential,
    market_id: &str,
    provider_market_ref: &str,
    price: f64,
    intent_typed_data: Option<&Value>,
    signed_order: &Value,
) -> Result<Value, ApiError> {
    match provider {
        ExternalProvider::Limitless => {
            let empty_typed_data = Value::Null;
            let typed_data = if signed_order.get("order").is_some() {
                &empty_typed_data
            } else if let Some(value) =
                intent_typed_data.or_else(|| submitted_typed_data(signed_order))
            {
                value
            } else {
                return Err(ApiError::bad_request(
                    "INVALID_SIGNED_ORDER",
                    "signed order must include typedData unless it is already provider-formatted",
                ));
            };

            build_limitless_submit_payload(
                state,
                credential,
                market_id,
                provider_market_ref,
                price,
                typed_data,
                signed_order,
            )
            .await
        }
        ExternalProvider::Polymarket => {
            if signed_order.get("order").is_some() && signed_order.get("owner").is_some() {
                return Ok(signed_order.clone());
            }

            let typed_data = match intent_typed_data {
                Some(value) => value,
                None => required_signed_order_typed_data(signed_order)?,
            };
            build_polymarket_submit_payload(credential, typed_data, signed_order)
        }
        ExternalProvider::Aerodrome => {
            // Parse pool address from market_id (format: "aerodrome:0x...")
            let pool_address = if market_id.contains(':') {
                market_id
                    .split_once(':')
                    .map(|(_, v)| v)
                    .unwrap_or(market_id)
            } else if !provider_market_ref.is_empty() {
                provider_market_ref
            } else {
                market_id
            };

            // Fetch current pool state
            let pool = crate::services::external::providers::aerodrome::fetch_pool_state(
                &state.evm_rpc,
                pool_address,
            )
            .await?;

            // Determine token_in/token_out based on side from signed_order or agent context
            let side = signed_order
                .get("side")
                .and_then(|v| v.as_str())
                .unwrap_or("buy");
            let (token_in, token_out, decimals_in) = if side == "buy" {
                // Buying outcome token: spend token1 (collateral) → receive token0
                (&pool.token1, &pool.token0, pool.token1_decimals)
            } else {
                // Selling outcome token: spend token0 → receive token1 (collateral)
                (&pool.token0, &pool.token1, pool.token0_decimals)
            };

            // Compute amount_in from quantity (in human-readable units)
            let quantity = signed_order
                .get("quantity")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            if quantity <= 0.0 {
                return Err(ApiError::bad_request(
                    "INVALID_SWAP_PARAMS",
                    "quantity must be greater than zero",
                ));
            }
            let amount_in = (quantity * 10.0_f64.powi(decimals_in as i32)) as u128;

            // Get base wallet from credential
            let base_wallet = payload_string(&credential.payload, &["baseWallet", "base_wallet"])
                .ok_or_else(|| {
                ApiError::bad_request(
                    "INVALID_CREDENTIALS",
                    "aerodrome credential must include baseWallet",
                )
            })?;

            // Quote the swap to get expected output
            let quote = crate::services::external::providers::aerodrome::quote_swap(
                &state.evm_rpc,
                state.config.aerodrome_quoter_address.as_str(),
                token_in,
                token_out,
                amount_in,
                pool.tick_spacing,
            )
            .await?;

            // Apply 1% slippage tolerance
            let amount_out_minimum = quote.amount_out * 99 / 100;
            let deadline = (Utc::now().timestamp() as u64) + 300; // 5 minutes

            // Encode swap calldata
            let calldata = crate::services::aerodrome::encode_swap_exact_input_single(
                token_in,
                token_out,
                pool.tick_spacing,
                &base_wallet,
                deadline,
                amount_in,
                amount_out_minimum,
            )?;

            Ok(json!({
                "mode": "aerodrome_swap",
                "chainId": 8453,
                "pool": pool_address,
                "tokenIn": token_in,
                "tokenOut": token_out,
                "amountIn": amount_in.to_string(),
                "amountOutMinimum": amount_out_minimum.to_string(),
                "quotedAmountOut": quote.amount_out.to_string(),
                "gasEstimate": quote.gas_estimate,
                "priceImpactBps": quote.price_impact_bps,
                "deadline": deadline,
                "calldata": calldata,
                "to": state.config.aerodrome_swap_router_address,
                "recipient": base_wallet,
                "side": side,
            }))
        }
    }
}

fn polymarket_request_body(payload: &Value) -> Result<String, ApiError> {
    serde_json::to_string(payload).map_err(|err| ApiError::internal(&err.to_string()))
}

fn polymarket_hmac_signature(
    secret: &str,
    method: &str,
    path: &str,
    body: &str,
    timestamp: &str,
    invalid_code: &str,
    invalid_field: &str,
) -> Result<String, ApiError> {
    let invalid_message = format!("{invalid_field} is invalid");
    let decoded_secret = URL_SAFE
        .decode(secret.trim())
        .map_err(|_| ApiError::bad_request(invalid_code, invalid_message.as_str()))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&decoded_secret)
        .map_err(|_| ApiError::bad_request(invalid_code, invalid_message.as_str()))?;
    mac.update(format!("{}{}{}{}", timestamp, method, path, body).as_bytes());
    Ok(URL_SAFE.encode(mac.finalize().into_bytes()))
}

fn polymarket_l2_signature(
    api_secret: &str,
    method: &str,
    path: &str,
    body: &str,
    timestamp: &str,
) -> Result<String, ApiError> {
    polymarket_hmac_signature(
        api_secret,
        method,
        path,
        body,
        timestamp,
        "INVALID_CREDENTIALS",
        "polymarket apiSecret",
    )
}

fn polymarket_builder_credentials(config: &AppConfig) -> Option<PolymarketBuilderCredentials<'_>> {
    let api_key = config.polymarket_builder_api_key.trim();
    let api_secret = config.polymarket_builder_api_secret.trim();
    let api_passphrase = config.polymarket_builder_api_passphrase.trim();
    if api_key.is_empty() || api_secret.is_empty() || api_passphrase.is_empty() {
        return None;
    }

    Some(PolymarketBuilderCredentials {
        api_key,
        api_secret,
        api_passphrase,
    })
}

fn polymarket_builder_headers(
    credentials: PolymarketBuilderCredentials<'_>,
    method: &str,
    path: &str,
    body: &str,
    timestamp: &str,
) -> Result<PolymarketBuilderHeaders, ApiError> {
    let signature = polymarket_hmac_signature(
        credentials.api_secret,
        method,
        path,
        body,
        timestamp,
        "INVALID_BUILDER_CREDENTIALS",
        "polymarket builder apiSecret",
    )?;

    Ok(PolymarketBuilderHeaders {
        api_key: credentials.api_key.to_string(),
        api_passphrase: credentials.api_passphrase.to_string(),
        signature,
        timestamp: timestamp.to_string(),
    })
}

fn polymarket_clob_url(config: &AppConfig, path: &str) -> String {
    format!(
        "{}{}",
        config.polymarket_clob_api_base.trim_end_matches('/'),
        path
    )
}

#[derive(Debug, Clone)]
struct PolymarketTrackedMarket {
    market_id: String,
    provider_market_ref: String,
    condition_id: String,
    market_category: Option<String>,
    token_outcomes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
struct PolymarketBackfillCounts {
    public_trades_ingested: u64,
    user_fill_events_ingested: u64,
    user_lifecycle_events_reconciled: u64,
}

fn polymarket_builder_configured(config: &AppConfig) -> bool {
    polymarket_builder_credentials(config).is_some()
}

fn map_polymarket_lifecycle_status(
    value: &str,
) -> external::polymarket_index::PolymarketTradeLifecycleStatus {
    match value.trim().to_ascii_uppercase().as_str() {
        "MATCHED" | "STATE_NEW" | "STATE_EXECUTED" => {
            external::polymarket_index::PolymarketTradeLifecycleStatus::Matched
        }
        "MINED" | "STATE_MINED" => {
            external::polymarket_index::PolymarketTradeLifecycleStatus::Mined
        }
        "RETRYING" => external::polymarket_index::PolymarketTradeLifecycleStatus::Retrying,
        "FAILED" | "STATE_FAILED" | "STATE_INVALID" => {
            external::polymarket_index::PolymarketTradeLifecycleStatus::Failed
        }
        "CONFIRMED" | "STATE_CONFIRMED" => {
            external::polymarket_index::PolymarketTradeLifecycleStatus::Confirmed
        }
        _ => external::polymarket_index::PolymarketTradeLifecycleStatus::Matched,
    }
}

fn build_polymarket_builder_request_headers(
    config: &AppConfig,
    method: &str,
    path: &str,
) -> Result<BTreeMap<String, String>, ApiError> {
    let credentials = polymarket_builder_credentials(config).ok_or_else(|| {
        ApiError::bad_request(
            "POLYMARKET_BUILDER_NOT_CONFIGURED",
            "Polymarket builder credentials are not configured",
        )
    })?;
    let timestamp = Utc::now().timestamp().to_string();
    let builder_headers =
        polymarket_builder_headers(credentials, method, path, "", timestamp.as_str())?;
    let mut headers =
        BTreeMap::from([("Content-Type".to_string(), "application/json".to_string())]);
    insert_polymarket_builder_headers(&mut headers, &builder_headers);
    Ok(headers)
}

fn polymarket_relayer_url(path: &str) -> String {
    format!("{}{}", POLYMARKET_RELAYER_API_BASE, path)
}

fn parse_polymarket_rfc3339(value: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn parse_polymarket_observed_at(payload: &Value) -> Option<chrono::DateTime<Utc>> {
    for key in [
        "last_update",
        "lastUpdate",
        "matchtime",
        "matchTime",
        "timestamp",
    ] {
        let raw = parse_string_value(payload.get(key));
        if raw.is_empty() {
            continue;
        }
        if let Ok(seconds) = raw.parse::<i64>() {
            if let Some(timestamp) = parse_polymarket_timestamp(seconds) {
                return Some(timestamp);
            }
        }
        if let Some(timestamp) = parse_polymarket_rfc3339(raw.as_str()) {
            return Some(timestamp);
        }
    }
    None
}

fn parse_polymarket_lifecycle_status(
    raw: Option<&str>,
) -> Option<external::polymarket_index::PolymarketTradeLifecycleStatus> {
    raw.map(map_polymarket_lifecycle_status)
}

fn lifecycle_rank(status: external::polymarket_index::PolymarketTradeLifecycleStatus) -> u8 {
    match status {
        external::polymarket_index::PolymarketTradeLifecycleStatus::Matched => 1,
        external::polymarket_index::PolymarketTradeLifecycleStatus::Mined => 2,
        external::polymarket_index::PolymarketTradeLifecycleStatus::Retrying => 3,
        external::polymarket_index::PolymarketTradeLifecycleStatus::Failed => 4,
        external::polymarket_index::PolymarketTradeLifecycleStatus::Confirmed => 5,
    }
}

fn best_polymarket_lifecycle_status(
    direct: Option<external::polymarket_index::PolymarketTradeLifecycleStatus>,
    relayer: Option<external::polymarket_index::PolymarketTradeLifecycleStatus>,
    tx_hash: Option<&str>,
) -> external::polymarket_index::PolymarketTradeLifecycleStatus {
    let mut best = direct.unwrap_or_else(|| {
        if tx_hash.is_some() {
            external::polymarket_index::PolymarketTradeLifecycleStatus::Mined
        } else {
            external::polymarket_index::PolymarketTradeLifecycleStatus::Matched
        }
    });
    if let Some(relayer) = relayer {
        if lifecycle_rank(relayer) > lifecycle_rank(best) {
            best = relayer;
        }
    }
    best
}

fn relayer_transaction_match_score(
    tx: &Value,
    refs: &external::polymarket_index::PolymarketReferenceCandidates,
) -> u8 {
    let tx_hash = parse_string_value(tx.get("transactionHash"));
    if !tx_hash.is_empty()
        && refs
            .tx_hashes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(tx_hash.as_str()))
    {
        return 3;
    }

    let metadata = parse_string_value(tx.get("metadata")).to_ascii_lowercase();
    if !metadata.is_empty()
        && refs
            .provider_order_refs
            .iter()
            .any(|candidate| metadata.contains(candidate.to_ascii_lowercase().as_str()))
    {
        return 2;
    }
    if !metadata.is_empty()
        && refs
            .builder_trade_refs
            .iter()
            .any(|candidate| metadata.contains(candidate.to_ascii_lowercase().as_str()))
    {
        return 1;
    }

    0
}

fn match_relayer_transaction<'a>(
    payload: &Value,
    relayer_transactions: &'a [Value],
) -> Option<&'a Value> {
    let refs = external::polymarket_index::reference_candidates_from_payload(payload);
    relayer_transactions
        .iter()
        .filter_map(|tx| {
            let score = relayer_transaction_match_score(tx, &refs);
            if score == 0 {
                return None;
            }
            let updated_at = parse_string_value(tx.get("updatedAt"));
            Some((score, updated_at, tx))
        })
        .max_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)))
        .map(|(_, _, tx)| tx)
}

#[derive(Debug, Default, Clone)]
struct PolymarketOrderContextMatch {
    agent_id: Option<String>,
    run_id: Option<String>,
    external_order_id: Option<String>,
    owner: Option<String>,
}

async fn load_polymarket_order_context_match(
    state: &AppState,
    market_id: &str,
    provider_order_refs: &[String],
) -> Result<PolymarketOrderContextMatch, ApiError> {
    if provider_order_refs.is_empty() {
        return Ok(PolymarketOrderContextMatch::default());
    }

    let refs = provider_order_refs
        .iter()
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    if refs.is_empty() {
        return Ok(PolymarketOrderContextMatch::default());
    }

    let row = sqlx::query(
        "SELECT
            eo.id AS external_order_id,
            eo.owner AS owner,
            ear.id AS run_id,
            ear.agent_id AS agent_id
         FROM external_orders eo
         LEFT JOIN external_order_agent_runs ear
            ON ear.external_order_id = eo.id
         WHERE eo.provider = 'polymarket'
           AND eo.market_id = $1
           AND eo.provider_order_id = ANY($2)
         ORDER BY ear.created_at DESC NULLS LAST, eo.created_at DESC
         LIMIT 1",
    )
    .bind(market_id)
    .bind(&refs)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| {
        ApiError::internal(&format!("failed to load polymarket order context: {err}"))
    })?;

    let Some(row) = row else {
        return Ok(PolymarketOrderContextMatch::default());
    };

    Ok(PolymarketOrderContextMatch {
        agent_id: row.try_get("agent_id").ok().flatten(),
        run_id: row.try_get("run_id").ok().flatten(),
        external_order_id: row.try_get("external_order_id").ok().flatten(),
        owner: row.try_get("owner").ok().flatten(),
    })
}

async fn fetch_polymarket_relayer_transactions(
    state: &AppState,
    client: &reqwest::Client,
) -> Result<Vec<Value>, ApiError> {
    if !polymarket_builder_configured(&state.config) {
        return Ok(Vec::new());
    }

    let path = "/transactions";
    let headers = build_polymarket_builder_request_headers(&state.config, "GET", path)?;
    let mut request = client.get(polymarket_relayer_url(path));
    for (key, value) in headers {
        request = request.header(key, value);
    }

    let response = request
        .send()
        .await
        .map_err(|err| {
            ApiError::internal(&format!(
                "failed to fetch polymarket relayer transactions: {err}"
            ))
        })?
        .error_for_status()
        .map_err(|err| {
            ApiError::internal(&format!(
                "polymarket relayer transactions response failed: {err}"
            ))
        })?
        .json::<Value>()
        .await
        .map_err(|err| {
            ApiError::internal(&format!(
                "invalid polymarket relayer transactions payload: {err}"
            ))
        })?;

    Ok(response.as_array().cloned().unwrap_or_default())
}

async fn ingest_polymarket_user_trade_event(
    state: &AppState,
    market: &PolymarketTrackedMarket,
    payload: &Value,
    relayer_transactions: &[Value],
) -> Result<(), ApiError> {
    let relayer_tx = match_relayer_transaction(payload, relayer_transactions);
    let tx_hash = [
        parse_string_value(payload.get("transactionHash")),
        parse_string_value(payload.get("txHash")),
        relayer_tx
            .map(|tx| parse_string_value(tx.get("transactionHash")))
            .unwrap_or_default(),
    ]
    .into_iter()
    .find(|value| !value.is_empty());
    let lifecycle_status = best_polymarket_lifecycle_status(
        parse_polymarket_lifecycle_status(
            payload
                .get("status")
                .and_then(Value::as_str)
                .or_else(|| payload.get("state").and_then(Value::as_str)),
        ),
        relayer_tx.and_then(|tx| {
            parse_polymarket_lifecycle_status(tx.get("state").and_then(Value::as_str))
        }),
        tx_hash.as_deref(),
    );
    let refs = external::polymarket_index::reference_candidates_from_payload(payload);
    let context = load_polymarket_order_context_match(
        state,
        market.market_id.as_str(),
        &refs.provider_order_refs,
    )
    .await?;
    let outcome = parse_string_value(payload.get("outcome")).to_ascii_lowercase();
    let side = parse_string_value(payload.get("side")).to_ascii_lowercase();
    let fee_usdc = parse_f64_value(payload.get("feeUsdc"));
    let observed_at = parse_polymarket_observed_at(payload).or_else(|| {
        relayer_tx.and_then(|tx| {
            parse_polymarket_rfc3339(parse_string_value(tx.get("updatedAt")).as_str())
        })
    });

    external::polymarket_index::upsert_user_trade_event(
        &external::polymarket_index::PolymarketUserTradeEventUpsert {
            agent_id: context.agent_id,
            run_id: context.run_id,
            external_order_id: context.external_order_id,
            owner: context.owner.or_else(|| {
                Some(parse_string_value(payload.get("owner"))).filter(|value| !value.is_empty())
            }),
            market_id: market.market_id.clone(),
            provider_market_ref: Some(market.provider_market_ref.clone()),
            provider_order_id: refs.provider_order_refs.first().cloned(),
            builder_trade_id: refs.builder_trade_refs.first().cloned(),
            taker_hash: [
                parse_string_value(payload.get("taker_order_id")),
                parse_string_value(payload.get("takerOrderId")),
                parse_string_value(payload.get("takerOrderHash")),
                refs.provider_order_refs
                    .first()
                    .cloned()
                    .unwrap_or_default(),
            ]
            .into_iter()
            .find(|value| !value.is_empty()),
            tx_hash,
            block_number: None,
            outcome: (!outcome.is_empty()).then_some(outcome),
            side: (!side.is_empty()).then_some(side),
            price: Some(parse_f64_value(payload.get("price"))).filter(|value| *value > 0.0),
            requested_quantity: Some(parse_f64_value(payload.get("original_size")))
                .filter(|value| *value > 0.0),
            filled_quantity: Some(
                parse_f64_value(payload.get("size"))
                    .max(parse_f64_value(payload.get("matched_amount"))),
            )
            .filter(|value| *value > 0.0),
            fee_usdc: (fee_usdc > 0.0).then_some(fee_usdc),
            lifecycle_status,
            attempt_count: 0,
            last_error: payload
                .get("error")
                .or_else(|| payload.get("errorMsg"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            raw_payload: json!({
                "event": payload,
                "relayerTransaction": relayer_tx.cloned(),
            }),
            observed_at,
        },
    )
    .await?;

    Ok(())
}

async fn ingest_polymarket_user_trade_events(
    state: &AppState,
    tracked_markets: &[PolymarketTrackedMarket],
    user_events: &[Value],
    relayer_transactions: &[Value],
) -> Result<u64, ApiError> {
    if user_events.is_empty() {
        return Ok(0);
    }

    let by_condition = tracked_markets
        .iter()
        .map(|market| (market.condition_id.as_str(), market))
        .collect::<BTreeMap<_, _>>();
    let mut ingested = 0_u64;

    for payload in user_events {
        let event_type = parse_string_value(payload.get("event_type")).to_ascii_lowercase();
        let event_kind = parse_string_value(payload.get("type")).to_ascii_lowercase();
        let status = parse_string_value(payload.get("status")).to_ascii_uppercase();
        if !(event_type == "trade" || event_kind == "trade") {
            continue;
        }
        if !matches!(
            status.as_str(),
            "MATCHED" | "MINED" | "CONFIRMED" | "RETRYING" | "FAILED"
        ) {
            continue;
        }

        let condition_id = parse_string_value(payload.get("market"));
        let Some(market) = by_condition.get(condition_id.as_str()) else {
            continue;
        };

        ingest_polymarket_user_trade_event(state, market, payload, relayer_transactions).await?;
        ingested = ingested.saturating_add(1);
    }

    Ok(ingested)
}

async fn load_polymarket_tracked_market_refs(
    state: &AppState,
    requested_market_id: Option<&str>,
    max_markets: u64,
) -> Result<Vec<String>, ApiError> {
    let mut refs = BTreeSet::new();
    if let Some(requested) = requested_market_id {
        let normalized = normalize_polymarket_market_ref(requested);
        if !normalized.is_empty() {
            refs.insert(normalized);
        }
    } else {
        let agent_rows = sqlx::query(
            "SELECT DISTINCT market_id FROM external_agents WHERE provider = 'polymarket'",
        )
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| {
            ApiError::internal(&format!("failed to load tracked polymarket agents: {err}"))
        })?;
        for row in agent_rows {
            let market_id: String = row.get("market_id");
            refs.insert(normalize_polymarket_market_ref(market_id.as_str()));
        }

        let order_rows = sqlx::query(
            "SELECT DISTINCT market_id FROM external_orders WHERE provider = 'polymarket'",
        )
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| {
            ApiError::internal(&format!("failed to load tracked polymarket orders: {err}"))
        })?;
        for row in order_rows {
            let market_id: String = row.get("market_id");
            refs.insert(normalize_polymarket_market_ref(market_id.as_str()));
        }

        let mirror_rows = sqlx::query(
            "SELECT DISTINCT external_market_id FROM mirror_market_links WHERE LOWER(external_provider) = 'polymarket'",
        )
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&format!("failed to load tracked polymarket mirrors: {err}")))?;
        for row in mirror_rows {
            let market_id: String = row.get("external_market_id");
            refs.insert(normalize_polymarket_market_ref(market_id.as_str()));
        }
    }

    let mut values = refs
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    values.truncate(max_markets.max(1) as usize);
    Ok(values)
}

async fn load_polymarket_tracked_market(
    client: &reqwest::Client,
    config: &AppConfig,
    provider_market_ref: &str,
) -> Result<PolymarketTrackedMarket, ApiError> {
    let response = client
        .get(format!(
            "{}/markets/{}",
            config.polymarket_gamma_api_base.trim_end_matches('/'),
            provider_market_ref.trim()
        ))
        .send()
        .await
        .map_err(|err| ApiError::internal(&format!("failed to fetch polymarket market: {err}")))?
        .error_for_status()
        .map_err(|err| ApiError::internal(&format!("polymarket market response failed: {err}")))?
        .json::<Value>()
        .await
        .map_err(|err| ApiError::internal(&format!("invalid polymarket market payload: {err}")))?;

    let condition_id = parse_string_value(response.get("conditionId"));
    if condition_id.is_empty() {
        return Err(ApiError::internal(
            "polymarket market payload missing conditionId",
        ));
    }

    let token_ids = parse_string_list(response.get("clobTokenIds"));
    let outcomes = parse_string_list(response.get("outcomes"));
    let mut token_outcomes = BTreeMap::new();
    for (index, token_id) in token_ids.iter().enumerate() {
        let outcome = outcomes
            .get(index)
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_else(|| String::new());
        token_outcomes.insert(token_id.clone(), outcome);
    }

    Ok(PolymarketTrackedMarket {
        market_id: format!("polymarket:{}", provider_market_ref.trim()),
        provider_market_ref: provider_market_ref.trim().to_string(),
        condition_id,
        market_category: response
            .get("category")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase()),
        token_outcomes,
    })
}

fn tracked_market_details(
    tracked_markets: &[PolymarketTrackedMarket],
) -> Vec<PolymarketIndexerTrackedMarketResponse> {
    tracked_markets
        .iter()
        .map(|market| PolymarketIndexerTrackedMarketResponse {
            market_id: market.market_id.clone(),
            provider_market_ref: market.provider_market_ref.clone(),
            condition_id: market.condition_id.clone(),
        })
        .collect()
}

async fn fetch_polymarket_public_trade_page(
    client: &reqwest::Client,
    condition_id: &str,
    limit: u64,
    offset: u64,
) -> Result<Vec<Value>, ApiError> {
    let response = client
        .get("https://data-api.polymarket.com/trades")
        .query(&[
            ("market", condition_id),
            ("limit", &limit.to_string()),
            ("offset", &offset.to_string()),
        ])
        .send()
        .await
        .map_err(|err| {
            ApiError::internal(&format!("failed to fetch polymarket public trades: {err}"))
        })?;

    if response.status() == reqwest::StatusCode::BAD_REQUEST {
        return Ok(Vec::new());
    }

    let response = response
        .error_for_status()
        .map_err(|err| {
            ApiError::internal(&format!("polymarket public trades response failed: {err}"))
        })?
        .json::<Value>()
        .await
        .map_err(|err| {
            ApiError::internal(&format!("invalid polymarket public trades payload: {err}"))
        })?;

    Ok(response.as_array().cloned().unwrap_or_default())
}

async fn fetch_polymarket_builder_trade_page(
    state: &AppState,
    client: &reqwest::Client,
    condition_id: &str,
    next_cursor: &str,
) -> Result<(Vec<Value>, String), ApiError> {
    let path = "/builder/trades";
    let headers = build_polymarket_builder_request_headers(&state.config, "GET", path)?;
    let mut request = client
        .get(polymarket_clob_url(&state.config, path))
        .query(&[("market", condition_id), ("next_cursor", next_cursor)]);
    for (key, value) in headers {
        request = request.header(key, value);
    }

    let response = request
        .send()
        .await
        .map_err(|err| {
            ApiError::internal(&format!("failed to fetch polymarket builder trades: {err}"))
        })?
        .error_for_status()
        .map_err(|err| {
            ApiError::internal(&format!("polymarket builder trades response failed: {err}"))
        })?
        .json::<Value>()
        .await
        .map_err(|err| {
            ApiError::internal(&format!("invalid polymarket builder trades payload: {err}"))
        })?;

    let trades = response
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cursor = response
        .get("next_cursor")
        .or_else(|| response.get("nextCursor"))
        .and_then(Value::as_str)
        .unwrap_or("LTE=")
        .to_string();

    Ok((trades, cursor))
}

fn polymarket_public_trade_event_id(row: &Value) -> String {
    let tx_hash = parse_string_value(row.get("transactionHash"));
    let asset = parse_string_value(row.get("asset"));
    let timestamp = parse_i64_value(row.get("timestamp"));
    let side = parse_string_value(row.get("side")).to_ascii_lowercase();
    let price = parse_f64_value(row.get("price"));
    let size = parse_f64_value(row.get("size"));
    format!("{tx_hash}:{asset}:{timestamp}:{side}:{price:.8}:{size:.8}")
}

fn parse_polymarket_timestamp(value: i64) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::from_timestamp(value, 0)
}

async fn backfill_polymarket_public_trades_for_market(
    _state: &AppState,
    client: &reqwest::Client,
    market: &PolymarketTrackedMarket,
    cutoff: chrono::DateTime<Utc>,
    max_pages: u64,
) -> Result<u64, ApiError> {
    let mut inserted = 0_u64;
    let mut offset = 0_u64;
    let mut indexed_from: Option<chrono::DateTime<Utc>> = None;
    let mut indexed_through: Option<chrono::DateTime<Utc>> = None;
    let mut reached_cutoff = false;
    let mut exhausted = false;

    for _ in 0..max_pages.max(1) {
        let rows =
            fetch_polymarket_public_trade_page(client, market.condition_id.as_str(), 200, offset)
                .await?;
        if rows.is_empty() {
            exhausted = true;
            break;
        }

        let mut upserts = Vec::new();
        for row in &rows {
            let timestamp = parse_i64_value(row.get("timestamp"));
            let Some(match_time) = parse_polymarket_timestamp(timestamp) else {
                continue;
            };
            if match_time < cutoff {
                reached_cutoff = true;
                break;
            }

            indexed_from = Some(indexed_from.map_or(match_time, |current| current.min(match_time)));
            indexed_through =
                Some(indexed_through.map_or(match_time, |current| current.max(match_time)));

            let token_id = parse_string_value(row.get("asset"));
            let outcome = market
                .token_outcomes
                .get(token_id.as_str())
                .cloned()
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| parse_string_value(row.get("outcome")).to_ascii_lowercase());
            let side = parse_string_value(row.get("side")).to_ascii_lowercase();
            upserts.push(external::polymarket_index::PolymarketPublicTradeUpsert {
                provider_trade_id: polymarket_public_trade_event_id(row),
                market_id: market.market_id.clone(),
                provider_market_ref: market.provider_market_ref.clone(),
                market_category: market.market_category.clone(),
                outcome,
                side: (!side.is_empty()).then_some(side),
                price: parse_f64_value(row.get("price")),
                quantity: parse_f64_value(row.get("size")),
                tx_hash: Some(parse_string_value(row.get("transactionHash")))
                    .filter(|value| !value.is_empty()),
                block_number: None,
                token_id: (!token_id.is_empty()).then_some(token_id),
                maker: Some(parse_string_value(row.get("proxyWallet")))
                    .filter(|value| !value.is_empty()),
                taker: None,
                match_time,
                raw_payload: row.clone(),
            });
        }

        inserted = inserted
            .saturating_add(external::polymarket_index::upsert_public_trades(&upserts).await?);

        if reached_cutoff {
            break;
        }
        offset = offset.saturating_add(rows.len() as u64);
        if rows.len() < 200 {
            exhausted = true;
            break;
        }
    }

    external::polymarket_index::upsert_index_state(
        external::polymarket_index::PolymarketIndexLane::PublicTape,
        market.market_id.as_str(),
        market.provider_market_ref.as_str(),
        if exhausted || reached_cutoff {
            "ready"
        } else {
            "partial"
        },
        indexed_from,
        indexed_through,
        !(exhausted || reached_cutoff),
        None,
    )
    .await?;

    Ok(inserted)
}

async fn backfill_polymarket_builder_trades_for_market(
    state: &AppState,
    client: &reqwest::Client,
    market: &PolymarketTrackedMarket,
    cutoff: chrono::DateTime<Utc>,
    max_pages: u64,
    relayer_transactions: &[Value],
) -> Result<u64, ApiError> {
    if !polymarket_builder_configured(&state.config) {
        external::polymarket_index::upsert_index_state(
            external::polymarket_index::PolymarketIndexLane::UserFills,
            market.market_id.as_str(),
            market.provider_market_ref.as_str(),
            "missing_credentials",
            None,
            None,
            true,
            Some("builder credentials are not configured"),
        )
        .await?;
        return Ok(0);
    }

    let mut inserted = 0_u64;
    let mut cursor = "MA==".to_string();
    let mut indexed_from: Option<chrono::DateTime<Utc>> = None;
    let mut indexed_through: Option<chrono::DateTime<Utc>> = None;
    let mut reached_cutoff = false;
    let mut exhausted = false;

    for _ in 0..max_pages.max(1) {
        let (rows, next_cursor) = fetch_polymarket_builder_trade_page(
            state,
            client,
            market.condition_id.as_str(),
            cursor.as_str(),
        )
        .await?;
        if rows.is_empty() {
            exhausted = true;
            break;
        }

        for row in &rows {
            let match_time_text = parse_string_value(row.get("matchTime"));
            let Some(match_time) = chrono::DateTime::parse_from_rfc3339(match_time_text.as_str())
                .ok()
                .map(|value| value.with_timezone(&Utc))
            else {
                continue;
            };
            if match_time < cutoff {
                reached_cutoff = true;
                break;
            }

            indexed_from = Some(indexed_from.map_or(match_time, |current| current.min(match_time)));
            indexed_through =
                Some(indexed_through.map_or(match_time, |current| current.max(match_time)));

            let builder_trade_id = parse_string_value(row.get("id"));
            let taker_hash = parse_string_value(row.get("takerOrderHash"));
            let outcome = parse_string_value(row.get("outcome")).to_ascii_lowercase();
            let side = parse_string_value(row.get("side")).to_ascii_lowercase();
            let owner = parse_string_value(row.get("owner")).to_ascii_lowercase();
            let relayer_tx = match_relayer_transaction(row, relayer_transactions);
            let mut tx_hash = parse_string_value(row.get("transactionHash"));
            if tx_hash.is_empty() {
                tx_hash = relayer_tx
                    .map(|tx| parse_string_value(tx.get("transactionHash")))
                    .unwrap_or_default();
            }
            let tx_hash = (!tx_hash.is_empty()).then_some(tx_hash);
            let lifecycle_status = best_polymarket_lifecycle_status(
                parse_polymarket_lifecycle_status(
                    row.get("status")
                        .and_then(Value::as_str)
                        .or_else(|| row.get("state").and_then(Value::as_str)),
                ),
                relayer_tx.and_then(|tx| {
                    parse_polymarket_lifecycle_status(tx.get("state").and_then(Value::as_str))
                }),
                tx_hash.as_deref(),
            );
            let context = load_polymarket_order_context_match(
                state,
                market.market_id.as_str(),
                &[taker_hash.clone()],
            )
            .await?;

            let _ = external::polymarket_index::upsert_user_trade_event(
                &external::polymarket_index::PolymarketUserTradeEventUpsert {
                    agent_id: context.agent_id,
                    run_id: context.run_id,
                    external_order_id: context.external_order_id,
                    owner: context.owner.or((!owner.is_empty()).then_some(owner)),
                    market_id: market.market_id.clone(),
                    provider_market_ref: Some(market.provider_market_ref.clone()),
                    provider_order_id: (!taker_hash.is_empty()).then_some(taker_hash.clone()),
                    builder_trade_id: (!builder_trade_id.is_empty()).then_some(builder_trade_id),
                    taker_hash: (!taker_hash.is_empty()).then_some(taker_hash),
                    tx_hash,
                    block_number: None,
                    outcome: (!outcome.is_empty()).then_some(outcome),
                    side: (!side.is_empty()).then_some(side),
                    price: Some(parse_f64_value(row.get("price"))),
                    requested_quantity: None,
                    filled_quantity: Some(parse_f64_value(row.get("size"))),
                    fee_usdc: Some(parse_f64_value(row.get("feeUsdc"))),
                    lifecycle_status,
                    attempt_count: 0,
                    last_error: None,
                    raw_payload: json!({
                        "builderTrade": row,
                        "relayerTransaction": relayer_tx.cloned(),
                    }),
                    observed_at: Some(match_time),
                },
            )
            .await?;
            inserted = inserted.saturating_add(1);
        }

        if reached_cutoff || next_cursor == "LTE=" || next_cursor == cursor {
            exhausted = next_cursor == "LTE=" || next_cursor == cursor;
            break;
        }
        cursor = next_cursor;
    }

    external::polymarket_index::upsert_index_state(
        external::polymarket_index::PolymarketIndexLane::UserFills,
        market.market_id.as_str(),
        market.provider_market_ref.as_str(),
        if exhausted || reached_cutoff {
            "ready"
        } else {
            "partial"
        },
        indexed_from,
        indexed_through,
        !(exhausted || reached_cutoff),
        None,
    )
    .await?;

    Ok(inserted)
}

async fn reconcile_polymarket_relayer_lifecycle(
    state: &AppState,
    client: &reqwest::Client,
    relayer_rows: Option<&[Value]>,
) -> Result<u64, ApiError> {
    if !polymarket_builder_configured(&state.config) {
        return Ok(0);
    }

    let rows = if let Some(rows) = relayer_rows {
        rows.to_vec()
    } else {
        fetch_polymarket_relayer_transactions(state, client).await?
    };
    let mut updated = 0_u64;

    for row in rows {
        let tx_hash = parse_string_value(row.get("transactionHash")).to_ascii_lowercase();
        let owner = parse_string_value(row.get("owner")).to_ascii_lowercase();
        if tx_hash.is_empty() || owner.is_empty() {
            continue;
        }

        let lifecycle_status =
            map_polymarket_lifecycle_status(parse_string_value(row.get("state")).as_str());
        let observed_at = parse_string_value(row.get("updatedAt"));
        let observed_at = chrono::DateTime::parse_from_rfc3339(observed_at.as_str())
            .ok()
            .map(|value| value.with_timezone(&Utc));
        let last_error = matches!(
            lifecycle_status,
            external::polymarket_index::PolymarketTradeLifecycleStatus::Failed
        )
        .then(|| parse_string_value(row.get("state")))
        .filter(|value| !value.is_empty());

        let existing_rows = sqlx::query(
            r#"
            SELECT market_id, provider_market_ref, provider_order_id, builder_trade_id
            FROM polymarket_user_trade_events
            WHERE LOWER(owner) = LOWER($1) AND LOWER(tx_hash) = LOWER($2)
            "#,
        )
        .bind(owner.as_str())
        .bind(tx_hash.as_str())
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| {
            ApiError::internal(&format!(
                "failed to load polymarket user events for relayer reconciliation: {err}"
            ))
        })?;

        for existing in existing_rows {
            let market_id = existing
                .try_get::<String, _>("market_id")
                .unwrap_or_default();
            if market_id.is_empty() {
                continue;
            }
            let provider_market_ref = existing
                .try_get::<Option<String>, _>("provider_market_ref")
                .ok()
                .flatten();
            let provider_order_id = existing
                .try_get::<Option<String>, _>("provider_order_id")
                .ok()
                .flatten();
            let builder_trade_id = existing
                .try_get::<Option<String>, _>("builder_trade_id")
                .ok()
                .flatten();

            external::polymarket_index::upsert_user_trade_event(
                &external::polymarket_index::PolymarketUserTradeEventUpsert {
                    agent_id: None,
                    run_id: None,
                    external_order_id: None,
                    owner: Some(owner.clone()),
                    market_id,
                    provider_market_ref,
                    provider_order_id,
                    builder_trade_id,
                    taker_hash: None,
                    tx_hash: Some(tx_hash.clone()),
                    block_number: None,
                    outcome: None,
                    side: None,
                    price: None,
                    requested_quantity: None,
                    filled_quantity: None,
                    fee_usdc: None,
                    lifecycle_status,
                    attempt_count: 0,
                    last_error: last_error.clone(),
                    raw_payload: row.clone(),
                    observed_at,
                },
            )
            .await?;
            updated = updated.saturating_add(1);
        }
    }

    Ok(updated)
}

async fn load_polymarket_lane_health(
    state: &AppState,
    lane: external::polymarket_index::PolymarketIndexLane,
    tracked_market_ids: &[String],
) -> Result<PolymarketIndexerLaneHealth, ApiError> {
    let builder_configured = polymarket_builder_configured(&state.config);
    if tracked_market_ids.is_empty() {
        return Ok(PolymarketIndexerLaneHealth {
            lane: lane.as_str().to_string(),
            status: if lane == external::polymarket_index::PolymarketIndexLane::UserFills
                && !builder_configured
            {
                "missing_credentials".to_string()
            } else {
                "idle".to_string()
            },
            tracked_markets: 0,
            indexed_markets: 0,
            indexed_from: None,
            indexed_through: None,
            is_partial_backfill: true,
            last_error: None,
            updated_at: None,
            builder_configured: (lane
                == external::polymarket_index::PolymarketIndexLane::UserFills)
                .then_some(builder_configured),
            matched_events: None,
            mined_events: None,
            confirmed_events: None,
            retrying_events: None,
            failed_events: None,
            last_event_at: None,
        });
    }

    let rows = sqlx::query(
        "SELECT market_id, index_status, indexed_from, indexed_through, is_partial_backfill, last_error, updated_at
         FROM polymarket_index_state
         WHERE lane = $1 AND market_id = ANY($2)",
    )
    .bind(lane.as_str())
    .bind(tracked_market_ids)
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&format!("failed to load polymarket index lane health: {err}")))?;

    let indexed_markets = rows.len() as u64;
    let mut indexed_from: Option<chrono::DateTime<Utc>> = None;
    let mut indexed_through: Option<chrono::DateTime<Utc>> = None;
    let mut last_error = None;
    let mut updated_at: Option<chrono::DateTime<Utc>> = None;
    let mut partial = false;
    let mut failed = false;

    for row in rows {
        let row_indexed_from = row
            .try_get::<Option<chrono::DateTime<Utc>>, _>("indexed_from")
            .ok()
            .flatten();
        let row_indexed_through = row
            .try_get::<Option<chrono::DateTime<Utc>>, _>("indexed_through")
            .ok()
            .flatten();
        let row_updated_at = row
            .try_get::<Option<chrono::DateTime<Utc>>, _>("updated_at")
            .ok()
            .flatten();
        let row_error = row
            .try_get::<Option<String>, _>("last_error")
            .ok()
            .flatten();
        let row_status = row
            .try_get::<String, _>("index_status")
            .unwrap_or_else(|_| "pending".to_string());
        partial |= row
            .try_get::<bool, _>("is_partial_backfill")
            .unwrap_or(true);
        failed |= matches!(row_status.as_str(), "failed" | "error");
        if last_error.is_none() {
            last_error = row_error;
        }
        indexed_from = match (indexed_from, row_indexed_from) {
            (Some(current), Some(row)) => Some(current.min(row)),
            (None, Some(row)) => Some(row),
            (Some(current), None) => Some(current),
            (None, None) => None,
        };
        indexed_through = match (indexed_through, row_indexed_through) {
            (Some(current), Some(row)) => Some(current.max(row)),
            (None, Some(row)) => Some(row),
            (Some(current), None) => Some(current),
            (None, None) => None,
        };
        updated_at = match (updated_at, row_updated_at) {
            (Some(current), Some(row)) => Some(current.max(row)),
            (None, Some(row)) => Some(row),
            (Some(current), None) => Some(current),
            (None, None) => None,
        };
    }

    let mut status = if lane == external::polymarket_index::PolymarketIndexLane::UserFills
        && !builder_configured
    {
        "missing_credentials"
    } else if failed && indexed_markets == 0 {
        "error"
    } else if failed {
        "partial"
    } else if indexed_markets == 0 {
        "pending"
    } else if partial || indexed_markets < tracked_market_ids.len() as u64 {
        "partial"
    } else {
        "ready"
    }
    .to_string();

    if status == "pending" && last_error.is_some() {
        status = "error".to_string();
    }

    let (
        matched_events,
        mined_events,
        confirmed_events,
        retrying_events,
        failed_events,
        last_event_at,
    ) = if lane == external::polymarket_index::PolymarketIndexLane::UserFills {
        let lifecycle_rows = sqlx::query(
            r#"
            SELECT lifecycle_status, COUNT(*) AS count
            FROM polymarket_user_trade_events
            WHERE market_id = ANY($1)
            GROUP BY lifecycle_status
            "#,
        )
        .bind(tracked_market_ids)
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| {
            ApiError::internal(&format!(
                "failed to load polymarket user event lifecycle counts: {err}"
            ))
        })?;

        let mut matched = 0_u64;
        let mut mined = 0_u64;
        let mut confirmed = 0_u64;
        let mut retrying = 0_u64;
        let mut failed_events = 0_u64;
        for row in lifecycle_rows {
            let count = row.try_get::<i64, _>("count").unwrap_or_default().max(0) as u64;
            match row
                .try_get::<String, _>("lifecycle_status")
                .unwrap_or_default()
                .as_str()
            {
                "MATCHED" => matched = count,
                "MINED" => mined = count,
                "CONFIRMED" => confirmed = count,
                "RETRYING" => retrying = count,
                "FAILED" => failed_events = count,
                _ => {}
            }
        }

        let last_event_at = sqlx::query(
            r#"
            SELECT MAX(COALESCE(confirmed_at, mined_at, matched_at, updated_at)) AS last_event_at
            FROM polymarket_user_trade_events
            WHERE market_id = ANY($1)
            "#,
        )
        .bind(tracked_market_ids)
        .fetch_one(state.db.pool())
        .await
        .ok()
        .and_then(|row| {
            row.try_get::<Option<chrono::DateTime<Utc>>, _>("last_event_at")
                .ok()
        })
        .flatten()
        .map(|value| value.to_rfc3339());

        (
            Some(matched),
            Some(mined),
            Some(confirmed),
            Some(retrying),
            Some(failed_events),
            last_event_at,
        )
    } else {
        (None, None, None, None, None, None)
    };

    Ok(PolymarketIndexerLaneHealth {
        lane: lane.as_str().to_string(),
        status,
        tracked_markets: tracked_market_ids.len() as u64,
        indexed_markets,
        indexed_from: indexed_from.map(|value| value.to_rfc3339()),
        indexed_through: indexed_through.map(|value| value.to_rfc3339()),
        is_partial_backfill: partial || indexed_markets < tracked_market_ids.len() as u64,
        last_error,
        updated_at: updated_at.map(|value| value.to_rfc3339()),
        builder_configured: (lane == external::polymarket_index::PolymarketIndexLane::UserFills)
            .then_some(builder_configured),
        matched_events,
        mined_events,
        confirmed_events,
        retrying_events,
        failed_events,
        last_event_at,
    })
}

fn build_polymarket_request_headers(
    state: &AppState,
    credential: &StoredCredential,
    method: &str,
    path: &str,
    body: &str,
) -> Result<BTreeMap<String, String>, ApiError> {
    let credentials = polymarket_credentials(credential)?;
    let owner = normalize_evm_wallet(credential.owner.as_str())?.to_ascii_lowercase();
    let timestamp = Utc::now().timestamp().to_string();
    let signature = polymarket_l2_signature(
        credentials.api_secret.as_str(),
        method,
        path,
        body,
        timestamp.as_str(),
    )?;
    let mut headers = BTreeMap::from([
        ("Content-Type".to_string(), "application/json".to_string()),
        ("POLY_ADDRESS".to_string(), owner),
        ("POLY_API_KEY".to_string(), credentials.api_key),
        ("POLY_PASSPHRASE".to_string(), credentials.api_passphrase),
        ("POLY_SIGNATURE".to_string(), signature),
        ("POLY_TIMESTAMP".to_string(), timestamp),
    ]);

    if let Some(builder_credentials) = polymarket_builder_credentials(&state.config) {
        let builder_headers = polymarket_builder_headers(
            builder_credentials,
            method,
            path,
            body,
            headers
                .get("POLY_TIMESTAMP")
                .map(String::as_str)
                .unwrap_or_default(),
        )?;
        insert_polymarket_builder_headers(&mut headers, &builder_headers);
    }

    Ok(headers)
}

fn polymarket_forwarder(config: &AppConfig) -> Option<PolymarketForwarder<'_>> {
    let url = config.polymarket_forwarder_url.trim();
    let shared_secret = config.polymarket_forwarder_shared_secret.trim();
    if url.is_empty() || shared_secret.is_empty() {
        return None;
    }

    Some(PolymarketForwarder { url, shared_secret })
}

fn insert_polymarket_builder_headers(
    headers: &mut BTreeMap<String, String>,
    builder_headers: &PolymarketBuilderHeaders,
) {
    headers.insert(
        "POLY_BUILDER_API_KEY".to_string(),
        builder_headers.api_key.clone(),
    );
    headers.insert(
        "POLY_BUILDER_PASSPHRASE".to_string(),
        builder_headers.api_passphrase.clone(),
    );
    headers.insert(
        "POLY_BUILDER_SIGNATURE".to_string(),
        builder_headers.signature.clone(),
    );
    headers.insert(
        "POLY_BUILDER_TIMESTAMP".to_string(),
        builder_headers.timestamp.clone(),
    );
}

fn prepare_polymarket_provider_request(
    state: &AppState,
    credential: &StoredCredential,
    method: &str,
    path: &str,
    payload: &Value,
) -> Result<PreparedExternalProviderRequestResponse, ApiError> {
    let body = polymarket_request_body(payload)?;
    let headers = build_polymarket_request_headers(state, credential, method, path, body.as_str())?;

    Ok(PreparedExternalProviderRequestResponse {
        provider: ExternalProvider::Polymarket.as_str().to_string(),
        url: polymarket_clob_url(&state.config, path),
        method: method.to_string(),
        headers,
        body,
    })
}

fn polymarket_provider_error_message<'a>(payload: &'a Value, fallback: &'a str) -> &'a str {
    if let Some(raw) = payload.as_str() {
        return raw;
    }

    payload
        .get("errorMsg")
        .or_else(|| payload.get("message"))
        .or_else(|| payload.get("error"))
        .and_then(|value| value.as_str())
        .unwrap_or(fallback)
}

fn provider_order_id(payload: &Value) -> String {
    payload
        .get("orderId")
        .or_else(|| payload.get("orderID"))
        .or_else(|| payload.get("id"))
        .or_else(|| payload.get("order_id"))
        .or_else(|| payload.get("order").and_then(|value| value.get("orderId")))
        .or_else(|| payload.get("order").and_then(|value| value.get("orderID")))
        .or_else(|| payload.get("order").and_then(|value| value.get("id")))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

async fn execute_polymarket_request(
    state: &AppState,
    method: &str,
    path: &str,
    headers: BTreeMap<String, String>,
    body: String,
    transport_error: &str,
    provider_error_code: &str,
    provider_error_fallback: &str,
) -> Result<Value, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let response = if let Some(forwarder) = polymarket_forwarder(&state.config) {
        client
            .post(format!("{}/forward", forwarder.url.trim_end_matches('/')))
            .header(
                "Authorization",
                format!("Bearer {}", forwarder.shared_secret),
            )
            .json(&PolymarketForwardRequest {
                method: method.to_string(),
                path: path.to_string(),
                headers,
                body,
            })
            .send()
            .await
            .map_err(|err| ApiError::internal(&format!("{transport_error}: {err}")))?
    } else {
        let mut request = match method {
            "POST" => client.post(format!(
                "{}{}",
                state.config.polymarket_clob_api_base.trim_end_matches('/'),
                path
            )),
            "DELETE" => client.delete(format!(
                "{}{}",
                state.config.polymarket_clob_api_base.trim_end_matches('/'),
                path
            )),
            _ => {
                return Err(ApiError::internal("unsupported polymarket request method"));
            }
        };
        for (name, value) in headers {
            request = request.header(name, value);
        }
        request
            .body(body)
            .send()
            .await
            .map_err(|err| ApiError::internal(&format!("{transport_error}: {err}")))?
    };

    let status = response.status();
    let payload = response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| json!({ "ok": status.is_success() }));

    if !status.is_success() {
        return Err(ApiError::bad_request(
            provider_error_code,
            polymarket_provider_error_message(&payload, provider_error_fallback),
        ));
    }

    Ok(payload)
}

async fn check_polymarket_auth_status(
    state: &AppState,
    owner: &str,
    api_key: &str,
    api_secret: &str,
    api_passphrase: &str,
) -> String {
    let address = match normalize_evm_wallet(owner) {
        Ok(value) => value.to_ascii_lowercase(),
        Err(_) => return "invalid_owner".to_string(),
    };

    let decoded_secret = match URL_SAFE.decode(api_secret.trim()) {
        Ok(value) => value,
        Err(_) => return "invalid_credentials".to_string(),
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(_) => return "unavailable".to_string(),
    };

    let timestamp = Utc::now().timestamp().to_string();
    let path = "/auth/api-keys";
    let message = format!("{}GET{}", timestamp, path);
    let mut mac = match Hmac::<Sha256>::new_from_slice(&decoded_secret) {
        Ok(value) => value,
        Err(_) => return "invalid_credentials".to_string(),
    };
    mac.update(message.as_bytes());
    let signature = URL_SAFE.encode(mac.finalize().into_bytes());

    let response = match client
        .get(format!(
            "{}{}",
            state.config.polymarket_clob_api_base.trim_end_matches('/'),
            path
        ))
        .header("POLY_ADDRESS", address)
        .header("POLY_API_KEY", api_key.trim())
        .header("POLY_PASSPHRASE", api_passphrase.trim())
        .header("POLY_SIGNATURE", signature)
        .header("POLY_TIMESTAMP", timestamp)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return "unavailable".to_string(),
    };

    match response.status().as_u16() {
        200..=299 => "ready".to_string(),
        401 | 403 => "invalid_credentials".to_string(),
        _ => "unavailable".to_string(),
    }
}

fn normalize_evm_wallet(raw: &str) -> Result<String, ApiError> {
    let wallet = raw.trim();
    if wallet.len() != 42
        || !wallet.starts_with("0x")
        || !wallet[2..].chars().all(|value| value.is_ascii_hexdigit())
    {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "baseWallet must be a valid 0x EVM address",
        ));
    }

    let lower = wallet[2..].to_ascii_lowercase();
    let mut hasher = Keccak256::new();
    hasher.update(lower.as_bytes());
    let hash = hasher.finalize();

    let mut checksummed = String::with_capacity(wallet.len());
    checksummed.push_str("0x");
    for (idx, ch) in lower.chars().enumerate() {
        if ch.is_ascii_digit() {
            checksummed.push(ch);
            continue;
        }

        let hash_byte = hash[idx / 2];
        let nibble = if idx % 2 == 0 {
            hash_byte >> 4
        } else {
            hash_byte & 0x0f
        };

        if nibble >= 8 {
            checksummed.push(ch.to_ascii_uppercase());
        } else {
            checksummed.push(ch);
        }
    }

    Ok(checksummed)
}

async fn check_limitless_profile_status(
    state: &AppState,
    base_wallet: &str,
    api_key: &str,
) -> String {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(_) => return "unavailable".to_string(),
    };

    let response = match client
        .get(format!(
            "{}/profiles/{}",
            state.config.limitless_api_base.trim_end_matches('/'),
            base_wallet
        ))
        .header("X-API-Key", api_key)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return "unavailable".to_string(),
    };

    match response.status().as_u16() {
        200..=299 => "ready".to_string(),
        401 | 403 => "invalid_api_key".to_string(),
        404 => "missing_profile".to_string(),
        _ => "unavailable".to_string(),
    }
}

async fn fetch_limitless_profile(
    state: &AppState,
    base_wallet: &str,
    api_key: &str,
) -> Result<LimitlessProfile, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let response = client
        .get(format!(
            "{}/profiles/{}",
            state.config.limitless_api_base.trim_end_matches('/'),
            base_wallet
        ))
        .header("X-API-Key", api_key)
        .send()
        .await
        .map_err(|err| ApiError::internal(&format!("failed to load limitless profile: {}", err)))?;

    match response.status().as_u16() {
        200..=299 => response.json::<LimitlessProfile>().await.map_err(|err| {
            ApiError::internal(&format!("invalid limitless profile payload: {}", err))
        }),
        401 | 403 => Err(ApiError::bad_request(
            "INVALID_CREDENTIALS",
            "Limitless rejected the stored API key",
        )),
        404 => Err(ApiError::bad_request(
            "LIMITLESS_PROFILE_MISSING",
            "Limitless profile not found for the bound wallet",
        )),
        _ => Err(ApiError::internal("failed to load limitless profile")),
    }
}

async fn build_external_credential_status(
    state: &AppState,
    provider: ExternalProvider,
    credential: Option<&StoredCredential>,
) -> Result<ExternalCredentialStatusResponse, ApiError> {
    let mut checks = Vec::new();
    let mut ready = true;
    let mut base_wallet = None;
    let mut profile_status = None;

    let Some(credential) = credential else {
        return Ok(ExternalCredentialStatusResponse {
            provider: provider.as_str().to_string(),
            credential_id: None,
            ready: false,
            base_wallet: None,
            profile_status: None,
            checks: vec![ExternalCredentialCheck {
                code: "credential".to_string(),
                ok: false,
                message: "No active credential saved for this provider.".to_string(),
            }],
        });
    };

    match provider {
        ExternalProvider::Limitless => {
            let api_key = payload_string(&credential.payload, &["apiKey", "api_key"]);
            let api_key_ok = api_key.is_some();
            ready &= api_key_ok;
            checks.push(ExternalCredentialCheck {
                code: "api_key".to_string(),
                ok: api_key_ok,
                message: if api_key_ok {
                    "API key is present.".to_string()
                } else {
                    "Limitless credentials must include apiKey.".to_string()
                },
            });

            let wallet_raw = payload_string(&credential.payload, &["baseWallet", "base_wallet"]);
            match wallet_raw {
                Some(raw) => match normalize_evm_wallet(raw.as_str()) {
                    Ok(normalized) => {
                        base_wallet = Some(normalized.clone());
                        checks.push(ExternalCredentialCheck {
                            code: "base_wallet".to_string(),
                            ok: true,
                            message: "Trading wallet is bound.".to_string(),
                        });

                        let profile = if let Some(api_key) = api_key.as_deref() {
                            check_limitless_profile_status(state, normalized.as_str(), api_key)
                                .await
                        } else {
                            "missing_api_key".to_string()
                        };
                        let profile_ok = profile == "ready";
                        ready &= profile_ok;
                        profile_status = Some(profile.clone());
                        checks.push(ExternalCredentialCheck {
                            code: "profile".to_string(),
                            ok: profile_ok,
                            message: match profile.as_str() {
                                "ready" => {
                                    "Limitless profile is active for the bound wallet.".to_string()
                                }
                                "missing_profile" => {
                                    "Limitless profile not found for the bound wallet.".to_string()
                                }
                                "invalid_api_key" => {
                                    "Limitless rejected the stored API key.".to_string()
                                }
                                _ => {
                                    "Limitless profile check is unavailable right now.".to_string()
                                }
                            },
                        });
                    }
                    Err(_) => {
                        ready = false;
                        checks.push(ExternalCredentialCheck {
                            code: "base_wallet".to_string(),
                            ok: false,
                            message: "Stored baseWallet is invalid.".to_string(),
                        });
                    }
                },
                None => {
                    ready = false;
                    checks.push(ExternalCredentialCheck {
                        code: "base_wallet".to_string(),
                        ok: false,
                        message: "Bind a Base wallet before using Limitless.".to_string(),
                    });
                }
            }
        }
        ExternalProvider::Polymarket => {
            let api_key = payload_string(&credential.payload, &["apiKey", "api_key"]);
            let api_secret = payload_string(&credential.payload, &["apiSecret", "api_secret"]);
            let api_passphrase =
                payload_string(&credential.payload, &["apiPassphrase", "api_passphrase"]);
            let api_key_ok = api_key.is_some();
            let api_secret_ok = api_secret.is_some();
            let api_passphrase_ok = api_passphrase.is_some();
            checks.push(ExternalCredentialCheck {
                code: "api_key".to_string(),
                ok: api_key_ok,
                message: if api_key_ok {
                    "API key is present.".to_string()
                } else {
                    "Polymarket credentials must include apiKey.".to_string()
                },
            });
            checks.push(ExternalCredentialCheck {
                code: "api_secret".to_string(),
                ok: api_secret_ok,
                message: if api_secret_ok {
                    "API secret is present.".to_string()
                } else {
                    "Polymarket credentials must include apiSecret.".to_string()
                },
            });
            checks.push(ExternalCredentialCheck {
                code: "api_passphrase".to_string(),
                ok: api_passphrase_ok,
                message: if api_passphrase_ok {
                    "API passphrase is present.".to_string()
                } else {
                    "Polymarket credentials must include apiPassphrase.".to_string()
                },
            });
            let funder_ok = payload_string(&credential.payload, &["funder"])
                .as_deref()
                .map(|value| normalize_evm_wallet(value).is_ok())
                .unwrap_or(false);
            checks.push(ExternalCredentialCheck {
                code: "funder".to_string(),
                ok: funder_ok,
                message: if funder_ok {
                    "Funder wallet is present.".to_string()
                } else {
                    "Polymarket credentials must include a valid funder wallet.".to_string()
                },
            });
            let signature_type = polymarket_signature_type_from_payload(&credential.payload).ok();
            checks.push(ExternalCredentialCheck {
                code: "signature_type".to_string(),
                ok: signature_type.is_some(),
                message: signature_type
                    .map(|value| {
                        format!(
                            "Signature type is set to {}.",
                            polymarket_signature_type_label(value)
                        )
                    })
                    .unwrap_or_else(|| {
                        "Polymarket credentials must include signatureType (0, 1, or 2)."
                            .to_string()
                    }),
            });
            let signing_path_ok = signature_type.map(|value| value != 1).unwrap_or(false);
            checks.push(ExternalCredentialCheck {
                code: "signing_path".to_string(),
                ok: signing_path_ok,
                message: if signing_path_ok {
                    "Connected-wallet signing supports this Polymarket account type."
                        .to_string()
                } else {
                    "Magic/email Polymarket accounts are not supported by the connected-wallet signing flow in this build.".to_string()
                },
            });
            let auth_ready = if let (Some(api_key), Some(api_secret), Some(api_passphrase)) = (
                api_key.as_deref(),
                api_secret.as_deref(),
                api_passphrase.as_deref(),
            ) {
                let auth_status = check_polymarket_auth_status(
                    state,
                    credential.owner.as_str(),
                    api_key,
                    api_secret,
                    api_passphrase,
                )
                .await;
                let auth_ready = auth_status == "ready";
                checks.push(ExternalCredentialCheck {
                    code: "auth".to_string(),
                    ok: auth_ready,
                    message: polymarket_authenticated_message(auth_status.as_str()),
                });
                auth_ready
            } else {
                checks.push(ExternalCredentialCheck {
                    code: "auth".to_string(),
                    ok: false,
                    message: "Polymarket auth check is blocked until apiKey, apiSecret, and apiPassphrase are all present.".to_string(),
                });
                false
            };
            let execution_ready = api_key_ok
                && api_secret_ok
                && api_passphrase_ok
                && funder_ok
                && signature_type.is_some()
                && auth_ready
                && signing_path_ok;
            ready = execution_ready;
            checks.push(ExternalCredentialCheck {
                code: "execution".to_string(),
                ok: execution_ready,
                message: if execution_ready {
                    "Polymarket CLOB execution path is available.".to_string()
                } else {
                    polymarket_live_execution_message().to_string()
                },
            });
        }
        ExternalProvider::Aerodrome => {
            let wallet_raw = payload_string(&credential.payload, &["baseWallet", "base_wallet"]);
            let wallet_ok = match wallet_raw {
                Some(raw) => match normalize_evm_wallet(raw.as_str()) {
                    Ok(normalized) => {
                        base_wallet = Some(normalized);
                        checks.push(ExternalCredentialCheck {
                            code: "base_wallet".to_string(),
                            ok: true,
                            message: "Base wallet is bound for Aerodrome swaps.".to_string(),
                        });
                        true
                    }
                    Err(_) => {
                        checks.push(ExternalCredentialCheck {
                            code: "base_wallet".to_string(),
                            ok: false,
                            message: "Stored baseWallet is invalid.".to_string(),
                        });
                        false
                    }
                },
                None => {
                    checks.push(ExternalCredentialCheck {
                        code: "base_wallet".to_string(),
                        ok: false,
                        message: "Aerodrome credential must include baseWallet.".to_string(),
                    });
                    false
                }
            };
            let has_private_key =
                payload_string(&credential.payload, &["privateKey", "private_key"]).is_some();
            checks.push(ExternalCredentialCheck {
                code: "private_key".to_string(),
                ok: has_private_key,
                message: if has_private_key {
                    "Private key is stored for autonomous execution.".to_string()
                } else {
                    "privateKey is required for autonomous Aerodrome swap execution.".to_string()
                },
            });
            ready = wallet_ok && has_private_key;
        }
    }

    Ok(ExternalCredentialStatusResponse {
        provider: provider.as_str().to_string(),
        credential_id: Some(credential.id.clone()),
        ready,
        base_wallet,
        profile_status,
        checks,
    })
}

async fn ensure_provider_credential_ready(
    state: &AppState,
    provider: ExternalProvider,
    credential: &StoredCredential,
) -> Result<ExternalCredentialStatusResponse, ApiError> {
    let status = build_external_credential_status(state, provider, Some(credential)).await?;
    if status.ready {
        return Ok(status);
    }

    let reason = status
        .checks
        .iter()
        .find(|check| !check.ok)
        .map(|check| check.message.clone())
        .unwrap_or_else(|| "credential is not ready".to_string());

    Err(ApiError::bad_request(
        "CREDENTIAL_NOT_READY",
        reason.as_str(),
    ))
}

fn build_preflight(provider: ExternalProvider, market: &Value) -> Value {
    match provider {
        ExternalProvider::Limitless => {
            let venue_exchange = market
                .get("venue")
                .and_then(|value| value.get("exchange"))
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            json!({
                "chainId": 8453,
                "mode": "manual",
                "checks": [
                    {
                        "type": "funding",
                        "token": "USDC",
                        "chainId": 8453,
                        "required": true,
                        "message": "Ensure the trading wallet is funded with Base USDC"
                    },
                    {
                        "type": "approval",
                        "token": "USDC",
                        "spender": venue_exchange,
                        "required": true,
                        "message": "Approve venue exchange to spend USDC"
                    }
                ]
            })
        }
        ExternalProvider::Polymarket => json!({
            "chainId": 137,
            "mode": "manual",
            "checks": [
                {
                    "type": "funding",
                    "token": "USDC",
                    "chainId": 137,
                    "required": true,
                    "message": "Fund Polygon wallet for Polymarket execution"
                },
                {
                    "type": "approval",
                    "token": "USDC",
                    "required": true,
                    "message": "Set required CLOB allowance(s) before trading"
                }
            ]
        }),
        ExternalProvider::Aerodrome => json!({
            "chainId": 8453,
            "mode": "swap",
            "checks": [
                {
                    "type": "funding",
                    "token": "USDC",
                    "chainId": 8453,
                    "required": true,
                    "message": "Fund Base wallet with USDC for Aerodrome swap"
                },
                {
                    "type": "approval",
                    "token": "USDC",
                    "spender": "aerodrome_swap_router",
                    "required": true,
                    "message": "Approve Aerodrome SwapRouter to spend USDC"
                }
            ]
        }),
    }
}

fn scale_limitless_decimal(value: f64, field: &str) -> Result<u128, ApiError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ApiError::bad_request(
            "INVALID_EXTERNAL_ORDER",
            format!("{} must be greater than zero", field).as_str(),
        ));
    }

    let normalized = format!("{:.6}", value);
    let mut parts = normalized.split('.');
    let whole = parts.next().unwrap_or("0");
    let fractional = parts.next().unwrap_or("0");
    let padded_fractional = format!("{:0<6}", fractional);
    let raw = format!("{}{}", whole, &padded_fractional[..6]);

    raw.parse::<u128>().map_err(|_| {
        ApiError::bad_request(
            "INVALID_EXTERNAL_ORDER",
            format!("{} could not be normalized", field).as_str(),
        )
    })
}

fn div_ceil_u128(numerator: u128, denominator: u128) -> Result<u128, ApiError> {
    if denominator == 0 {
        return Err(ApiError::internal(
            "limitless order math used zero denominator",
        ));
    }
    Ok((numerator + denominator - 1) / denominator)
}

fn build_limitless_order_message(
    owner: &str,
    outcome: &str,
    side: &str,
    price: f64,
    quantity: f64,
    token_id: &str,
    fee_rate_bps: u64,
) -> Result<Value, ApiError> {
    let price_int = scale_limitless_decimal(price, "price")?;
    if price_int >= LIMITLESS_SCALE {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "price must be between 0 and 1",
        ));
    }
    if price_int % LIMITLESS_PRICE_TICK_INT != 0 {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "price must align to 0.001 increments for Limitless",
        ));
    }

    let shares = scale_limitless_decimal(quantity, "quantity")?;
    let shares_step = LIMITLESS_SCALE / LIMITLESS_PRICE_TICK_INT;
    if shares % shares_step != 0 {
        return Err(ApiError::bad_request(
            "INVALID_QUANTITY",
            "quantity must align to the venue share step for Limitless",
        ));
    }

    let numerator = shares
        .checked_mul(price_int)
        .and_then(|value| value.checked_mul(LIMITLESS_SCALE))
        .ok_or_else(|| ApiError::internal("limitless order amount overflow"))?;
    let denominator = LIMITLESS_SCALE
        .checked_mul(LIMITLESS_SCALE)
        .ok_or_else(|| ApiError::internal("limitless order scale overflow"))?;

    let side_value = match side {
        "buy" => 0u64,
        "sell" => 1u64,
        _ => {
            return Err(ApiError::bad_request(
                "INVALID_SIDE",
                "side must be one of: buy, sell",
            ))
        }
    };

    let collateral = if side_value == 0 {
        div_ceil_u128(numerator, denominator)?
    } else {
        numerator / denominator
    };

    let (maker_amount, taker_amount) = if side_value == 0 {
        (collateral, shares)
    } else {
        (shares, collateral)
    };

    let salt = Utc::now().timestamp_micros().max(0) as u128
        + (Uuid::new_v4().as_u128() % 1_000_000)
        + 86_400_000u128;

    let _ = outcome;

    Ok(json!({
        "salt": salt.to_string(),
        "maker": owner,
        "signer": owner,
        "taker": LIMITLESS_ZERO_ADDRESS,
        "tokenId": token_id,
        "makerAmount": maker_amount.to_string(),
        "takerAmount": taker_amount.to_string(),
        "expiration": "0",
        "nonce": "0",
        "feeRateBps": fee_rate_bps.to_string(),
        "side": side_value,
        "signatureType": 0,
    }))
}

fn extract_limitless_exchange(market: &Value) -> Result<String, ApiError> {
    let exchange = market
        .get("venue")
        .and_then(|value| value.get("exchange"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            ApiError::bad_request(
                "LIMITLESS_MARKET_UNAVAILABLE",
                "Limitless market payload did not include a venue exchange address",
            )
        })?;
    normalize_evm_wallet(exchange)
}

fn extract_limitless_token_id(market: &Value, outcome: &str) -> Result<String, ApiError> {
    if let Some(token_id) = market
        .get("tokens")
        .and_then(|value| value.get(outcome))
        .and_then(|value| value.as_str())
    {
        return Ok(token_id.to_string());
    }

    let index = if outcome == "yes" { 0 } else { 1 };
    if let Some(token_id) = market
        .get("positionIds")
        .and_then(|value| value.as_array())
        .and_then(|value| value.get(index))
        .and_then(|value| value.as_str())
    {
        return Ok(token_id.to_string());
    }

    Err(ApiError::bad_request(
        "LIMITLESS_MARKET_UNAVAILABLE",
        "Limitless market payload did not include outcome token ids",
    ))
}

fn build_limitless_typed_data(
    owner: &str,
    request: &CreateExternalOrderIntentRequest,
    market: &Value,
    fee_rate_bps: u64,
) -> Result<Value, ApiError> {
    let contract_address = extract_limitless_exchange(market)?;
    let token_id = extract_limitless_token_id(market, request.outcome.as_str())?;
    let message = build_limitless_order_message(
        owner,
        request.outcome.as_str(),
        request.side.as_str(),
        request.price,
        request.quantity,
        token_id.as_str(),
        fee_rate_bps,
    )?;

    Ok(json!({
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "version", "type": "string" },
                { "name": "chainId", "type": "uint256" },
                { "name": "verifyingContract", "type": "address" }
            ],
            "Order": [
                { "name": "salt", "type": "uint256" },
                { "name": "maker", "type": "address" },
                { "name": "signer", "type": "address" },
                { "name": "taker", "type": "address" },
                { "name": "tokenId", "type": "uint256" },
                { "name": "makerAmount", "type": "uint256" },
                { "name": "takerAmount", "type": "uint256" },
                { "name": "expiration", "type": "uint256" },
                { "name": "nonce", "type": "uint256" },
                { "name": "feeRateBps", "type": "uint256" },
                { "name": "side", "type": "uint8" },
                { "name": "signatureType", "type": "uint8" }
            ]
        },
        "domain": {
            "name": LIMITLESS_SIGNING_NAME,
            "version": LIMITLESS_SIGNING_VERSION,
            "chainId": 8453,
            "verifyingContract": contract_address,
        },
        "primaryType": "Order",
        "message": message,
    }))
}

fn build_typed_data(
    owner: &str,
    provider: ExternalProvider,
    request: &CreateExternalOrderIntentRequest,
    _market_ref: &str,
    provider_market_payload: &Value,
    fee_rate_bps: Option<u64>,
) -> Result<Value, ApiError> {
    match provider {
        ExternalProvider::Limitless => {
            let owner = normalize_evm_wallet(owner)?;
            build_limitless_typed_data(
                owner.as_str(),
                request,
                provider_market_payload,
                fee_rate_bps.unwrap_or(300),
            )
        }
        ExternalProvider::Polymarket => Err(ApiError::internal(
            "polymarket typed data must be built through the provider-specific path",
        )),
        ExternalProvider::Aerodrome => Err(ApiError::bad_request(
            "AERODROME_NO_TYPED_DATA",
            "aerodrome uses on-chain swaps, not signed typed data",
        )),
    }
}

async fn build_limitless_submit_payload(
    state: &AppState,
    credential: &StoredCredential,
    market_id: &str,
    provider_market_ref: &str,
    price: f64,
    typed_data: &Value,
    signed_order: &Value,
) -> Result<Value, ApiError> {
    if signed_order.get("order").is_some() {
        let mut payload = signed_order.clone();
        let market_slug = provider_market_ref
            .trim()
            .strip_prefix("limitless:")
            .unwrap_or(provider_market_ref.trim());
        if let Some(object) = payload.as_object_mut() {
            object
                .entry("marketSlug".to_string())
                .or_insert_with(|| json!(market_slug));
            if !object.contains_key("ownerId") {
                let api_key = api_key_from_payload(&credential.payload, &["apiKey", "api_key"])
                    .ok_or_else(|| {
                        ApiError::bad_request(
                            "INVALID_CREDENTIALS",
                            "limitless credential must include apiKey",
                        )
                    })?;
                let base_wallet =
                    payload_string(&credential.payload, &["baseWallet", "base_wallet"])
                        .ok_or_else(|| {
                            ApiError::bad_request(
                                "INVALID_CREDENTIALS",
                                "limitless credential must include baseWallet",
                            )
                        })?;
                let profile =
                    fetch_limitless_profile(state, base_wallet.as_str(), api_key.as_str()).await?;
                object.insert("ownerId".to_string(), json!(profile.id));
            }
            if let Some(order) = object
                .get_mut("order")
                .and_then(|value| value.as_object_mut())
            {
                ensure_limitless_order_price(order, price);
            }
        }
        return Ok(payload);
    }

    let signature = signed_order
        .get("signature")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_SIGNED_ORDER",
                "signed order must include a signature",
            )
        })?;
    let order_message = typed_data
        .get("message")
        .and_then(|value| value.as_object())
        .ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_SIGNED_ORDER",
                "order intent is missing the typed-data message payload",
            )
        })?;

    let api_key =
        api_key_from_payload(&credential.payload, &["apiKey", "api_key"]).ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_CREDENTIALS",
                "limitless credential must include apiKey",
            )
        })?;
    let base_wallet = payload_string(&credential.payload, &["baseWallet", "base_wallet"])
        .ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_CREDENTIALS",
                "limitless credential must include baseWallet",
            )
        })?;
    let profile = fetch_limitless_profile(state, base_wallet.as_str(), api_key.as_str()).await?;

    let market_slug = if !provider_market_ref.trim().is_empty() {
        provider_market_ref.trim().to_string()
    } else {
        ExternalMarketId::parse(market_id)?.value
    };

    let mut order = serde_json::Map::with_capacity(order_message.len() + 1);
    for (key, value) in order_message {
        let normalized = match key.as_str() {
            "salt" | "makerAmount" | "takerAmount" | "nonce" | "feeRateBps" => {
                let parsed = value
                    .as_u64()
                    .or_else(|| value.as_str().and_then(|raw| raw.parse::<u64>().ok()))
                    .ok_or_else(|| {
                        ApiError::bad_request(
                            "INVALID_SIGNED_ORDER",
                            format!("signed order field {} must be numeric", key).as_str(),
                        )
                    })?;
                json!(parsed)
            }
            _ => value.clone(),
        };
        order.insert(key.clone(), normalized);
    }
    order.insert("signature".to_string(), json!(signature));
    ensure_limitless_order_price(&mut order, price);

    Ok(json!({
        "order": order,
        "orderType": "GTC",
        "marketSlug": market_slug,
        "ownerId": profile.id,
    }))
}

fn ensure_limitless_order_price(order: &mut serde_json::Map<String, Value>, price: f64) {
    order
        .entry("price".to_string())
        .or_insert_with(|| json!(price));
}

async fn submit_polymarket_order(
    state: &AppState,
    credential: &StoredCredential,
    signed_order: &Value,
) -> Result<Value, ApiError> {
    let body = polymarket_request_body(signed_order)?;
    let path = "/order";
    let headers = build_polymarket_request_headers(state, credential, "POST", path, body.as_str())?;

    execute_polymarket_request(
        state,
        "POST",
        path,
        headers,
        body,
        "polymarket submit failed",
        "POLYMARKET_SUBMIT_FAILED",
        "polymarket order submission failed",
    )
    .await
}

async fn submit_to_provider(
    state: &AppState,
    provider: ExternalProvider,
    credential: &StoredCredential,
    signed_order: &Value,
) -> Result<Value, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    match provider {
        ExternalProvider::Limitless => {
            let api_key = api_key_from_payload(&credential.payload, &["apiKey", "api_key"])
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "INVALID_CREDENTIALS",
                        "limitless credential must include apiKey",
                    )
                })?;

            let response = client
                .post(format!(
                    "{}/orders",
                    state.config.limitless_api_base.trim_end_matches('/')
                ))
                .header("X-API-Key", api_key)
                .json(signed_order)
                .send()
                .await
                .map_err(|err| ApiError::internal(&format!("limitless submit failed: {}", err)))?;

            let status = response.status();
            let payload = response
                .json::<Value>()
                .await
                .unwrap_or_else(|_| json!({ "ok": status.is_success() }));

            if !status.is_success() {
                let message = payload
                    .get("message")
                    .map(|value| {
                        if let Some(raw) = value.as_str() {
                            return raw.to_string();
                        }

                        if let Some(items) = value.as_array() {
                            let parts = items
                                .iter()
                                .map(|item| {
                                    let field = item
                                        .get("field")
                                        .and_then(|entry| entry.as_str())
                                        .unwrap_or("field");
                                    let detail = item
                                        .get("message")
                                        .and_then(|entry| entry.as_str())
                                        .unwrap_or("invalid value");
                                    format!("{}: {}", field, detail)
                                })
                                .collect::<Vec<_>>();
                            if !parts.is_empty() {
                                return parts.join("; ");
                            }
                        }

                        value.to_string()
                    })
                    .unwrap_or_else(|| "limitless order submission failed".to_string());
                return Err(ApiError::bad_request(
                    "LIMITLESS_SUBMIT_FAILED",
                    message.as_str(),
                ));
            }

            Ok(payload)
        }
        ExternalProvider::Polymarket => {
            submit_polymarket_order(state, credential, signed_order).await
        }
        ExternalProvider::Aerodrome => {
            // Extract private key from credential (encrypted at rest)
            let private_key = payload_string(&credential.payload, &["privateKey", "private_key"])
                .ok_or_else(|| {
                ApiError::bad_request(
                    "INVALID_CREDENTIALS",
                    "aerodrome credential must include privateKey for autonomous execution",
                )
            })?;

            // Validate derived address matches stored baseWallet
            let derived_address =
                crate::services::evm_signer::address_from_private_key(&private_key)?;
            let stored_wallet = payload_string(&credential.payload, &["baseWallet", "base_wallet"])
                .unwrap_or_default()
                .to_ascii_lowercase();
            if derived_address.to_ascii_lowercase() != stored_wallet {
                return Err(ApiError::bad_request(
                    "CREDENTIAL_MISMATCH",
                    "derived address from privateKey does not match stored baseWallet",
                ));
            }

            // Extract swap parameters from the prepared payload
            let calldata = signed_order
                .get("calldata")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "INVALID_SWAP_PAYLOAD",
                        "missing calldata in swap payload",
                    )
                })?;
            let to = signed_order
                .get("to")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "INVALID_SWAP_PAYLOAD",
                        "missing target address in swap payload",
                    )
                })?;

            // Fetch nonce
            let nonce = state
                .evm_rpc
                .eth_get_transaction_count(&derived_address, "pending")
                .await
                .map_err(|e| ApiError::internal(&format!("failed to fetch nonce: {}", e)))?;

            // Fetch gas prices
            let base_fee =
                state.evm_rpc.eth_gas_price().await.map_err(|e| {
                    ApiError::internal(&format!("failed to fetch gas price: {}", e))
                })?;
            let priority_fee = state
                .evm_rpc
                .eth_max_priority_fee_per_gas()
                .await
                .map_err(|e| ApiError::internal(&format!("failed to fetch priority fee: {}", e)))?;
            let max_fee = base_fee + priority_fee;

            // Estimate gas (with 20% buffer)
            let gas_estimate = signed_order
                .get("gasEstimate")
                .and_then(|v| v.as_u64())
                .filter(|g| *g > 0)
                .unwrap_or(300_000);
            let gas_limit = gas_estimate + gas_estimate / 5; // +20% buffer

            // Sign EIP-1559 transaction
            let tx_params = crate::services::evm_signer::Eip1559TxParams {
                chain_id: 8453, // Base
                nonce,
                max_priority_fee_per_gas: priority_fee,
                max_fee_per_gas: max_fee,
                gas_limit,
                to: to.to_string(),
                value: 0, // ERC-20 swap, no ETH value
                data: calldata.to_string(),
                private_key: private_key.clone(),
            };
            let raw_tx = crate::services::evm_signer::sign_eip1559_transaction(&tx_params)?;

            // Broadcast transaction
            let tx_hash = state
                .evm_rpc
                .eth_send_raw_transaction(&raw_tx)
                .await
                .map_err(|e| {
                    ApiError::internal(&format!("failed to broadcast transaction: {}", e))
                })?;

            Ok(json!({
                "ok": true,
                "provider": "aerodrome",
                "mode": "submitted",
                "txHash": tx_hash,
                "from": derived_address,
                "nonce": nonce,
                "gasLimit": gas_limit,
            }))
        }
    }
}

async fn cancel_on_provider(
    state: &AppState,
    provider: ExternalProvider,
    credential: &StoredCredential,
    provider_order_id: &str,
    payload: Option<Value>,
) -> Result<Value, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    match provider {
        ExternalProvider::Limitless => {
            let api_key = api_key_from_payload(&credential.payload, &["apiKey", "api_key"])
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "INVALID_CREDENTIALS",
                        "limitless credential must include apiKey",
                    )
                })?;

            let response = client
                .delete(format!(
                    "{}/orders/{}",
                    state.config.limitless_api_base.trim_end_matches('/'),
                    provider_order_id
                ))
                .header("X-API-Key", api_key)
                .send()
                .await
                .map_err(|err| ApiError::internal(&format!("limitless cancel failed: {}", err)))?;

            let status = response.status();
            let body = response
                .json::<Value>()
                .await
                .unwrap_or_else(|_| json!({ "ok": status.is_success() }));

            if !status.is_success() {
                return Err(ApiError::bad_request(
                    "LIMITLESS_CANCEL_FAILED",
                    body.get("message")
                        .and_then(|value| value.as_str())
                        .unwrap_or("limitless cancel failed"),
                ));
            }
            Ok(body)
        }
        ExternalProvider::Polymarket => {
            let body = payload.unwrap_or_else(|| json!({ "orderId": provider_order_id }));
            let body_string = polymarket_request_body(&body)?;
            let path = "/order";
            let headers = build_polymarket_request_headers(
                state,
                credential,
                "DELETE",
                path,
                body_string.as_str(),
            )?;

            execute_polymarket_request(
                state,
                "DELETE",
                path,
                headers,
                body_string,
                "polymarket cancel failed",
                "POLYMARKET_CANCEL_FAILED",
                "polymarket cancel failed",
            )
            .await
        }
        ExternalProvider::Aerodrome => {
            // Aerodrome swaps are atomic on-chain transactions — no cancel possible.
            Ok(json!({
                "ok": true,
                "provider": "aerodrome",
                "message": "aerodrome swaps are atomic and cannot be cancelled"
            }))
        }
    }
}

fn build_external_order_response(
    row: sqlx::postgres::PgRow,
) -> Result<ExternalOrderResponse, ApiError> {
    let created_at: chrono::DateTime<Utc> = row
        .try_get("created_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let updated_at: chrono::DateTime<Utc> = row
        .try_get("updated_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(ExternalOrderResponse {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider: row
            .try_get("provider")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        market_id: row
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider_order_id: row
            .try_get("provider_order_id")
            .unwrap_or_else(|_| String::new()),
        status: row
            .try_get("status")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
        response_payload: row
            .try_get("response_payload")
            .unwrap_or_else(|_| json!({})),
        error_message: row.try_get("error_message").ok(),
    })
}

fn parse_external_agent(
    row: sqlx::postgres::PgRow,
    source: Option<&str>,
) -> Result<ExternalAgentResponse, ApiError> {
    let created_at: chrono::DateTime<Utc> = row
        .try_get("created_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let updated_at: chrono::DateTime<Utc> = row
        .try_get("updated_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let next_execution_at: chrono::DateTime<Utc> = row
        .try_get("next_execution_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let last_executed_at: Option<chrono::DateTime<Utc>> = row.try_get("last_executed_at").ok();
    let execution_mode_raw: String = row
        .try_get("execution_mode")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let strategy: String = row
        .try_get("strategy")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let execution_mode = parse_external_execution_mode(execution_mode_raw.as_str())?;

    Ok(ExternalAgentResponse {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        owner: row
            .try_get("owner")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        cohort: row
            .try_get("cohort")
            .unwrap_or_else(|_| PRIVATE_ALPHA_COHORT.to_string()),
        name: row
            .try_get("name")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider: row
            .try_get("provider")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        market_id: row
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        outcome: row
            .try_get("outcome")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        side: row
            .try_get("side")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        price: row
            .try_get("price")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        quantity: row
            .try_get("quantity")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        cadence_seconds: row
            .try_get::<i64, _>("cadence_seconds")
            .map_err(|err| ApiError::internal(&err.to_string()))?
            .max(1) as u64,
        strategy_label: strategy_label(strategy.as_str()),
        strategy,
        strategy_params: row.try_get("strategy_params").unwrap_or_else(|_| json!({})),
        execution_mode: execution_mode.as_str().to_string(),
        credential_id: row.try_get("credential_id").ok(),
        max_notional_per_execution: row.try_get("max_notional_per_execution").ok().flatten(),
        max_daily_spend_usdc: row.try_get("max_daily_spend_usdc").ok().flatten(),
        max_slippage_bps: row.try_get("max_slippage_bps").ok().flatten(),
        paper_performance: None,
        source: source.map(str::to_string),
        active: row
            .try_get("active")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        last_executed_at: last_executed_at.map(|entry| entry.to_rfc3339()),
        next_execution_at: next_execution_at.to_rfc3339(),
        consecutive_failures: row.try_get::<i32, _>("consecutive_failures").unwrap_or(0),
        last_error_code: row.try_get("last_error_code").ok(),
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    })
}

fn parse_external_signal(row: sqlx::postgres::PgRow) -> Result<ExternalSignalResponse, ApiError> {
    let created_at: chrono::DateTime<Utc> = row
        .try_get("created_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let updated_at: chrono::DateTime<Utc> = row
        .try_get("updated_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let expires_at: chrono::DateTime<Utc> = row
        .try_get("expires_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let invalidators_value: Value = row.try_get("invalidators").unwrap_or_else(|_| json!([]));
    let invalidators = invalidators_value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let metadata: Value = row.try_get("metadata").unwrap_or_else(|_| json!({}));
    let sources = metadata_string_list(&metadata, "sources");
    let resolution_hazards = metadata_string_list(&metadata, "resolutionHazards");

    Ok(ExternalSignalResponse {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        publisher: row
            .try_get("publisher")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider: row
            .try_get("provider")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        market_id: row
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        signal_type: row
            .try_get("signal_type")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        direction: row
            .try_get("direction")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        confidence_bps: row
            .try_get("confidence_bps")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        fair_value_low: row
            .try_get("fair_value_low")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        fair_value_high: row
            .try_get("fair_value_high")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        midpoint_delta_bps: row
            .try_get("midpoint_delta_bps")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        catalyst_summary: row
            .try_get("catalyst_summary")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        invalidators,
        rationale: row.try_get("rationale").ok(),
        metadata: metadata.clone(),
        memo_mode: metadata_string(&metadata, "memoMode"),
        sources,
        resolution_rules_read: metadata_bool(&metadata, "resolutionRulesRead").unwrap_or(false),
        resolution_criteria: metadata_string(&metadata, "resolutionCriteria"),
        resolution_hazards,
        has_live_reference: metadata_bool(&metadata, "hasLiveReference").unwrap_or(false),
        repricing_half_life_minutes: metadata_i32(&metadata, "repricingHalfLifeMinutes"),
        confidence_reasoning: metadata_string(&metadata, "confidenceReasoning"),
        active: row.try_get("active").unwrap_or(true),
        expires_at: expires_at.to_rfc3339(),
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
        agent_id: row.try_get("agent_id").ok(),
    })
}

async fn load_active_market_signal(
    state: &AppState,
    market_id: &str,
) -> Result<Option<ExternalSignalResponse>, ApiError> {
    let row = sqlx::query(
        "SELECT id, publisher, provider, market_id, signal_type, direction, confidence_bps,
                fair_value_low, fair_value_high, midpoint_delta_bps, catalyst_summary, invalidators,
                rationale, metadata, active, expires_at, created_at, updated_at, agent_id
         FROM external_market_signals
         WHERE market_id = $1
           AND active = TRUE
           AND expires_at > NOW()
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(market_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    row.map(parse_external_signal).transpose()
}

fn calculate_max_drawdown(points: &[f64]) -> f64 {
    let mut equity = 0.0;
    let mut peak = 0.0;
    let mut max_drawdown = 0.0;
    for point in points {
        equity += point;
        if equity > peak {
            peak = equity;
        }
        let drawdown = peak - equity;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }
    max_drawdown
}

fn median_f64(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[mid - 1] + values[mid]) / 2.0)
    } else {
        Some(values[mid])
    }
}

async fn record_paper_mark(
    state: &AppState,
    position_id: &str,
    agent: &ExternalAgentRecord,
    mark_price: f64,
    unrealized_pnl_usdc: f64,
    notional_usdc: f64,
    metadata: &Value,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO paper_marks (
            id, position_id, agent_id, owner, market_id, outcome, mark_price,
            unrealized_pnl_usdc, notional_usdc, metadata, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,NOW())",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(position_id)
    .bind(agent.id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.market_id.as_str())
    .bind(agent.outcome.as_str())
    .bind(mark_price)
    .bind(unrealized_pnl_usdc)
    .bind(notional_usdc)
    .bind(metadata)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(())
}

async fn record_live_mark(
    state: &AppState,
    position_id: &str,
    agent: &ExternalAgentRecord,
    mark_price: f64,
    unrealized_pnl_usdc: f64,
    notional_usdc: f64,
    metadata: &Value,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO external_marks (
            id, position_id, agent_id, owner, market_id, outcome, mark_price,
            unrealized_pnl_usdc, notional_usdc, metadata, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,NOW())
         ON CONFLICT (position_id) DO UPDATE SET
             mark_price = EXCLUDED.mark_price,
             unrealized_pnl_usdc = EXCLUDED.unrealized_pnl_usdc,
             notional_usdc = EXCLUDED.notional_usdc,
             metadata = EXCLUDED.metadata,
             created_at = EXCLUDED.created_at",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(position_id)
    .bind(agent.id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.market_id.as_str())
    .bind(agent.outcome.as_str())
    .bind(mark_price)
    .bind(unrealized_pnl_usdc)
    .bind(notional_usdc)
    .bind(metadata)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(())
}

async fn record_live_fill(
    state: &AppState,
    run_id: &str,
    position_id: &str,
    agent: &ExternalAgentRecord,
    fill_type: &str,
    requested_quantity: f64,
    filled_quantity: f64,
    price: f64,
    mark_price: f64,
    fee_usdc: f64,
    provider_order_id: Option<&str>,
    tx_hash: Option<&str>,
    block_number: Option<u64>,
    metadata: &Value,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO external_fills (
            id, run_id, position_id, agent_id, owner, provider, market_id, outcome, side, fill_type,
            requested_quantity, filled_quantity, price, mark_price, notional_usdc, fee_usdc,
            provider_order_id, tx_hash, block_number, metadata, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,NOW())
         ON CONFLICT (run_id) WHERE run_id IS NOT NULL DO UPDATE SET
             position_id = EXCLUDED.position_id,
             filled_quantity = EXCLUDED.filled_quantity,
             price = EXCLUDED.price,
             mark_price = EXCLUDED.mark_price,
             notional_usdc = EXCLUDED.notional_usdc,
             fee_usdc = EXCLUDED.fee_usdc,
             provider_order_id = EXCLUDED.provider_order_id,
             tx_hash = EXCLUDED.tx_hash,
             block_number = EXCLUDED.block_number,
             metadata = EXCLUDED.metadata,
             created_at = EXCLUDED.created_at",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(run_id)
    .bind(position_id)
    .bind(agent.id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.provider.as_str())
    .bind(agent.market_id.as_str())
    .bind(agent.outcome.as_str())
    .bind(agent.side.as_str())
    .bind(fill_type)
    .bind(requested_quantity)
    .bind(filled_quantity)
    .bind(price)
    .bind(mark_price)
    .bind(filled_quantity * price)
    .bind(fee_usdc)
    .bind(provider_order_id)
    .bind(tx_hash)
    .bind(block_number.map(|value| value as i64))
    .bind(metadata)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(())
}

async fn record_live_outcome(
    state: &AppState,
    position_id: &str,
    agent: &ExternalAgentRecord,
    entry_price: f64,
    exit_price: f64,
    quantity: f64,
    fee_usdc: f64,
    opened_at: chrono::DateTime<Utc>,
    closed_at: chrono::DateTime<Utc>,
    metadata: &Value,
) -> Result<(), ApiError> {
    let gross = unrealized_pnl(agent.side.as_str(), entry_price, exit_price, quantity);
    let realized = realized_pnl(
        agent.side.as_str(),
        entry_price,
        exit_price,
        quantity,
        fee_usdc,
    );

    sqlx::query(
        "INSERT INTO external_outcomes (
            id, position_id, agent_id, owner, provider, market_id, outcome, side, strategy,
            entry_price, exit_price, quantity, gross_pnl_usdc, fee_usdc, realized_pnl_usdc,
            hold_seconds, metadata, created_at, closed_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,NOW(),$18)
         ON CONFLICT (position_id) DO UPDATE SET
             entry_price = EXCLUDED.entry_price,
             exit_price = EXCLUDED.exit_price,
             quantity = EXCLUDED.quantity,
             gross_pnl_usdc = EXCLUDED.gross_pnl_usdc,
             fee_usdc = EXCLUDED.fee_usdc,
             realized_pnl_usdc = EXCLUDED.realized_pnl_usdc,
             hold_seconds = EXCLUDED.hold_seconds,
             metadata = EXCLUDED.metadata,
             closed_at = EXCLUDED.closed_at,
             created_at = EXCLUDED.created_at",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(position_id)
    .bind(agent.id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.provider.as_str())
    .bind(agent.market_id.as_str())
    .bind(agent.outcome.as_str())
    .bind(agent.side.as_str())
    .bind(agent.strategy.as_str())
    .bind(entry_price)
    .bind(exit_price)
    .bind(quantity)
    .bind(gross)
    .bind(fee_usdc)
    .bind(realized)
    .bind((closed_at - opened_at).num_seconds().max(0))
    .bind(metadata)
    .bind(closed_at)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    sqlx::query(
        "UPDATE external_positions
         SET status = 'closed',
             mark_price = $2,
             fees_paid_usdc = $3,
             realized_pnl_usdc = $4,
             unrealized_pnl_usdc = 0,
             closed_at = $5,
             last_marked_at = $5,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(position_id)
    .bind(exit_price)
    .bind(fee_usdc)
    .bind(realized)
    .bind(closed_at)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(())
}

async fn upsert_live_position(
    state: &AppState,
    position_id: &str,
    agent: &ExternalAgentRecord,
    entry_price: f64,
    mark_price: f64,
    requested_quantity: f64,
    filled_quantity: f64,
    fees_paid_usdc: f64,
    unrealized_pnl_usdc: f64,
    hold_until: chrono::DateTime<Utc>,
    opened_at: chrono::DateTime<Utc>,
    marked_at: chrono::DateTime<Utc>,
    metadata: &Value,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO external_positions (
            id, agent_id, owner, provider, market_id, outcome, side, strategy, status,
            entry_price, mark_price, requested_quantity, filled_quantity, notional_usdc,
            fees_paid_usdc, realized_pnl_usdc, unrealized_pnl_usdc, hold_until, opened_at,
            last_marked_at, metadata, created_at, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'open',$9,$10,$11,$12,$13,$14,0,$15,$16,$17,$18,$19,NOW(),NOW())
         ON CONFLICT (agent_id) WHERE status = 'open' DO UPDATE SET
             entry_price = EXCLUDED.entry_price,
             mark_price = EXCLUDED.mark_price,
             requested_quantity = EXCLUDED.requested_quantity,
             filled_quantity = EXCLUDED.filled_quantity,
             notional_usdc = EXCLUDED.notional_usdc,
             fees_paid_usdc = EXCLUDED.fees_paid_usdc,
             unrealized_pnl_usdc = EXCLUDED.unrealized_pnl_usdc,
             hold_until = EXCLUDED.hold_until,
             opened_at = EXCLUDED.opened_at,
             last_marked_at = EXCLUDED.last_marked_at,
             metadata = EXCLUDED.metadata,
             updated_at = NOW()",
    )
    .bind(position_id)
    .bind(agent.id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.provider.as_str())
    .bind(agent.market_id.as_str())
    .bind(agent.outcome.as_str())
    .bind(agent.side.as_str())
    .bind(agent.strategy.as_str())
    .bind(entry_price)
    .bind(mark_price)
    .bind(requested_quantity)
    .bind(filled_quantity)
    .bind(filled_quantity * entry_price)
    .bind(fees_paid_usdc)
    .bind(unrealized_pnl_usdc)
    .bind(hold_until)
    .bind(opened_at)
    .bind(marked_at)
    .bind(metadata)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(())
}

async fn reconcile_polymarket_live_agent_execution(
    state: &AppState,
    agent: &ExternalAgentRecord,
    run_id: &str,
    external_order_id: &str,
    provider_order_id: &str,
    provider_payload: &Value,
    submit_payload: &Value,
    market: &external::types::ExternalMarketSnapshot,
    orderbook: &external::types::ExternalOrderBookSnapshot,
    now: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    let refs = external::polymarket_index::reference_candidates_for_reconciliation(
        provider_order_id,
        provider_payload,
        submit_payload,
    );
    let confirmed_fill = external::polymarket_index::load_confirmed_user_fill(
        agent.market_id.as_str(),
        run_id,
        external_order_id,
        &refs,
    )
    .await?;
    let existing_position = load_open_live_position(state, agent.id.as_str()).await?;
    let mark_price = orderbook
        .bids
        .first()
        .zip(orderbook.asks.first())
        .map(|(bid, ask)| (bid.price + ask.price) / 2.0)
        .or_else(|| orderbook.bids.first().map(|entry| entry.price))
        .or_else(|| orderbook.asks.first().map(|entry| entry.price))
        .unwrap_or(market.yes_price);

    let Some(fill) = confirmed_fill else {
        if let Some(position) = existing_position {
            let unrealized = unrealized_pnl(
                agent.side.as_str(),
                position.entry_price,
                mark_price,
                position.filled_quantity,
            ) - position.fees_paid_usdc;
            upsert_live_position(
                state,
                position.id.as_str(),
                agent,
                position.entry_price,
                mark_price,
                position.filled_quantity,
                position.filled_quantity,
                position.fees_paid_usdc,
                unrealized,
                position.hold_until,
                position.opened_at,
                now,
                &json!({
                    "mode": "live",
                    "provider": "polymarket",
                    "reconciledAt": now.to_rfc3339(),
                    "providerOrderId": provider_order_id,
                    "providerResponse": submit_payload,
                    "providerPayload": provider_payload,
                }),
            )
            .await?;

            record_live_mark(
                state,
                position.id.as_str(),
                agent,
                mark_price,
                unrealized,
                position.filled_quantity * mark_price,
                &json!({
                    "mode": "live",
                    "provider": "polymarket",
                    "reconciledAt": now.to_rfc3339(),
                    "providerOrderId": provider_order_id,
                    "providerResponse": submit_payload,
                    "providerPayload": provider_payload,
                }),
            )
            .await?;

            if market.resolved
                && external::types::is_binary_yes_no(&market.outcomes)
                && market.outcome.is_some()
            {
                let resolved_yes = market.outcome.as_deref() == Some("yes");
                let exit_price = if agent.outcome.eq_ignore_ascii_case("yes") {
                    if resolved_yes {
                        1.0
                    } else {
                        0.0
                    }
                } else if resolved_yes {
                    0.0
                } else {
                    1.0
                };
                record_live_outcome(
                    state,
                    position.id.as_str(),
                    agent,
                    position.entry_price,
                    exit_price,
                    position.filled_quantity,
                    position.fees_paid_usdc,
                    position.opened_at,
                    now,
                    &json!({
                        "mode": "live",
                        "provider": "polymarket",
                        "providerOrderId": provider_order_id,
                        "providerResponse": submit_payload,
                        "providerPayload": provider_payload,
                        "resolvedOutcome": market.outcome,
                        "reconciledAt": now.to_rfc3339(),
                    }),
                )
                .await?;
            }
        }

        return Ok(());
    };

    let fill_price = fill.price.unwrap_or(market.yes_price);
    let filled_quantity = fill.filled_quantity.unwrap_or(agent.quantity).max(0.0);
    let fees_paid_usdc = fill.fee_usdc;
    let position_id = existing_position
        .as_ref()
        .map(|position| position.id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let hold_until = if market.close_time > 0 {
        chrono::DateTime::from_timestamp(market.close_time as i64, 0).unwrap_or(now)
    } else {
        now + Duration::seconds(agent.cadence_seconds.max(1))
    };
    let unrealized = unrealized_pnl(agent.side.as_str(), fill_price, mark_price, filled_quantity)
        - fees_paid_usdc;

    upsert_live_position(
        state,
        position_id.as_str(),
        agent,
        fill_price,
        mark_price,
        agent.quantity,
        filled_quantity,
        fees_paid_usdc,
        unrealized,
        hold_until,
        existing_position
            .as_ref()
            .map(|position| position.opened_at)
            .unwrap_or(now),
        now,
        &json!({
            "mode": "live",
            "provider": "polymarket",
            "runId": run_id,
            "externalOrderId": external_order_id,
            "providerOrderId": provider_order_id,
            "providerTradeEventId": fill.id,
            "providerTradeId": fill.builder_trade_id,
            "txHash": fill.tx_hash,
            "blockNumber": fill.block_number,
            "providerResponse": submit_payload,
            "providerPayload": provider_payload,
            "reconciledAt": now.to_rfc3339()
        }),
    )
    .await?;

    record_live_fill(
        state,
        run_id,
        position_id.as_str(),
        agent,
        "open",
        agent.quantity,
        filled_quantity,
        fill_price,
        mark_price,
        fees_paid_usdc,
        Some(provider_order_id),
        fill.tx_hash.as_deref(),
        fill.block_number,
        &json!({
            "mode": "live",
            "provider": "polymarket",
            "providerTradeEventId": fill.id,
            "providerTradeId": fill.builder_trade_id,
            "providerResponse": submit_payload,
            "providerPayload": provider_payload,
            "reconciledAt": now.to_rfc3339()
        }),
    )
    .await?;

    record_live_mark(
        state,
        position_id.as_str(),
        agent,
        mark_price,
        unrealized,
        filled_quantity * mark_price,
        &json!({
            "mode": "live",
            "provider": "polymarket",
            "providerTradeEventId": fill.id,
            "providerTradeId": fill.builder_trade_id,
            "providerResponse": submit_payload,
            "providerPayload": provider_payload,
            "reconciledAt": now.to_rfc3339()
        }),
    )
    .await?;

    if market.resolved
        && external::types::is_binary_yes_no(&market.outcomes)
        && market.outcome.is_some()
    {
        let resolved_yes = market.outcome.as_deref() == Some("yes");
        let exit_price = if agent.outcome.eq_ignore_ascii_case("yes") {
            if resolved_yes {
                1.0
            } else {
                0.0
            }
        } else if resolved_yes {
            0.0
        } else {
            1.0
        };
        record_live_outcome(
            state,
            position_id.as_str(),
            agent,
            fill_price,
            exit_price,
            filled_quantity,
            fees_paid_usdc,
            existing_position
                .as_ref()
                .map(|position| position.opened_at)
                .unwrap_or(now),
            now,
            &json!({
                "mode": "live",
                "provider": "polymarket",
                "providerTradeEventId": fill.id,
                "providerTradeId": fill.builder_trade_id,
                "providerResponse": submit_payload,
                "providerPayload": provider_payload,
                "resolvedOutcome": market.outcome,
                "reconciledAt": now.to_rfc3339()
            }),
        )
        .await?;
    }

    Ok(())
}

async fn reconcile_live_agent_execution(
    state: &AppState,
    agent: &ExternalAgentRecord,
    run_id: &str,
    external_order_id: &str,
    provider_order_id: &str,
    provider_payload: &Value,
    submit_payload: &Value,
    market: &external::types::ExternalMarketSnapshot,
    orderbook: &external::types::ExternalOrderBookSnapshot,
    now: chrono::DateTime<Utc>,
) -> Result<(), ApiError> {
    if agent.provider == ExternalProvider::Polymarket {
        return reconcile_polymarket_live_agent_execution(
            state,
            agent,
            run_id,
            external_order_id,
            provider_order_id,
            provider_payload,
            submit_payload,
            market,
            orderbook,
            now,
        )
        .await;
    }

    if !ledger::live_trade_reconciliation_supported(agent.provider) {
        return Ok(());
    }

    let mut reference_payload = submit_payload.clone();
    if let Value::Object(map) = &mut reference_payload {
        if !provider_order_id.trim().is_empty() {
            map.insert(
                "providerOrderId".to_string(),
                Value::String(provider_order_id.to_string()),
            );
            map.insert(
                "orderId".to_string(),
                Value::String(provider_order_id.to_string()),
            );
        }
    }

    let trade_snapshot = if let Ok(trades) = external::fetch_trades_with_rpc(
        &state.config,
        &state.redis,
        &ExternalMarketId::parse(agent.market_id.as_str())?,
        Some(agent.outcome.as_str()),
        200,
        0,
        Some(&state.evm_rpc),
    )
    .await
    {
        trades
            .trades
            .into_iter()
            .find(|trade| ledger::trade_matches_reference(trade, &reference_payload))
    } else {
        None
    };

    let existing_position = load_open_live_position(state, agent.id.as_str()).await?;
    let mark_price = orderbook
        .bids
        .first()
        .zip(orderbook.asks.first())
        .map(|(bid, ask)| (bid.price + ask.price) / 2.0)
        .or_else(|| orderbook.bids.first().map(|entry| entry.price))
        .or_else(|| orderbook.asks.first().map(|entry| entry.price))
        .unwrap_or(market.yes_price);

    let Some(trade) = trade_snapshot else {
        if let Some(position) = existing_position {
            let unrealized = unrealized_pnl(
                agent.side.as_str(),
                position.entry_price,
                mark_price,
                position.filled_quantity,
            ) - position.fees_paid_usdc;
            upsert_live_position(
                state,
                position.id.as_str(),
                agent,
                position.entry_price,
                mark_price,
                position.filled_quantity,
                position.filled_quantity,
                position.fees_paid_usdc,
                unrealized,
                position.hold_until,
                position.opened_at,
                now,
                &json!({
                    "mode": "live",
                    "reconciledAt": now.to_rfc3339(),
                    "providerOrderId": provider_order_id,
                    "providerResponse": submit_payload,
                    "providerPayload": provider_payload,
                }),
            )
            .await?;

            record_live_mark(
                state,
                position.id.as_str(),
                agent,
                mark_price,
                unrealized,
                position.filled_quantity * mark_price,
                &json!({
                    "mode": "live",
                    "reconciledAt": now.to_rfc3339(),
                    "providerOrderId": provider_order_id,
                    "providerResponse": submit_payload,
                    "providerPayload": provider_payload,
                }),
            )
            .await?;

            if market.resolved
                && external::types::is_binary_yes_no(&market.outcomes)
                && market.outcome.is_some()
            {
                let resolved_yes = market.outcome.as_deref() == Some("yes");
                let exit_price = if agent.outcome.eq_ignore_ascii_case("yes") {
                    if resolved_yes {
                        1.0
                    } else {
                        0.0
                    }
                } else if resolved_yes {
                    0.0
                } else {
                    1.0
                };
                record_live_outcome(
                    state,
                    position.id.as_str(),
                    agent,
                    position.entry_price,
                    exit_price,
                    position.filled_quantity,
                    position.fees_paid_usdc,
                    position.opened_at,
                    now,
                    &json!({
                        "mode": "live",
                        "reconciledAt": now.to_rfc3339(),
                        "providerOrderId": provider_order_id,
                        "providerResponse": submit_payload,
                        "providerPayload": provider_payload,
                        "resolvedOutcome": market.outcome,
                    }),
                )
                .await?;
            }
        }

        return Ok(());
    };

    let position_id = existing_position
        .as_ref()
        .map(|position| position.id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let filled_quantity = trade.quantity as f64;
    let fees_paid_usdc = existing_position
        .as_ref()
        .map(|position| position.fees_paid_usdc)
        .unwrap_or(0.0);
    let hold_until = if market.close_time > 0 {
        chrono::DateTime::from_timestamp(market.close_time as i64, 0).unwrap_or(now)
    } else {
        now + Duration::seconds(agent.cadence_seconds.max(1))
    };
    let unrealized = unrealized_pnl(
        agent.side.as_str(),
        trade.price,
        mark_price,
        filled_quantity,
    ) - fees_paid_usdc;

    upsert_live_position(
        state,
        position_id.as_str(),
        agent,
        trade.price,
        mark_price,
        filled_quantity,
        filled_quantity,
        fees_paid_usdc,
        unrealized,
        hold_until,
        existing_position
            .as_ref()
            .map(|position| position.opened_at)
            .unwrap_or(now),
        now,
        &json!({
            "mode": "live",
            "runId": run_id,
            "externalOrderId": external_order_id,
            "providerOrderId": provider_order_id,
            "providerTradeId": trade.id,
            "txHash": trade.tx_hash,
            "blockNumber": trade.block_number,
            "providerResponse": submit_payload,
            "providerPayload": provider_payload,
            "reconciledAt": now.to_rfc3339()
        }),
    )
    .await?;

    record_live_fill(
        state,
        run_id,
        position_id.as_str(),
        agent,
        "open",
        agent.quantity,
        filled_quantity,
        trade.price,
        mark_price,
        0.0,
        Some(provider_order_id),
        Some(trade.tx_hash.as_str()),
        Some(trade.block_number),
        &json!({
            "mode": "live",
            "providerTradeId": trade.id,
            "providerResponse": submit_payload,
            "providerPayload": provider_payload,
            "reconciledAt": now.to_rfc3339()
        }),
    )
    .await?;

    record_live_mark(
        state,
        position_id.as_str(),
        agent,
        mark_price,
        unrealized,
        filled_quantity * mark_price,
        &json!({
            "mode": "live",
            "providerTradeId": trade.id,
            "providerResponse": submit_payload,
            "providerPayload": provider_payload,
            "reconciledAt": now.to_rfc3339()
        }),
    )
    .await?;

    if market.resolved
        && external::types::is_binary_yes_no(&market.outcomes)
        && market.outcome.is_some()
    {
        let resolved_yes = market.outcome.as_deref() == Some("yes");
        let exit_price = if agent.outcome.eq_ignore_ascii_case("yes") {
            if resolved_yes {
                1.0
            } else {
                0.0
            }
        } else if resolved_yes {
            0.0
        } else {
            1.0
        };
        record_live_outcome(
            state,
            position_id.as_str(),
            agent,
            trade.price,
            exit_price,
            filled_quantity,
            fees_paid_usdc,
            existing_position
                .as_ref()
                .map(|position| position.opened_at)
                .unwrap_or(now),
            now,
            &json!({
                "mode": "live",
                "providerTradeId": trade.id,
                "providerResponse": submit_payload,
                "providerPayload": provider_payload,
                "resolvedOutcome": market.outcome,
                "reconciledAt": now.to_rfc3339()
            }),
        )
        .await?;
    }

    Ok(())
}

async fn sync_live_external_ledgers(
    state: &AppState,
    owner_filter: Option<&str>,
) -> Result<(), ApiError> {
    let rows = if let Some(owner) = owner_filter {
        sqlx::query(
            "SELECT
                ea.id AS agent_id,
                ear.id AS run_id,
                eo.id AS external_order_id,
                eo.provider_order_id,
                eo.request_payload,
                eo.response_payload
             FROM external_agent_runs ear
             JOIN external_agents ea ON ea.id = ear.agent_id
             LEFT JOIN external_orders eo ON eo.id = ear.external_order_id
             WHERE ea.owner = $1
               AND ea.execution_mode = 'live'
               AND ear.status = 'submitted'
             ORDER BY ear.created_at ASC
             LIMIT 100",
        )
        .bind(owner)
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT
                ea.id AS agent_id,
                ear.id AS run_id,
                eo.id AS external_order_id,
                eo.provider_order_id,
                eo.request_payload,
                eo.response_payload
             FROM external_agent_runs ear
             JOIN external_agents ea ON ea.id = ear.agent_id
             LEFT JOIN external_orders eo ON eo.id = ear.external_order_id
             WHERE ea.execution_mode = 'live'
               AND ear.status = 'submitted'
             ORDER BY ear.created_at ASC
             LIMIT 100",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    for row in rows {
        let agent_id: String = row
            .try_get("agent_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let agent = match load_external_agent_by_id(state, agent_id.as_str()).await {
            Ok(agent) => agent,
            Err(err) => {
                log::warn!(
                    "skipping live ledger sync for {}: {}",
                    agent_id,
                    err.message
                );
                continue;
            }
        };
        let run_id: String = row
            .try_get("run_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let external_order_id: String = row.try_get("external_order_id").unwrap_or_default();
        let provider_order_id: String = row.try_get("provider_order_id").unwrap_or_default();
        let provider_payload: Value = row.try_get("request_payload").unwrap_or_else(|_| json!({}));
        let submit_payload: Value = row
            .try_get("response_payload")
            .unwrap_or_else(|_| json!({}));

        if external_order_id.is_empty() || submit_payload.is_null() {
            continue;
        }

        let market_id = match ExternalMarketId::parse(agent.market_id.as_str()) {
            Ok(market_id) => market_id,
            Err(err) => {
                log::warn!(
                    "skipping live ledger sync for {} due to invalid market id: {}",
                    agent.id,
                    err.message
                );
                continue;
            }
        };
        let market = match external::fetch_market_by_id(&state.config, &market_id).await {
            Ok(market) => market,
            Err(err) => {
                log::warn!(
                    "skipping live ledger sync for {} due to market fetch error: {}",
                    agent.id,
                    err.message
                );
                continue;
            }
        };
        let orderbook = match external::fetch_orderbook_with_rpc(
            &state.config,
            &state.redis,
            &market_id,
            agent.outcome.as_str(),
            20,
            Some(&state.evm_rpc),
        )
        .await
        {
            Ok(orderbook) => orderbook,
            Err(err) => {
                log::warn!(
                    "skipping live ledger sync for {} due to orderbook fetch error: {}",
                    agent.id,
                    err.message
                );
                continue;
            }
        };

        if let Err(err) = reconcile_live_agent_execution(
            state,
            &agent,
            run_id.as_str(),
            external_order_id.as_str(),
            provider_order_id.as_str(),
            &provider_payload,
            &submit_payload,
            &market,
            &orderbook,
            Utc::now(),
        )
        .await
        {
            log::warn!(
                "live external ledger sync failed for {}: {}",
                agent.id,
                err.message
            );
        }
    }

    Ok(())
}

async fn close_due_paper_position(
    state: &AppState,
    agent: &ExternalAgentRecord,
    position: &PaperPositionRecord,
    now: chrono::DateTime<Utc>,
    market: &external::types::ExternalMarketSnapshot,
    orderbook: &external::types::ExternalOrderBookSnapshot,
) -> Result<(bool, Value), ApiError> {
    let exit_side = if agent.side == "buy" { "sell" } else { "buy" };
    let fill = simulate_fill(
        market,
        orderbook,
        agent.outcome.as_str(),
        exit_side,
        position.filled_quantity,
        state.config.paper_fee_bps,
        None,
    );

    if fill.filled_quantity <= 0.0 {
        let unrealized = unrealized_pnl(
            agent.side.as_str(),
            position.entry_price,
            fill.mark_price,
            position.filled_quantity,
        ) - position.fees_paid_usdc;
        sqlx::query(
            "UPDATE paper_positions
             SET mark_price = $2,
                 unrealized_pnl_usdc = $3,
                 last_marked_at = $4,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(position.id.as_str())
        .bind(fill.mark_price)
        .bind(unrealized)
        .bind(now)
        .execute(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

        record_paper_mark(
            state,
            position.id.as_str(),
            agent,
            fill.mark_price,
            unrealized,
            position.filled_quantity * fill.mark_price,
            &json!({
                "reason": "no_exit_liquidity",
                "holdExpired": true
            }),
        )
        .await?;

        return Ok((
            false,
            json!({
                "status": "holding",
                "reason": "no_exit_liquidity",
                "positionId": position.id,
                "markPrice": fill.mark_price,
                "unrealizedPnlUsdc": unrealized
            }),
        ));
    }

    let original_quantity = position.filled_quantity.max(fill.filled_quantity);
    let closed_quantity = fill.filled_quantity;
    let remaining_quantity = (position.filled_quantity - closed_quantity).max(0.0);
    let allocated_open_fees = if original_quantity > 0.0 {
        position.fees_paid_usdc * (closed_quantity / original_quantity)
    } else {
        0.0
    };
    let remaining_open_fees = (position.fees_paid_usdc - allocated_open_fees).max(0.0);
    let realized = realized_pnl(
        agent.side.as_str(),
        position.entry_price,
        fill.average_price,
        closed_quantity,
        allocated_open_fees + fill.fee_usdc,
    );
    let gross = unrealized_pnl(
        agent.side.as_str(),
        position.entry_price,
        fill.average_price,
        closed_quantity,
    );

    sqlx::query(
        "INSERT INTO paper_fills (
            id, run_id, position_id, agent_id, owner, provider, market_id, outcome, side, fill_type,
            requested_quantity, filled_quantity, price, mark_price, notional_usdc, fee_usdc, metadata, created_at
        ) VALUES ($1,NULL,$2,$3,$4,$5,$6,$7,$8,'close',$9,$10,$11,$12,$13,$14,$15,NOW())",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(position.id.as_str())
    .bind(agent.id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.provider.as_str())
    .bind(agent.market_id.as_str())
    .bind(agent.outcome.as_str())
    .bind(exit_side)
    .bind(position.filled_quantity)
    .bind(closed_quantity)
    .bind(fill.average_price)
    .bind(fill.mark_price)
    .bind(fill.notional_usdc)
    .bind(fill.fee_usdc)
    .bind(json!({
        "partialFill": fill.partial_fill,
        "slippageBps": fill.slippage_bps,
        "usedOrderbookDepth": fill.used_orderbook_depth
    }))
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    sqlx::query(
        "INSERT INTO paper_outcomes (
            id, position_id, agent_id, owner, provider, market_id, outcome, side, strategy,
            entry_price, exit_price, quantity, gross_pnl_usdc, fee_usdc, realized_pnl_usdc,
            hold_seconds, metadata, created_at, closed_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,NOW(),$18)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(position.id.as_str())
    .bind(agent.id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.provider.as_str())
    .bind(agent.market_id.as_str())
    .bind(agent.outcome.as_str())
    .bind(agent.side.as_str())
    .bind(agent.strategy.as_str())
    .bind(position.entry_price)
    .bind(fill.average_price)
    .bind(closed_quantity)
    .bind(gross)
    .bind(allocated_open_fees + fill.fee_usdc)
    .bind(realized)
    .bind((now - position.opened_at).num_seconds().max(0))
    .bind(json!({
        "partialClose": remaining_quantity > 0.0,
        "markPrice": fill.mark_price
    }))
    .bind(now)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    if remaining_quantity > 0.0 {
        let unrealized = unrealized_pnl(
            agent.side.as_str(),
            position.entry_price,
            fill.mark_price,
            remaining_quantity,
        ) - remaining_open_fees;
        sqlx::query(
            "UPDATE paper_positions
             SET filled_quantity = $2,
                 mark_price = $3,
                 notional_usdc = $4,
                 fees_paid_usdc = $5,
                 realized_pnl_usdc = realized_pnl_usdc + $6,
                 unrealized_pnl_usdc = $7,
                 hold_until = $8,
                 last_marked_at = $9,
                 updated_at = NOW()
             WHERE id = $1",
        )
        .bind(position.id.as_str())
        .bind(remaining_quantity)
        .bind(fill.mark_price)
        .bind(remaining_quantity * position.entry_price)
        .bind(remaining_open_fees)
        .bind(realized)
        .bind(unrealized)
        .bind(now)
        .bind(now)
        .execute(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

        record_paper_mark(
            state,
            position.id.as_str(),
            agent,
            fill.mark_price,
            unrealized,
            remaining_quantity * fill.mark_price,
            &json!({
                "reason": "partial_close",
                "realizedPnlUsdc": realized
            }),
        )
        .await?;

        return Ok((
            false,
            json!({
                "status": "holding",
                "reason": "partial_close",
                "positionId": position.id,
                "closedQuantity": closed_quantity,
                "remainingQuantity": remaining_quantity,
                "exitPrice": fill.average_price,
                "markPrice": fill.mark_price,
                "realizedPnlUsdc": realized
            }),
        ));
    }

    sqlx::query(
        "UPDATE paper_positions
         SET status = 'closed',
             mark_price = $2,
             fees_paid_usdc = $3,
             realized_pnl_usdc = $4,
             unrealized_pnl_usdc = 0,
             closed_at = $5,
             last_marked_at = $5,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(position.id.as_str())
    .bind(fill.mark_price)
    .bind(position.fees_paid_usdc + fill.fee_usdc)
    .bind(realized)
    .bind(now)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok((
        true,
        json!({
            "status": "closed",
            "positionId": position.id,
            "closedQuantity": closed_quantity,
            "exitPrice": fill.average_price,
            "markPrice": fill.mark_price,
            "realizedPnlUsdc": realized
        }),
    ))
}

async fn open_paper_position(
    state: &AppState,
    agent: &ExternalAgentRecord,
    now: chrono::DateTime<Utc>,
    market: &external::types::ExternalMarketSnapshot,
    orderbook: &external::types::ExternalOrderBookSnapshot,
) -> Result<AgentExecutionOutcome, ApiError> {
    // Evaluate strategy to decide whether to execute and at what params.
    let best_bid = orderbook.bids.first().map(|l| l.price);
    let best_ask = orderbook.asks.first().map(|l| l.price);
    let mid = best_bid
        .zip(best_ask)
        .map(|(b, a)| (b + a) / 2.0)
        .or(best_bid)
        .or(best_ask)
        .unwrap_or(market.yes_price);
    let active_signal = load_active_market_signal(state, agent.market_id.as_str()).await?;
    let signal_source_count = active_signal
        .as_ref()
        .map(|signal| signal.sources.len() as u64)
        .unwrap_or(0);
    let signal_resolution_rules_read = active_signal
        .as_ref()
        .map(|signal| signal.resolution_rules_read)
        .unwrap_or(false);
    let signal_has_live_reference = active_signal
        .as_ref()
        .map(|signal| signal.has_live_reference)
        .unwrap_or(false);
    let signal_resolution_hazard_count = active_signal
        .as_ref()
        .map(|signal| signal.resolution_hazards.len() as u64)
        .unwrap_or(0);
    let time_to_resolution_seconds = if market.close_time > 0 {
        Some(market.close_time as i64 - now.timestamp())
    } else {
        None
    };

    let market_state = crate::services::external::strategy::MarketState {
        yes_price: market.yes_price,
        no_price: market.no_price,
        best_bid,
        best_ask,
        mid_price: mid,
        agent_price: agent.price,
        agent_side: agent.side.clone(),
        agent_outcome: agent.outcome.clone(),
        agent_quantity: agent.quantity,
        time_to_resolution_seconds,
        fair_value_low: active_signal.as_ref().map(|signal| signal.fair_value_low),
        fair_value_high: active_signal.as_ref().map(|signal| signal.fair_value_high),
        midpoint_delta_bps: active_signal
            .as_ref()
            .map(|signal| signal.midpoint_delta_bps),
        signal_source_count,
        signal_resolution_rules_read,
        signal_has_live_reference,
        signal_resolution_hazard_count,
    };
    let signal = crate::services::external::strategy::evaluate_strategy(
        agent.strategy.as_str(),
        &market_state,
        &agent.strategy_params,
    );

    if !signal.execute {
        let run_id = Uuid::new_v4().to_string();
        let next_execution_at = now + Duration::seconds(agent.cadence_seconds.max(1));
        update_external_agent_schedule(state, agent.id.as_str(), now, next_execution_at).await?;
        insert_external_agent_run(
            state,
            run_id.as_str(),
            agent,
            "paper_skipped",
            None,
            Some("strategy_skip"),
            &json!({
                "mode": "paper",
                "reason": "strategy_skip",
                "strategy": agent.strategy,
                "strategyParams": agent.strategy_params,
                "signal": signal.reason,
                "signalMetrics": signal.metadata,
                "signalId": active_signal.as_ref().map(|item| item.id.as_str()),
                "midPrice": mid
            }),
        )
        .await?;

        return Ok(AgentExecutionOutcome {
            executed: false,
            skip_reason: Some("strategy_skip".to_string()),
            run_status: "paper_skipped".to_string(),
            run_id,
            external_order_id: None,
            provider_order_id: None,
            next_execution_at,
            response: json!({
                "mode": "paper",
                "status": "strategy_skip",
                "signal": signal.reason,
                "signalMetrics": signal.metadata
            }),
        });
    }

    let reference_price = if agent.side == "buy" {
        best_ask.or(best_bid).unwrap_or(mid)
    } else {
        best_bid.or(best_ask).unwrap_or(mid)
    };
    if let Err(err) = check_execution_guardrails(
        state,
        agent,
        signal.price,
        signal.quantity,
        Some(reference_price),
    )
    .await
    {
        return skip_agent_for_guardrail(
            state,
            agent,
            now,
            "paper_skipped",
            &err,
            json!({
                "mode": "paper",
                "strategy": agent.strategy,
                "strategyParams": agent.strategy_params,
                "signal": signal.reason.clone(),
                "signalMetrics": signal.metadata.clone(),
                "signalId": active_signal.as_ref().map(|item| item.id.as_str()),
                "referencePrice": reference_price,
                "plannedPrice": signal.price,
                "plannedQuantity": signal.quantity,
                "midPrice": mid
            }),
        )
        .await;
    }

    let fill = simulate_fill(
        market,
        orderbook,
        agent.outcome.as_str(),
        agent.side.as_str(),
        signal.quantity,
        state.config.paper_fee_bps,
        Some(signal.price),
    );
    let fill_slippage_bps = if mid > 0.0 {
        (((fill.average_price - mid).abs() / mid) * 10_000.0).round()
    } else {
        0.0
    };
    let fill_slippage_ticks = ((fill.average_price - signal.price).abs() / 0.001).round();

    if fill.filled_quantity <= 0.0 {
        let run_id = Uuid::new_v4().to_string();
        let next_execution_at = now + Duration::seconds(agent.cadence_seconds.max(1));
        update_external_agent_schedule(state, agent.id.as_str(), now, next_execution_at).await?;
        insert_external_agent_run(
            state,
            run_id.as_str(),
            agent,
            "paper_skipped",
            None,
            Some("no_fill_liquidity"),
            &json!({
                "mode": "paper",
                "reason": "no_fill_liquidity",
                "marketQuestion": market.question,
                "strategy": agent.strategy,
                "strategyParams": agent.strategy_params,
                "signal": signal.reason,
                "signalMetrics": signal.metadata,
                "signalId": active_signal.as_ref().map(|item| item.id.as_str()),
                "markPrice": fill.mark_price
            }),
        )
        .await?;

        return Ok(AgentExecutionOutcome {
            executed: false,
            skip_reason: Some("no_fill_liquidity".to_string()),
            run_status: "paper_skipped".to_string(),
            run_id,
            external_order_id: None,
            provider_order_id: None,
            next_execution_at,
            response: json!({
                "mode": "paper",
                "status": "skipped",
                "reason": "no_fill_liquidity",
                "markPrice": fill.mark_price
            }),
        });
    }

    let order_id = Uuid::new_v4().to_string();
    let position_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();
    let hold_until = now + Duration::seconds(state.config.paper_hold_duration_seconds as i64);
    let next_execution_at = now + Duration::seconds(agent.cadence_seconds.max(1));
    let unrealized = unrealized_pnl(
        agent.side.as_str(),
        fill.average_price,
        fill.mark_price,
        fill.filled_quantity,
    ) - fill.fee_usdc;

    sqlx::query(
        "INSERT INTO external_orders (
            id, owner, provider, intent_id, market_id, provider_order_id, status,
            request_payload, response_payload, error_message, created_at, updated_at
        ) VALUES ($1,$2,$3,NULL,$4,'','paper_filled',$5,$6,NULL,$7,$7)",
    )
    .bind(order_id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.provider.as_str())
    .bind(agent.market_id.as_str())
    .bind(json!({
        "mode": "paper",
        "side": agent.side,
        "outcome": agent.outcome,
        "quantity": signal.quantity,
        "price": signal.price
    }))
    .bind(json!({
        "mode": "paper",
        "positionId": position_id,
        "fill": fill
    }))
    .bind(now)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    sqlx::query(
        "INSERT INTO paper_positions (
            id, agent_id, owner, provider, market_id, outcome, side, strategy, status,
            entry_price, mark_price, requested_quantity, filled_quantity, notional_usdc,
            fees_paid_usdc, realized_pnl_usdc, unrealized_pnl_usdc, hold_until, opened_at,
            last_marked_at, metadata, created_at, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'open',$9,$10,$11,$12,$13,$14,0,$15,$16,$17,$17,$18,$17,$17)",
    )
    .bind(position_id.as_str())
    .bind(agent.id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.provider.as_str())
    .bind(agent.market_id.as_str())
    .bind(agent.outcome.as_str())
    .bind(agent.side.as_str())
    .bind(agent.strategy.as_str())
    .bind(fill.average_price)
    .bind(fill.mark_price)
    .bind(fill.requested_quantity)
    .bind(fill.filled_quantity)
    .bind(fill.notional_usdc)
    .bind(fill.fee_usdc)
    .bind(unrealized)
    .bind(hold_until)
    .bind(now)
    .bind(json!({
        "agentName": agent.name,
        "marketQuestion": market.question,
        "partialFill": fill.partial_fill
    }))
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    insert_external_agent_run(
        state,
        run_id.as_str(),
        agent,
        "paper_opened",
        Some(order_id.as_str()),
        None,
        &json!({
            "mode": "paper",
            "positionId": position_id,
            "holdUntil": hold_until.to_rfc3339(),
            "fill": fill,
            "strategy": agent.strategy,
            "strategyParams": agent.strategy_params,
            "signal": signal.reason,
            "signalMetrics": signal.metadata,
            "signalId": active_signal.as_ref().map(|item| item.id.as_str()),
            "fillSlippageBps": fill_slippage_bps,
            "fillSlippageTicks": fill_slippage_ticks
        }),
    )
    .await?;

    sqlx::query(
        "INSERT INTO paper_fills (
            id, run_id, position_id, agent_id, owner, provider, market_id, outcome, side, fill_type,
            requested_quantity, filled_quantity, price, mark_price, notional_usdc, fee_usdc, metadata, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'open',$10,$11,$12,$13,$14,$15,$16,$17)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(run_id.as_str())
    .bind(position_id.as_str())
    .bind(agent.id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.provider.as_str())
    .bind(agent.market_id.as_str())
    .bind(agent.outcome.as_str())
    .bind(agent.side.as_str())
    .bind(fill.requested_quantity)
    .bind(fill.filled_quantity)
    .bind(fill.average_price)
    .bind(fill.mark_price)
    .bind(fill.notional_usdc)
    .bind(fill.fee_usdc)
    .bind(json!({
        "partialFill": fill.partial_fill,
        "slippageBps": fill.slippage_bps,
        "usedOrderbookDepth": fill.used_orderbook_depth
    }))
    .bind(now)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    record_paper_mark(
        state,
        position_id.as_str(),
        agent,
        fill.mark_price,
        unrealized,
        fill.filled_quantity * fill.mark_price,
        &json!({
            "reason": "opened",
            "holdUntil": hold_until.to_rfc3339()
        }),
    )
    .await?;

    update_external_agent_schedule(state, agent.id.as_str(), now, next_execution_at).await?;

    Ok(AgentExecutionOutcome {
        executed: true,
        skip_reason: None,
        run_status: "paper_opened".to_string(),
        run_id,
        external_order_id: Some(order_id),
        provider_order_id: None,
        next_execution_at,
        response: json!({
            "mode": "paper",
            "status": "opened",
            "positionId": position_id,
            "fill": fill,
            "holdUntil": hold_until.to_rfc3339(),
            "signal": signal.reason,
            "signalMetrics": signal.metadata,
            "fillSlippageBps": fill_slippage_bps,
            "fillSlippageTicks": fill_slippage_ticks
        }),
    })
}

fn market_is_closed_for_paper_entry(
    market: &external::types::ExternalMarketSnapshot,
    now: chrono::DateTime<Utc>,
) -> bool {
    if market.resolved {
        return true;
    }

    if market.close_time > 0 && market.close_time as i64 <= now.timestamp() {
        return true;
    }

    matches!(
        market.status.trim().to_ascii_lowercase().as_str(),
        "closed" | "expired" | "resolved"
    )
}

async fn execute_paper_agent(
    state: &AppState,
    agent: &ExternalAgentRecord,
) -> Result<AgentExecutionOutcome, ApiError> {
    let now = Utc::now();
    let market_id = ExternalMarketId::parse(agent.market_id.as_str())?;

    let (market, position) = tokio::try_join!(
        external::fetch_market_by_id(&state.config, &market_id),
        async { load_open_paper_position(state, agent.id.as_str()).await }
    )?;
    let market_closed = market_is_closed_for_paper_entry(&market, now);

    if let Some(position) = position {
        let orderbook = external::fetch_orderbook(
            &state.config,
            &state.redis,
            &market_id,
            agent.outcome.as_str(),
            20,
        )
        .await?;

        if !market_closed && now < position.hold_until {
            let fill = simulate_fill(
                &market,
                &orderbook,
                agent.outcome.as_str(),
                agent.side.as_str(),
                position.filled_quantity,
                state.config.paper_fee_bps,
                None,
            );
            let unrealized = unrealized_pnl(
                agent.side.as_str(),
                position.entry_price,
                fill.mark_price,
                position.filled_quantity,
            ) - position.fees_paid_usdc;
            let next_execution_at = now + Duration::seconds(agent.cadence_seconds.max(1));

            sqlx::query(
                "UPDATE paper_positions
                 SET mark_price = $2,
                     unrealized_pnl_usdc = $3,
                     last_marked_at = $4,
                     updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(position.id.as_str())
            .bind(fill.mark_price)
            .bind(unrealized)
            .bind(now)
            .execute(state.db.pool())
            .await
            .map_err(|err| ApiError::internal(&err.to_string()))?;

            record_paper_mark(
                state,
                position.id.as_str(),
                agent,
                fill.mark_price,
                unrealized,
                position.filled_quantity * fill.mark_price,
                &json!({
                    "reason": "holding_open_position",
                    "holdUntil": position.hold_until.to_rfc3339()
                }),
            )
            .await?;

            update_external_agent_schedule(state, agent.id.as_str(), now, next_execution_at)
                .await?;
            let run_id = Uuid::new_v4().to_string();
            insert_external_agent_run(
                state,
                run_id.as_str(),
                agent,
                "paper_skipped",
                None,
                Some("holding_open_position"),
                &json!({
                    "mode": "paper",
                    "positionId": position.id,
                    "markPrice": fill.mark_price,
                    "unrealizedPnlUsdc": unrealized,
                    "holdUntil": position.hold_until.to_rfc3339()
                }),
            )
            .await?;

            return Ok(AgentExecutionOutcome {
                executed: false,
                skip_reason: Some("holding_open_position".to_string()),
                run_status: "paper_skipped".to_string(),
                run_id,
                external_order_id: None,
                provider_order_id: None,
                next_execution_at,
                response: json!({
                    "mode": "paper",
                    "status": "holding",
                    "positionId": position.id,
                    "markPrice": fill.mark_price,
                    "unrealizedPnlUsdc": unrealized
                }),
            });
        }

        let (fully_closed, close_response) =
            close_due_paper_position(state, agent, &position, now, &market, &orderbook).await?;
        if !fully_closed {
            let next_execution_at = now + Duration::seconds(agent.cadence_seconds.max(1));
            update_external_agent_schedule(state, agent.id.as_str(), now, next_execution_at)
                .await?;
            let run_id = Uuid::new_v4().to_string();
            insert_external_agent_run(
                state,
                run_id.as_str(),
                agent,
                "paper_partial_close",
                None,
                Some(
                    close_response
                        .get("reason")
                        .and_then(|value| value.as_str())
                        .unwrap_or("partial_close"),
                ),
                &json!({
                    "mode": "paper",
                    "close": close_response
                }),
            )
            .await?;

            return Ok(AgentExecutionOutcome {
                executed: true,
                skip_reason: None,
                run_status: "paper_partial_close".to_string(),
                run_id,
                external_order_id: None,
                provider_order_id: None,
                next_execution_at,
                response: json!({
                    "mode": "paper",
                    "status": "partial_close",
                    "close": close_response
                }),
            });
        }

        if market_closed {
            deactivate_external_agent(state, agent.id.as_str(), now).await?;
            let run_id = Uuid::new_v4().to_string();
            insert_external_agent_run(
                state,
                run_id.as_str(),
                agent,
                "paper_closed",
                None,
                None,
                &json!({
                    "mode": "paper",
                    "retired": true,
                    "reason": "market_closed",
                    "close": close_response
                }),
            )
            .await?;

            return Ok(AgentExecutionOutcome {
                executed: true,
                skip_reason: None,
                run_status: "paper_closed".to_string(),
                run_id,
                external_order_id: None,
                provider_order_id: None,
                next_execution_at: now,
                response: json!({
                    "mode": "paper",
                    "status": "closed",
                    "retired": true,
                    "reason": "market_closed",
                    "close": close_response
                }),
            });
        }
    }

    if market_closed {
        let run_id = Uuid::new_v4().to_string();
        deactivate_external_agent(state, agent.id.as_str(), now).await?;
        insert_external_agent_run(
            state,
            run_id.as_str(),
            agent,
            "paper_retired",
            None,
            Some("market_closed"),
            &json!({
                "mode": "paper",
                "retired": true,
                "reason": "market_closed",
                "marketQuestion": market.question,
                "marketStatus": market.status,
                "marketResolved": market.resolved,
                "marketCloseTime": market.close_time
            }),
        )
        .await?;

        return Ok(AgentExecutionOutcome {
            executed: false,
            skip_reason: Some("market_closed".to_string()),
            run_status: "paper_retired".to_string(),
            run_id,
            external_order_id: None,
            provider_order_id: None,
            next_execution_at: now,
            response: json!({
                "mode": "paper",
                "status": "retired",
                "retired": true,
                "reason": "market_closed"
            }),
        });
    }

    let orderbook = external::fetch_orderbook(
        &state.config,
        &state.redis,
        &market_id,
        agent.outcome.as_str(),
        20,
    )
    .await?;

    open_paper_position(state, agent, now, &market, &orderbook).await
}

/// Check per-agent execution guardrails. Returns Err if a limit is breached.
async fn check_execution_guardrails(
    state: &AppState,
    agent: &ExternalAgentRecord,
    order_price: f64,
    order_quantity: f64,
    reference_price: Option<f64>,
) -> Result<(), ApiError> {
    let notional = order_price * order_quantity;

    // Check per-execution notional limit.
    if let Some(max) = agent.max_notional_per_execution {
        if max > 0.0 && notional > max {
            return Err(ApiError::bad_request(
                "GUARDRAIL_MAX_NOTIONAL",
                &format!(
                    "execution notional {:.2} USDC exceeds agent limit {:.2}",
                    notional, max
                ),
            ));
        }
    }

    // Check 24h rolling spend limit.
    if let Some(max_daily) = agent.max_daily_spend_usdc {
        if max_daily > 0.0 {
            let since = Utc::now() - Duration::hours(24);
            let spent: f64 = sqlx::query_scalar(
                "SELECT COALESCE(SUM(
                    COALESCE((metadata->>'notionalUsdc')::double precision, 0)
                ), 0)
                 FROM external_agent_runs
                 WHERE agent_id = $1 AND status IN ('submitted', 'paper_opened')
                   AND created_at >= $2",
            )
            .bind(agent.id.as_str())
            .bind(since)
            .fetch_one(state.db.pool())
            .await
            .unwrap_or(0.0);

            if spent + notional > max_daily {
                return Err(ApiError::bad_request(
                    "GUARDRAIL_MAX_DAILY_SPEND",
                    &format!(
                        "24h spend {:.2} + {:.2} exceeds daily limit {:.2} USDC",
                        spent, notional, max_daily
                    ),
                ));
            }
        }
    }

    if let Some(max_slippage_bps) = agent.max_slippage_bps {
        if let Some(reference_price) = reference_price {
            if reference_price > 0.0 {
                let slippage_bps = (((order_price - reference_price).abs() / reference_price)
                    * 10_000.0)
                    .round() as i32;
                if slippage_bps > max_slippage_bps {
                    return Err(ApiError::bad_request(
                        "GUARDRAIL_MAX_SLIPPAGE",
                        &format!(
                            "price slippage {}bps exceeds agent limit {}bps",
                            slippage_bps, max_slippage_bps
                        ),
                    ));
                }
            }
        }
    }

    Ok(())
}

async fn skip_agent_for_guardrail(
    state: &AppState,
    agent: &ExternalAgentRecord,
    now: chrono::DateTime<Utc>,
    run_status: &str,
    err: &ApiError,
    metadata: Value,
) -> Result<AgentExecutionOutcome, ApiError> {
    let reason = skip_reason_from_error(err);
    let next_execution_at = now + Duration::seconds(agent.cadence_seconds.max(1));
    update_external_agent_schedule(state, agent.id.as_str(), now, next_execution_at).await?;
    let run_id = Uuid::new_v4().to_string();
    insert_external_agent_run(
        state,
        run_id.as_str(),
        agent,
        run_status,
        None,
        Some(reason.as_str()),
        &json!({
            "mode": agent.execution_mode.as_str(),
            "reason": reason,
            "error": {
                "code": err.code,
                "message": err.message,
                "details": err.details
            },
            "guardrail": metadata.clone()
        }),
    )
    .await?;

    Ok(AgentExecutionOutcome {
        executed: false,
        skip_reason: Some(reason),
        run_status: run_status.to_string(),
        run_id,
        external_order_id: None,
        provider_order_id: None,
        next_execution_at,
        response: json!({
            "mode": agent.execution_mode.as_str(),
            "status": "skipped",
            "reason": "guardrail",
            "guardrail": metadata
        }),
    })
}

async fn execute_live_agent(
    state: &AppState,
    agent: &ExternalAgentRecord,
    signed_order_override: Option<Value>,
) -> Result<AgentExecutionOutcome, ApiError> {
    ensure_live_write_mode(state)?;
    let now = Utc::now();
    let market_id = ExternalMarketId::parse(agent.market_id.as_str())?;
    let market = external::fetch_market_by_id(&state.config, &market_id).await?;
    if market_is_closed_for_paper_entry(&market, now) {
        deactivate_external_agent(state, agent.id.as_str(), now).await?;
        let run_id = Uuid::new_v4().to_string();
        insert_external_agent_run(
            state,
            run_id.as_str(),
            agent,
            "skipped",
            None,
            Some("market_closed"),
            &json!({
                "mode": "live",
                "reason": "market_closed",
                "marketQuestion": market.question,
                "marketStatus": market.status,
                "marketResolved": market.resolved,
                "marketCloseTime": market.close_time
            }),
        )
        .await?;

        return Ok(AgentExecutionOutcome {
            executed: false,
            skip_reason: Some("market_closed".to_string()),
            run_status: "skipped".to_string(),
            run_id,
            external_order_id: None,
            provider_order_id: None,
            next_execution_at: now,
            response: json!({
                "mode": "live",
                "status": "skipped",
                "reason": "market_closed"
            }),
        });
    }

    let orderbook = external::fetch_orderbook(
        &state.config,
        &state.redis,
        &market_id,
        agent.outcome.as_str(),
        20,
    )
    .await?;
    let best_bid = orderbook.bids.first().map(|level| level.price);
    let best_ask = orderbook.asks.first().map(|level| level.price);
    let mid = best_bid
        .zip(best_ask)
        .map(|(bid, ask)| (bid + ask) / 2.0)
        .or(best_bid)
        .or(best_ask)
        .unwrap_or(market.yes_price);
    let active_signal = load_active_market_signal(state, agent.market_id.as_str()).await?;
    let signal_source_count = active_signal
        .as_ref()
        .map(|signal| signal.sources.len() as u64)
        .unwrap_or(0);
    let signal_resolution_rules_read = active_signal
        .as_ref()
        .map(|signal| signal.resolution_rules_read)
        .unwrap_or(false);
    let signal_has_live_reference = active_signal
        .as_ref()
        .map(|signal| signal.has_live_reference)
        .unwrap_or(false);
    let signal_resolution_hazard_count = active_signal
        .as_ref()
        .map(|signal| signal.resolution_hazards.len() as u64)
        .unwrap_or(0);
    let market_state = crate::services::external::strategy::MarketState {
        yes_price: market.yes_price,
        no_price: market.no_price,
        best_bid,
        best_ask,
        mid_price: mid,
        agent_price: agent.price,
        agent_side: agent.side.clone(),
        agent_outcome: agent.outcome.clone(),
        agent_quantity: agent.quantity,
        time_to_resolution_seconds: if market.close_time > 0 {
            Some(market.close_time as i64 - now.timestamp())
        } else {
            None
        },
        fair_value_low: active_signal.as_ref().map(|signal| signal.fair_value_low),
        fair_value_high: active_signal.as_ref().map(|signal| signal.fair_value_high),
        midpoint_delta_bps: active_signal
            .as_ref()
            .map(|signal| signal.midpoint_delta_bps),
        signal_source_count,
        signal_resolution_rules_read,
        signal_has_live_reference,
        signal_resolution_hazard_count,
    };
    let strategy_signal = crate::services::external::strategy::evaluate_strategy(
        agent.strategy.as_str(),
        &market_state,
        &agent.strategy_params,
    );
    if !strategy_signal.execute {
        let run_id = Uuid::new_v4().to_string();
        let next_execution_at = now + Duration::seconds(agent.cadence_seconds.max(1));
        update_external_agent_schedule(state, agent.id.as_str(), now, next_execution_at).await?;
        insert_external_agent_run(
            state,
            run_id.as_str(),
            agent,
            "skipped",
            None,
            Some("strategy_skip"),
            &json!({
                "mode": "live",
                "reason": "strategy_skip",
                "strategy": agent.strategy,
                "strategyParams": agent.strategy_params,
                "signal": strategy_signal.reason,
                "signalMetrics": strategy_signal.metadata,
                "signalId": active_signal.as_ref().map(|item| item.id.as_str()),
                "midPrice": mid
            }),
        )
        .await?;

        return Ok(AgentExecutionOutcome {
            executed: false,
            skip_reason: Some("strategy_skip".to_string()),
            run_status: "skipped".to_string(),
            run_id,
            external_order_id: None,
            provider_order_id: None,
            next_execution_at,
            response: json!({
                "mode": "live",
                "status": "skipped",
                "signal": strategy_signal.reason
            }),
        });
    }

    let reference_price = if agent.side == "buy" {
        best_ask.or(best_bid).unwrap_or(mid)
    } else {
        best_bid.or(best_ask).unwrap_or(mid)
    };
    if let Err(err) = check_execution_guardrails(
        state,
        agent,
        strategy_signal.price,
        strategy_signal.quantity,
        Some(reference_price),
    )
    .await
    {
        return skip_agent_for_guardrail(
            state,
            agent,
            now,
            "skipped",
            &err,
            json!({
                "mode": "live",
                "strategy": agent.strategy,
                "strategyParams": agent.strategy_params,
                "signal": strategy_signal.reason.clone(),
                "signalMetrics": strategy_signal.metadata.clone(),
                "signalId": active_signal.as_ref().map(|item| item.id.as_str()),
                "referencePrice": reference_price,
                "plannedPrice": strategy_signal.price,
                "plannedQuantity": strategy_signal.quantity,
                "midPrice": mid
            }),
        )
        .await;
    }

    let credential = load_credential(
        state,
        agent.owner.as_str(),
        agent.provider,
        agent.credential_id.as_deref(),
    )
    .await?;
    ensure_provider_credential_ready(state, agent.provider, &credential).await?;

    let signed_order = if let Some(order) = signed_order_override {
        order
    } else if let Some(default_order) = credential.payload.get("defaultSignedOrder") {
        default_order.clone()
    } else {
        return Err(ApiError::bad_request(
            "SIGNED_ORDER_REQUIRED",
            "external agent execution requires signedOrder in request or credential.defaultSignedOrder",
        ));
    };

    let provider_payload = build_provider_submit_payload(
        state,
        agent.provider,
        &credential,
        agent.market_id.as_str(),
        "",
        agent.price,
        None,
        &signed_order,
    )
    .await?;
    let submit_payload =
        submit_to_provider(state, agent.provider, &credential, &provider_payload).await?;
    let provider_order_id = provider_order_id_from_payload(&submit_payload);
    let next_execution_at = now + Duration::seconds(agent.cadence_seconds.max(1));
    let order_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO external_orders (
            id, owner, provider, intent_id, market_id, provider_order_id, status,
            request_payload, response_payload, error_message, created_at, updated_at
        ) VALUES ($1,$2,$3,NULL,$4,$5,'submitted',$6,$7,NULL,$8,$8)",
    )
    .bind(order_id.as_str())
    .bind(agent.owner.as_str())
    .bind(agent.provider.as_str())
    .bind(agent.market_id.as_str())
    .bind(provider_order_id.as_str())
    .bind(&provider_payload)
    .bind(&submit_payload)
    .bind(now)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    update_external_agent_schedule(state, agent.id.as_str(), now, next_execution_at).await?;
    insert_external_agent_run(
        state,
        run_id.as_str(),
        agent,
        "submitted",
        Some(order_id.as_str()),
        None,
        &json!({
            "mode": "live",
            "providerOrderId": provider_order_id,
            "strategy": agent.strategy,
            "strategyParams": agent.strategy_params,
            "signal": strategy_signal.reason,
            "signalMetrics": strategy_signal.metadata,
            "signalId": active_signal.as_ref().map(|item| item.id.as_str()),
            "response": submit_payload
        }),
    )
    .await?;

    if let Err(err) = reconcile_live_agent_execution(
        state,
        agent,
        run_id.as_str(),
        order_id.as_str(),
        provider_order_id.as_str(),
        &provider_payload,
        &submit_payload,
        &market,
        &orderbook,
        now,
    )
    .await
    {
        log::warn!(
            "live external ledger reconciliation failed for {}: {}",
            agent.id,
            err.message
        );
    }

    Ok(AgentExecutionOutcome {
        executed: true,
        skip_reason: None,
        run_status: "submitted".to_string(),
        run_id,
        external_order_id: Some(order_id),
        provider_order_id: Some(provider_order_id),
        next_execution_at,
        response: json!({
            "mode": "live",
            "response": submit_payload
        }),
    })
}

pub(crate) async fn execute_agent_record(
    state: &AppState,
    agent: &ExternalAgentRecord,
    signed_order_override: Option<Value>,
) -> Result<AgentExecutionOutcome, ApiError> {
    ensure_live_strategy_allowed(agent.strategy.as_str(), agent.execution_mode)?;
    match agent.execution_mode {
        ExternalExecutionMode::Paper => execute_paper_agent(state, agent).await,
        ExternalExecutionMode::Live => {
            execute_live_agent(state, agent, signed_order_override).await
        }
    }
}

/// Load and execute a single agent by ID. Used by the internal scheduler.
/// Returns Ok(true) if executed, Ok(false) if skipped, Err on error.
pub(crate) async fn execute_agent_record_by_id(
    state: &AppState,
    agent_id: &str,
) -> Result<bool, String> {
    let row = sqlx::query(
        "SELECT id, owner, cohort, name, provider, market_id, outcome, side, price, quantity,
                cadence_seconds, strategy, strategy_params, execution_mode, credential_id, active,
                last_executed_at, next_execution_at, consecutive_failures, last_error_code,
                max_notional_per_execution, max_daily_spend_usdc, max_slippage_bps
         FROM external_agents
         WHERE id = $1 AND active = TRUE",
    )
    .bind(agent_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    let row = match row {
        Some(r) => r,
        None => return Ok(false),
    };

    let agent = parse_external_agent_record(row).map_err(|e| e.message)?;

    match execute_agent_record(state, &agent, None).await {
        Ok(outcome) => {
            if outcome.executed {
                state
                    .event_bus
                    .emit(crate::services::event_bus::PlatformEvent::AgentExecuted(
                        crate::services::event_bus::AgentExecutedEvent {
                            agent_id: agent.id.clone(),
                            owner: agent.owner.clone(),
                            provider: agent.provider.as_str().to_string(),
                            market_id: agent.market_id.clone(),
                            strategy: agent.strategy.clone(),
                            execution_mode: agent.execution_mode.as_str().to_string(),
                            run_id: outcome.run_id.clone(),
                            run_status: outcome.run_status.clone(),
                            side: agent.side.clone(),
                            outcome: agent.outcome.clone(),
                            price: agent.price,
                            metadata: outcome.response,
                            timestamp: chrono::Utc::now(),
                        },
                    ));
            }
            if agent.consecutive_failures > 0 {
                let _ = reset_agent_failures(state, agent.id.as_str()).await;
            }
            Ok(outcome.executed)
        }
        Err(err) => {
            state
                .event_bus
                .emit(crate::services::event_bus::PlatformEvent::AgentFailed(
                    crate::services::event_bus::AgentFailedEvent {
                        agent_id: agent.id.clone(),
                        owner: agent.owner.clone(),
                        provider: agent.provider.as_str().to_string(),
                        market_id: agent.market_id.clone(),
                        error_code: err.code.clone(),
                        error_message: err.message.clone(),
                        consecutive_failures: agent.consecutive_failures + 1,
                        timestamp: chrono::Utc::now(),
                    },
                ));
            let _ = record_agent_failure(
                state,
                agent.id.as_str(),
                err.code.as_str(),
                agent.cadence_seconds,
                agent.consecutive_failures,
                chrono::Utc::now(),
            )
            .await;
            Err(format!("{}: {}", err.code, err.message))
        }
    }
}

pub async fn list_external_credentials(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListExternalCredentialsQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    let provider_filter = query
        .provider
        .as_ref()
        .map(|value| normalize_provider(value))
        .transpose()?;

    let rows = if let Some(provider) = provider_filter {
        sqlx::query(
            "SELECT id, provider, label, encrypted_payload, key_id, created_at, updated_at
             FROM external_credentials
             WHERE owner = $1 AND provider = $2 AND revoked_at IS NULL
             ORDER BY updated_at DESC",
        )
        .bind(user.wallet_address.as_str())
        .bind(provider.as_str())
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
    } else {
        sqlx::query(
            "SELECT id, provider, label, encrypted_payload, key_id, created_at, updated_at
             FROM external_credentials
             WHERE owner = $1 AND revoked_at IS NULL
             ORDER BY updated_at DESC",
        )
        .bind(user.wallet_address.as_str())
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
    };

    let mut credentials = Vec::new();
    for row in rows {
        let encrypted_payload: String = row
            .try_get("encrypted_payload")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let key_id: String = row
            .try_get("key_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let payload = decrypt_json(
            state.config.external_credentials_master_key.as_str(),
            key_id.as_str(),
            encrypted_payload.as_str(),
        )
        .unwrap_or_else(|_| json!({}));

        let created_at: chrono::DateTime<Utc> = row
            .try_get("created_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let updated_at: chrono::DateTime<Utc> = row
            .try_get("updated_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?;

        credentials.push(ExternalCredentialResponse {
            id: row
                .try_get("id")
                .map_err(|err| ApiError::internal(&err.to_string()))?,
            provider: row
                .try_get("provider")
                .map_err(|err| ApiError::internal(&err.to_string()))?,
            label: row
                .try_get("label")
                .map_err(|err| ApiError::internal(&err.to_string()))?,
            key_id,
            created_at: created_at.to_rfc3339(),
            updated_at: updated_at.to_rfc3339(),
            credentials: mask_credentials(&payload),
        });
    }

    Ok(HttpResponse::Ok().json(ExternalCredentialsListResponse {
        total: credentials.len() as u64,
        credentials,
    }))
}

pub async fn get_external_credential_status(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ExternalCredentialStatusQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;
    let provider = normalize_provider(query.provider.as_str())?;
    let credential = match load_credential(
        &state,
        user.wallet_address.as_str(),
        provider,
        query.credential_id.as_deref(),
    )
    .await
    {
        Ok(credential) => Some(credential),
        Err(err) if err.status == 404 => None,
        Err(err) => return Err(err),
    };
    let status = build_external_credential_status(&state, provider, credential.as_ref()).await?;
    Ok(HttpResponse::Ok().json(status))
}

pub async fn bind_limitless_wallet(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<BindLimitlessWalletRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;
    let base_wallet = normalize_evm_wallet(body.base_wallet.as_str())?;

    let row = sqlx::query(
        "SELECT id, encrypted_payload, key_id
         FROM external_credentials
         WHERE id = $1 AND owner = $2 AND provider = 'limitless' AND revoked_at IS NULL",
    )
    .bind(body.credential_id.as_str())
    .bind(user.wallet_address.as_str())
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .ok_or_else(|| ApiError::not_found("Limitless credential"))?;

    let encrypted_payload: String = row
        .try_get("encrypted_payload")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let key_id: String = row
        .try_get("key_id")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let mut payload = decrypt_json(
        state.config.external_credentials_master_key.as_str(),
        key_id.as_str(),
        encrypted_payload.as_str(),
    )?;

    let payload_map = payload.as_object_mut().ok_or_else(|| {
        ApiError::bad_request("INVALID_CREDENTIALS", "credentials must be an object")
    })?;
    payload_map.insert("baseWallet".to_string(), json!(base_wallet));

    let encrypted_payload = encrypt_json(
        state.config.external_credentials_master_key.as_str(),
        state.config.external_credentials_key_id.as_str(),
        &payload,
    )?;

    sqlx::query(
        "UPDATE external_credentials
         SET encrypted_payload = $1, key_id = $2, updated_at = NOW()
         WHERE id = $3 AND owner = $4",
    )
    .bind(encrypted_payload)
    .bind(state.config.external_credentials_key_id.as_str())
    .bind(body.credential_id.as_str())
    .bind(user.wallet_address.as_str())
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let credential = StoredCredential {
        id: body.credential_id.clone(),
        owner: user.wallet_address.clone(),
        payload,
    };
    let status =
        build_external_credential_status(&state, ExternalProvider::Limitless, Some(&credential))
            .await?;
    Ok(HttpResponse::Ok().json(status))
}

pub async fn upsert_external_credentials(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<UpsertExternalCredentialRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    let provider = normalize_provider(body.provider.as_str())?;
    let label = body
        .label
        .as_deref()
        .unwrap_or("default")
        .trim()
        .to_ascii_lowercase();
    if label.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_LABEL",
            "label must not be empty",
        ));
    }

    if !body.credentials.is_object() {
        return Err(ApiError::bad_request(
            "INVALID_CREDENTIALS",
            "credentials must be an object",
        ));
    }

    match provider {
        ExternalProvider::Limitless => {
            if payload_string(&body.credentials, &["apiKey", "api_key"]).is_none() {
                return Err(ApiError::bad_request(
                    "INVALID_CREDENTIALS",
                    "limitless credential must include apiKey",
                ));
            }
            if let Some(base_wallet) =
                payload_string(&body.credentials, &["baseWallet", "base_wallet"])
            {
                normalize_evm_wallet(base_wallet.as_str())?;
            }
        }
        ExternalProvider::Polymarket => {
            if payload_string(&body.credentials, &["apiKey", "api_key"]).is_none() {
                return Err(ApiError::bad_request(
                    "INVALID_CREDENTIALS",
                    "polymarket credential must include apiKey",
                ));
            }
            if payload_string(&body.credentials, &["apiSecret", "api_secret"]).is_none() {
                return Err(ApiError::bad_request(
                    "INVALID_CREDENTIALS",
                    "polymarket credential must include apiSecret",
                ));
            }
            if payload_string(&body.credentials, &["apiPassphrase", "api_passphrase"]).is_none() {
                return Err(ApiError::bad_request(
                    "INVALID_CREDENTIALS",
                    "polymarket credential must include apiPassphrase",
                ));
            }
            let funder = payload_string(&body.credentials, &["funder"]).ok_or_else(|| {
                ApiError::bad_request(
                    "INVALID_CREDENTIALS",
                    "polymarket credential must include funder",
                )
            })?;
            normalize_evm_wallet(funder.as_str())?;
            polymarket_signature_type_from_payload(&body.credentials)?;
        }
        ExternalProvider::Aerodrome => {
            let base_wallet = payload_string(&body.credentials, &["baseWallet", "base_wallet"])
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "INVALID_CREDENTIALS",
                        "aerodrome credential must include baseWallet",
                    )
                })?;
            normalize_evm_wallet(base_wallet.as_str())?;

            // Validate privateKey if provided (required for autonomous execution)
            if let Some(pk) = payload_string(&body.credentials, &["privateKey", "private_key"]) {
                let derived = crate::services::evm_signer::address_from_private_key(&pk)?;
                if derived.to_ascii_lowercase()
                    != normalize_evm_wallet(base_wallet.as_str())?.to_ascii_lowercase()
                {
                    return Err(ApiError::bad_request(
                        "CREDENTIAL_MISMATCH",
                        "privateKey does not derive to the provided baseWallet address",
                    ));
                }
            }
        }
    }

    let encrypted_payload = encrypt_json(
        state.config.external_credentials_master_key.as_str(),
        state.config.external_credentials_key_id.as_str(),
        &body.credentials,
    )?;

    let row = sqlx::query(
        "INSERT INTO external_credentials (
            id, owner, provider, label, encrypted_payload, key_id, created_at, updated_at, revoked_at
        ) VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW(), NULL)
        ON CONFLICT (owner, provider, label)
        DO UPDATE SET encrypted_payload = EXCLUDED.encrypted_payload,
                      key_id = EXCLUDED.key_id,
                      updated_at = NOW(),
                      revoked_at = NULL
        RETURNING id, provider, label, key_id, created_at, updated_at",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user.wallet_address.as_str())
    .bind(provider.as_str())
    .bind(label)
    .bind(encrypted_payload)
    .bind(state.config.external_credentials_key_id.as_str())
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let created_at: chrono::DateTime<Utc> = row
        .try_get("created_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let updated_at: chrono::DateTime<Utc> = row
        .try_get("updated_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(ExternalCredentialResponse {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider: row
            .try_get("provider")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        label: row
            .try_get("label")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        key_id: row
            .try_get("key_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
        credentials: mask_credentials(&body.credentials),
    }))
}

pub async fn delete_external_credentials(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    credential_id: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    let result = sqlx::query(
        "UPDATE external_credentials
         SET revoked_at = NOW(), updated_at = NOW()
         WHERE id = $1 AND owner = $2 AND revoked_at IS NULL",
    )
    .bind(credential_id.as_str())
    .bind(user.wallet_address.as_str())
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("External credential"));
    }

    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

pub async fn create_external_order_intent(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateExternalOrderIntentRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    if !state.config.external_trading_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_TRADING_DISABLED",
            "external trading is disabled",
        ));
    }
    ensure_live_write_mode(&state)?;

    let user = extract_authenticated_user(&req, &state).await?;
    let provider = normalize_provider(body.provider.as_str())?;
    ensure_provider_action_allowed(&req, provider, ProviderRailAction::TradeOpen)?;
    let outcome = normalize_outcome(body.outcome.as_str())?;
    let side = normalize_side(body.side.as_str())?;

    if body.price <= 0.0 || body.price >= 1.0 {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "price must be between 0 and 1",
        ));
    }
    if body.quantity <= 0.0 {
        return Err(ApiError::bad_request(
            "INVALID_QUANTITY",
            "quantity must be greater than zero",
        ));
    }

    let namespaced_market_id = normalize_namespaced_market_id(provider, body.market_id.as_str());
    let parsed_market_id = ExternalMarketId::parse(namespaced_market_id.as_str())?;
    let market = external::fetch_market_by_id(&state.config, &parsed_market_id).await?;

    if !market.execution_users {
        return Err(ApiError::bad_request(
            "MARKET_NOT_EXECUTABLE",
            "market is not executable for users under current launch policy",
        ));
    }

    let credential = load_credential(
        &state,
        user.wallet_address.as_str(),
        provider,
        body.credential_id.as_deref(),
    )
    .await?;
    let credential_status = ensure_provider_credential_ready(&state, provider, &credential).await?;

    let market_ref = if !market.provider_market_ref.is_empty() {
        market.provider_market_ref.clone()
    } else {
        parsed_market_id.value.clone()
    };

    let provider_market_payload = match provider {
        ExternalProvider::Limitless => {
            let client = reqwest::Client::new();
            match client
                .get(format!(
                    "{}/markets/{}",
                    state.config.limitless_api_base.trim_end_matches('/'),
                    parsed_market_id.value
                ))
                .send()
                .await
            {
                Ok(response) => response.json::<Value>().await.unwrap_or_else(|_| json!({})),
                Err(_) => json!({}),
            }
        }
        ExternalProvider::Polymarket => {
            let client = reqwest::Client::new();
            match client
                .get(format!(
                    "{}/markets/{}",
                    state.config.polymarket_gamma_api_base.trim_end_matches('/'),
                    parsed_market_id.value
                ))
                .send()
                .await
            {
                Ok(response) => response.json::<Value>().await.unwrap_or_else(|_| json!({})),
                Err(_) => json!({}),
            }
        }
        ExternalProvider::Aerodrome => {
            // Fetch pool state from on-chain
            match crate::services::external::providers::aerodrome::fetch_pool_state(
                &state.evm_rpc,
                parsed_market_id.value.as_str(),
            )
            .await
            {
                Ok(pool) => serde_json::to_value(&pool).unwrap_or_else(|_| json!({})),
                Err(_) => json!({}),
            }
        }
    };

    let mut preflight = build_preflight(provider, &provider_market_payload);
    if let Some(preflight_map) = preflight.as_object_mut() {
        preflight_map.insert(
            "credentialReady".to_string(),
            json!(credential_status.ready),
        );
        if let Some(base_wallet) = credential_status.base_wallet.as_ref() {
            preflight_map.insert("baseWallet".to_string(), json!(base_wallet));
        }
        if let Some(profile_status) = credential_status.profile_status.as_ref() {
            preflight_map.insert("profileStatus".to_string(), json!(profile_status));
        }
        preflight_map.insert(
            "credentialChecks".to_string(),
            json!(credential_status.checks),
        );
    }
    let intent_for_signing = CreateExternalOrderIntentRequest {
        provider: provider.as_str().to_string(),
        market_id: namespaced_market_id.clone(),
        outcome,
        side,
        price: body.price,
        quantity: body.quantity,
        credential_id: body.credential_id.clone(),
    };
    let fee_rate_bps = match provider {
        ExternalProvider::Limitless => {
            let api_key = api_key_from_payload(&credential.payload, &["apiKey", "api_key"])
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "INVALID_CREDENTIALS",
                        "limitless credential must include apiKey",
                    )
                })?;
            let base_wallet = payload_string(&credential.payload, &["baseWallet", "base_wallet"])
                .ok_or_else(|| {
                ApiError::bad_request(
                    "INVALID_CREDENTIALS",
                    "limitless credential must include baseWallet",
                )
            })?;
            let profile =
                fetch_limitless_profile(&state, base_wallet.as_str(), api_key.as_str()).await?;
            Some(profile.rank.map(|rank| rank.fee_rate_bps).unwrap_or(300))
        }
        ExternalProvider::Polymarket => None,
        ExternalProvider::Aerodrome => None,
    };
    let typed_data = match provider {
        ExternalProvider::Limitless => build_typed_data(
            user.wallet_address.as_str(),
            provider,
            &intent_for_signing,
            market_ref.as_str(),
            &provider_market_payload,
            fee_rate_bps,
        )?,
        ExternalProvider::Polymarket => {
            build_polymarket_typed_data(
                &state,
                user.wallet_address.as_str(),
                &credential,
                &intent_for_signing,
                &provider_market_payload,
            )
            .await?
        }
        ExternalProvider::Aerodrome => {
            // Aerodrome doesn't use EIP-712 signed orders.
            // Return swap parameters that will be used to build calldata.
            json!({
                "provider": "aerodrome",
                "mode": "swap",
                "chainId": 8453,
                "pool": parsed_market_id.value,
                "outcome": intent_for_signing.outcome,
                "side": intent_for_signing.side,
                "price": intent_for_signing.price,
                "quantity": intent_for_signing.quantity,
            })
        }
    };

    let intent_id = Uuid::new_v4().to_string();
    let expires_at = (Utc::now() + Duration::hours(2)).to_rfc3339();

    sqlx::query(
        "INSERT INTO external_order_intents (
            id, owner, provider, market_id, provider_market_ref, outcome, side,
            price, quantity, preflight, typed_data, status, credential_id, created_at, updated_at
         ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'prepared',$12,NOW(),NOW())",
    )
    .bind(intent_id.as_str())
    .bind(user.wallet_address.as_str())
    .bind(provider.as_str())
    .bind(namespaced_market_id.as_str())
    .bind(market_ref.as_str())
    .bind(intent_for_signing.outcome.as_str())
    .bind(intent_for_signing.side.as_str())
    .bind(intent_for_signing.price)
    .bind(intent_for_signing.quantity)
    .bind(&preflight)
    .bind(&typed_data)
    .bind(credential.id.as_str())
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(ExternalOrderIntentResponse {
        id: intent_id,
        provider: provider.as_str().to_string(),
        market_id: namespaced_market_id,
        preflight,
        typed_data,
        status: "prepared".to_string(),
        expires_at,
    }))
}

pub async fn prepare_external_order_submit(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareExternalOrderSubmitRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    if !state.config.external_trading_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_TRADING_DISABLED",
            "external trading is disabled",
        ));
    }
    ensure_live_write_mode(&state)?;

    let user = extract_authenticated_user(&req, &state).await?;
    let intent = load_external_order_intent_record(
        &state,
        user.wallet_address.as_str(),
        body.intent_id.as_str(),
    )
    .await?;
    ensure_provider_action_allowed(&req, intent.provider, ProviderRailAction::TradeOpen)?;

    if intent.provider != ExternalProvider::Polymarket {
        return Err(ApiError::bad_request(
            "CLIENT_EXECUTION_UNSUPPORTED",
            "prepared client execution is only available for polymarket",
        ));
    }

    let credential_id = body
        .credential_id
        .as_deref()
        .map(ToOwned::to_owned)
        .or(intent.credential_id.clone());
    let credential = load_credential(
        &state,
        user.wallet_address.as_str(),
        intent.provider,
        credential_id.as_deref(),
    )
    .await?;
    ensure_provider_credential_ready(&state, intent.provider, &credential).await?;

    let provider_payload =
        build_polymarket_submit_payload(&credential, &intent.typed_data, &body.signed_order)?;
    let prepared = prepare_polymarket_provider_request(
        &state,
        &credential,
        "POST",
        "/order",
        &provider_payload,
    )?;

    Ok(HttpResponse::Ok().json(prepared))
}

pub async fn submit_external_order(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<SubmitExternalOrderRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    if !state.config.external_trading_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_TRADING_DISABLED",
            "external trading is disabled",
        ));
    }
    ensure_live_write_mode(&state)?;

    let user = extract_authenticated_user(&req, &state).await?;
    let intent = load_external_order_intent_record(
        &state,
        user.wallet_address.as_str(),
        body.intent_id.as_str(),
    )
    .await?;
    let provider = intent.provider;
    ensure_provider_action_allowed(&req, provider, ProviderRailAction::TradeOpen)?;

    let credential_id = body
        .credential_id
        .as_deref()
        .map(ToOwned::to_owned)
        .or(intent.credential_id.clone());

    let credential = load_credential(
        &state,
        user.wallet_address.as_str(),
        provider,
        credential_id.as_deref(),
    )
    .await?;
    ensure_provider_credential_ready(&state, provider, &credential).await?;

    let provider_payload = build_provider_submit_payload(
        &state,
        provider,
        &credential,
        intent.market_id.as_str(),
        intent.provider_market_ref.as_str(),
        intent.price,
        Some(&intent.typed_data),
        &body.signed_order,
    )
    .await?;

    let now = Utc::now();
    let order_id = Uuid::new_v4().to_string();
    let (status, payload, error_message, provider_order_id) =
        if provider == ExternalProvider::Polymarket {
            if let Some(client_payload) = body.provider_response.clone() {
                let provider_status = body.provider_status.unwrap_or(200);
                if (200..300).contains(&provider_status) {
                    (
                        "submitted".to_string(),
                        client_payload.clone(),
                        None,
                        provider_order_id(&client_payload),
                    )
                } else {
                    (
                        "failed".to_string(),
                        client_payload.clone(),
                        Some(
                            polymarket_provider_error_message(
                                &client_payload,
                                "polymarket order submission failed",
                            )
                            .to_string(),
                        ),
                        String::new(),
                    )
                }
            } else {
                match submit_to_provider(&state, provider, &credential, &provider_payload).await {
                    Ok(payload) => (
                        "submitted".to_string(),
                        payload.clone(),
                        None,
                        provider_order_id(&payload),
                    ),
                    Err(err) => (
                        "failed".to_string(),
                        json!({ "error": err.message }),
                        Some(err.message),
                        String::new(),
                    ),
                }
            }
        } else {
            if body.provider_response.is_some() || body.provider_status.is_some() {
                return Err(ApiError::bad_request(
                    "CLIENT_EXECUTION_UNSUPPORTED",
                    "prepared client execution is only available for polymarket",
                ));
            }

            match submit_to_provider(&state, provider, &credential, &provider_payload).await {
                Ok(payload) => (
                    "submitted".to_string(),
                    payload.clone(),
                    None,
                    provider_order_id(&payload),
                ),
                Err(err) => (
                    "failed".to_string(),
                    json!({ "error": err.message }),
                    Some(err.message),
                    String::new(),
                ),
            }
        };

    sqlx::query(
        "INSERT INTO external_orders (
            id, owner, provider, intent_id, market_id, provider_order_id, status,
            request_payload, response_payload, error_message, created_at, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(order_id.as_str())
    .bind(user.wallet_address.as_str())
    .bind(provider.as_str())
    .bind(body.intent_id.as_str())
    .bind(intent.market_id.as_str())
    .bind(provider_order_id.as_str())
    .bind(status.as_str())
    .bind(&provider_payload)
    .bind(&payload)
    .bind(error_message.as_deref())
    .bind(now)
    .bind(now)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let next_intent_status = if status == "submitted" {
        "submitted"
    } else {
        "failed"
    };
    sqlx::query("UPDATE external_order_intents SET status = $2, updated_at = NOW() WHERE id = $1")
        .bind(body.intent_id.as_str())
        .bind(next_intent_status)
        .execute(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    if status != "submitted" {
        return Err(ApiError::bad_request(
            "EXTERNAL_ORDER_SUBMIT_FAILED",
            error_message
                .as_deref()
                .unwrap_or("external order submission failed"),
        ));
    }

    Ok(HttpResponse::Ok().json(ExternalOrderResponse {
        id: order_id,
        provider: provider.as_str().to_string(),
        market_id: intent.market_id,
        provider_order_id,
        status,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
        response_payload: payload,
        error_message: None,
    }))
}

pub async fn prepare_external_order_cancel(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CancelExternalOrderRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    if !state.config.external_trading_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_TRADING_DISABLED",
            "external trading is disabled",
        ));
    }
    ensure_live_write_mode(&state)?;

    let user = extract_authenticated_user(&req, &state).await?;
    let provider = normalize_provider(body.provider.as_str())?;
    ensure_provider_action_allowed(&req, provider, ProviderRailAction::TradeClose)?;

    if provider != ExternalProvider::Polymarket {
        return Err(ApiError::bad_request(
            "CLIENT_EXECUTION_UNSUPPORTED",
            "prepared client execution is only available for polymarket",
        ));
    }

    let credential = load_credential(
        &state,
        user.wallet_address.as_str(),
        provider,
        body.credential_id.as_deref(),
    )
    .await?;
    ensure_provider_credential_ready(&state, provider, &credential).await?;

    let payload = body
        .payload
        .clone()
        .unwrap_or_else(|| json!({ "orderId": body.provider_order_id }));
    let prepared =
        prepare_polymarket_provider_request(&state, &credential, "DELETE", "/order", &payload)?;

    Ok(HttpResponse::Ok().json(prepared))
}

pub async fn cancel_external_order(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CancelExternalOrderRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    if !state.config.external_trading_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_TRADING_DISABLED",
            "external trading is disabled",
        ));
    }
    ensure_live_write_mode(&state)?;

    let user = extract_authenticated_user(&req, &state).await?;
    let provider = normalize_provider(body.provider.as_str())?;
    ensure_provider_action_allowed(&req, provider, ProviderRailAction::TradeClose)?;
    let credential = load_credential(
        &state,
        user.wallet_address.as_str(),
        provider,
        body.credential_id.as_deref(),
    )
    .await?;
    ensure_provider_credential_ready(&state, provider, &credential).await?;

    let response_payload = if provider == ExternalProvider::Polymarket {
        if let Some(client_payload) = body.provider_response.clone() {
            let provider_status = body.provider_status.unwrap_or(200);
            if !(200..300).contains(&provider_status) {
                return Err(ApiError::bad_request(
                    "POLYMARKET_CANCEL_FAILED",
                    polymarket_provider_error_message(&client_payload, "polymarket cancel failed"),
                ));
            }
            client_payload
        } else {
            cancel_on_provider(
                &state,
                provider,
                &credential,
                body.provider_order_id.as_str(),
                body.payload.clone(),
            )
            .await?
        }
    } else {
        if body.provider_response.is_some() || body.provider_status.is_some() {
            return Err(ApiError::bad_request(
                "CLIENT_EXECUTION_UNSUPPORTED",
                "prepared client execution is only available for polymarket",
            ));
        }

        cancel_on_provider(
            &state,
            provider,
            &credential,
            body.provider_order_id.as_str(),
            body.payload.clone(),
        )
        .await?
    };

    sqlx::query(
        "UPDATE external_orders
         SET status = 'cancelled',
             provider_order_id = CASE
                 WHEN COALESCE(provider_order_id, '') = '' THEN $4
                 ELSE provider_order_id
             END,
             response_payload = $1,
             updated_at = NOW()
         WHERE owner = $2 AND provider = $3
           AND (
               provider_order_id = $4
               OR (
                   COALESCE(provider_order_id, '') = ''
                   AND (
                       COALESCE(response_payload ->> 'orderID', '') = $4
                       OR COALESCE(response_payload ->> 'orderId', '') = $4
                       OR COALESCE(response_payload ->> 'order_id', '') = $4
                       OR COALESCE(response_payload -> 'order' ->> 'orderID', '') = $4
                       OR COALESCE(response_payload -> 'order' ->> 'orderId', '') = $4
                       OR COALESCE(response_payload -> 'order' ->> 'order_id', '') = $4
                       OR COALESCE(response_payload -> 'order' ->> 'id', '') = $4
                   )
               )
           )",
    )
    .bind(&response_payload)
    .bind(user.wallet_address.as_str())
    .bind(provider.as_str())
    .bind(body.provider_order_id.as_str())
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({
        "ok": true,
        "provider": provider.as_str(),
        "providerOrderId": body.provider_order_id,
        "response": response_payload,
    })))
}

pub async fn list_external_orders(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListExternalOrdersQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    let limit = query.limit.unwrap_or(50).clamp(1, MAX_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0).max(0);

    let rows = if let Some(provider_raw) = query.provider.as_ref() {
        let provider = normalize_provider(provider_raw.as_str())?;
        sqlx::query(
            "SELECT id, provider, market_id, provider_order_id, status, response_payload, error_message, created_at, updated_at
             FROM external_orders
             WHERE owner = $1 AND provider = $2
             ORDER BY created_at DESC
             LIMIT $3 OFFSET $4",
        )
        .bind(user.wallet_address.as_str())
        .bind(provider.as_str())
        .bind(limit)
        .bind(offset)
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
    } else {
        sqlx::query(
            "SELECT id, provider, market_id, provider_order_id, status, response_payload, error_message, created_at, updated_at
             FROM external_orders
             WHERE owner = $1
             ORDER BY created_at DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(user.wallet_address.as_str())
        .bind(limit)
        .bind(offset)
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
    };

    let total_row = sqlx::query("SELECT COUNT(*) AS total FROM external_orders WHERE owner = $1")
        .bind(user.wallet_address.as_str())
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let total: i64 = total_row
        .try_get("total")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut orders = Vec::new();
    for row in rows {
        orders.push(build_external_order_response(row)?);
    }

    Ok(HttpResponse::Ok().json(ExternalOrdersListResponse {
        orders,
        total: total.max(0) as u64,
        limit: limit as u64,
        offset: offset as u64,
    }))
}

pub async fn list_external_agents(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListExternalAgentsQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_jwt_user(&req, &state)?;

    let limit = query.limit.unwrap_or(50).clamp(1, MAX_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0).max(0);
    let owner_filter =
        resolve_external_agent_owner_scope(&user, query.scope.as_deref(), query.owner.as_deref())?;

    let mut sql = QueryBuilder::<Postgres>::new(
        "SELECT id, owner, cohort, name, provider, market_id, outcome, side, price, quantity, cadence_seconds,
                strategy, strategy_params, execution_mode, credential_id, active, last_executed_at, next_execution_at,
                consecutive_failures, last_error_code, max_notional_per_execution, max_daily_spend_usdc, max_slippage_bps,
                created_at, updated_at
         FROM external_agents
         WHERE TRUE",
    );
    let mut count_sql = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*) AS total
         FROM external_agents
         WHERE TRUE",
    );

    if let Some(owner) = owner_filter.as_deref() {
        sql.push(" AND owner = ").push_bind(owner);
        count_sql.push(" AND owner = ").push_bind(owner);
    }

    if let Some(provider_raw) = query.provider.as_ref() {
        let provider = normalize_provider(provider_raw.as_str())?;
        sql.push(" AND provider = ").push_bind(provider.as_str());
        count_sql
            .push(" AND provider = ")
            .push_bind(provider.as_str());
    }
    if let Some(active) = query.active {
        sql.push(" AND active = ").push_bind(active);
        count_sql.push(" AND active = ").push_bind(active);
    }

    sql.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows = sql
        .build()
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let count_row = count_sql
        .build()
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let total: i64 = count_row
        .try_get("total")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut agents = Vec::new();
    for row in rows {
        agents.push(parse_external_agent(row, None)?);
    }

    Ok(HttpResponse::Ok().json(ExternalAgentsListResponse {
        agents,
        total: total.max(0) as u64,
        limit: limit as u64,
        offset: offset as u64,
    }))
}

fn empty_public_agents_response(limit: i64, offset: i64) -> ExternalAgentsListResponse {
    ExternalAgentsListResponse {
        agents: Vec::new(),
        total: 0,
        limit: limit.max(0) as u64,
        offset: offset.max(0) as u64,
    }
}

fn empty_agent_paper_performance() -> ExternalAgentPaperPerformance {
    ExternalAgentPaperPerformance {
        open_positions: 0,
        closed_positions: 0,
        fills: 0,
        volume_usdc: 0.0,
        fees_usdc: 0.0,
        realized_pnl_usdc: 0.0,
        unrealized_pnl_usdc: 0.0,
        net_pnl_usdc: 0.0,
        max_drawdown_usdc: 0.0,
    }
}

async fn load_external_agent_paper_performance_map(
    state: &AppState,
    agent_ids: &[String],
) -> Result<BTreeMap<String, ExternalAgentPaperPerformance>, ApiError> {
    let mut performance_map = agent_ids
        .iter()
        .cloned()
        .map(|agent_id| (agent_id, empty_agent_paper_performance()))
        .collect::<BTreeMap<_, _>>();
    if agent_ids.is_empty() {
        return Ok(performance_map);
    }

    let scoped_ids = agent_ids.to_vec();
    let position_rows = sqlx::query(
        "SELECT ea.id AS agent_id,
                COUNT(pp.id) FILTER (WHERE pp.status = 'open') AS open_positions,
                COUNT(pp.id) FILTER (WHERE pp.status = 'closed') AS closed_positions,
                COALESCE(SUM(CASE WHEN pp.status = 'open' THEN pp.unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
         FROM external_agents ea
         LEFT JOIN paper_positions pp
           ON pp.agent_id = ea.id
          AND pp.market_id = ea.market_id
         WHERE ea.id = ANY($1)
         GROUP BY ea.id",
    )
    .bind(scoped_ids.clone())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    for row in position_rows {
        let agent_id = row
            .try_get::<String, _>("agent_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        if let Some(entry) = performance_map.get_mut(agent_id.as_str()) {
            entry.open_positions =
                row.try_get::<i64, _>("open_positions").unwrap_or(0).max(0) as u64;
            entry.closed_positions = row
                .try_get::<i64, _>("closed_positions")
                .unwrap_or(0)
                .max(0) as u64;
            entry.unrealized_pnl_usdc = row.try_get::<f64, _>("unrealized_pnl_usdc").unwrap_or(0.0);
        }
    }

    let fill_rows = sqlx::query(
        "SELECT ea.id AS agent_id,
                COUNT(pf.id) AS fills,
                COALESCE(SUM(pf.notional_usdc), 0) AS volume_usdc,
                COALESCE(SUM(pf.fee_usdc), 0) AS fees_usdc
         FROM external_agents ea
         LEFT JOIN paper_fills pf
           ON pf.agent_id = ea.id
          AND pf.market_id = ea.market_id
         WHERE ea.id = ANY($1)
         GROUP BY ea.id",
    )
    .bind(scoped_ids.clone())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    for row in fill_rows {
        let agent_id = row
            .try_get::<String, _>("agent_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        if let Some(entry) = performance_map.get_mut(agent_id.as_str()) {
            entry.fills = row.try_get::<i64, _>("fills").unwrap_or(0).max(0) as u64;
            entry.volume_usdc = row.try_get::<f64, _>("volume_usdc").unwrap_or(0.0);
            entry.fees_usdc = row.try_get::<f64, _>("fees_usdc").unwrap_or(0.0);
        }
    }

    let outcome_rows = sqlx::query(
        "SELECT ea.id AS agent_id,
                COALESCE(SUM(po.realized_pnl_usdc), 0) AS realized_pnl_usdc
         FROM external_agents ea
         LEFT JOIN paper_outcomes po
           ON po.agent_id = ea.id
          AND po.market_id = ea.market_id
         WHERE ea.id = ANY($1)
         GROUP BY ea.id",
    )
    .bind(scoped_ids.clone())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    for row in outcome_rows {
        let agent_id = row
            .try_get::<String, _>("agent_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        if let Some(entry) = performance_map.get_mut(agent_id.as_str()) {
            entry.realized_pnl_usdc = row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0);
        }
    }

    let drawdown_rows = sqlx::query(
        "SELECT ea.id AS agent_id, po.realized_pnl_usdc
         FROM external_agents ea
         JOIN paper_outcomes po
           ON po.agent_id = ea.id
          AND po.market_id = ea.market_id
         WHERE ea.id = ANY($1)
         ORDER BY ea.id ASC, po.closed_at ASC, po.id ASC",
    )
    .bind(scoped_ids)
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut drawdown_points = BTreeMap::<String, Vec<f64>>::new();
    for row in drawdown_rows {
        let agent_id = row
            .try_get::<String, _>("agent_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        drawdown_points
            .entry(agent_id)
            .or_default()
            .push(row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0));
    }

    for (agent_id, points) in drawdown_points {
        if let Some(entry) = performance_map.get_mut(agent_id.as_str()) {
            entry.max_drawdown_usdc = calculate_max_drawdown(points.as_slice());
        }
    }

    for entry in performance_map.values_mut() {
        entry.net_pnl_usdc = entry.realized_pnl_usdc + entry.unrealized_pnl_usdc;
    }

    Ok(performance_map)
}

fn empty_public_agents_performance(owner: Option<String>) -> ExternalAgentPerformanceResponse {
    ExternalAgentPerformanceResponse {
        scope: "public".to_string(),
        owner,
        totals: ExternalAgentPerformanceTotals {
            agents: 0,
            active_agents: 0,
            open_positions: 0,
            closed_positions: 0,
            fills: 0,
            volume_usdc: 0.0,
            fees_usdc: 0.0,
            realized_pnl_usdc: 0.0,
            unrealized_pnl_usdc: 0.0,
            net_pnl_usdc: 0.0,
            max_drawdown_usdc: 0.0,
            runner_reliability: 0.0,
            p50_detection_to_order_ms: None,
            p50_slippage_ticks: None,
        },
        strategies: Vec::new(),
        timeline: Vec::new(),
        updated_at: Utc::now().to_rfc3339(),
    }
}

pub async fn list_public_external_agents(
    state: web::Data<Arc<AppState>>,
    query: web::Query<PublicExternalAgentsQuery>,
) -> Result<impl Responder, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, MAX_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0).max(0);

    let Some(owner) = public_paper_cohort_owner(&state).map(str::to_string) else {
        return Ok(HttpResponse::Ok().json(empty_public_agents_response(limit, offset)));
    };

    let mut sql = QueryBuilder::<Postgres>::new(
        "SELECT id, owner, cohort, name, provider, market_id, outcome, side, price, quantity, cadence_seconds,
                strategy, strategy_params, execution_mode, credential_id, active, last_executed_at, next_execution_at,
                consecutive_failures, last_error_code, max_notional_per_execution, max_daily_spend_usdc, max_slippage_bps,
                created_at, updated_at
         FROM external_agents
         WHERE owner = ",
    );
    sql.push_bind(owner.as_str())
        .push(" AND cohort = 'public_research' AND execution_mode = 'paper' AND LOWER(name) LIKE 'paper-%'");

    let mut count_sql = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*) AS total
         FROM external_agents
         WHERE owner = ",
    );
    count_sql.push_bind(owner.as_str()).push(
        " AND cohort = 'public_research' AND execution_mode = 'paper' AND LOWER(name) LIKE 'paper-%'",
    );

    if let Some(provider_raw) = query.provider.as_ref() {
        let provider = normalize_provider(provider_raw.as_str())?;
        sql.push(" AND provider = ").push_bind(provider.as_str());
        count_sql
            .push(" AND provider = ")
            .push_bind(provider.as_str());
    }

    if let Some(active) = query.active {
        sql.push(" AND active = ").push_bind(active);
        count_sql.push(" AND active = ").push_bind(active);
    }

    sql.push(" ORDER BY last_executed_at DESC NULLS LAST, next_execution_at ASC, id ASC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows = sql
        .build()
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let count_row = count_sql
        .build()
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let total: i64 = count_row
        .try_get("total")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut agents = Vec::with_capacity(rows.len());
    for row in rows {
        agents.push(parse_external_agent(row, Some(PUBLIC_PAPER_AGENT_SOURCE))?);
    }

    let agent_ids = agents
        .iter()
        .map(|agent| agent.id.clone())
        .collect::<Vec<_>>();
    let performance_map =
        load_external_agent_paper_performance_map(state.get_ref().as_ref(), agent_ids.as_slice())
            .await?;
    for agent in &mut agents {
        agent.paper_performance = performance_map.get(agent.id.as_str()).cloned();
    }

    Ok(HttpResponse::Ok().json(ExternalAgentsListResponse {
        agents,
        total: total.max(0) as u64,
        limit: limit as u64,
        offset: offset as u64,
    }))
}

pub async fn get_external_market_snapshot(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let market_id = ExternalMarketId::parse(path.as_str())?;
    let market =
        external::fetch_market_by_id_with_rpc(&state.config, &market_id, Some(&state.evm_rpc))
            .await?;
    Ok(HttpResponse::Ok().json(market))
}

pub async fn get_external_market_orderbook(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<ExternalMarketOrderbookQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let market_id = ExternalMarketId::parse(path.as_str())?;
    let outcome = normalize_outcome(query.outcome.as_str())?;
    let depth = query.depth.unwrap_or(20).clamp(1, 100);
    let orderbook = external::fetch_orderbook_with_rpc(
        &state.config,
        &state.redis,
        &market_id,
        outcome.as_str(),
        depth,
        Some(&state.evm_rpc),
    )
    .await?;
    Ok(HttpResponse::Ok().json(orderbook))
}

pub async fn get_external_market_trades(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<ExternalMarketTradesQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let market_id = ExternalMarketId::parse(path.as_str())?;
    let outcome = match query.outcome.as_deref() {
        None => None,
        Some(raw) => Some(normalize_outcome(raw)?.to_string()),
    };
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0);
    let snapshot = external::fetch_trades_with_rpc(
        &state.config,
        &state.redis,
        &market_id,
        outcome.as_deref(),
        limit,
        offset,
        Some(&state.evm_rpc),
    )
    .await?;
    Ok(HttpResponse::Ok().json(snapshot))
}

pub async fn get_polymarket_public_trades(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<PolymarketPublicTradesQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let market_id = query
        .market_id
        .as_deref()
        .map(|value| normalize_namespaced_market_id(ExternalProvider::Polymarket, value));
    let wallet = query
        .wallet
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let limit = query.limit.unwrap_or(100).clamp(1, MAX_PAGE_SIZE) as u64;
    let offset = query.offset.unwrap_or(0).max(0) as u64;
    let page = external::polymarket_index::query_public_trades(
        market_id.as_deref(),
        wallet.as_deref(),
        limit,
        offset,
    )
    .await?;

    Ok(HttpResponse::Ok().json(page))
}

pub async fn get_polymarket_orderbook_history(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<PolymarketOrderbookHistoryQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let market_id = query
        .market_id
        .as_deref()
        .map(|value| normalize_namespaced_market_id(ExternalProvider::Polymarket, value));
    let outcome = match query.outcome.as_deref() {
        Some(raw) => Some(normalize_outcome(raw)?),
        None => None,
    };
    let from = parse_query_datetime(query.from.as_deref(), "from")?;
    let to = parse_query_datetime(query.to.as_deref(), "to")?;
    let limit = query.limit.unwrap_or(200).clamp(1, 500) as u64;
    let items = external::polymarket_index::fetch_orderbook_history(
        market_id.as_deref(),
        outcome.as_deref(),
        from,
        to,
        limit,
    )
    .await?;

    Ok(HttpResponse::Ok().json(json!({
        "items": items,
        "limit": limit
    })))
}

pub async fn list_research_wallets(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ResearchWalletsQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let market_category = query
        .market_category
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let window_hours = query.window.unwrap_or(24 * 7).clamp(1, 24 * 30) as u64;
    let limit = query.limit.unwrap_or(25).clamp(1, 250) as u64;
    let items = external::polymarket_index::compute_wallet_scores(
        market_category.as_deref(),
        window_hours,
        limit,
    )
    .await?;

    Ok(HttpResponse::Ok().json(json!({
        "items": items,
        "marketCategory": market_category,
        "windowHours": window_hours,
        "limit": limit
    })))
}

pub async fn create_strategy_replay(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateReplayRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let strategy = body.strategy.trim().to_string();
    if strategy.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_REPLAY_STRATEGY",
            "strategy is required",
        ));
    }

    let market_id = body
        .market_id
        .as_deref()
        .map(|value| normalize_namespaced_market_id(ExternalProvider::Polymarket, value));
    let request = external::polymarket_index::StrategyReplayRequest {
        created_by: Some(user.wallet_address.to_ascii_lowercase()),
        strategy,
        baseline: body
            .baseline
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        market_id,
        market_category: body
            .market_category
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase()),
        target_wallet: body
            .target_wallet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase()),
        delay_ms: body.delay_ms.unwrap_or(800),
        window_hours: body.window_hours.unwrap_or(24 * 7),
        follow_ratio: body.follow_ratio.unwrap_or(0.8),
        markout_minutes: body.markout_minutes.unwrap_or(30),
        max_trades: body.max_trades.unwrap_or(500),
    };
    let run = external::polymarket_index::run_strategy_replay(&request).await?;
    let fills = external::polymarket_index::load_strategy_replay_fills(run.id.as_str()).await?;

    Ok(HttpResponse::Ok().json(StrategyReplayResponse { run, fills }))
}

pub async fn get_strategy_replay(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let replay_run_id = path.into_inner();
    let run = external::polymarket_index::load_strategy_replay_run(replay_run_id.as_str())
        .await?
        .ok_or_else(|| ApiError::not_found("Strategy replay"))?;
    let fills =
        external::polymarket_index::load_strategy_replay_fills(replay_run_id.as_str()).await?;

    Ok(HttpResponse::Ok().json(StrategyReplayResponse { run, fills }))
}
pub async fn create_external_signal(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateExternalSignalRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;
    let provider = normalize_provider(body.provider.as_str())?;
    let market_id = normalize_namespaced_market_id(provider, body.market_id.as_str());
    let direction = normalize_direction(body.direction.as_str())?;
    if !(0..=10_000).contains(&body.confidence_bps) {
        return Err(ApiError::bad_request(
            "INVALID_CONFIDENCE_BPS",
            "confidenceBps must be between 0 and 10000",
        ));
    }
    if !(0.0..=1.0).contains(&body.fair_value_low)
        || !(0.0..=1.0).contains(&body.fair_value_high)
        || body.fair_value_low > body.fair_value_high
    {
        return Err(ApiError::bad_request(
            "INVALID_FAIR_VALUE_RANGE",
            "fairValueLow/fairValueHigh must be between 0 and 1 with low <= high",
        ));
    }
    if body.catalyst_summary.trim().is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_CATALYST_SUMMARY",
            "catalystSummary is required",
        ));
    }
    let expires_at = chrono::DateTime::parse_from_rfc3339(body.expires_at.as_str())
        .map_err(|_| ApiError::bad_request("INVALID_EXPIRES_AT", "expiresAt must be RFC3339"))?
        .with_timezone(&Utc);
    if expires_at <= Utc::now() {
        return Err(ApiError::bad_request(
            "INVALID_EXPIRES_AT",
            "expiresAt must be in the future",
        ));
    }
    let parsed_market_id = ExternalMarketId::parse(market_id.as_str())?;
    let market = external::fetch_market_by_id(&state.config, &parsed_market_id).await?;

    let publisher = body
        .publisher
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(user.wallet_address.as_str())
        .to_ascii_lowercase();
    let row = sqlx::query(
        "INSERT INTO external_market_signals (
            id, publisher, provider, market_id, signal_type, direction, confidence_bps,
            fair_value_low, fair_value_high, midpoint_delta_bps, catalyst_summary, invalidators,
            rationale, metadata, active, expires_at, created_at, updated_at, agent_id
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,TRUE,$15,NOW(),NOW(),$16)
        RETURNING id, publisher, provider, market_id, signal_type, direction, confidence_bps,
                  fair_value_low, fair_value_high, midpoint_delta_bps, catalyst_summary, invalidators,
                  rationale, metadata, active, expires_at, created_at, updated_at, agent_id",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(publisher)
    .bind(provider.as_str())
    .bind(market_id)
    .bind(
        body.signal_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("scenario_lab"),
    )
    .bind(direction)
    .bind(body.confidence_bps)
    .bind(body.fair_value_low)
    .bind(body.fair_value_high)
    .bind(body.midpoint_delta_bps)
    .bind(body.catalyst_summary.trim())
    .bind(json!(body.invalidators))
    .bind(body.rationale.as_deref())
    .bind(merge_signal_metadata(&body, &market))
    .bind(expires_at)
    .bind(body.agent_id.as_deref())
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(parse_external_signal(row)?))
}

pub async fn list_external_signals(
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListExternalSignalsQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let limit = query.limit.unwrap_or(50).clamp(1, MAX_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0).max(0);
    let active_only = query.active_only.unwrap_or(true);

    let mut sql = QueryBuilder::<Postgres>::new(
        "SELECT id, publisher, provider, market_id, signal_type, direction, confidence_bps,
                fair_value_low, fair_value_high, midpoint_delta_bps, catalyst_summary, invalidators,
                rationale, metadata, active, expires_at, created_at, updated_at, agent_id
         FROM external_market_signals
         WHERE TRUE",
    );
    let mut count_sql = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*) AS total
         FROM external_market_signals
         WHERE TRUE",
    );

    if active_only {
        sql.push(" AND active = TRUE AND expires_at > NOW()");
        count_sql.push(" AND active = TRUE AND expires_at > NOW()");
    }
    if let Some(provider_raw) = query.provider.as_ref() {
        let provider = normalize_provider(provider_raw.as_str())?;
        sql.push(" AND provider = ").push_bind(provider.as_str());
        count_sql
            .push(" AND provider = ")
            .push_bind(provider.as_str());
    }
    if let Some(market_id_raw) = query.market_id.as_ref() {
        let market_id = market_id_raw.trim();
        if !market_id.is_empty() {
            sql.push(" AND market_id = ").push_bind(market_id);
            count_sql.push(" AND market_id = ").push_bind(market_id);
        }
    }
    if let Some(publisher) = query.publisher.as_ref() {
        let normalized = publisher.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            sql.push(" AND publisher = ").push_bind(normalized.clone());
            count_sql.push(" AND publisher = ").push_bind(normalized);
        }
    }

    sql.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows = sql
        .build()
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let count_row = count_sql
        .build()
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let total: i64 = count_row.try_get("total").unwrap_or(0);

    let mut signals = Vec::with_capacity(rows.len());
    for row in rows {
        signals.push(parse_external_signal(row)?);
    }

    Ok(HttpResponse::Ok().json(ExternalSignalsListResponse {
        signals,
        total: total.max(0) as u64,
        limit: limit as u64,
        offset: offset as u64,
    }))
}

pub async fn create_external_agent(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateExternalAgentRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    if !state.config.external_agents_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_AGENTS_DISABLED",
            "external agents are disabled",
        ));
    }

    let user = extract_authenticated_user(&req, &state).await?;
    let user_role = extract_jwt_user(&req, &state)?.role;
    let provider = normalize_provider(body.provider.as_str())?;
    ensure_provider_action_allowed(&req, provider, ProviderRailAction::TradeOpen)?;
    let outcome = normalize_outcome(body.outcome.as_str())?;
    let side = normalize_side(body.side.as_str())?;
    let execution_mode = requested_execution_mode(
        execution_mode(&state),
        user_role,
        body.execution_mode.as_deref(),
    )?;
    let cohort = resolve_agent_cohort(
        user_role,
        user.wallet_address.as_str(),
        execution_mode,
        body.cohort.as_deref(),
        public_paper_cohort_owner(&state),
    )?;
    let strategy = body.strategy.trim().to_string();
    let strategy_params =
        normalize_strategy_params(strategy.as_str(), body.strategy_params.as_ref())?;

    if body.name.trim().is_empty() {
        return Err(ApiError::bad_request("INVALID_NAME", "name is required"));
    }
    if strategy.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_STRATEGY",
            "strategy is required",
        ));
    }
    if body.cadence_seconds == 0 {
        return Err(ApiError::bad_request(
            "INVALID_CADENCE",
            "cadenceSeconds must be greater than zero",
        ));
    }
    if body.price <= 0.0 || body.price >= 1.0 {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "price must be between 0 and 1",
        ));
    }
    if body.quantity <= 0.0 {
        return Err(ApiError::bad_request(
            "INVALID_QUANTITY",
            "quantity must be greater than zero",
        ));
    }
    validate_agent_risk_fields(
        body.max_notional_per_execution,
        body.max_daily_spend_usdc,
        body.max_slippage_bps,
    )?;
    ensure_live_strategy_allowed(strategy.as_str(), execution_mode)?;

    let namespaced_market_id = normalize_namespaced_market_id(provider, body.market_id.as_str());
    let parsed_market_id = ExternalMarketId::parse(namespaced_market_id.as_str())?;
    let market = external::fetch_market_by_id(&state.config, &parsed_market_id).await?;
    if !market.execution_agents {
        return Err(ApiError::bad_request(
            "MARKET_NOT_EXECUTABLE",
            "market is not executable for external agents",
        ));
    }
    ensure_event_repricing_v2_candidate_eligible(
        &state,
        &market,
        strategy.as_str(),
        &strategy_params,
        body.active.unwrap_or(true),
    )
    .await?;

    let credential_id =
        if execution_mode == ExternalExecutionMode::Live || body.credential_id.is_some() {
            let credential = load_credential(
                &state,
                user.wallet_address.as_str(),
                provider,
                body.credential_id.as_deref(),
            )
            .await?;
            ensure_provider_credential_ready(&state, provider, &credential).await?;
            Some(credential.id)
        } else {
            None
        };

    let id = Uuid::new_v4().to_string();
    let row = sqlx::query(
        "INSERT INTO external_agents (
            id, owner, name, provider, market_id, provider_market_ref,
            outcome, side, price, quantity, cadence_seconds, strategy, strategy_params, execution_mode,
            credential_id, cohort, active, max_notional_per_execution, max_daily_spend_usdc, max_slippage_bps,
            next_execution_at, created_at, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,NOW(),NOW(),NOW())
        RETURNING id, owner, cohort, name, provider, market_id, outcome, side, price, quantity,
                  cadence_seconds, strategy, strategy_params, execution_mode, credential_id, active, last_executed_at,
                  next_execution_at, consecutive_failures, last_error_code, max_notional_per_execution,
                  max_daily_spend_usdc, max_slippage_bps, created_at, updated_at",
    )
    .bind(id.as_str())
    .bind(user.wallet_address.as_str())
    .bind(body.name.trim())
    .bind(provider.as_str())
    .bind(namespaced_market_id.as_str())
    .bind(market.provider_market_ref)
    .bind(outcome)
    .bind(side)
    .bind(body.price)
    .bind(body.quantity)
    .bind(body.cadence_seconds as i64)
    .bind(strategy)
    .bind(strategy_params)
    .bind(execution_mode.as_str())
    .bind(credential_id.as_deref())
    .bind(cohort)
    .bind(body.active.unwrap_or(true))
    .bind(body.max_notional_per_execution)
    .bind(body.max_daily_spend_usdc)
    .bind(body.max_slippage_bps)
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(parse_external_agent(row, None)?))
}

pub async fn update_external_agent(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<UpdateExternalAgentRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_jwt_user(&req, &state)?;

    let agent_id = path.into_inner();
    let current = if matches!(user.role, UserRole::Admin) {
        sqlx::query(
            "SELECT id, owner, cohort, provider, market_id, outcome, side, price, quantity, cadence_seconds, strategy, strategy_params, execution_mode, credential_id, active,
                    max_notional_per_execution, max_daily_spend_usdc, max_slippage_bps
             FROM external_agents
             WHERE id = $1",
        )
        .bind(agent_id.as_str())
        .fetch_optional(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
        .ok_or_else(|| ApiError::not_found("External agent"))?
    } else {
        sqlx::query(
            "SELECT id, owner, cohort, provider, market_id, outcome, side, price, quantity, cadence_seconds, strategy, strategy_params, execution_mode, credential_id, active,
                    max_notional_per_execution, max_daily_spend_usdc, max_slippage_bps
             FROM external_agents
             WHERE id = $1 AND owner = $2",
        )
        .bind(agent_id.as_str())
        .bind(user.wallet_address.as_str())
        .fetch_optional(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
        .ok_or_else(|| ApiError::not_found("External agent"))?
    };

    let provider_raw: String = current
        .try_get("provider")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let provider = normalize_provider(provider_raw.as_str())?;
    let owner: String = current
        .try_get("owner")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let current_cohort = current
        .try_get::<String, _>("cohort")
        .unwrap_or_else(|_| PRIVATE_ALPHA_COHORT.to_string());
    let current_execution_mode = parse_external_execution_mode(
        current
            .try_get::<String, _>("execution_mode")
            .map_err(|err| ApiError::internal(&err.to_string()))?
            .as_str(),
    )?;

    let next_outcome = if let Some(outcome) = body.outcome.as_deref() {
        normalize_outcome(outcome)?
    } else {
        current
            .try_get("outcome")
            .map_err(|err| ApiError::internal(&err.to_string()))?
    };
    let next_side = if let Some(side) = body.side.as_deref() {
        normalize_side(side)?
    } else {
        current
            .try_get("side")
            .map_err(|err| ApiError::internal(&err.to_string()))?
    };
    let next_price = body
        .price
        .unwrap_or_else(|| current.try_get("price").unwrap_or(0.5));
    let next_quantity = body
        .quantity
        .unwrap_or_else(|| current.try_get("quantity").unwrap_or(0.0));
    let next_cadence = body
        .cadence_seconds
        .unwrap_or_else(|| current.try_get::<i64, _>("cadence_seconds").unwrap_or(60) as u64);
    let next_strategy = body
        .strategy
        .as_deref()
        .unwrap_or_else(|| current.try_get("strategy").unwrap_or("external"))
        .trim()
        .to_string();
    let next_strategy_params = match body.strategy_params.as_ref() {
        Some(value) => normalize_strategy_params(next_strategy.as_str(), Some(value))?,
        None if body.strategy.is_some() => normalize_strategy_params(next_strategy.as_str(), None)?,
        None => current
            .try_get("strategy_params")
            .unwrap_or_else(|_| json!({})),
    };
    let next_name = body.name.as_deref().unwrap_or("").trim().to_string();
    let next_active = body
        .active
        .unwrap_or_else(|| current.try_get("active").unwrap_or(true));
    let next_execution_mode = match body.execution_mode.as_deref() {
        Some(raw) if matches!(user.role, UserRole::Admin) => parse_external_execution_mode(raw)?,
        Some(_) => {
            return Err(ApiError::forbidden(
                "Only admins can override external agent execution mode",
            ));
        }
        None => current_execution_mode,
    };
    let next_cohort = match body.cohort.as_deref() {
        Some(raw) => resolve_agent_cohort(
            user.role,
            owner.as_str(),
            next_execution_mode,
            Some(raw),
            public_paper_cohort_owner(&state),
        )?,
        None => resolve_agent_cohort(
            UserRole::Admin,
            owner.as_str(),
            next_execution_mode,
            Some(current_cohort.as_str()),
            public_paper_cohort_owner(&state),
        )?,
    };
    let next_max_notional_per_execution = body.max_notional_per_execution.or_else(|| {
        current
            .try_get::<Option<f64>, _>("max_notional_per_execution")
            .ok()
            .flatten()
    });
    let next_max_daily_spend_usdc = body.max_daily_spend_usdc.or_else(|| {
        current
            .try_get::<Option<f64>, _>("max_daily_spend_usdc")
            .ok()
            .flatten()
    });
    let next_max_slippage_bps = body.max_slippage_bps.or_else(|| {
        current
            .try_get::<Option<i32>, _>("max_slippage_bps")
            .ok()
            .flatten()
    });

    if next_price <= 0.0 || next_price >= 1.0 {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "price must be between 0 and 1",
        ));
    }
    if next_quantity <= 0.0 {
        return Err(ApiError::bad_request(
            "INVALID_QUANTITY",
            "quantity must be greater than zero",
        ));
    }
    if next_cadence == 0 {
        return Err(ApiError::bad_request(
            "INVALID_CADENCE",
            "cadenceSeconds must be greater than zero",
        ));
    }
    validate_agent_risk_fields(
        next_max_notional_per_execution,
        next_max_daily_spend_usdc,
        next_max_slippage_bps,
    )?;
    ensure_live_strategy_allowed(next_strategy.as_str(), next_execution_mode)?;
    if next_active && is_event_repricing_v2_strategy(next_strategy.as_str()) {
        let current_market_id: String = current
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let parsed_market_id = ExternalMarketId::parse(current_market_id.as_str())?;
        let market = external::fetch_market_by_id(&state.config, &parsed_market_id).await?;
        ensure_event_repricing_v2_candidate_eligible(
            &state,
            &market,
            next_strategy.as_str(),
            &next_strategy_params,
            next_active,
        )
        .await?;
    }

    let credential_id = if let Some(id) = body.credential_id.as_deref() {
        let credential = load_credential(&state, owner.as_str(), provider, Some(id)).await?;
        ensure_provider_credential_ready(&state, provider, &credential).await?;
        Some(credential.id)
    } else {
        current.try_get::<String, _>("credential_id").ok()
    };

    if next_execution_mode == ExternalExecutionMode::Live && credential_id.is_none() {
        return Err(ApiError::bad_request(
            "CREDENTIAL_REQUIRED",
            "live external agents require a ready credential",
        ));
    }

    let row = if matches!(user.role, UserRole::Admin) {
        sqlx::query(
            "UPDATE external_agents
             SET name = COALESCE(NULLIF($2, ''), name),
                 outcome = $3,
                 side = $4,
                 price = $5,
                 quantity = $6,
                 cadence_seconds = $7,
                 strategy = $8,
                 strategy_params = $9,
                 execution_mode = $10,
                 credential_id = $11,
                 cohort = $12,
                 active = $13,
                 max_notional_per_execution = $14,
                 max_daily_spend_usdc = $15,
                 max_slippage_bps = $16,
                 consecutive_failures = CASE WHEN $13 = TRUE THEN 0 ELSE consecutive_failures END,
                 last_error_code = CASE WHEN $13 = TRUE THEN NULL ELSE last_error_code END,
                 updated_at = NOW()
             WHERE id = $1
             RETURNING id, owner, cohort, name, provider, market_id, outcome, side, price, quantity,
                       cadence_seconds, strategy, strategy_params, execution_mode, credential_id, active, last_executed_at,
                       next_execution_at, consecutive_failures, last_error_code, max_notional_per_execution,
                       max_daily_spend_usdc, max_slippage_bps, created_at, updated_at",
        )
        .bind(agent_id.as_str())
        .bind(next_name)
        .bind(next_outcome)
        .bind(next_side)
        .bind(next_price)
        .bind(next_quantity)
        .bind(next_cadence as i64)
        .bind(next_strategy)
        .bind(next_strategy_params)
        .bind(next_execution_mode.as_str())
        .bind(credential_id.as_deref())
        .bind(next_cohort)
        .bind(next_active)
        .bind(next_max_notional_per_execution)
        .bind(next_max_daily_spend_usdc)
        .bind(next_max_slippage_bps)
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
    } else {
        sqlx::query(
            "UPDATE external_agents
             SET name = COALESCE(NULLIF($3, ''), name),
                 outcome = $4,
                 side = $5,
                 price = $6,
                 quantity = $7,
                 cadence_seconds = $8,
                 strategy = $9,
                 strategy_params = $10,
                 execution_mode = $11,
                 credential_id = $12,
                 cohort = $13,
                 active = $14,
                 max_notional_per_execution = $15,
                 max_daily_spend_usdc = $16,
                 max_slippage_bps = $17,
                 consecutive_failures = CASE WHEN $14 = TRUE THEN 0 ELSE consecutive_failures END,
                 last_error_code = CASE WHEN $14 = TRUE THEN NULL ELSE last_error_code END,
                 updated_at = NOW()
             WHERE id = $1 AND owner = $2
             RETURNING id, owner, cohort, name, provider, market_id, outcome, side, price, quantity,
                       cadence_seconds, strategy, strategy_params, execution_mode, credential_id, active, last_executed_at,
                       next_execution_at, consecutive_failures, last_error_code, max_notional_per_execution,
                       max_daily_spend_usdc, max_slippage_bps, created_at, updated_at",
        )
        .bind(agent_id.as_str())
        .bind(user.wallet_address.as_str())
        .bind(next_name)
        .bind(next_outcome)
        .bind(next_side)
        .bind(next_price)
        .bind(next_quantity)
        .bind(next_cadence as i64)
        .bind(next_strategy)
        .bind(next_strategy_params)
        .bind(next_execution_mode.as_str())
        .bind(credential_id.as_deref())
        .bind(next_cohort)
        .bind(next_active)
        .bind(next_max_notional_per_execution)
        .bind(next_max_daily_spend_usdc)
        .bind(next_max_slippage_bps)
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
    };

    Ok(HttpResponse::Ok().json(parse_external_agent(row, None)?))
}

pub async fn execute_external_agent(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<ExecuteExternalAgentRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    if !state.config.external_agents_enabled || !state.config.external_trading_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_AGENT_EXECUTION_DISABLED",
            "external agent execution is disabled",
        ));
    }

    let user = extract_authenticated_user(&req, &state).await?;
    let agent_id = path.into_inner();
    let agent =
        load_external_agent_for_owner(&state, agent_id.as_str(), user.wallet_address.as_str())
            .await?;

    if !agent.active {
        return Err(ApiError::bad_request(
            "EXTERNAL_AGENT_INACTIVE",
            "external agent is inactive",
        ));
    }

    if !body.force.unwrap_or(false) && Utc::now() < agent.next_execution_at {
        return Err(ApiError::bad_request(
            "EXTERNAL_AGENT_COOLDOWN",
            "agent cannot execute yet",
        ));
    }

    ensure_provider_action_allowed(&req, agent.provider, ProviderRailAction::TradeOpen)?;
    let outcome = execute_agent_record(&state, &agent, body.signed_order.clone()).await?;

    if agent.consecutive_failures > 0 {
        let _ = reset_agent_failures(&state, agent.id.as_str()).await;
    }

    Ok(HttpResponse::Ok().json(json!({
        "ok": outcome.executed,
        "mode": agent.execution_mode.as_str(),
        "agentId": agent_id,
        "runId": outcome.run_id,
        "externalOrderId": outcome.external_order_id,
        "providerOrderId": outcome.provider_order_id,
        "nextExecutionAt": outcome.next_execution_at.to_rfc3339(),
        "status": outcome.run_status,
        "skipReason": outcome.skip_reason,
        "response": outcome.response,
    })))
}

pub async fn run_external_agents_tick(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<RunnerTickRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    if !state.config.external_agents_enabled || !state.config.external_trading_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_AGENT_EXECUTION_DISABLED",
            "external agent execution is disabled",
        ));
    }

    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let max_limit = state.config.paper_runner_scan_limit.max(1) as i64;
    let limit = body.limit.unwrap_or(max_limit).clamp(1, max_limit);
    let now = Utc::now();
    let agents = load_due_external_agents(&state, limit).await?;
    let mut agents_executed = 0_u64;
    let mut skips_by_reason = BTreeMap::new();

    for agent in &agents {
        if now < agent.next_execution_at {
            increment_skip_reason(&mut skips_by_reason, "not_due");
            continue;
        }

        if agent.consecutive_failures >= MAX_CONSECUTIVE_FAILURES_BEFORE_DEACTIVATE {
            log::warn!(
                "auto-deactivating agent {} after {} consecutive failures (last_error={})",
                agent.id,
                agent.consecutive_failures,
                agent.last_error_code.as_deref().unwrap_or("unknown")
            );
            deactivate_external_agent(&state, agent.id.as_str(), now).await?;
            state
                .event_bus
                .emit(crate::services::event_bus::PlatformEvent::AgentDeactivated(
                    crate::services::event_bus::AgentLifecycleEvent {
                        agent_id: agent.id.clone(),
                        owner: agent.owner.clone(),
                        reason: format!(
                            "auto_deactivated after {} consecutive failures",
                            agent.consecutive_failures
                        ),
                        timestamp: now,
                    },
                ));
            increment_skip_reason(&mut skips_by_reason, "auto_deactivated");
            continue;
        }

        if let Err(err) =
            ensure_provider_action_allowed(&req, agent.provider, ProviderRailAction::TradeOpen)
        {
            let reason = skip_reason_from_error(&err);
            increment_skip_reason(&mut skips_by_reason, reason.as_str());
            let run_id = Uuid::new_v4().to_string();
            insert_external_agent_run(
                &state,
                run_id.as_str(),
                agent,
                run_skip_status_for_mode(agent.execution_mode),
                None,
                Some(reason.as_str()),
                &json!({
                    "mode": agent.execution_mode.as_str(),
                    "error": {
                        "code": err.code,
                        "message": err.message,
                        "details": err.details
                    }
                }),
            )
            .await?;
            continue;
        }

        match execute_agent_record(&state, agent, None).await {
            Ok(outcome) => {
                if outcome.executed {
                    agents_executed += 1;
                    state
                        .event_bus
                        .emit(crate::services::event_bus::PlatformEvent::AgentExecuted(
                            crate::services::event_bus::AgentExecutedEvent {
                                agent_id: agent.id.clone(),
                                owner: agent.owner.clone(),
                                provider: agent.provider.as_str().to_string(),
                                market_id: agent.market_id.clone(),
                                strategy: agent.strategy.clone(),
                                execution_mode: agent.execution_mode.as_str().to_string(),
                                run_id: outcome.run_id.clone(),
                                run_status: outcome.run_status.clone(),
                                side: agent.side.clone(),
                                outcome: agent.outcome.clone(),
                                price: agent.price,
                                metadata: outcome.response.clone(),
                                timestamp: now,
                            },
                        ));
                } else if let Some(reason) = outcome.skip_reason.as_deref() {
                    increment_skip_reason(&mut skips_by_reason, reason);
                }
                if agent.consecutive_failures > 0 {
                    if let Err(reset_err) = reset_agent_failures(&state, agent.id.as_str()).await {
                        log::warn!(
                            "failed to reset failure counter for agent {}: {}",
                            agent.id,
                            reset_err.message
                        );
                    }
                }
            }
            Err(err) => {
                let reason = skip_reason_from_error(&err);
                let run_status = run_status_from_error(&err);
                log::error!(
                    "external runner failed agent_id={} provider={} market_id={} strategy={} side={} outcome={} code={} message={} details={}",
                    agent.id,
                    agent.provider.as_str(),
                    agent.market_id,
                    agent.strategy,
                    agent.side,
                    agent.outcome,
                    err.code,
                    err.message,
                    err.details
                        .as_ref()
                        .map(Value::to_string)
                        .unwrap_or_else(|| "null".to_string()),
                );
                state
                    .event_bus
                    .emit(crate::services::event_bus::PlatformEvent::AgentFailed(
                        crate::services::event_bus::AgentFailedEvent {
                            agent_id: agent.id.clone(),
                            owner: agent.owner.clone(),
                            provider: agent.provider.as_str().to_string(),
                            market_id: agent.market_id.clone(),
                            error_code: err.code.clone(),
                            error_message: err.message.clone(),
                            consecutive_failures: agent.consecutive_failures + 1,
                            timestamp: now,
                        },
                    ));
                increment_skip_reason(&mut skips_by_reason, reason.as_str());
                let run_id = Uuid::new_v4().to_string();
                insert_external_agent_run(
                    &state,
                    run_id.as_str(),
                    agent,
                    run_status,
                    None,
                    Some(reason.as_str()),
                    &json!({
                        "mode": agent.execution_mode.as_str(),
                        "error": {
                            "code": err.code,
                            "message": err.message,
                            "details": err.details
                        }
                    }),
                )
                .await?;
                if let Err(backoff_err) = record_agent_failure(
                    &state,
                    agent.id.as_str(),
                    err.code.as_str(),
                    agent.cadence_seconds,
                    agent.consecutive_failures,
                    now,
                )
                .await
                {
                    log::error!(
                        "failed to record agent failure backoff agent_id={}: {}",
                        agent.id,
                        backoff_err.message
                    );
                }
            }
        }
    }

    Ok(HttpResponse::Ok().json(RunnerTickResponse {
        executed: agents_executed > 0,
        agents_scanned: agents.len() as u64,
        agents_executed,
        skips_by_reason,
    }))
}

pub async fn get_polymarket_indexer_health(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_external_state_import_admin(&req, &state).await?;

    let tracked_market_refs = load_polymarket_tracked_market_refs(&state, None, 250).await?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| {
            ApiError::internal(&format!("failed to build polymarket indexer client: {err}"))
        })?;
    let mut tracked_markets = Vec::new();
    for provider_market_ref in &tracked_market_refs {
        if let Ok(market) =
            load_polymarket_tracked_market(&client, &state.config, provider_market_ref.as_str())
                .await
        {
            tracked_markets.push(market);
        }
    }
    let tracked_market_ids = tracked_market_refs
        .iter()
        .map(|value| format!("polymarket:{value}"))
        .collect::<Vec<_>>();
    let public_tape = load_polymarket_lane_health(
        &state,
        external::polymarket_index::PolymarketIndexLane::PublicTape,
        &tracked_market_ids,
    )
    .await?;
    let user_fills = load_polymarket_lane_health(
        &state,
        external::polymarket_index::PolymarketIndexLane::UserFills,
        &tracked_market_ids,
    )
    .await?;

    Ok(HttpResponse::Ok().json(PolymarketIndexerHealthResponse {
        ok: !matches!(public_tape.status.as_str(), "error")
            && !matches!(user_fills.status.as_str(), "error"),
        tracked_markets: tracked_market_ids.len() as u64,
        tracked_market_details: tracked_market_details(&tracked_markets),
        public_tape,
        user_fills,
    }))
}

pub async fn trigger_polymarket_indexer_backfill(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: Option<web::Json<PolymarketIndexerBackfillRequest>>,
) -> Result<impl Responder, ApiError> {
    ensure_external_state_import_admin(&req, &state).await?;
    let body = body
        .map(web::Json::into_inner)
        .unwrap_or(PolymarketIndexerBackfillRequest {
            market_id: None,
            days: None,
            public_tape: None,
            user_fills: None,
            max_markets: None,
            max_pages_per_market: None,
            user_events: Vec::new(),
            relayer_transactions: Vec::new(),
        });
    let max_markets = body.max_markets.unwrap_or(25).clamp(1, 250);
    let tracked_market_refs =
        load_polymarket_tracked_market_refs(&state, body.market_id.as_deref(), max_markets).await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| {
            ApiError::internal(&format!("failed to build polymarket indexer client: {err}"))
        })?;
    let cutoff = Utc::now() - Duration::days(body.days.unwrap_or(90).clamp(1, 365) as i64);
    let max_pages_per_market = body.max_pages_per_market.unwrap_or(20).clamp(1, 250);
    let mut counts = PolymarketBackfillCounts::default();
    let mut tracked_markets = Vec::new();

    for provider_market_ref in &tracked_market_refs {
        let tracked_market = match load_polymarket_tracked_market(
            &client,
            &state.config,
            provider_market_ref.as_str(),
        )
        .await
        {
            Ok(market) => market,
            Err(err) => {
                external::polymarket_index::upsert_index_state(
                    external::polymarket_index::PolymarketIndexLane::PublicTape,
                    format!("polymarket:{provider_market_ref}").as_str(),
                    provider_market_ref.as_str(),
                    "failed",
                    None,
                    None,
                    true,
                    Some(err.message.as_str()),
                )
                .await?;
                external::polymarket_index::upsert_index_state(
                    external::polymarket_index::PolymarketIndexLane::UserFills,
                    format!("polymarket:{provider_market_ref}").as_str(),
                    provider_market_ref.as_str(),
                    "failed",
                    None,
                    None,
                    true,
                    Some(err.message.as_str()),
                )
                .await?;
                continue;
            }
        };
        tracked_markets.push(tracked_market.clone());

        if body.public_tape.unwrap_or(true) {
            match backfill_polymarket_public_trades_for_market(
                &state,
                &client,
                &tracked_market,
                cutoff,
                max_pages_per_market,
            )
            .await
            {
                Ok(inserted) => {
                    counts.public_trades_ingested =
                        counts.public_trades_ingested.saturating_add(inserted);
                }
                Err(err) => {
                    external::polymarket_index::upsert_index_state(
                        external::polymarket_index::PolymarketIndexLane::PublicTape,
                        tracked_market.market_id.as_str(),
                        tracked_market.provider_market_ref.as_str(),
                        "failed",
                        None,
                        None,
                        true,
                        Some(err.message.as_str()),
                    )
                    .await?;
                }
            }
        }

        if body.user_fills.unwrap_or(true) {
            match backfill_polymarket_builder_trades_for_market(
                &state,
                &client,
                &tracked_market,
                cutoff,
                max_pages_per_market,
                &body.relayer_transactions,
            )
            .await
            {
                Ok(inserted) => {
                    counts.user_fill_events_ingested =
                        counts.user_fill_events_ingested.saturating_add(inserted);
                }
                Err(err) => {
                    external::polymarket_index::upsert_index_state(
                        external::polymarket_index::PolymarketIndexLane::UserFills,
                        tracked_market.market_id.as_str(),
                        tracked_market.provider_market_ref.as_str(),
                        "failed",
                        None,
                        None,
                        true,
                        Some(err.message.as_str()),
                    )
                    .await?;
                }
            }
        }
    }

    if body.user_fills.unwrap_or(true) && !body.user_events.is_empty() {
        counts.user_fill_events_ingested = counts.user_fill_events_ingested.saturating_add(
            ingest_polymarket_user_trade_events(
                &state,
                &tracked_markets,
                &body.user_events,
                &body.relayer_transactions,
            )
            .await?,
        );
    }

    if body.user_fills.unwrap_or(true) {
        match reconcile_polymarket_relayer_lifecycle(
            &state,
            &client,
            (!body.relayer_transactions.is_empty()).then_some(body.relayer_transactions.as_slice()),
        )
        .await
        {
            Ok(updated) => {
                counts.user_lifecycle_events_reconciled = counts
                    .user_lifecycle_events_reconciled
                    .saturating_add(updated);
            }
            Err(err) => {
                for provider_market_ref in &tracked_market_refs {
                    let market_id = format!("polymarket:{provider_market_ref}");
                    external::polymarket_index::upsert_index_state(
                        external::polymarket_index::PolymarketIndexLane::UserFills,
                        market_id.as_str(),
                        provider_market_ref.as_str(),
                        "failed",
                        None,
                        None,
                        true,
                        Some(err.message.as_str()),
                    )
                    .await?;
                }
            }
        }
    }

    let tracked_market_ids = tracked_market_refs
        .iter()
        .map(|value| format!("polymarket:{value}"))
        .collect::<Vec<_>>();
    let public_tape = load_polymarket_lane_health(
        &state,
        external::polymarket_index::PolymarketIndexLane::PublicTape,
        &tracked_market_ids,
    )
    .await?;
    let user_fills = load_polymarket_lane_health(
        &state,
        external::polymarket_index::PolymarketIndexLane::UserFills,
        &tracked_market_ids,
    )
    .await?;

    Ok(HttpResponse::Ok().json(PolymarketIndexerBackfillResponse {
        ok: !matches!(public_tape.status.as_str(), "error")
            && !matches!(user_fills.status.as_str(), "error"),
        tracked_markets: tracked_market_refs.len() as u64,
        tracked_market_details: tracked_market_details(&tracked_markets),
        public_trades_ingested: counts.public_trades_ingested,
        user_fill_events_ingested: counts.user_fill_events_ingested,
        user_lifecycle_events_reconciled: counts.user_lifecycle_events_reconciled,
        public_tape,
        user_fills,
    }))
}

pub async fn reset_imported_external_state(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_external_state_import_admin(&req, &state).await?;
    reset_external_state(&state).await?;

    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

pub async fn import_external_state_batch(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    table: web::Path<String>,
    body: web::Json<ImportExternalStateBatchRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_external_state_import_admin(&req, &state).await?;
    let table = ExternalStateImportTable::parse(table.as_str())?;
    let imported = import_external_state_rows(&state, table, body.into_inner().rows).await?;

    Ok(HttpResponse::Ok().json(ImportExternalStateResponse {
        ok: true,
        table: table.as_str().to_string(),
        imported,
    }))
}

pub async fn get_external_agents_performance(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ExternalAgentPerformanceQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_external_features_enabled(&state)?;
    let user = extract_jwt_user(&req, &state)?;
    let is_admin = matches!(user.role, UserRole::Admin);
    let requested_owner = query
        .owner
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let scope = query
        .scope
        .as_deref()
        .unwrap_or(if is_admin { "all" } else { "self" })
        .trim()
        .to_ascii_lowercase();

    let owner_filter = match scope.as_str() {
        "all" if is_admin => None,
        "owner" if is_admin => requested_owner.clone(),
        _ => Some(requested_owner.unwrap_or_else(|| user.wallet_address.to_ascii_lowercase())),
    };

    if !is_admin && query.owner.is_some() {
        let requested = query
            .owner
            .as_ref()
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_default();
        if requested != user.wallet_address.to_ascii_lowercase() {
            return Err(ApiError::forbidden("Insufficient permissions"));
        }
    }

    if let Err(err) = sync_live_external_ledgers(&state, owner_filter.as_deref()).await {
        log::warn!("live external ledger sync failed: {}", err.message);
    }

    let agents_row = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT COUNT(*) AS agents,
                    COUNT(*) FILTER (WHERE active) AS active_agents
             FROM external_agents
             WHERE owner = $1",
        )
        .bind(owner.as_str())
        .fetch_one(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT COUNT(*) AS agents,
                    COUNT(*) FILTER (WHERE active) AS active_agents
             FROM external_agents",
        )
        .fetch_one(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let positions_row = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT COUNT(*) FILTER (WHERE status = 'open') AS open_positions,
                    COUNT(*) FILTER (WHERE status = 'closed') AS closed_positions,
                    COALESCE(SUM(CASE WHEN status = 'open' THEN unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
             FROM paper_positions
             WHERE owner = $1",
        )
        .bind(owner.as_str())
        .fetch_one(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT COUNT(*) FILTER (WHERE status = 'open') AS open_positions,
                    COUNT(*) FILTER (WHERE status = 'closed') AS closed_positions,
                    COALESCE(SUM(CASE WHEN status = 'open' THEN unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
             FROM paper_positions",
        )
        .fetch_one(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let fills_row = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT COUNT(*) AS fills,
                    COALESCE(SUM(notional_usdc), 0) AS volume_usdc,
                    COALESCE(SUM(fee_usdc), 0) AS fees_usdc
             FROM paper_fills
             WHERE owner = $1",
        )
        .bind(owner.as_str())
        .fetch_one(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT COUNT(*) AS fills,
                    COALESCE(SUM(notional_usdc), 0) AS volume_usdc,
                    COALESCE(SUM(fee_usdc), 0) AS fees_usdc
             FROM paper_fills",
        )
        .fetch_one(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let outcomes_row = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT COALESCE(SUM(realized_pnl_usdc), 0) AS realized_pnl_usdc
             FROM paper_outcomes
             WHERE owner = $1",
        )
        .bind(owner.as_str())
        .fetch_one(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT COALESCE(SUM(realized_pnl_usdc), 0) AS realized_pnl_usdc
             FROM paper_outcomes",
        )
        .fetch_one(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut outcome_drawdown_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT closed_at, realized_pnl_usdc
             FROM paper_outcomes
             WHERE owner = $1
             ORDER BY closed_at ASC, id ASC",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT closed_at, realized_pnl_usdc
             FROM paper_outcomes
             ORDER BY closed_at ASC, id ASC",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut run_metric_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT ear.status, ear.metadata
             FROM external_agent_runs ear
             JOIN external_agents ea ON ea.id = ear.agent_id
             WHERE ea.owner = $1
               AND ea.execution_mode = 'paper'
               AND LOWER(ea.name) LIKE 'paper-%'
               AND ear.created_at >= NOW() - INTERVAL '30 days'",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT ear.status, ear.metadata
             FROM external_agent_runs ear
             JOIN external_agents ea ON ea.id = ear.agent_id
             WHERE ea.execution_mode = 'paper'
               AND LOWER(ea.name) LIKE 'paper-%'
               AND ear.created_at >= NOW() - INTERVAL '30 days'",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_tables = PerformanceLedgerKind::Live.tables();
    let live_agents_row = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT COUNT(*) AS agents,
                    COUNT(*) FILTER (WHERE active) AS active_agents
             FROM external_agents
             WHERE owner = $1
               AND execution_mode = 'live'",
        )
        .bind(owner.as_str())
        .fetch_one(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT COUNT(*) AS agents,
                    COUNT(*) FILTER (WHERE active) AS active_agents
             FROM external_agents
             WHERE execution_mode = 'live'",
        )
        .fetch_one(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_positions_row = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT COUNT(*) FILTER (WHERE status = 'open') AS open_positions,
                    COUNT(*) FILTER (WHERE status = 'closed') AS closed_positions,
                    COALESCE(SUM(CASE WHEN status = 'open' THEN unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
             FROM {} pp
             JOIN external_agents ea ON ea.id = pp.agent_id
             WHERE ea.owner = $1
               AND ea.execution_mode = 'live'",
            live_tables.positions
        ))
        .bind(owner.as_str())
        .fetch_one(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT COUNT(*) FILTER (WHERE status = 'open') AS open_positions,
                    COUNT(*) FILTER (WHERE status = 'closed') AS closed_positions,
                    COALESCE(SUM(CASE WHEN status = 'open' THEN unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
             FROM {} pp
             JOIN external_agents ea ON ea.id = pp.agent_id
             WHERE ea.execution_mode = 'live'",
            live_tables.positions
        ))
        .fetch_one(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_fills_row = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT COUNT(*) AS fills,
                    COALESCE(SUM(notional_usdc), 0) AS volume_usdc,
                    COALESCE(SUM(fee_usdc), 0) AS fees_usdc
             FROM {}
             WHERE owner = $1",
            live_tables.fills
        ))
        .bind(owner.as_str())
        .fetch_one(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT COUNT(*) AS fills,
                    COALESCE(SUM(notional_usdc), 0) AS volume_usdc,
                    COALESCE(SUM(fee_usdc), 0) AS fees_usdc
             FROM {}",
            live_tables.fills
        ))
        .fetch_one(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_outcomes_row = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT COALESCE(SUM(realized_pnl_usdc), 0) AS realized_pnl_usdc
             FROM {}
             WHERE owner = $1",
            live_tables.outcomes
        ))
        .bind(owner.as_str())
        .fetch_one(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT COALESCE(SUM(realized_pnl_usdc), 0) AS realized_pnl_usdc
             FROM {}",
            live_tables.outcomes
        ))
        .fetch_one(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_outcome_drawdown_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT closed_at, realized_pnl_usdc
             FROM {}
             WHERE owner = $1
             ORDER BY closed_at ASC, id ASC",
            live_tables.outcomes
        ))
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT closed_at, realized_pnl_usdc
             FROM {}
             ORDER BY closed_at ASC, id ASC",
            live_tables.outcomes
        ))
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_run_metric_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT ear.status, ear.metadata
             FROM external_agent_runs ear
             JOIN external_agents ea ON ea.id = ear.agent_id
             WHERE ea.owner = $1
               AND ea.execution_mode = 'live'
               AND ear.created_at >= NOW() - INTERVAL '30 days'",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT ear.status, ear.metadata
             FROM external_agent_runs ear
             JOIN external_agents ea ON ea.id = ear.agent_id
             WHERE ea.execution_mode = 'live'
               AND ear.created_at >= NOW() - INTERVAL '30 days'",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    outcome_drawdown_rows.extend(live_outcome_drawdown_rows);
    run_metric_rows.extend(live_run_metric_rows);
    let agents = agents_row.try_get::<i64, _>("agents").unwrap_or(0).max(0) as u64;
    let active_agents = agents_row
        .try_get::<i64, _>("active_agents")
        .unwrap_or(0)
        .max(0) as u64;
    let open_positions = positions_row
        .try_get::<i64, _>("open_positions")
        .unwrap_or(0)
        .max(0) as u64;
    let closed_positions = positions_row
        .try_get::<i64, _>("closed_positions")
        .unwrap_or(0)
        .max(0) as u64;
    let fills = fills_row.try_get::<i64, _>("fills").unwrap_or(0).max(0) as u64;
    let volume_usdc = fills_row.try_get::<f64, _>("volume_usdc").unwrap_or(0.0);
    let fees_usdc = fills_row.try_get::<f64, _>("fees_usdc").unwrap_or(0.0);
    let realized_pnl_usdc = outcomes_row
        .try_get::<f64, _>("realized_pnl_usdc")
        .unwrap_or(0.0);
    let unrealized_pnl_usdc = positions_row
        .try_get::<f64, _>("unrealized_pnl_usdc")
        .unwrap_or(0.0);
    let live_agents = live_agents_row
        .try_get::<i64, _>("agents")
        .unwrap_or(0)
        .max(0) as u64;
    let live_active_agents = live_agents_row
        .try_get::<i64, _>("active_agents")
        .unwrap_or(0)
        .max(0) as u64;
    let live_open_positions = live_positions_row
        .try_get::<i64, _>("open_positions")
        .unwrap_or(0)
        .max(0) as u64;
    let live_closed_positions = live_positions_row
        .try_get::<i64, _>("closed_positions")
        .unwrap_or(0)
        .max(0) as u64;
    let live_fills = live_fills_row
        .try_get::<i64, _>("fills")
        .unwrap_or(0)
        .max(0) as u64;
    let live_volume_usdc = live_fills_row
        .try_get::<f64, _>("volume_usdc")
        .unwrap_or(0.0);
    let live_fees_usdc = live_fills_row.try_get::<f64, _>("fees_usdc").unwrap_or(0.0);
    let live_realized_pnl_usdc = live_outcomes_row
        .try_get::<f64, _>("realized_pnl_usdc")
        .unwrap_or(0.0);
    let live_unrealized_pnl_usdc = live_positions_row
        .try_get::<f64, _>("unrealized_pnl_usdc")
        .unwrap_or(0.0);
    let agents = agents + live_agents;
    let active_agents = active_agents + live_active_agents;
    let open_positions = open_positions + live_open_positions;
    let closed_positions = closed_positions + live_closed_positions;
    let fills = fills + live_fills;
    let volume_usdc = volume_usdc + live_volume_usdc;
    let fees_usdc = fees_usdc + live_fees_usdc;
    let realized_pnl_usdc = realized_pnl_usdc + live_realized_pnl_usdc;
    let unrealized_pnl_usdc = unrealized_pnl_usdc + live_unrealized_pnl_usdc;

    let mut overall_drawdown_points = outcome_drawdown_rows
        .iter()
        .map(|row| {
            let closed_at: chrono::DateTime<Utc> =
                row.try_get("closed_at").unwrap_or_else(|_| Utc::now());
            let realized = row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0);
            (closed_at, realized)
        })
        .collect::<Vec<_>>();
    overall_drawdown_points.sort_by(|left, right| {
        left.0.cmp(&right.0).then_with(|| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    let max_drawdown_usdc = calculate_max_drawdown(
        &overall_drawdown_points
            .iter()
            .map(|(_, realized)| *realized)
            .collect::<Vec<_>>(),
    );
    let mut successful_runs = 0_u64;
    let mut detection_latencies = Vec::<f64>::new();
    let mut slippage_ticks = Vec::<f64>::new();
    for row in &run_metric_rows {
        let status = row.try_get::<String, _>("status").unwrap_or_default();
        if status != "failed" {
            successful_runs += 1;
        }
        let metadata = row
            .try_get::<Value, _>("metadata")
            .unwrap_or_else(|_| json!({}));
        if let Some(value) = metadata
            .pointer("/signalMetrics/detectionToOrderMs")
            .and_then(|entry| entry.as_f64())
            .or_else(|| {
                metadata
                    .get("detectionToOrderMs")
                    .and_then(|entry| entry.as_f64())
            })
        {
            detection_latencies.push(value);
        }
        if let Some(value) = metadata
            .get("fillSlippageTicks")
            .and_then(|entry| entry.as_f64())
            .or_else(|| {
                metadata
                    .pointer("/signalMetrics/slippageTicks")
                    .and_then(|entry| entry.as_f64())
            })
        {
            slippage_ticks.push(value);
        }
    }
    let runner_reliability = if run_metric_rows.is_empty() {
        0.0
    } else {
        successful_runs as f64 / run_metric_rows.len() as f64
    };
    let p50_detection_to_order_ms = median_f64(&mut detection_latencies);
    let p50_slippage_ticks = median_f64(&mut slippage_ticks);

    let mut strategy_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT strategy,
                    COUNT(*) AS agents,
                    COUNT(*) FILTER (WHERE active) AS active_agents
             FROM external_agents
             WHERE owner = $1
               AND execution_mode = 'paper'
               AND LOWER(name) LIKE 'paper-%'
             GROUP BY strategy
             ORDER BY strategy ASC",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT strategy,
                    COUNT(*) AS agents,
                    COUNT(*) FILTER (WHERE active) AS active_agents
             FROM external_agents
             WHERE execution_mode = 'paper'
               AND LOWER(name) LIKE 'paper-%'
             GROUP BY strategy
             ORDER BY strategy ASC",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut position_strategy_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT strategy,
                    COUNT(*) FILTER (WHERE status = 'open') AS open_positions,
                    COUNT(*) FILTER (WHERE status = 'closed') AS closed_positions,
                    COALESCE(SUM(CASE WHEN status = 'open' THEN unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
             FROM paper_positions
             WHERE owner = $1
             GROUP BY strategy",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT strategy,
                    COUNT(*) FILTER (WHERE status = 'open') AS open_positions,
                    COUNT(*) FILTER (WHERE status = 'closed') AS closed_positions,
                    COALESCE(SUM(CASE WHEN status = 'open' THEN unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
             FROM paper_positions
             GROUP BY strategy",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut fill_strategy_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT strategy,
                    COUNT(*) AS fills,
                    COALESCE(SUM(notional_usdc), 0) AS volume_usdc,
                    COALESCE(SUM(fee_usdc), 0) AS fees_usdc
             FROM (
                SELECT pf.*, ea.strategy
                FROM paper_fills pf
                JOIN external_agents ea ON ea.id = pf.agent_id
                WHERE pf.owner = $1
             ) AS scoped
             GROUP BY strategy",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT strategy,
                    COUNT(*) AS fills,
                    COALESCE(SUM(notional_usdc), 0) AS volume_usdc,
                    COALESCE(SUM(fee_usdc), 0) AS fees_usdc
             FROM (
                SELECT pf.*, ea.strategy
                FROM paper_fills pf
                JOIN external_agents ea ON ea.id = pf.agent_id
             ) AS scoped
             GROUP BY strategy",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut outcome_strategy_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT strategy,
                    COALESCE(SUM(realized_pnl_usdc), 0) AS realized_pnl_usdc,
                    COALESCE(AVG(CASE WHEN realized_pnl_usdc > 0 THEN 1.0 ELSE 0.0 END), 0) AS win_rate
             FROM paper_outcomes
             WHERE owner = $1
             GROUP BY strategy",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT strategy,
                    COALESCE(SUM(realized_pnl_usdc), 0) AS realized_pnl_usdc,
                    COALESCE(AVG(CASE WHEN realized_pnl_usdc > 0 THEN 1.0 ELSE 0.0 END), 0) AS win_rate
             FROM paper_outcomes
             GROUP BY strategy",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut outcome_strategy_drawdown_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT strategy, closed_at, realized_pnl_usdc
             FROM paper_outcomes
             WHERE owner = $1
             ORDER BY strategy ASC, closed_at ASC, id ASC",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT strategy, closed_at, realized_pnl_usdc
             FROM paper_outcomes
             ORDER BY strategy ASC, closed_at ASC, id ASC",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_strategy_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT strategy,
                    COUNT(*) AS agents,
                    COUNT(*) FILTER (WHERE active) AS active_agents
             FROM external_agents
             WHERE owner = $1
               AND execution_mode = 'live'
             GROUP BY strategy
             ORDER BY strategy ASC",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT strategy,
                    COUNT(*) AS agents,
                    COUNT(*) FILTER (WHERE active) AS active_agents
             FROM external_agents
             WHERE execution_mode = 'live'
             GROUP BY strategy
             ORDER BY strategy ASC",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_position_strategy_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT ea.strategy,
                    COUNT(*) FILTER (WHERE pp.status = 'open') AS open_positions,
                    COUNT(*) FILTER (WHERE pp.status = 'closed') AS closed_positions,
                    COALESCE(SUM(CASE WHEN pp.status = 'open' THEN pp.unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
             FROM {} pp
             JOIN external_agents ea ON ea.id = pp.agent_id
             WHERE ea.owner = $1
               AND ea.execution_mode = 'live'
             GROUP BY ea.strategy",
            live_tables.positions
        ))
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT ea.strategy,
                    COUNT(*) FILTER (WHERE pp.status = 'open') AS open_positions,
                    COUNT(*) FILTER (WHERE pp.status = 'closed') AS closed_positions,
                    COALESCE(SUM(CASE WHEN pp.status = 'open' THEN pp.unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
             FROM {} pp
             JOIN external_agents ea ON ea.id = pp.agent_id
             WHERE ea.execution_mode = 'live'
             GROUP BY ea.strategy",
            live_tables.positions
        ))
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_fill_strategy_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT ea.strategy,
                    COUNT(*) AS fills,
                    COALESCE(SUM(ef.notional_usdc), 0) AS volume_usdc,
                    COALESCE(SUM(ef.fee_usdc), 0) AS fees_usdc
             FROM {} ef
             JOIN external_agents ea ON ea.id = ef.agent_id
             WHERE ea.owner = $1
               AND ea.execution_mode = 'live'
             GROUP BY ea.strategy",
            live_tables.fills
        ))
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT ea.strategy,
                    COUNT(*) AS fills,
                    COALESCE(SUM(ef.notional_usdc), 0) AS volume_usdc,
                    COALESCE(SUM(ef.fee_usdc), 0) AS fees_usdc
             FROM {} ef
             JOIN external_agents ea ON ea.id = ef.agent_id
             WHERE ea.execution_mode = 'live'
             GROUP BY ea.strategy",
            live_tables.fills
        ))
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_outcome_strategy_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT ea.strategy,
                    COALESCE(SUM(eo.realized_pnl_usdc), 0) AS realized_pnl_usdc,
                    COALESCE(AVG(CASE WHEN eo.realized_pnl_usdc > 0 THEN 1.0 ELSE 0.0 END), 0) AS win_rate
             FROM {} eo
             JOIN external_agents ea ON ea.id = eo.agent_id
             WHERE ea.owner = $1
               AND ea.execution_mode = 'live'
             GROUP BY ea.strategy",
            live_tables.outcomes
        ))
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT ea.strategy,
                    COALESCE(SUM(eo.realized_pnl_usdc), 0) AS realized_pnl_usdc,
                    COALESCE(AVG(CASE WHEN eo.realized_pnl_usdc > 0 THEN 1.0 ELSE 0.0 END), 0) AS win_rate
             FROM {} eo
             JOIN external_agents ea ON ea.id = eo.agent_id
             WHERE ea.execution_mode = 'live'
             GROUP BY ea.strategy",
            live_tables.outcomes
        ))
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_outcome_strategy_drawdown_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT ea.strategy, eo.closed_at, eo.realized_pnl_usdc
             FROM {} eo
             JOIN external_agents ea ON ea.id = eo.agent_id
             WHERE ea.owner = $1
               AND ea.execution_mode = 'live'
             ORDER BY ea.strategy ASC, eo.closed_at ASC, eo.id ASC",
            live_tables.outcomes
        ))
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT ea.strategy, eo.closed_at, eo.realized_pnl_usdc
             FROM {} eo
             JOIN external_agents ea ON ea.id = eo.agent_id
             WHERE ea.execution_mode = 'live'
             ORDER BY ea.strategy ASC, eo.closed_at ASC, eo.id ASC",
            live_tables.outcomes
        ))
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    strategy_rows.extend(live_strategy_rows);
    position_strategy_rows.extend(live_position_strategy_rows);
    fill_strategy_rows.extend(live_fill_strategy_rows);
    outcome_strategy_rows.extend(live_outcome_strategy_rows);
    outcome_strategy_drawdown_rows.extend(live_outcome_strategy_drawdown_rows);
    let mut strategy_map = BTreeMap::new();
    for row in strategy_rows {
        let strategy = row
            .try_get::<String, _>("strategy")
            .unwrap_or_else(|_| "unclassified".to_string());
        let entry =
            strategy_map
                .entry(strategy.clone())
                .or_insert(ExternalAgentStrategyPerformance {
                    strategy,
                    agents: 0,
                    active_agents: 0,
                    open_positions: 0,
                    closed_positions: 0,
                    fills: 0,
                    volume_usdc: 0.0,
                    fees_usdc: 0.0,
                    realized_pnl_usdc: 0.0,
                    unrealized_pnl_usdc: 0.0,
                    net_pnl_usdc: 0.0,
                    win_rate: 0.0,
                    max_drawdown_usdc: 0.0,
                });
        entry.agents += row.try_get::<i64, _>("agents").unwrap_or(0).max(0) as u64;
        entry.active_agents += row.try_get::<i64, _>("active_agents").unwrap_or(0).max(0) as u64;
    }

    for row in position_strategy_rows {
        let strategy = row
            .try_get::<String, _>("strategy")
            .unwrap_or_else(|_| "unclassified".to_string());
        let entry =
            strategy_map
                .entry(strategy.clone())
                .or_insert(ExternalAgentStrategyPerformance {
                    strategy,
                    agents: 0,
                    active_agents: 0,
                    open_positions: 0,
                    closed_positions: 0,
                    fills: 0,
                    volume_usdc: 0.0,
                    fees_usdc: 0.0,
                    realized_pnl_usdc: 0.0,
                    unrealized_pnl_usdc: 0.0,
                    net_pnl_usdc: 0.0,
                    win_rate: 0.0,
                    max_drawdown_usdc: 0.0,
                });
        entry.open_positions += row.try_get::<i64, _>("open_positions").unwrap_or(0).max(0) as u64;
        entry.closed_positions += row
            .try_get::<i64, _>("closed_positions")
            .unwrap_or(0)
            .max(0) as u64;
        entry.unrealized_pnl_usdc += row.try_get::<f64, _>("unrealized_pnl_usdc").unwrap_or(0.0);
    }

    for row in fill_strategy_rows {
        let strategy = row
            .try_get::<String, _>("strategy")
            .unwrap_or_else(|_| "unclassified".to_string());
        let entry =
            strategy_map
                .entry(strategy.clone())
                .or_insert(ExternalAgentStrategyPerformance {
                    strategy,
                    agents: 0,
                    active_agents: 0,
                    open_positions: 0,
                    closed_positions: 0,
                    fills: 0,
                    volume_usdc: 0.0,
                    fees_usdc: 0.0,
                    realized_pnl_usdc: 0.0,
                    unrealized_pnl_usdc: 0.0,
                    net_pnl_usdc: 0.0,
                    win_rate: 0.0,
                    max_drawdown_usdc: 0.0,
                });
        entry.fills += row.try_get::<i64, _>("fills").unwrap_or(0).max(0) as u64;
        entry.volume_usdc += row.try_get::<f64, _>("volume_usdc").unwrap_or(0.0);
        entry.fees_usdc += row.try_get::<f64, _>("fees_usdc").unwrap_or(0.0);
    }

    for row in outcome_strategy_rows {
        let strategy = row
            .try_get::<String, _>("strategy")
            .unwrap_or_else(|_| "unclassified".to_string());
        let entry =
            strategy_map
                .entry(strategy.clone())
                .or_insert(ExternalAgentStrategyPerformance {
                    strategy,
                    agents: 0,
                    active_agents: 0,
                    open_positions: 0,
                    closed_positions: 0,
                    fills: 0,
                    volume_usdc: 0.0,
                    fees_usdc: 0.0,
                    realized_pnl_usdc: 0.0,
                    unrealized_pnl_usdc: 0.0,
                    net_pnl_usdc: 0.0,
                    win_rate: 0.0,
                    max_drawdown_usdc: 0.0,
                });
        entry.realized_pnl_usdc += row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0);
        if entry.win_rate == 0.0 {
            entry.win_rate = row.try_get::<f64, _>("win_rate").unwrap_or(0.0);
        }
    }

    let mut strategy_drawdown_points = BTreeMap::<String, Vec<(chrono::DateTime<Utc>, f64)>>::new();
    for row in outcome_strategy_drawdown_rows {
        let strategy = row
            .try_get::<String, _>("strategy")
            .unwrap_or_else(|_| "unclassified".to_string());
        let closed_at: chrono::DateTime<Utc> =
            row.try_get("closed_at").unwrap_or_else(|_| Utc::now());
        let realized = row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0);
        strategy_drawdown_points
            .entry(strategy)
            .or_default()
            .push((closed_at, realized));
    }
    for (strategy, points) in strategy_drawdown_points {
        if let Some(entry) = strategy_map.get_mut(strategy.as_str()) {
            let mut ordered_points = points;
            ordered_points.sort_by(|left, right| {
                left.0.cmp(&right.0).then_with(|| {
                    left.1
                        .partial_cmp(&right.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
            entry.max_drawdown_usdc = calculate_max_drawdown(
                &ordered_points
                    .iter()
                    .map(|(_, realized)| *realized)
                    .collect::<Vec<_>>(),
            );
        }
    }

    let mut strategies = strategy_map.into_values().collect::<Vec<_>>();
    for entry in &mut strategies {
        entry.net_pnl_usdc = entry.realized_pnl_usdc + entry.unrealized_pnl_usdc;
    }

    let mut volume_timeline_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT date_trunc('hour', created_at) AS bucket,
                    COALESCE(SUM(notional_usdc), 0) AS volume_usdc
             FROM paper_fills
             WHERE owner = $1
               AND created_at >= NOW() - INTERVAL '24 hours'
             GROUP BY bucket
             ORDER BY bucket ASC",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT date_trunc('hour', created_at) AS bucket,
                    COALESCE(SUM(notional_usdc), 0) AS volume_usdc
             FROM paper_fills
             WHERE created_at >= NOW() - INTERVAL '24 hours'
             GROUP BY bucket
             ORDER BY bucket ASC",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut realized_timeline_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT date_trunc('hour', closed_at) AS bucket,
                    COALESCE(SUM(realized_pnl_usdc), 0) AS realized_pnl_usdc
             FROM paper_outcomes
             WHERE owner = $1
               AND closed_at >= NOW() - INTERVAL '24 hours'
             GROUP BY bucket
             ORDER BY bucket ASC",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT date_trunc('hour', closed_at) AS bucket,
                    COALESCE(SUM(realized_pnl_usdc), 0) AS realized_pnl_usdc
             FROM paper_outcomes
             WHERE closed_at >= NOW() - INTERVAL '24 hours'
             GROUP BY bucket
             ORDER BY bucket ASC",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut unrealized_timeline_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT bucket, COALESCE(SUM(unrealized_pnl_usdc), 0) AS unrealized_pnl_usdc
             FROM (
                 SELECT DISTINCT ON (position_id, bucket)
                        position_id,
                        date_trunc('hour', created_at) AS bucket,
                        unrealized_pnl_usdc
                 FROM paper_marks
                 WHERE owner = $1
                   AND created_at >= NOW() - INTERVAL '24 hours'
                 ORDER BY position_id, bucket, created_at DESC
             ) AS scoped
             GROUP BY bucket
             ORDER BY bucket ASC",
        )
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT bucket, COALESCE(SUM(unrealized_pnl_usdc), 0) AS unrealized_pnl_usdc
             FROM (
                 SELECT DISTINCT ON (position_id, bucket)
                        position_id,
                        date_trunc('hour', created_at) AS bucket,
                        unrealized_pnl_usdc
                 FROM paper_marks
                 WHERE created_at >= NOW() - INTERVAL '24 hours'
                 ORDER BY position_id, bucket, created_at DESC
             ) AS scoped
             GROUP BY bucket
             ORDER BY bucket ASC",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_volume_timeline_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT date_trunc('hour', ef.created_at) AS bucket,
                    COALESCE(SUM(ef.notional_usdc), 0) AS volume_usdc
             FROM {} ef
             JOIN external_agents ea ON ea.id = ef.agent_id
             WHERE ea.owner = $1
               AND ea.execution_mode = 'live'
               AND ef.created_at >= NOW() - INTERVAL '24 hours'
             GROUP BY bucket
             ORDER BY bucket ASC",
            live_tables.fills
        ))
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT date_trunc('hour', ef.created_at) AS bucket,
                    COALESCE(SUM(ef.notional_usdc), 0) AS volume_usdc
             FROM {} ef
             JOIN external_agents ea ON ea.id = ef.agent_id
             WHERE ea.execution_mode = 'live'
               AND ef.created_at >= NOW() - INTERVAL '24 hours'
             GROUP BY bucket
             ORDER BY bucket ASC",
            live_tables.fills
        ))
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_realized_timeline_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT date_trunc('hour', eo.closed_at) AS bucket,
                    COALESCE(SUM(eo.realized_pnl_usdc), 0) AS realized_pnl_usdc
             FROM {} eo
             JOIN external_agents ea ON ea.id = eo.agent_id
             WHERE ea.owner = $1
               AND ea.execution_mode = 'live'
               AND eo.closed_at >= NOW() - INTERVAL '24 hours'
             GROUP BY bucket
             ORDER BY bucket ASC",
            live_tables.outcomes
        ))
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT date_trunc('hour', eo.closed_at) AS bucket,
                    COALESCE(SUM(eo.realized_pnl_usdc), 0) AS realized_pnl_usdc
             FROM {} eo
             JOIN external_agents ea ON ea.id = eo.agent_id
             WHERE ea.execution_mode = 'live'
               AND eo.closed_at >= NOW() - INTERVAL '24 hours'
             GROUP BY bucket
             ORDER BY bucket ASC",
            live_tables.outcomes
        ))
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let live_unrealized_timeline_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(&format!(
            "SELECT bucket, COALESCE(SUM(unrealized_pnl_usdc), 0) AS unrealized_pnl_usdc
             FROM (
                 SELECT DISTINCT ON (position_id, bucket)
                        position_id,
                        date_trunc('hour', created_at) AS bucket,
                        unrealized_pnl_usdc
                 FROM {}
                 JOIN external_agents ea ON ea.id = agent_id
                 WHERE ea.owner = $1
                   AND ea.execution_mode = 'live'
                   AND created_at >= NOW() - INTERVAL '24 hours'
                 ORDER BY position_id, bucket, created_at DESC
             ) AS scoped
             GROUP BY bucket
             ORDER BY bucket ASC",
            live_tables.marks
        ))
        .bind(owner.as_str())
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(&format!(
            "SELECT bucket, COALESCE(SUM(unrealized_pnl_usdc), 0) AS unrealized_pnl_usdc
             FROM (
                 SELECT DISTINCT ON (position_id, bucket)
                        position_id,
                        date_trunc('hour', created_at) AS bucket,
                        unrealized_pnl_usdc
                 FROM {}
                 JOIN external_agents ea ON ea.id = agent_id
                 WHERE ea.execution_mode = 'live'
                   AND created_at >= NOW() - INTERVAL '24 hours'
                 ORDER BY position_id, bucket, created_at DESC
             ) AS scoped
             GROUP BY bucket
             ORDER BY bucket ASC",
            live_tables.marks
        ))
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    volume_timeline_rows.extend(live_volume_timeline_rows);
    realized_timeline_rows.extend(live_realized_timeline_rows);
    unrealized_timeline_rows.extend(live_unrealized_timeline_rows);

    let mut timeline_map: BTreeMap<String, ExternalAgentPerformancePoint> = BTreeMap::new();
    for row in volume_timeline_rows {
        let bucket: chrono::DateTime<Utc> = row
            .try_get("bucket")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let key = bucket.to_rfc3339();
        let entry = timeline_map
            .entry(key.clone())
            .or_insert(ExternalAgentPerformancePoint {
                bucket: key,
                volume_usdc: 0.0,
                realized_pnl_usdc: 0.0,
                unrealized_pnl_usdc: 0.0,
                net_pnl_usdc: 0.0,
            });
        entry.volume_usdc += row.try_get::<f64, _>("volume_usdc").unwrap_or(0.0);
    }

    for row in realized_timeline_rows {
        let bucket: chrono::DateTime<Utc> = row
            .try_get("bucket")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let key = bucket.to_rfc3339();
        let entry = timeline_map
            .entry(key.clone())
            .or_insert(ExternalAgentPerformancePoint {
                bucket: key,
                volume_usdc: 0.0,
                realized_pnl_usdc: 0.0,
                unrealized_pnl_usdc: 0.0,
                net_pnl_usdc: 0.0,
            });
        entry.realized_pnl_usdc += row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0);
    }

    for row in unrealized_timeline_rows {
        let bucket: chrono::DateTime<Utc> = row
            .try_get("bucket")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let key = bucket.to_rfc3339();
        let entry = timeline_map
            .entry(key.clone())
            .or_insert(ExternalAgentPerformancePoint {
                bucket: key,
                volume_usdc: 0.0,
                realized_pnl_usdc: 0.0,
                unrealized_pnl_usdc: 0.0,
                net_pnl_usdc: 0.0,
            });
        entry.unrealized_pnl_usdc += row.try_get::<f64, _>("unrealized_pnl_usdc").unwrap_or(0.0);
    }

    let mut cumulative_realized = 0.0;
    let mut timeline = timeline_map.into_values().collect::<Vec<_>>();
    for point in &mut timeline {
        cumulative_realized += point.realized_pnl_usdc;
        point.realized_pnl_usdc = cumulative_realized;
        point.net_pnl_usdc = point.realized_pnl_usdc + point.unrealized_pnl_usdc;
    }

    Ok(HttpResponse::Ok().json(ExternalAgentPerformanceResponse {
        scope: if owner_filter.is_none() {
            "all".to_string()
        } else {
            "owner".to_string()
        },
        owner: owner_filter,
        totals: ExternalAgentPerformanceTotals {
            agents,
            active_agents,
            open_positions,
            closed_positions,
            fills,
            volume_usdc,
            fees_usdc,
            realized_pnl_usdc,
            unrealized_pnl_usdc,
            net_pnl_usdc: realized_pnl_usdc + unrealized_pnl_usdc,
            max_drawdown_usdc,
            runner_reliability,
            p50_detection_to_order_ms,
            p50_slippage_ticks,
        },
        strategies,
        timeline,
        updated_at: Utc::now().to_rfc3339(),
    }))
}

pub async fn get_public_external_agents_performance(
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let Some(owner) = public_paper_cohort_owner(&state).map(str::to_string) else {
        return Ok(HttpResponse::Ok().json(empty_public_agents_performance(None)));
    };

    let agents_row = sqlx::query(
        "SELECT COUNT(*) AS agents,
                COUNT(*) FILTER (WHERE ea.active) AS active_agents
         FROM external_agents ea
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'",
    )
    .bind(owner.as_str())
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let positions_row = sqlx::query(
        "SELECT COUNT(*) FILTER (WHERE pp.status = 'open') AS open_positions,
                COUNT(*) FILTER (WHERE pp.status = 'closed') AS closed_positions,
                COALESCE(SUM(CASE WHEN pp.status = 'open' THEN pp.unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
         FROM paper_positions pp
         JOIN external_agents ea ON ea.id = pp.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'",
    )
    .bind(owner.as_str())
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let fills_row = sqlx::query(
        "SELECT COUNT(*) AS fills,
                COALESCE(SUM(pf.notional_usdc), 0) AS volume_usdc,
                COALESCE(SUM(pf.fee_usdc), 0) AS fees_usdc
         FROM paper_fills pf
         JOIN external_agents ea ON ea.id = pf.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'",
    )
    .bind(owner.as_str())
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let outcomes_row = sqlx::query(
        "SELECT COALESCE(SUM(po.realized_pnl_usdc), 0) AS realized_pnl_usdc
         FROM paper_outcomes po
         JOIN external_agents ea ON ea.id = po.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'",
    )
    .bind(owner.as_str())
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let outcome_drawdown_rows = sqlx::query(
        "SELECT po.realized_pnl_usdc
         FROM paper_outcomes po
         JOIN external_agents ea ON ea.id = po.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'
         ORDER BY po.closed_at ASC, po.id ASC",
    )
    .bind(owner.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let run_metric_rows = sqlx::query(
        "SELECT ear.status, ear.metadata
         FROM external_agent_runs ear
         JOIN external_agents ea ON ea.id = ear.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'
           AND ear.created_at >= NOW() - INTERVAL '30 days'",
    )
    .bind(owner.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let agents = agents_row.try_get::<i64, _>("agents").unwrap_or(0).max(0) as u64;
    let active_agents = agents_row
        .try_get::<i64, _>("active_agents")
        .unwrap_or(0)
        .max(0) as u64;
    let open_positions = positions_row
        .try_get::<i64, _>("open_positions")
        .unwrap_or(0)
        .max(0) as u64;
    let closed_positions = positions_row
        .try_get::<i64, _>("closed_positions")
        .unwrap_or(0)
        .max(0) as u64;
    let fills = fills_row.try_get::<i64, _>("fills").unwrap_or(0).max(0) as u64;
    let volume_usdc = fills_row.try_get::<f64, _>("volume_usdc").unwrap_or(0.0);
    let fees_usdc = fills_row.try_get::<f64, _>("fees_usdc").unwrap_or(0.0);
    let realized_pnl_usdc = outcomes_row
        .try_get::<f64, _>("realized_pnl_usdc")
        .unwrap_or(0.0);
    let unrealized_pnl_usdc = positions_row
        .try_get::<f64, _>("unrealized_pnl_usdc")
        .unwrap_or(0.0);
    let max_drawdown_usdc = calculate_max_drawdown(
        &outcome_drawdown_rows
            .iter()
            .map(|row| row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0))
            .collect::<Vec<_>>(),
    );
    let mut successful_runs = 0_u64;
    let mut detection_latencies = Vec::<f64>::new();
    let mut slippage_ticks = Vec::<f64>::new();
    for row in &run_metric_rows {
        let status = row.try_get::<String, _>("status").unwrap_or_default();
        if status != "failed" {
            successful_runs += 1;
        }
        let metadata = row
            .try_get::<Value, _>("metadata")
            .unwrap_or_else(|_| json!({}));
        if let Some(value) = metadata
            .pointer("/signalMetrics/detectionToOrderMs")
            .and_then(|entry| entry.as_f64())
            .or_else(|| {
                metadata
                    .get("detectionToOrderMs")
                    .and_then(|entry| entry.as_f64())
            })
        {
            detection_latencies.push(value);
        }
        if let Some(value) = metadata
            .get("fillSlippageTicks")
            .and_then(|entry| entry.as_f64())
            .or_else(|| {
                metadata
                    .pointer("/signalMetrics/slippageTicks")
                    .and_then(|entry| entry.as_f64())
            })
        {
            slippage_ticks.push(value);
        }
    }
    let runner_reliability = if run_metric_rows.is_empty() {
        0.0
    } else {
        successful_runs as f64 / run_metric_rows.len() as f64
    };
    let p50_detection_to_order_ms = median_f64(&mut detection_latencies);
    let p50_slippage_ticks = median_f64(&mut slippage_ticks);

    let strategy_rows = sqlx::query(
        "SELECT ea.strategy,
                COUNT(*) AS agents,
                COUNT(*) FILTER (WHERE ea.active) AS active_agents
         FROM external_agents ea
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'
         GROUP BY ea.strategy
         ORDER BY ea.strategy ASC",
    )
    .bind(owner.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let position_strategy_rows = sqlx::query(
        "SELECT ea.strategy,
                COUNT(*) FILTER (WHERE pp.status = 'open') AS open_positions,
                COUNT(*) FILTER (WHERE pp.status = 'closed') AS closed_positions,
                COALESCE(SUM(CASE WHEN pp.status = 'open' THEN pp.unrealized_pnl_usdc ELSE 0 END), 0) AS unrealized_pnl_usdc
         FROM paper_positions pp
         JOIN external_agents ea ON ea.id = pp.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'
         GROUP BY ea.strategy",
    )
    .bind(owner.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let fill_strategy_rows = sqlx::query(
        "SELECT ea.strategy,
                COUNT(*) AS fills,
                COALESCE(SUM(pf.notional_usdc), 0) AS volume_usdc,
                COALESCE(SUM(pf.fee_usdc), 0) AS fees_usdc
         FROM paper_fills pf
         JOIN external_agents ea ON ea.id = pf.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'
         GROUP BY ea.strategy",
    )
    .bind(owner.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let outcome_strategy_rows = sqlx::query(
        "SELECT ea.strategy,
                COALESCE(SUM(po.realized_pnl_usdc), 0) AS realized_pnl_usdc,
                COALESCE(AVG(CASE WHEN po.realized_pnl_usdc > 0 THEN 1.0 ELSE 0.0 END), 0) AS win_rate
         FROM paper_outcomes po
         JOIN external_agents ea ON ea.id = po.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'
         GROUP BY ea.strategy",
    )
    .bind(owner.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let outcome_strategy_drawdown_rows = sqlx::query(
        "SELECT ea.strategy, po.realized_pnl_usdc
         FROM paper_outcomes po
         JOIN external_agents ea ON ea.id = po.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'
         ORDER BY ea.strategy ASC, po.closed_at ASC, po.id ASC",
    )
    .bind(owner.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut strategy_map = BTreeMap::<String, ExternalAgentStrategyPerformance>::new();

    for row in strategy_rows {
        let strategy: String = row
            .try_get("strategy")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        strategy_map.insert(
            strategy.clone(),
            ExternalAgentStrategyPerformance {
                strategy: strategy_label(strategy.as_str()),
                agents: row.try_get::<i64, _>("agents").unwrap_or(0).max(0) as u64,
                active_agents: row.try_get::<i64, _>("active_agents").unwrap_or(0).max(0) as u64,
                open_positions: 0,
                closed_positions: 0,
                fills: 0,
                volume_usdc: 0.0,
                fees_usdc: 0.0,
                realized_pnl_usdc: 0.0,
                unrealized_pnl_usdc: 0.0,
                net_pnl_usdc: 0.0,
                win_rate: 0.0,
                max_drawdown_usdc: 0.0,
            },
        );
    }

    for row in position_strategy_rows {
        let strategy: String = row
            .try_get("strategy")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let entry =
            strategy_map
                .entry(strategy.clone())
                .or_insert(ExternalAgentStrategyPerformance {
                    strategy: strategy_label(strategy.as_str()),
                    agents: 0,
                    active_agents: 0,
                    open_positions: 0,
                    closed_positions: 0,
                    fills: 0,
                    volume_usdc: 0.0,
                    fees_usdc: 0.0,
                    realized_pnl_usdc: 0.0,
                    unrealized_pnl_usdc: 0.0,
                    net_pnl_usdc: 0.0,
                    win_rate: 0.0,
                    max_drawdown_usdc: 0.0,
                });
        entry.open_positions = row.try_get::<i64, _>("open_positions").unwrap_or(0).max(0) as u64;
        entry.closed_positions = row
            .try_get::<i64, _>("closed_positions")
            .unwrap_or(0)
            .max(0) as u64;
        entry.unrealized_pnl_usdc = row.try_get::<f64, _>("unrealized_pnl_usdc").unwrap_or(0.0);
        entry.net_pnl_usdc = entry.realized_pnl_usdc + entry.unrealized_pnl_usdc;
    }

    for row in fill_strategy_rows {
        let strategy: String = row
            .try_get("strategy")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let entry =
            strategy_map
                .entry(strategy.clone())
                .or_insert(ExternalAgentStrategyPerformance {
                    strategy: strategy_label(strategy.as_str()),
                    agents: 0,
                    active_agents: 0,
                    open_positions: 0,
                    closed_positions: 0,
                    fills: 0,
                    volume_usdc: 0.0,
                    fees_usdc: 0.0,
                    realized_pnl_usdc: 0.0,
                    unrealized_pnl_usdc: 0.0,
                    net_pnl_usdc: 0.0,
                    win_rate: 0.0,
                    max_drawdown_usdc: 0.0,
                });
        entry.fills = row.try_get::<i64, _>("fills").unwrap_or(0).max(0) as u64;
        entry.volume_usdc = row.try_get::<f64, _>("volume_usdc").unwrap_or(0.0);
        entry.fees_usdc = row.try_get::<f64, _>("fees_usdc").unwrap_or(0.0);
    }

    for row in outcome_strategy_rows {
        let strategy: String = row
            .try_get("strategy")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let entry =
            strategy_map
                .entry(strategy.clone())
                .or_insert(ExternalAgentStrategyPerformance {
                    strategy: strategy_label(strategy.as_str()),
                    agents: 0,
                    active_agents: 0,
                    open_positions: 0,
                    closed_positions: 0,
                    fills: 0,
                    volume_usdc: 0.0,
                    fees_usdc: 0.0,
                    realized_pnl_usdc: 0.0,
                    unrealized_pnl_usdc: 0.0,
                    net_pnl_usdc: 0.0,
                    win_rate: 0.0,
                    max_drawdown_usdc: 0.0,
                });
        entry.realized_pnl_usdc = row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0);
        entry.win_rate = row.try_get::<f64, _>("win_rate").unwrap_or(0.0);
        entry.net_pnl_usdc = entry.realized_pnl_usdc + entry.unrealized_pnl_usdc;
    }

    let mut strategy_drawdown_points = BTreeMap::<String, Vec<f64>>::new();
    for row in outcome_strategy_drawdown_rows {
        let strategy: String = row
            .try_get("strategy")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let realized = row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0);
        strategy_drawdown_points
            .entry(strategy)
            .or_default()
            .push(realized);
    }
    for (strategy, points) in strategy_drawdown_points {
        if let Some(entry) = strategy_map.get_mut(strategy.as_str()) {
            entry.max_drawdown_usdc = calculate_max_drawdown(points.as_slice());
        }
    }

    let strategies = strategy_map.into_values().collect::<Vec<_>>();

    let volume_timeline_rows = sqlx::query(
        "SELECT date_trunc('hour', pf.created_at) AS bucket,
                COALESCE(SUM(pf.notional_usdc), 0) AS volume_usdc
         FROM paper_fills pf
         JOIN external_agents ea ON ea.id = pf.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'
           AND pf.created_at >= NOW() - INTERVAL '24 hours'
         GROUP BY bucket
         ORDER BY bucket ASC",
    )
    .bind(owner.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let realized_timeline_rows = sqlx::query(
        "SELECT date_trunc('hour', po.closed_at) AS bucket,
                COALESCE(SUM(po.realized_pnl_usdc), 0) AS realized_pnl_usdc
         FROM paper_outcomes po
         JOIN external_agents ea ON ea.id = po.agent_id
         WHERE ea.owner = $1
           AND ea.cohort = 'public_research'
           AND ea.execution_mode = 'paper'
           AND LOWER(ea.name) LIKE 'paper-%'
           AND po.closed_at >= NOW() - INTERVAL '24 hours'
         GROUP BY bucket
         ORDER BY bucket ASC",
    )
    .bind(owner.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let unrealized_timeline_rows = sqlx::query(
        "SELECT bucket, COALESCE(SUM(unrealized_pnl_usdc), 0) AS unrealized_pnl_usdc
         FROM (
             SELECT DISTINCT ON (pm.position_id, bucket)
                    pm.position_id,
                    date_trunc('hour', pm.created_at) AS bucket,
                    pm.unrealized_pnl_usdc
             FROM paper_marks pm
             JOIN external_agents ea ON ea.id = pm.agent_id
             WHERE ea.owner = $1
               AND ea.cohort = 'public_research'
               AND ea.execution_mode = 'paper'
               AND LOWER(ea.name) LIKE 'paper-%'
               AND pm.created_at >= NOW() - INTERVAL '24 hours'
             ORDER BY pm.position_id, bucket, pm.created_at DESC
         ) AS scoped
         GROUP BY bucket
         ORDER BY bucket ASC",
    )
    .bind(owner.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut timeline_map: BTreeMap<String, ExternalAgentPerformancePoint> = BTreeMap::new();
    for row in volume_timeline_rows {
        let bucket: chrono::DateTime<Utc> = row
            .try_get("bucket")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let key = bucket.to_rfc3339();
        timeline_map.insert(
            key.clone(),
            ExternalAgentPerformancePoint {
                bucket: key,
                volume_usdc: row.try_get::<f64, _>("volume_usdc").unwrap_or(0.0),
                realized_pnl_usdc: 0.0,
                unrealized_pnl_usdc: 0.0,
                net_pnl_usdc: 0.0,
            },
        );
    }

    for row in realized_timeline_rows {
        let bucket: chrono::DateTime<Utc> = row
            .try_get("bucket")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let key = bucket.to_rfc3339();
        let entry = timeline_map
            .entry(key.clone())
            .or_insert(ExternalAgentPerformancePoint {
                bucket: key,
                volume_usdc: 0.0,
                realized_pnl_usdc: 0.0,
                unrealized_pnl_usdc: 0.0,
                net_pnl_usdc: 0.0,
            });
        entry.realized_pnl_usdc = row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0);
    }

    for row in unrealized_timeline_rows {
        let bucket: chrono::DateTime<Utc> = row
            .try_get("bucket")
            .map_err(|err| ApiError::internal(&err.to_string()))?;
        let key = bucket.to_rfc3339();
        let entry = timeline_map
            .entry(key.clone())
            .or_insert(ExternalAgentPerformancePoint {
                bucket: key,
                volume_usdc: 0.0,
                realized_pnl_usdc: 0.0,
                unrealized_pnl_usdc: 0.0,
                net_pnl_usdc: 0.0,
            });
        entry.unrealized_pnl_usdc = row.try_get::<f64, _>("unrealized_pnl_usdc").unwrap_or(0.0);
    }

    let mut cumulative_realized = 0.0;
    let mut timeline = timeline_map.into_values().collect::<Vec<_>>();
    for point in &mut timeline {
        cumulative_realized += point.realized_pnl_usdc;
        point.realized_pnl_usdc = cumulative_realized;
        point.net_pnl_usdc = point.realized_pnl_usdc + point.unrealized_pnl_usdc;
    }

    Ok(HttpResponse::Ok().json(ExternalAgentPerformanceResponse {
        scope: "public".to_string(),
        owner: Some(owner),
        totals: ExternalAgentPerformanceTotals {
            agents,
            active_agents,
            open_positions,
            closed_positions,
            fills,
            volume_usdc,
            fees_usdc,
            realized_pnl_usdc,
            unrealized_pnl_usdc,
            net_pnl_usdc: realized_pnl_usdc + unrealized_pnl_usdc,
            max_drawdown_usdc,
            runner_reliability,
            p50_detection_to_order_ms,
            p50_slippage_ticks,
        },
        strategies,
        timeline,
        updated_at: Utc::now().to_rfc3339(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::external::types::ExternalOutcome;

    fn sample_market() -> external::types::ExternalMarketSnapshot {
        external::types::ExternalMarketSnapshot {
            id: "limitless:test".to_string(),
            question: "q".to_string(),
            description: "d".to_string(),
            category: "c".to_string(),
            status: "active".to_string(),
            close_time: 0,
            resolved: false,
            outcome: None,
            yes_price: 0.6,
            no_price: 0.4,
            volume: 1000.0,
            source: "external_limitless".to_string(),
            provider: "limitless".to_string(),
            is_external: true,
            external_url: "https://example.com".to_string(),
            chain_id: 8453,
            requires_credentials: false,
            execution_users: true,
            execution_agents: true,
            outcomes: vec![
                ExternalOutcome {
                    label: "Yes".to_string(),
                    probability: 0.6,
                },
                ExternalOutcome {
                    label: "No".to_string(),
                    probability: 0.4,
                },
            ],
            provider_market_ref: "ref".to_string(),
        }
    }

    fn sample_polymarket_market(description: &str) -> external::types::ExternalMarketSnapshot {
        external::types::ExternalMarketSnapshot {
            id: "polymarket:540843".to_string(),
            question: "Will China invade Taiwan before GTA VI?".to_string(),
            description: description.to_string(),
            category: "politics".to_string(),
            status: "active".to_string(),
            close_time: 1_785_499_200,
            resolved: false,
            outcome: None,
            yes_price: 0.52,
            no_price: 0.48,
            volume: 1_500_000.0,
            source: "external_polymarket".to_string(),
            provider: "polymarket".to_string(),
            is_external: true,
            external_url:
                "https://polymarket.com/event/will-china-invades-taiwan-before-gta-vi-716"
                    .to_string(),
            chain_id: 137,
            requires_credentials: true,
            execution_users: true,
            execution_agents: true,
            outcomes: vec![
                ExternalOutcome {
                    label: "Yes".to_string(),
                    probability: 0.52,
                },
                ExternalOutcome {
                    label: "No".to_string(),
                    probability: 0.48,
                },
            ],
            provider_market_ref: "540843".to_string(),
        }
    }

    fn sample_signal_request() -> CreateExternalSignalRequest {
        CreateExternalSignalRequest {
            publisher: None,
            provider: "polymarket".to_string(),
            market_id: "polymarket:540843".to_string(),
            direction: "yes".to_string(),
            confidence_bps: 6200,
            fair_value_low: 0.58,
            fair_value_high: 0.66,
            midpoint_delta_bps: 900,
            catalyst_summary: "sample".to_string(),
            invalidators: vec!["invalidator".to_string()],
            rationale: Some("sample rationale".to_string()),
            expires_at: "2099-01-01T00:00:00Z".to_string(),
            signal_type: Some("scenario_lab".to_string()),
            metadata: None,
            agent_id: None,
            memo_mode: None,
            sources: None,
            resolution_rules_read: None,
            resolution_criteria: None,
            resolution_hazards: None,
            has_live_reference: None,
            repricing_half_life_minutes: None,
            confidence_reasoning: None,
        }
    }

    fn sample_signal_response(market_id: &str) -> ExternalSignalResponse {
        ExternalSignalResponse {
            id: "sig-1".to_string(),
            publisher: "test".to_string(),
            provider: "polymarket".to_string(),
            market_id: market_id.to_string(),
            signal_type: "scenario_lab".to_string(),
            direction: "yes".to_string(),
            confidence_bps: 6200,
            fair_value_low: 0.58,
            fair_value_high: 0.66,
            midpoint_delta_bps: 900,
            catalyst_summary: "sample".to_string(),
            invalidators: vec!["invalidator".to_string()],
            rationale: Some("sample rationale".to_string()),
            metadata: json!({}),
            memo_mode: Some("fair_value".to_string()),
            sources: vec![
                "https://polymarket.com/event/test".to_string(),
                "https://example.com/live".to_string(),
            ],
            resolution_rules_read: true,
            resolution_criteria: Some("sample criteria".to_string()),
            resolution_hazards: Vec::new(),
            has_live_reference: true,
            repricing_half_life_minutes: Some(30),
            confidence_reasoning: Some("sample".to_string()),
            active: true,
            expires_at: "2099-01-01T00:00:00Z".to_string(),
            created_at: "2099-01-01T00:00:00Z".to_string(),
            updated_at: "2099-01-01T00:00:00Z".to_string(),
            agent_id: None,
        }
    }

    #[test]
    fn market_is_closed_for_paper_entry_when_resolved() {
        let mut market = sample_market();
        market.resolved = true;

        assert!(market_is_closed_for_paper_entry(&market, Utc::now()));
    }

    #[test]
    fn market_is_closed_for_paper_entry_when_close_time_has_passed() {
        let mut market = sample_market();
        let now = Utc::now();
        market.close_time = now.timestamp().saturating_sub(1) as u64;

        assert!(market_is_closed_for_paper_entry(&market, now));
    }

    #[test]
    fn market_is_not_closed_for_paper_entry_while_active() {
        let mut market = sample_market();
        let now = Utc::now();
        market.close_time = now.timestamp().saturating_add(60) as u64;

        assert!(!market_is_closed_for_paper_entry(&market, now));
    }

    #[test]
    fn non_admin_agent_scope_stays_self() {
        let user = AuthenticatedUserWithRole {
            wallet_address: "0x1111111111111111111111111111111111111111".to_string(),
            role: UserRole::User,
        };

        let scope = resolve_external_agent_owner_scope(
            &user,
            Some("all"),
            Some("0x2222222222222222222222222222222222222222"),
        );

        assert!(scope.is_err());
        assert_eq!(
            resolve_external_agent_owner_scope(&user, None, None).unwrap(),
            Some("0x1111111111111111111111111111111111111111".to_string())
        );
    }

    #[test]
    fn admin_agent_scope_supports_all_and_owner_filters() {
        let user = AuthenticatedUserWithRole {
            wallet_address: "0x1111111111111111111111111111111111111111".to_string(),
            role: UserRole::Admin,
        };

        assert_eq!(
            resolve_external_agent_owner_scope(&user, Some("all"), None).unwrap(),
            None
        );
        assert_eq!(
            resolve_external_agent_owner_scope(
                &user,
                Some("owner"),
                Some("0x2222222222222222222222222222222222222222"),
            )
            .unwrap(),
            Some("0x2222222222222222222222222222222222222222".to_string())
        );
    }

    #[test]
    fn normalize_evm_wallet_preserves_checksum() {
        let wallet = "0xdCE731717296De1f68F88c6819a41fDbA9c8E8aB";

        assert_eq!(normalize_evm_wallet(wallet).unwrap(), wallet);
    }

    #[test]
    fn normalize_evm_wallet_upgrades_lowercase_input() {
        let wallet = "0xdce731717296de1f68f88c6819a41fdba9c8e8ab";

        assert_eq!(
            normalize_evm_wallet(wallet).unwrap(),
            "0xdCE731717296De1f68F88c6819a41fDbA9c8E8aB"
        );
    }

    #[test]
    fn build_typed_data_uses_checksummed_limitless_owner() {
        let request = CreateExternalOrderIntentRequest {
            provider: "limitless".to_string(),
            market_id: "limitless:test-market".to_string(),
            outcome: "yes".to_string(),
            side: "buy".to_string(),
            price: 0.5,
            quantity: 1.0,
            credential_id: None,
        };
        let market = json!({
            "venue": {
                "exchange": "0x05c748E2f4DcDe0ec9Fa8DDc40DE6b867f923fa5"
            },
            "tokens": {
                "yes": "123",
                "no": "456"
            }
        });

        let typed_data = build_typed_data(
            "0xdce731717296de1f68f88c6819a41fdba9c8e8ab",
            ExternalProvider::Limitless,
            &request,
            "limitless:test-market",
            &market,
            Some(300),
        )
        .unwrap();
        let message = typed_data
            .get("message")
            .and_then(|value| value.as_object())
            .unwrap();

        assert_eq!(
            message
                .get("maker")
                .and_then(|value| value.as_str())
                .unwrap(),
            "0xdCE731717296De1f68F88c6819a41fDbA9c8E8aB"
        );
        assert_eq!(
            message
                .get("signer")
                .and_then(|value| value.as_str())
                .unwrap(),
            "0xdCE731717296De1f68F88c6819a41fDbA9c8E8aB"
        );
    }

    #[test]
    fn ensure_limitless_order_price_inserts_missing_value() {
        let mut order = serde_json::Map::new();
        ensure_limitless_order_price(&mut order, 0.42);

        assert_eq!(
            order.get("price").and_then(|value| value.as_f64()),
            Some(0.42)
        );
    }

    #[test]
    fn ensure_limitless_order_price_preserves_existing_value() {
        let mut order = serde_json::Map::new();
        order.insert("price".to_string(), json!(0.73));

        ensure_limitless_order_price(&mut order, 0.42);

        assert_eq!(
            order.get("price").and_then(|value| value.as_f64()),
            Some(0.73)
        );
    }

    #[test]
    fn build_polymarket_order_message_uses_funder_for_proxy_accounts() {
        let credentials = PolymarketCredentials {
            api_key: "00000000-0000-0000-0000-000000000000".to_string(),
            api_secret: "secret".to_string(),
            api_passphrase: "passphrase".to_string(),
            funder: "0x2222222222222222222222222222222222222222".to_string(),
            signature_type: 2,
        };
        let context = PolymarketOrderContext {
            token_id: "123".to_string(),
            fee_rate_bps: 10,
            minimum_tick_size: 0.01,
            neg_risk: false,
        };

        let message = build_polymarket_order_message(
            "0x1111111111111111111111111111111111111111",
            &credentials,
            "buy",
            0.34,
            100.0,
            &context,
        )
        .unwrap();

        assert_eq!(
            message.get("maker").and_then(|value| value.as_str()),
            Some("0x2222222222222222222222222222222222222222")
        );
        assert_eq!(
            message.get("signer").and_then(|value| value.as_str()),
            Some("0x1111111111111111111111111111111111111111")
        );
        assert_eq!(
            message.get("makerAmount").and_then(|value| value.as_str()),
            Some("34000000")
        );
        assert_eq!(
            message.get("takerAmount").and_then(|value| value.as_str()),
            Some("100000000")
        );
        assert_eq!(
            message.get("side").and_then(|value| value.as_u64()),
            Some(0)
        );
    }

    #[test]
    fn build_polymarket_submit_payload_uses_clob_shape() {
        let credential = StoredCredential {
            id: "cred-1".to_string(),
            owner: "0x1111111111111111111111111111111111111111".to_string(),
            payload: json!({
                "apiKey": "00000000-0000-0000-0000-000000000000",
                "apiSecret": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                "apiPassphrase": "passphrase",
                "funder": "0x2222222222222222222222222222222222222222",
                "signatureType": 2
            }),
        };
        let typed_data = json!({
            "message": {
                "salt": 123,
                "maker": "0x2222222222222222222222222222222222222222",
                "signer": "0x1111111111111111111111111111111111111111",
                "taker": "0x0000000000000000000000000000000000000000",
                "tokenId": "123",
                "makerAmount": "34000000",
                "takerAmount": "100000000",
                "expiration": "0",
                "nonce": "0",
                "feeRateBps": "10",
                "side": 0,
                "signatureType": 2
            }
        });
        let signed_order = json!({
            "typedData": typed_data,
            "signature": "0xabc"
        });

        let payload =
            build_polymarket_submit_payload(&credential, &typed_data, &signed_order).unwrap();

        assert_eq!(
            payload.get("owner").and_then(|value| value.as_str()),
            Some("00000000-0000-0000-0000-000000000000")
        );
        assert_eq!(
            payload.get("orderType").and_then(|value| value.as_str()),
            Some("GTC")
        );
        assert_eq!(
            payload
                .get("order")
                .and_then(|value| value.get("side"))
                .and_then(|value| value.as_str()),
            Some("BUY")
        );
        assert_eq!(
            payload
                .get("order")
                .and_then(|value| value.get("signature"))
                .and_then(|value| value.as_str()),
            Some("0xabc")
        );
    }

    #[test]
    fn submitted_typed_data_supports_camel_and_snake_case() {
        let camel = json!({ "typedData": { "message": { "nonce": "1" } } });
        let snake = json!({ "typed_data": { "message": { "nonce": "2" } } });

        assert_eq!(
            submitted_typed_data(&camel)
                .and_then(|value| value.get("message"))
                .and_then(|value| value.get("nonce"))
                .and_then(|value| value.as_str()),
            Some("1")
        );
        assert_eq!(
            submitted_typed_data(&snake)
                .and_then(|value| value.get("message"))
                .and_then(|value| value.get("nonce"))
                .and_then(|value| value.as_str()),
            Some("2")
        );
    }

    #[test]
    fn required_signed_order_typed_data_rejects_missing_payload() {
        let err = required_signed_order_typed_data(&json!({ "signature": "0xabc" })).unwrap_err();

        assert_eq!(err.code, "INVALID_SIGNED_ORDER");
        assert_eq!(
            err.message,
            "signed order must include typedData unless it is already provider-formatted"
        );
    }

    #[test]
    fn polymarket_builder_headers_match_sdk_contract() {
        let headers = polymarket_builder_headers(
            PolymarketBuilderCredentials {
                api_key: "builder-key",
                api_secret: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                api_passphrase: "builder-passphrase",
            },
            "POST",
            "/order",
            r#"{"foo":"bar"}"#,
            "1",
        )
        .unwrap();

        assert_eq!(headers.api_key, "builder-key");
        assert_eq!(headers.api_passphrase, "builder-passphrase");
        assert_eq!(headers.timestamp, "1");
        assert_eq!(
            headers.signature,
            "lrkaEs3ANc-KEbkGfYeyM-7_fqL3fatsQnztOq-_wXw="
        );
    }

    #[test]
    fn polymarket_builder_headers_reject_invalid_secret() {
        let err = polymarket_builder_headers(
            PolymarketBuilderCredentials {
                api_key: "builder-key",
                api_secret: "not-base64",
                api_passphrase: "builder-passphrase",
            },
            "DELETE",
            "/order",
            r#"{"orderId":"123"}"#,
            "1",
        )
        .unwrap_err();

        assert_eq!(err.code, "INVALID_BUILDER_CREDENTIALS");
        assert_eq!(err.message, "polymarket builder apiSecret is invalid");
    }

    #[test]
    fn polymarket_provider_error_message_accepts_string_payloads() {
        assert_eq!(
            polymarket_provider_error_message(&json!("plain error"), "fallback"),
            "plain error"
        );
    }

    #[test]
    fn provider_order_id_prefers_top_level_order_id() {
        assert_eq!(
            provider_order_id(&json!({ "orderId": "order-1", "id": "order-2" })),
            "order-1"
        );
        assert_eq!(
            provider_order_id(&json!({ "orderID": "order-4", "id": "order-2" })),
            "order-4"
        );
        assert_eq!(
            provider_order_id(&json!({ "order": { "id": "order-3" } })),
            "order-3"
        );
        assert_eq!(
            provider_order_id(&json!({ "order": { "orderID": "order-5" } })),
            "order-5"
        );
        assert_eq!(
            provider_order_id_from_payload(&json!({ "orderID": "order-6" })),
            "order-6"
        );
        assert_eq!(
            provider_order_id_from_payload(&json!({ "order": { "id": "order-7" } })),
            "order-7"
        );
    }

    #[test]
    fn polymarket_signature_type_accepts_supported_values() {
        for value in [0_u64, 1, 2] {
            let payload = json!({ "signatureType": value });
            assert_eq!(
                polymarket_signature_type_from_payload(&payload).unwrap(),
                value as u8
            );
        }
    }

    #[test]
    fn polymarket_signature_type_rejects_out_of_range_values() {
        let payload = json!({ "signatureType": 3 });
        assert!(polymarket_signature_type_from_payload(&payload).is_err());
    }

    #[test]
    fn parse_polymarket_lifecycle_status_accepts_user_and_relayer_states() {
        assert!(matches!(
            parse_polymarket_lifecycle_status(Some("MATCHED")),
            Some(external::polymarket_index::PolymarketTradeLifecycleStatus::Matched)
        ));
        assert!(matches!(
            parse_polymarket_lifecycle_status(Some("STATE_MINED")),
            Some(external::polymarket_index::PolymarketTradeLifecycleStatus::Mined)
        ));
        assert!(matches!(
            parse_polymarket_lifecycle_status(Some("STATE_CONFIRMED")),
            Some(external::polymarket_index::PolymarketTradeLifecycleStatus::Confirmed)
        ));
        assert!(matches!(
            parse_polymarket_lifecycle_status(Some("FAILED")),
            Some(external::polymarket_index::PolymarketTradeLifecycleStatus::Failed)
        ));
    }

    #[test]
    fn best_polymarket_lifecycle_status_prefers_relayer_confirmation() {
        let status = best_polymarket_lifecycle_status(
            Some(external::polymarket_index::PolymarketTradeLifecycleStatus::Matched),
            Some(external::polymarket_index::PolymarketTradeLifecycleStatus::Confirmed),
            Some("0xtx"),
        );

        assert!(matches!(
            status,
            external::polymarket_index::PolymarketTradeLifecycleStatus::Confirmed
        ));
    }

    #[test]
    fn best_polymarket_lifecycle_status_uses_mined_when_only_tx_hash_exists() {
        let status = best_polymarket_lifecycle_status(None, None, Some("0xtx"));

        assert!(matches!(
            status,
            external::polymarket_index::PolymarketTradeLifecycleStatus::Mined
        ));
    }

    #[test]
    fn skip_reason_from_error_maps_provider_and_readiness_codes() {
        assert_eq!(
            skip_reason_from_error(&ApiError::bad_request("CREDENTIAL_NOT_READY", "x")),
            "credential_not_ready"
        );
        assert_eq!(
            skip_reason_from_error(&ApiError::bad_request(
                "POLYMARKET_EXECUTION_NOT_IMPLEMENTED",
                "x"
            )),
            "provider_not_ready"
        );
        assert_eq!(
            run_status_from_error(&ApiError::bad_request("CREDENTIAL_NOT_READY", "x")),
            "skipped"
        );
        assert_eq!(run_status_from_error(&ApiError::internal("boom")), "failed");
    }

    #[test]
    fn parse_external_execution_mode_accepts_live_and_paper() {
        assert_eq!(
            parse_external_execution_mode("live").unwrap(),
            ExternalExecutionMode::Live
        );
        assert_eq!(
            parse_external_execution_mode("paper").unwrap(),
            ExternalExecutionMode::Paper
        );
        assert!(parse_external_execution_mode("demo").is_err());
    }

    #[test]
    fn strategy_label_maps_public_cohort_terms() {
        assert_eq!(strategy_label("momentum"), "proving");
        assert_eq!(strategy_label("mean-revert"), "research");
        assert_eq!(strategy_label("market-maker"), "optimization");
        assert_eq!(strategy_label("maker_reward"), "rebates");
        assert_eq!(strategy_label("event_repricing"), "scenario");
        assert_eq!(strategy_label("event_repricing_v2"), "scenario");
        assert_eq!(strategy_label("wallet_follow"), "mirror");
        assert_eq!(strategy_label("wallet_follow_v2"), "mirror");
        assert_eq!(strategy_label("custom"), "custom");
    }

    #[test]
    fn normalize_agent_cohort_accepts_hyphenated_alias() {
        assert_eq!(
            normalize_agent_cohort("public-research").unwrap(),
            PUBLIC_RESEARCH_COHORT
        );
        assert_eq!(
            normalize_agent_cohort("private_alpha").unwrap(),
            PRIVATE_ALPHA_COHORT
        );
    }

    #[test]
    fn live_strategy_gate_only_allows_wallet_follow_v2() {
        assert!(
            ensure_live_strategy_allowed("wallet_follow_v2", ExternalExecutionMode::Live).is_ok()
        );

        let err =
            ensure_live_strategy_allowed("maker_reward", ExternalExecutionMode::Live).unwrap_err();
        assert_eq!(err.code, "LIVE_STRATEGY_RESTRICTED");
    }

    #[test]
    fn event_repricing_v2_candidate_gate_accepts_clean_signal() {
        let market = sample_polymarket_market("Clean resolution rules.");
        let signal = sample_signal_response(market.id.as_str());
        let requirements = crate::services::external::strategy::EventRepricingV2Requirements {
            min_hours_to_resolution: 24,
            min_signal_sources: 2,
            require_resolution_rules: true,
            require_live_reference: true,
            max_resolution_hazards: 0,
        };

        let reason =
            event_repricing_v2_candidate_ineligibility(&signal, &market, &requirements, Utc::now());

        assert_eq!(reason, None);
    }

    #[test]
    fn event_repricing_v2_candidate_gate_rejects_hazardous_signal() {
        let market = sample_polymarket_market("Hazardous resolution rules.");
        let mut signal = sample_signal_response(market.id.as_str());
        signal.resolution_hazards = vec!["relative_event_dependency".to_string()];
        let requirements = crate::services::external::strategy::EventRepricingV2Requirements {
            min_hours_to_resolution: 24,
            min_signal_sources: 2,
            require_resolution_rules: true,
            require_live_reference: true,
            max_resolution_hazards: 0,
        };

        let reason =
            event_repricing_v2_candidate_ineligibility(&signal, &market, &requirements, Utc::now());

        assert_eq!(
            reason,
            Some("event_repricing_v2: 1 unresolved resolution hazards".to_string())
        );
    }

    #[test]
    fn requested_execution_mode_blocks_non_admin_paper_override() {
        let err =
            requested_execution_mode(ExternalExecutionMode::Live, UserRole::User, Some("paper"))
                .unwrap_err();

        assert_eq!(
            err.message,
            "Only admins can override external agent execution mode"
        );
    }

    #[test]
    fn merge_signal_metadata_infers_polymarket_rule_metadata() {
        let body = sample_signal_request();
        let market = sample_polymarket_market(
            "This market will resolve to \"Yes\" if China commences a military offensive intended \
             to establish control over any portion of the Republic of China (Taiwan) before Grand \
             Theft Auto VI is officially released in the US. Otherwise, this market will resolve \
             to \"No\". If neither occurs by July 31, 2026, 11:59 PM ET, this market will resolve \
             to 50-50. Early access or leaks will not count. The resolution source for the release \
             of GTA VI is official information from Rockstar Games. A consensus of credible media \
             reporting will suffice.",
        );

        let metadata = merge_signal_metadata(&body, &market);

        assert_eq!(metadata_bool(&metadata, "resolutionRulesRead"), Some(true));
        assert_eq!(
            metadata_string(&metadata, "resolutionCriteria"),
            Some(normalize_whitespace(market.description.as_str()))
        );
        assert_eq!(
            metadata_string_list(&metadata, "sources"),
            vec![
                market.external_url.clone(),
                "https://www.rockstargames.com/newswire".to_string(),
                "https://apnews.com/hub/china".to_string(),
            ]
        );
        assert_eq!(metadata_bool(&metadata, "hasLiveReference"), Some(true));

        let hazards = metadata_string_list(&metadata, "resolutionHazards");
        assert!(hazards.contains(&"fallback_split_resolution".to_string()));
        assert!(hazards.contains(&"relative_event_dependency".to_string()));
        assert!(hazards.contains(&"narrow_qualification_clauses".to_string()));
        assert!(hazards.contains(&"credible_reporting_discretion".to_string()));
        assert!(hazards.contains(&"official_source_dependency".to_string()));
    }

    #[test]
    fn merge_signal_metadata_infers_nba_live_sources() {
        let body = sample_signal_request();
        let market = external::types::ExternalMarketSnapshot {
            id: "polymarket:553856".to_string(),
            question: "Will the Thunder reach the NBA Finals before GTA VI?".to_string(),
            description: "This market resolves to Yes if the Oklahoma City Thunder qualify for the NBA Finals before Grand Theft Auto VI is officially released in the US.".to_string(),
            category: "sports".to_string(),
            status: "active".to_string(),
            close_time: 1_785_499_200,
            resolved: false,
            outcome: None,
            yes_price: 0.38,
            no_price: 0.62,
            volume: 250_000.0,
            source: "external_polymarket".to_string(),
            provider: "polymarket".to_string(),
            is_external: true,
            external_url: "https://polymarket.com/event/will-the-thunder-reach-the-nba-finals-before-gta-vi".to_string(),
            chain_id: 137,
            requires_credentials: true,
            execution_users: true,
            execution_agents: true,
            outcomes: vec![
                ExternalOutcome {
                    label: "Yes".to_string(),
                    probability: 0.38,
                },
                ExternalOutcome {
                    label: "No".to_string(),
                    probability: 0.62,
                },
            ],
            provider_market_ref: "553856".to_string(),
        };

        let metadata = merge_signal_metadata(&body, &market);
        let sources = metadata_string_list(&metadata, "sources");

        assert!(sources.contains(&market.external_url));
        assert!(sources.contains(&"https://www.nba.com/standings".to_string()));
        assert!(sources.contains(&"https://www.rockstargames.com/newswire".to_string()));
        assert_eq!(metadata_bool(&metadata, "hasLiveReference"), Some(true));
    }

    #[test]
    fn merge_signal_metadata_preserves_explicit_research_fields() {
        let mut body = sample_signal_request();
        body.sources = Some(vec![
            "https://example.com/live-feed".to_string(),
            "https://example.com/research".to_string(),
        ]);
        body.resolution_rules_read = Some(false);
        body.resolution_criteria = Some("manual criteria".to_string());
        body.resolution_hazards = Some(vec!["manual_hazard".to_string()]);
        body.has_live_reference = Some(true);

        let market = sample_polymarket_market("This market resolves from a description.");
        let metadata = merge_signal_metadata(&body, &market);

        assert_eq!(metadata_bool(&metadata, "resolutionRulesRead"), Some(false));
        assert_eq!(
            metadata_string(&metadata, "resolutionCriteria"),
            Some("manual criteria".to_string())
        );
        assert_eq!(
            metadata_string_list(&metadata, "resolutionHazards"),
            vec!["manual_hazard".to_string()]
        );
        assert_eq!(metadata_bool(&metadata, "hasLiveReference"), Some(true));

        let sources = metadata_string_list(&metadata, "sources");
        assert!(sources.contains(&"https://example.com/live-feed".to_string()));
        assert!(sources.contains(&"https://example.com/research".to_string()));
        assert!(sources.contains(&market.external_url));
        assert_eq!(sources.len(), 3);
    }
}
