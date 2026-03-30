use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::{DateTime, Duration as ChronoDuration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha3::{Digest, Keccak256};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::auth::extract_jwt_user;
use crate::api::jwt::{check_role, UserRole};
use crate::api::ApiError;
use crate::services::database::{
    BaseMarketBootstrapAgentRecord, BaseMarketBootstrapAgentUpsert,
    BaseMarketBootstrapConfigRecord, BaseMarketBootstrapUpsert, PayoutJobRecord,
};
use crate::services::external::types::ExternalMarketId;
use crate::services::external::{
    self, ExternalMarketSource, ExternalMarketsRequest, TradableFilter,
};
use crate::services::provider_rails::{evaluate_provider_access, ProviderRailAction, RailProvider};
use crate::services::x402::{self, X402Resource};
use crate::AppState;

const ERC20_TOTAL_SUPPLY_SELECTOR: &str = "0x18160ddd";
const ERC20_DECIMALS_SELECTOR: &str = "0x313ce567";
const MARKET_CORE_COUNT_SELECTOR: &str = "0xec979082";
const MARKET_CORE_MARKETS_SELECTOR: &str = "0xb1283e77";
const MARKET_CORE_METADATA_SELECTOR: &str = "0x6b6445b6";
const MARKET_CORE_CREATE_RICH_SELECTOR: &str = "0xddabefe7";
const MARKET_CORE_RESOLVE_SELECTOR: &str = "0x57bde446";
const MARKET_CREATED_TOPIC: &str =
    "0x550857481380e1875f94e5eac6470eff69ecd368405067d9d5dfdf645d3d1f8e";
const AGENT_CREATED_TOPIC: &str =
    "0xb8300ae81d50fe3e07f6ea631ab27e47cfc97f4ac11caef13f608d1471d428f9";
const ORDER_BOOK_COUNT_SELECTOR: &str = "0x2453ffa8";
const ORDER_BOOK_ORDERS_SELECTOR: &str = "0xa85c38ef";
const ORDER_BOOK_PLACE_SELECTOR: &str = "0xa8dd6515";
const ORDER_BOOK_CANCEL_SELECTOR: &str = "0x514fcac7";
const ORDER_BOOK_CLAIM_SELECTOR: &str = "0x379607f5";
const ORDER_BOOK_CLAIM_FOR_SELECTOR: &str = "0x0de05659";
const ORDER_BOOK_MATCH_SELECTOR: &str = "0xc6437097";
const ORDER_BOOK_POSITIONS_SELECTOR: &str = "0xe684d718";
const AGENT_RUNTIME_COUNT_SELECTOR: &str = "0xb7dc1284";
const AGENT_RUNTIME_AGENTS_SELECTOR: &str = "0x513856c8";
const AGENT_RUNTIME_CREATE_SELECTOR: &str = "0x325993ba";
const AGENT_RUNTIME_EXECUTE_SELECTOR: &str = "0xe2a343a5";
const AGENT_RUNTIME_MANAGER_APPROVALS_SELECTOR: &str = "0xfd9ac808";
const AGENT_RUNTIME_SET_MANAGER_APPROVAL_SELECTOR: &str = "0xf3ea4160";
const AGENT_RUNTIME_CREATE_FOR_SELECTOR: &str = "0x77c19c89";
const AGENT_RUNTIME_UPDATE_BATCH_SELECTOR: &str = "0x689fd457";
const AGENT_RUNTIME_DEACTIVATE_BATCH_SELECTOR: &str = "0x7fa805d3";
const AGENT_RUNTIME_SET_MANAGER_SELECTOR: &str = "0x0ceddbbb";
const ERC8004_IDENTITY_PROFILE_SELECTOR: &str = "0x9dd9d0fd";
const ERC8004_REPUTATION_OF_SELECTOR: &str = "0xdb89c044";
const ERC8004_IDENTITY_REGISTER_SELECTOR: &str = "0x07e49598";
const ERC8004_IDENTITY_SET_TIER_SELECTOR: &str = "0x93e2282d";
const ERC8004_IDENTITY_SET_ACTIVE_SELECTOR: &str = "0x2ce962cf";
const ERC8004_REPUTATION_SUBMIT_OUTCOME_SELECTOR: &str = "0x30a51426";
const ERC8004_VALIDATION_STATUS_SELECTOR: &str = "0xff2febfc";
const ERC8004_VALIDATION_REQUEST_SELECTOR: &str = "0xaaf400c4";
const ERC8004_VALIDATION_RESPONSE_SELECTOR: &str = "0x30e5993a";
const ORDER_FILLED_TOPIC: &str =
    "0x5aac01386940f75e601757cfe5dc1d4ab2bac84f98d30664486114a8abb38a45";
const MAX_MARKETS_PAGE_SIZE: u64 = 200;
const MAX_EXTERNAL_MARKETS_FETCH_WINDOW: u64 = 500;
const MAX_ORDERBOOK_DEPTH: u64 = 100;
const MAX_TRADES_PAGE_SIZE: u64 = 200;
const MAX_AGENTS_PAGE_SIZE: u64 = 200;
const ORDERBOOK_SCAN_WINDOW: u64 = 150;
const TRADES_BLOCK_SCAN_WINDOW: u64 = 25_000;
const MAX_MARKET_TEXT_LENGTH: usize = 2_048;
const ERC8004_MAX_TIER: u8 = 100;
const MATCHER_STATE_REDIS_KEY: &str = "ops:matcher:state";
const MATCHER_STATS_REDIS_KEY: &str = "ops:matcher:stats";
const INDEXER_CURSOR_KEY: &str = "evm_indexer_main";
const PAR_PRICE_BPS: u64 = 10_000;
const BOOTSTRAP_MIN_SEED_USDC: f64 = 50.0;
const BOOTSTRAP_MAX_SEED_USDC: f64 = 1_000_000.0;
const BOOTSTRAP_DEFAULT_LEVELS: u64 = 5;
const BOOTSTRAP_DEFAULT_BASE_SPREAD_BPS: u64 = 150;
const BOOTSTRAP_DEFAULT_STEP_BPS: u64 = 100;
const BOOTSTRAP_DEFAULT_CADENCE_SECONDS: u64 = 300;
const BOOTSTRAP_DEFAULT_EXPIRY_SECONDS: u64 = 900;
const BOOTSTRAP_DEFAULT_DEPTH_WINDOW_BPS: u64 = 500;
const BOOTSTRAP_DEFAULT_TARGET_DEPTH_MULTIPLIER: f64 = 2.0;
const BOOTSTRAP_DEFAULT_TARGET_VOLUME_MULTIPLIER: f64 = 10.0;
const BOOTSTRAP_DEFAULT_MAX_AGE_SECONDS: u64 = 7 * 24 * 60 * 60;
const BOOTSTRAP_DEFAULT_EXPOSURE_CAP_BPS: u64 = 6_500;
const BOOTSTRAP_QUALIFY_DURATION_HOURS: i64 = 24;
const BOOTSTRAP_LIQUIDITY_MODE_CLOB_ONLY: &str = "clob_only";
const BOOTSTRAP_LIQUIDITY_MODE_HYBRID: &str = "bootstrap_hybrid";
const BOOTSTRAP_STATUS_ACTIVE: &str = "active";
const BOOTSTRAP_STATUS_DISABLED: &str = "disabled";
const BOOTSTRAP_STATUS_PENDING_AUTHORIZATION: &str = "pending_authorization";
const BOOTSTRAP_STATUS_PENDING_FUNDING: &str = "pending_funding";
const BOOTSTRAP_STATUS_PENDING_LAUNCH: &str = "pending_launch";
const BOOTSTRAP_STATUS_PAUSED: &str = "paused";
const BOOTSTRAP_STATUS_ERROR: &str = "error";
const BOOTSTRAP_STRATEGY_LADDER_V1: &str = "ladder_v1";
const BOOTSTRAP_STRATEGY_LMSR_EXPERIMENTAL: &str = "ls_lmsr_v1";
const BOOTSTRAP_STRATEGY_PMM_EXPERIMENTAL: &str = "pmm_experimental";
const COLLATERAL_AVAILABLE_SELECTOR: &str = "0xa0821be3";
const BOOTSTRAP_RUNNER_ACTION_LAUNCH: &str = "launch";
const BOOTSTRAP_RUNNER_ACTION_UPDATE: &str = "update";
const BOOTSTRAP_RUNNER_ACTION_DEACTIVATE: &str = "deactivate";
const BOOTSTRAP_RUNNER_ACTION_EXECUTE: &str = "execute";

#[derive(Serialize)]
pub struct BaseTokenStateResponse {
    pub chain_id: u64,
    pub token_address: String,
    pub total_supply_hex: String,
    pub decimals: u8,
}

#[derive(Deserialize)]
pub struct BaseMarketsQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub source: Option<String>,
    pub tradable: Option<String>,
    #[serde(rename = "includeLowLiquidity")]
    pub include_low_liquidity: Option<bool>,
}

#[derive(Deserialize)]
pub struct BaseOrderBookQuery {
    pub outcome: Option<String>,
    pub depth: Option<u64>,
}

#[derive(Deserialize)]
pub struct BaseTradesQuery {
    pub outcome: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Deserialize)]
pub struct BaseAgentsQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub owner: Option<String>,
    pub market_id: Option<u64>,
    pub active: Option<bool>,
}

#[derive(Deserialize)]
pub struct BasePayoutCandidatesQuery {
    pub limit: Option<u64>,
}

#[derive(Deserialize)]
pub struct BasePayoutJobsQuery {
    pub status: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatcherReportRequest {
    pub attempted: u64,
    pub matched: u64,
    pub failed: u64,
    pub backlog: u64,
    pub tx_latency_ms: u64,
    pub last_tx_hash: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatcherPauseRequest {
    pub reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterBaseMarketBootstrapRequest {
    pub tx_hash: String,
    pub liquidity_mode: String,
    pub seed_usdc: f64,
    pub initial_yes_bps: u64,
    pub manager: Option<String>,
    pub strategy: String,
    pub levels: u64,
    pub base_spread_bps: u64,
    pub step_bps: u64,
    pub cadence_seconds: u64,
    pub expiry_seconds: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBaseMarketBootstrapRuntimeRequest {
    pub inventory_skew_bps: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapOperatorStatusQuery {
    pub owner: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayoutReportRequest {
    pub market_id: u64,
    pub wallet: String,
    pub status: String,
    pub last_tx: Option<String>,
    pub last_error: Option<String>,
    pub retry_after_seconds: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexerBackfillRequest {
    pub from_block: Option<u64>,
}

#[derive(Serialize)]
pub struct BaseMarketsResponse {
    pub markets: Vec<BaseMarketSnapshot>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<BaseFeedWarning>>,
}

#[derive(Clone, Serialize)]
pub struct BaseFeedWarning {
    pub source: String,
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct BaseMarketOutcome {
    pub label: String,
    pub probability: f64,
}

#[derive(Clone, Serialize)]
pub struct BaseMarketSnapshot {
    pub id: String,
    pub question_hash: String,
    pub question: String,
    pub description: String,
    pub category: String,
    pub resolution_source: String,
    pub resolver: String,
    pub close_time: u64,
    pub resolve_time: u64,
    pub resolved: bool,
    pub outcome: Option<String>,
    pub status: String,
    pub source: String,
    pub provider: String,
    pub is_external: bool,
    pub external_url: Option<String>,
    pub chain_id: u64,
    pub requires_credentials: bool,
    pub execution_users: bool,
    pub execution_agents: bool,
    pub outcomes: Vec<BaseMarketOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yes_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_seed_usdc: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_manager: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_levels: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_initial_yes_bps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_base_spread_bps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_step_bps: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_cadence_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_expiry_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_graduated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_launch_tx_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_last_reconciled_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_last_error: Option<String>,
}

#[derive(Serialize)]
pub struct BaseOrderBookLevel {
    pub price: f64,
    pub quantity: f64,
    pub orders: u64,
}

#[derive(Serialize)]
pub struct BaseOrderBookResponse {
    pub market_id: String,
    pub outcome: String,
    pub bids: Vec<BaseOrderBookLevel>,
    pub asks: Vec<BaseOrderBookLevel>,
    pub last_updated: String,
    pub source: String,
    pub provider: String,
    pub chain_id: u64,
    pub provider_market_ref: String,
    pub is_synthetic: bool,
}

#[derive(Serialize)]
pub struct BaseTradeSnapshot {
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

#[derive(Serialize)]
pub struct BaseTradesResponse {
    pub trades: Vec<BaseTradeSnapshot>,
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

#[derive(Serialize)]
pub struct BaseAgentsResponse {
    pub agents: Vec<BaseAgentSnapshot>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
    pub source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasePayoutCandidate {
    pub owner: String,
    pub market_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasePayoutCandidatesResponse {
    pub candidates: Vec<BasePayoutCandidate>,
    pub total: u64,
    pub limit: u64,
    pub source: String,
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatcherRuntimeState {
    pub paused: bool,
    pub reason: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatcherRuntimeStats {
    pub attempted: u64,
    pub matched: u64,
    pub failed: u64,
    pub backlog: u64,
    pub tx_latency_ms: u64,
    pub success_ratio: f64,
    pub last_tx_hash: Option<String>,
    pub last_cycle_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatcherHealthResponse {
    pub running: bool,
    pub paused: bool,
    pub reason: Option<String>,
    pub backlog: u64,
    pub updated_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasePayoutHealthResponse {
    pub seed_inserted: u64,
    pub pending: u64,
    pub processing: u64,
    pub retry: u64,
    pub failed: u64,
    pub oldest_pending_seconds: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasePayoutJobsResponse {
    pub jobs: Vec<PayoutJobRecord>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexerHealthResponse {
    pub enabled: bool,
    pub lag_blocks: u64,
    pub latest_block: u64,
    pub last_indexed_block: u64,
    pub confirmations: u64,
    pub source_block: u64,
}

#[derive(Serialize)]
pub struct BaseIdentityResponse {
    pub wallet: String,
    pub identity_id: Option<String>,
    pub tier: Option<u8>,
    pub active: Option<bool>,
    pub updated_at: Option<u64>,
    pub source: String,
}

#[derive(Serialize)]
pub struct BaseReputationResponse {
    pub wallet: String,
    pub score_bps: Option<u32>,
    pub confidence_bps: Option<u32>,
    pub events: Option<u64>,
    pub notional_microusdc: Option<String>,
    pub source: String,
}

#[derive(Serialize)]
pub struct BaseValidationResponse {
    pub request_hash: String,
    pub validator: String,
    pub agent_id: String,
    pub response: u8,
    pub response_hash: String,
    pub tag: String,
    pub last_update: u64,
    pub responded: bool,
    pub source: String,
}

#[derive(Clone, Serialize)]
pub struct BaseAgentSnapshot {
    pub id: String,
    pub owner: String,
    pub manager: Option<String>,
    pub market_id: String,
    pub is_yes: bool,
    pub price_bps: u64,
    pub size: String,
    pub cadence: u64,
    pub expiry_window: u64,
    pub last_executed_at: u64,
    pub next_execution_at: u64,
    pub can_execute: bool,
    pub active: bool,
    pub status: String,
    pub strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_tier: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_updated_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reputation_score_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reputation_confidence_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reputation_events: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reputation_notional_microusdc: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PreparedEvmWriteResponse {
    pub chain_id: u64,
    pub from: Option<String>,
    pub to: String,
    pub data: String,
    pub value: String,
    pub method: String,
}

#[derive(Serialize)]
pub struct RelayRawTransactionResponse {
    pub chain_id: u64,
    pub tx_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapOperatorStatusResponse {
    pub operator: String,
    pub owner: String,
    pub approved: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapRunnerSlot {
    pub side: String,
    pub level_index: u64,
    pub agent_id: Option<u64>,
    pub is_yes: bool,
    pub price_bps: u64,
    pub size: String,
    pub cadence: u64,
    pub expiry_window: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapRunnerAction {
    pub kind: String,
    pub market_id: u64,
    pub config_status: String,
    pub prepared_write: PreparedEvmWriteResponse,
    pub slots: Vec<BootstrapRunnerSlot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapRunnerTickResponse {
    pub scanned: u64,
    pub actions: Vec<BootstrapRunnerAction>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapRunnerTickRequest {
    pub limit: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapRunnerReportRequest {
    pub market_id: u64,
    pub kind: String,
    pub tx_hash: String,
    pub success: bool,
    pub slots: Vec<BootstrapRunnerSlot>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareCreateMarketWriteRequest {
    pub from: Option<String>,
    pub question: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub resolution_source: Option<String>,
    pub close_time: u64,
    pub resolver: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparePlaceOrderWriteRequest {
    pub from: Option<String>,
    pub market_id: u64,
    pub outcome: String,
    pub price_bps: u64,
    pub size: String,
    pub expiry: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareCancelOrderWriteRequest {
    pub from: Option<String>,
    pub order_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareClaimWriteRequest {
    pub from: Option<String>,
    pub market_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareResolveMarketWriteRequest {
    pub from: Option<String>,
    pub market_id: u64,
    pub outcome: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareClaimForWriteRequest {
    pub from: Option<String>,
    pub user: String,
    pub market_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareMatchOrdersWriteRequest {
    pub from: Option<String>,
    pub first_order_id: u64,
    pub second_order_id: u64,
    pub fill_size: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareCreateAgentWriteRequest {
    pub from: Option<String>,
    pub market_id: u64,
    pub is_yes: bool,
    pub price_bps: u64,
    pub size: String,
    pub cadence: u64,
    pub expiry_window: u64,
    pub strategy: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareExecuteAgentWriteRequest {
    pub from: Option<String>,
    pub agent_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareSetManagerApprovalWriteRequest {
    pub from: Option<String>,
    pub manager: String,
    pub approved: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapAgentConfigInput {
    pub market_id: u64,
    pub is_yes: bool,
    pub price_bps: u64,
    pub size: String,
    pub cadence: u64,
    pub expiry_window: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareBootstrapCreateAgentsWriteRequest {
    pub from: Option<String>,
    pub owner: String,
    pub manager: String,
    pub strategy: String,
    pub agents: Vec<BootstrapAgentConfigInput>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapAgentUpdateInput {
    pub agent_id: u64,
    pub is_yes: bool,
    pub price_bps: u64,
    pub size: String,
    pub cadence: u64,
    pub expiry_window: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUpdateAgentsWriteRequest {
    pub from: Option<String>,
    pub strategy: String,
    pub updates: Vec<BootstrapAgentUpdateInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareDeactivateAgentsWriteRequest {
    pub from: Option<String>,
    pub agent_ids: Vec<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareSetAgentManagerWriteRequest {
    pub from: Option<String>,
    pub agent_id: u64,
    pub manager: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareErc8004RegisterIdentityWriteRequest {
    pub from: Option<String>,
    pub wallet: String,
    pub tier: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareErc8004SetTierWriteRequest {
    pub from: Option<String>,
    pub wallet: String,
    pub tier: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareErc8004SetActiveWriteRequest {
    pub from: Option<String>,
    pub wallet: String,
    pub active: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareErc8004SubmitOutcomeWriteRequest {
    pub from: Option<String>,
    pub wallet: String,
    pub success: bool,
    pub notional_microusdc: String,
    pub confidence_weight_bps: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareErc8004ValidationRequestWriteRequest {
    pub from: Option<String>,
    pub validator: String,
    pub agent_id: String,
    pub request_uri: String,
    pub request_hash: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareErc8004ValidationResponseWriteRequest {
    pub from: Option<String>,
    pub request_hash: String,
    pub response: u8,
    pub response_uri: String,
    pub response_hash: String,
    pub tag: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayRawTransactionRequest {
    pub raw_tx: String,
}

#[derive(Clone, Copy, Default)]
struct LevelAggregate {
    quantity: u64,
    orders: u64,
}

#[derive(Clone)]
struct BootstrapSyntheticBook {
    yes_bids: BTreeMap<u64, LevelAggregate>,
    no_bids: BTreeMap<u64, LevelAggregate>,
}

#[derive(Clone, Debug)]
struct ValidatedBootstrapRegistration {
    liquidity_mode: String,
    status: String,
    seed_usdc: f64,
    initial_yes_bps: u64,
    strategy: String,
    levels: u64,
    base_spread_bps: u64,
    step_bps: u64,
    cadence_seconds: u64,
    expiry_seconds: u64,
    organic_depth_window_bps: u64,
    target_depth_multiplier: f64,
    target_volume_multiplier: f64,
    max_age_seconds: u64,
    exposure_cap_bps: u64,
}

struct BaseRawOrder {
    market_id: u64,
    is_yes: bool,
    price_bps: u64,
    remaining: u64,
    expiry: u64,
    canceled: bool,
}

struct BaseRawAgent {
    owner: String,
    manager: Option<String>,
    market_id: u64,
    is_yes: bool,
    price_bps: u64,
    size: u128,
    cadence: u64,
    expiry_window: u64,
    last_executed_at: u64,
    active: bool,
    strategy: String,
}

#[derive(Clone)]
struct BasePositionSnapshot {
    yes_shares: u128,
    no_shares: u128,
    claimed: bool,
}

#[derive(Clone)]
struct BootstrapDesiredAgent {
    side: &'static str,
    level_index: u64,
    is_yes: bool,
    price_bps: u64,
    size: u64,
    cadence: u64,
    expiry_window: u64,
}

#[derive(Clone)]
struct Erc8004Identity {
    identity_id: u128,
    tier: u8,
    active: bool,
    updated_at: u64,
}

#[derive(Clone)]
struct Erc8004Reputation {
    score_bps: u32,
    confidence_bps: u32,
    events: u64,
    notional_microusdc: u128,
}

#[derive(Clone)]
struct Erc8004Validation {
    validator: String,
    agent_id: u128,
    response: u8,
    response_hash: String,
    tag: String,
    last_update: u64,
}

impl Erc8004Validation {
    fn responded(&self) -> bool {
        self.response > 0
            || self
                .response_hash
                .trim_start_matches("0x")
                .chars()
                .any(|ch| ch != '0')
    }
}

#[derive(Clone)]
struct PendingTrade {
    id: String,
    order_id: u64,
    block_number: u64,
    log_index: u64,
    tx_hash: String,
    quantity: u64,
    outcome: String,
    price_bps: u64,
    created_at: String,
}

pub async fn get_r44_token_state(
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }

    let token_address = state.config.r44_token_address.trim();
    if token_address.is_empty() {
        return Err(ApiError::bad_request(
            "TOKEN_ADDRESS_NOT_CONFIGURED",
            "R44_TOKEN_ADDRESS is not configured",
        ));
    }
    if !is_valid_evm_address(token_address) {
        return Err(ApiError::bad_request(
            "INVALID_TOKEN_ADDRESS",
            "R44_TOKEN_ADDRESS must be a valid 0x EVM address",
        ));
    }

    let total_supply_hex = state
        .evm_rpc
        .eth_call(token_address, ERC20_TOTAL_SUPPLY_SELECTOR)
        .await
        .map_err(map_evm_rpc_error)?;
    let decimals_hex = state
        .evm_rpc
        .eth_call(token_address, ERC20_DECIMALS_SELECTOR)
        .await
        .map_err(map_evm_rpc_error)?;
    let decimals = parse_u8_hex(&decimals_hex)?;

    Ok(HttpResponse::Ok().json(BaseTokenStateResponse {
        chain_id: state.config.base_chain_id,
        token_address: token_address.to_ascii_lowercase(),
        total_supply_hex,
        decimals,
    }))
}

fn is_external_market_id(raw: &str) -> bool {
    raw.trim().contains(':')
}

fn source_label(source: ExternalMarketSource) -> &'static str {
    match source {
        ExternalMarketSource::All => "all",
        ExternalMarketSource::Internal => "internal",
        ExternalMarketSource::Limitless => "limitless",
        ExternalMarketSource::Polymarket => "polymarket",
    }
}

fn to_rail_provider(provider: external::types::ExternalProvider) -> RailProvider {
    match provider {
        external::types::ExternalProvider::Limitless => RailProvider::Limitless,
        external::types::ExternalProvider::Polymarket => RailProvider::Polymarket,
    }
}

fn provider_blocked_error(
    req: &HttpRequest,
    provider: RailProvider,
    action: ProviderRailAction,
) -> ApiError {
    let decision = evaluate_provider_access(req, provider, action);
    ApiError::legal_restricted(
        "REGION_PROVIDER_RESTRICTED",
        "provider unavailable in your region for this action",
        Some(json!({
            "provider": provider.as_str(),
            "action": action.as_str(),
            "country": decision.country,
            "regionClass": decision.region_class.as_str(),
            "routingMode": decision.mode.as_str(),
            "legacyCloseOnly": decision.legacy_close_only,
            "safeFallbackRestriction": decision.safe_fallback_restriction,
            "detail": decision.reason
        })),
    )
}

fn ensure_provider_action_allowed(
    req: &HttpRequest,
    provider: RailProvider,
    action: ProviderRailAction,
) -> Result<(), ApiError> {
    let decision = evaluate_provider_access(req, provider, action);
    if decision.allowed {
        Ok(())
    } else {
        Err(provider_blocked_error(req, provider, action))
    }
}

fn restriction_warning(source: &str, action: ProviderRailAction) -> BaseFeedWarning {
    BaseFeedWarning {
        source: source.to_string(),
        message: format!(
            "{} feed omitted by region policy for {}",
            source,
            action.as_str()
        ),
    }
}

fn from_external_market(snapshot: external::types::ExternalMarketSnapshot) -> BaseMarketSnapshot {
    BaseMarketSnapshot {
        id: snapshot.id,
        question_hash: snapshot.provider_market_ref.clone(),
        question: snapshot.question,
        description: snapshot.description,
        category: snapshot.category,
        resolution_source: snapshot.external_url.clone(),
        resolver: String::new(),
        close_time: snapshot.close_time,
        resolve_time: snapshot.close_time,
        resolved: snapshot.resolved,
        outcome: snapshot.outcome,
        status: snapshot.status,
        source: snapshot.source,
        provider: snapshot.provider,
        is_external: true,
        external_url: Some(snapshot.external_url),
        chain_id: snapshot.chain_id,
        requires_credentials: snapshot.requires_credentials,
        execution_users: snapshot.execution_users,
        execution_agents: snapshot.execution_agents,
        outcomes: snapshot
            .outcomes
            .into_iter()
            .map(|entry| BaseMarketOutcome {
                label: entry.label,
                probability: entry.probability,
            })
            .collect(),
        yes_price: Some(snapshot.yes_price),
        no_price: Some(snapshot.no_price),
        volume: Some(snapshot.volume),
        liquidity_mode: None,
        bootstrap_status: None,
        bootstrap_active: None,
        bootstrap_seed_usdc: None,
        bootstrap_manager: None,
        bootstrap_strategy: None,
        bootstrap_levels: None,
        bootstrap_initial_yes_bps: None,
        bootstrap_base_spread_bps: None,
        bootstrap_step_bps: None,
        bootstrap_cadence_seconds: None,
        bootstrap_expiry_seconds: None,
        bootstrap_graduated_at: None,
        bootstrap_launch_tx_hash: None,
        bootstrap_last_reconciled_at: None,
        bootstrap_last_error: None,
    }
}

fn internal_feed_warning(err: &ApiError) -> BaseFeedWarning {
    let message = if err.message.contains("429 Too Many Requests") {
        "internal Base feed temporarily rate limited".to_string()
    } else {
        "internal Base feed unavailable".to_string()
    };

    BaseFeedWarning {
        source: "internal".to_string(),
        message,
    }
}

fn now_seconds() -> Result<u64, ApiError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::internal("System time error"))
        .map(|duration| duration.as_secs())
}

fn price_from_bps(price_bps: u64) -> f64 {
    (price_bps as f64) / 10_000.0
}

fn clamp_u64(value: u64, min: u64, max: u64) -> u64 {
    value.max(min).min(max)
}

fn insert_level(levels: &mut BTreeMap<u64, LevelAggregate>, price_bps: u64, quantity: u64) {
    if quantity == 0 || !(1..PAR_PRICE_BPS).contains(&price_bps) {
        return;
    }

    let level = levels.entry(price_bps).or_default();
    level.quantity = level.quantity.saturating_add(quantity);
    level.orders = level.orders.saturating_add(1);
}

fn merge_level_maps(
    destination: &mut BTreeMap<u64, LevelAggregate>,
    source: &BTreeMap<u64, LevelAggregate>,
) {
    for (price_bps, level) in source {
        let entry = destination.entry(*price_bps).or_default();
        entry.quantity = entry.quantity.saturating_add(level.quantity);
        entry.orders = entry.orders.saturating_add(level.orders);
    }
}

fn book_side_from_levels(
    levels: &BTreeMap<u64, LevelAggregate>,
    depth: u64,
) -> Vec<BaseOrderBookLevel> {
    levels
        .iter()
        .rev()
        .take(depth as usize)
        .map(|(price_bps, level)| BaseOrderBookLevel {
            price: price_from_bps(*price_bps),
            quantity: level.quantity as f64,
            orders: level.orders,
        })
        .collect()
}

fn complementary_asks_from_levels(
    opposing_levels: &BTreeMap<u64, LevelAggregate>,
    depth: u64,
) -> Vec<BaseOrderBookLevel> {
    opposing_levels
        .iter()
        .rev()
        .filter_map(|(price_bps, level)| {
            let ask_price_bps = PAR_PRICE_BPS.saturating_sub(*price_bps);
            if !(1..PAR_PRICE_BPS).contains(&ask_price_bps) {
                return None;
            }

            Some(BaseOrderBookLevel {
                price: price_from_bps(ask_price_bps),
                quantity: level.quantity as f64,
                orders: level.orders,
            })
        })
        .take(depth as usize)
        .collect()
}

fn bootstrap_reference_yes_bps(config: &BaseMarketBootstrapConfigRecord) -> u64 {
    let exposure_cap = config.exposure_cap_bps.max(1) as i64;
    let skew = (config.inventory_skew_bps as i64).clamp(-exposure_cap, exposure_cap);
    let max_shift = (config.step_bps.saturating_mul(config.levels.max(1)) as i64)
        .max(config.base_spread_bps as i64);
    let adjusted = config.initial_yes_bps as i64 - (skew * max_shift / exposure_cap);
    adjusted.clamp(1, (PAR_PRICE_BPS - 1) as i64) as u64
}

fn bootstrap_side_budget_bps(config: &BaseMarketBootstrapConfigRecord) -> (u64, u64) {
    let exposure_cap = config.exposure_cap_bps.max(1) as i64;
    let skew = (config.inventory_skew_bps as i64).clamp(-exposure_cap, exposure_cap);
    let transfer = 5_000_i64 * skew.abs() / exposure_cap;

    if skew >= 0 {
        ((5_000 - transfer) as u64, (5_000 + transfer) as u64)
    } else {
        ((5_000 + transfer) as u64, (5_000 - transfer) as u64)
    }
}

fn bootstrap_budget_levels(total: u64, levels: u64) -> Vec<u64> {
    if levels == 0 {
        return Vec::new();
    }

    let base = total / levels;
    let remainder = total % levels;

    (0..levels)
        .map(|index| base + u64::from(index < remainder))
        .collect()
}

fn bootstrap_seed_microusdc(seed_usdc: f64) -> u64 {
    (seed_usdc.max(0.0) * 1_000_000.0).round() as u64
}

fn bootstrap_desired_agents(
    config: &BaseMarketBootstrapConfigRecord,
) -> Vec<BootstrapDesiredAgent> {
    let levels = config.levels.max(1);
    let reference_yes_bps = bootstrap_reference_yes_bps(config);
    let reference_no_bps = PAR_PRICE_BPS.saturating_sub(reference_yes_bps);
    let (yes_budget_bps, no_budget_bps) = bootstrap_side_budget_bps(config);
    let seed_microusdc = bootstrap_seed_microusdc(config.seed_usdc);
    let yes_budget = seed_microusdc.saturating_mul(yes_budget_bps) / PAR_PRICE_BPS;
    let no_budget = seed_microusdc.saturating_mul(no_budget_bps) / PAR_PRICE_BPS;
    let yes_quantities = bootstrap_budget_levels(yes_budget, levels);
    let no_quantities = bootstrap_budget_levels(no_budget, levels);

    let exposure_cap = config.exposure_cap_bps.max(1) as i64;
    let skew = (config.inventory_skew_bps as i64).clamp(-exposure_cap, exposure_cap);
    let spread_extra = (config.step_bps.saturating_mul(levels) as i64 * skew.abs()) / exposure_cap;
    let tighten = (spread_extra / 2) as u64;

    let mut desired = Vec::with_capacity((levels * 2) as usize);
    for level_index in 0..levels {
        let base_spread = config.base_spread_bps + level_index.saturating_mul(config.step_bps);
        let yes_spread = if skew > 0 {
            base_spread.saturating_add(spread_extra as u64)
        } else {
            base_spread.saturating_sub(tighten)
        };
        let no_spread = if skew < 0 {
            base_spread.saturating_add(spread_extra as u64)
        } else {
            base_spread.saturating_sub(tighten)
        };

        desired.push(BootstrapDesiredAgent {
            side: "yes",
            level_index,
            is_yes: true,
            price_bps: reference_yes_bps
                .saturating_sub(yes_spread)
                .clamp(1, PAR_PRICE_BPS - 1),
            size: yes_quantities
                .get(level_index as usize)
                .copied()
                .unwrap_or_default(),
            cadence: config.cadence_seconds,
            expiry_window: config.expiry_seconds,
        });
        desired.push(BootstrapDesiredAgent {
            side: "no",
            level_index,
            is_yes: false,
            price_bps: reference_no_bps
                .saturating_sub(no_spread)
                .clamp(1, PAR_PRICE_BPS - 1),
            size: no_quantities
                .get(level_index as usize)
                .copied()
                .unwrap_or_default(),
            cadence: config.cadence_seconds,
            expiry_window: config.expiry_seconds,
        });
    }

    desired
}

fn generate_bootstrap_synthetic_book(
    config: &BaseMarketBootstrapConfigRecord,
) -> BootstrapSyntheticBook {
    let mut yes_bids = BTreeMap::new();
    let mut no_bids = BTreeMap::new();

    for agent in bootstrap_desired_agents(config) {
        if agent.is_yes {
            insert_level(&mut yes_bids, agent.price_bps, agent.size);
        } else {
            insert_level(&mut no_bids, agent.price_bps, agent.size);
        }
    }

    BootstrapSyntheticBook { yes_bids, no_bids }
}

fn sum_depth_near_mid(
    levels: &BTreeMap<u64, LevelAggregate>,
    midpoint_bps: u64,
    window_bps: u64,
) -> u64 {
    levels
        .iter()
        .filter(|(price_bps, _)| price_bps.abs_diff(midpoint_bps) <= window_bps)
        .map(|(_, level)| level.quantity)
        .sum()
}

fn parse_rfc3339_utc(value: Option<&DateTime<Utc>>) -> Option<String> {
    value.map(DateTime::<Utc>::to_rfc3339)
}

fn configured_bootstrap_operator(state: &AppState) -> Result<String, ApiError> {
    configured_address(
        state.config.bootstrap_operator_address.as_str(),
        "BOOTSTRAP_OPERATOR_ADDRESS_NOT_CONFIGURED",
        "BOOTSTRAP_OPERATOR_ADDRESS must be configured for bootstrap automation",
    )
}

fn bootstrap_slot_key(side: &str, level_index: u64) -> String {
    format!("{side}:{level_index}")
}

async fn collect_internal_order_levels(
    state: &AppState,
    market_id: u64,
    now: u64,
) -> Result<(BTreeMap<u64, LevelAggregate>, BTreeMap<u64, LevelAggregate>), ApiError> {
    let order_book = configured_address(
        &state.config.order_book_address,
        "ORDER_BOOK_ADDRESS_NOT_CONFIGURED",
        "ORDER_BOOK_ADDRESS must be configured for Base order book reads",
    )?;
    let total_hex = state
        .evm_rpc
        .eth_call(order_book.as_str(), ORDER_BOOK_COUNT_SELECTOR)
        .await
        .map_err(map_evm_rpc_error)?;
    let total = parse_u64_hex(total_hex.as_str())?;
    let start = total
        .saturating_sub(ORDERBOOK_SCAN_WINDOW.saturating_sub(1))
        .max(1);

    let mut yes_bid_levels: BTreeMap<u64, LevelAggregate> = BTreeMap::new();
    let mut no_bid_levels: BTreeMap<u64, LevelAggregate> = BTreeMap::new();

    if total == 0 {
        return Ok((yes_bid_levels, no_bid_levels));
    }

    for order_id in (start..=total).rev() {
        let calldata = format!(
            "{}{}",
            ORDER_BOOK_ORDERS_SELECTOR,
            encode_u256_hex(order_id)
        );
        let payload = state
            .evm_rpc
            .eth_call(order_book.as_str(), &calldata)
            .await
            .map_err(map_evm_rpc_error)?;
        let Some(order) = decode_order_snapshot(&payload)? else {
            continue;
        };

        if order.market_id != market_id
            || order.canceled
            || order.remaining == 0
            || order.expiry < now
            || !(1..PAR_PRICE_BPS).contains(&order.price_bps)
        {
            continue;
        }

        if order.is_yes {
            insert_level(&mut yes_bid_levels, order.price_bps, order.remaining);
        } else {
            insert_level(&mut no_bid_levels, order.price_bps, order.remaining);
        }
    }

    Ok((yes_bid_levels, no_bid_levels))
}

async fn fetch_manager_approval(
    state: &AppState,
    owner: &str,
    manager: &str,
) -> Result<bool, ApiError> {
    let agent_runtime = configured_address(
        &state.config.agent_runtime_address,
        "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
        "AGENT_RUNTIME_ADDRESS must be configured for Base agents",
    )?;
    let calldata = format!(
        "{}{}{}",
        AGENT_RUNTIME_MANAGER_APPROVALS_SELECTOR,
        encode_address_word(owner)?,
        encode_address_word(manager)?,
    );
    let payload = state
        .evm_rpc
        .eth_call(agent_runtime.as_str(), calldata.as_str())
        .await
        .map_err(map_evm_rpc_error)?;
    parse_bool_word(word_at(payload.as_str(), 0)?)
}

async fn fetch_vault_available_balance(state: &AppState, wallet: &str) -> Result<u64, ApiError> {
    let vault = configured_address(
        &state.config.collateral_vault_address,
        "COLLATERAL_VAULT_ADDRESS_NOT_CONFIGURED",
        "COLLATERAL_VAULT_ADDRESS must be configured for bootstrap funding checks",
    )?;
    let calldata = format!(
        "{}{}",
        COLLATERAL_AVAILABLE_SELECTOR,
        encode_address_word(wallet)?,
    );
    let payload = state
        .evm_rpc
        .eth_call(vault.as_str(), calldata.as_str())
        .await
        .map_err(map_evm_rpc_error)?;
    parse_u64_hex(word_at(payload.as_str(), 0)?)
}

async fn fetch_position_snapshot(
    state: &AppState,
    market_id: u64,
    owner: &str,
) -> Result<BasePositionSnapshot, ApiError> {
    let order_book = configured_address(
        &state.config.order_book_address,
        "ORDER_BOOK_ADDRESS_NOT_CONFIGURED",
        "ORDER_BOOK_ADDRESS must be configured for Base order book reads",
    )?;
    let calldata = format!(
        "{}{}{}",
        ORDER_BOOK_POSITIONS_SELECTOR,
        encode_u256_hex(market_id),
        encode_address_word(owner)?,
    );
    let payload = state
        .evm_rpc
        .eth_call(order_book.as_str(), calldata.as_str())
        .await
        .map_err(map_evm_rpc_error)?;

    Ok(BasePositionSnapshot {
        yes_shares: parse_u128_hex(word_at(payload.as_str(), 0)?)?,
        no_shares: parse_u128_hex(word_at(payload.as_str(), 1)?)?,
        claimed: parse_bool_word(word_at(payload.as_str(), 2)?)?,
    })
}

fn bootstrap_inventory_skew_bps(
    config: &BaseMarketBootstrapConfigRecord,
    position: &BasePositionSnapshot,
) -> i32 {
    if position.claimed {
        return 0;
    }

    let exposure_cap_bps = config.exposure_cap_bps.max(1) as i128;
    let cap_notional = (bootstrap_seed_microusdc(config.seed_usdc) as i128 * exposure_cap_bps)
        / PAR_PRICE_BPS as i128;
    if cap_notional <= 0 {
        return 0;
    }

    let net = position.yes_shares as i128 - position.no_shares as i128;
    let raw = (net * exposure_cap_bps) / cap_notional;
    raw.clamp(-exposure_cap_bps, exposure_cap_bps) as i32
}

fn extract_created_agent_ids(
    receipt: &crate::services::evm_rpc::RpcTransactionReceipt,
    agent_runtime: &str,
    market_id: u64,
) -> Result<Vec<u64>, ApiError> {
    let mut agent_ids = Vec::new();
    for log in &receipt.logs {
        let Some(address) = log.address.as_ref() else {
            continue;
        };
        if !address.eq_ignore_ascii_case(agent_runtime) {
            continue;
        }
        if log.topics.len() < 4 || !log.topics[0].eq_ignore_ascii_case(AGENT_CREATED_TOPIC) {
            continue;
        }
        if parse_u64_hex(log.topics[3].as_str()).ok() != Some(market_id) {
            continue;
        }
        agent_ids.push(parse_u64_hex(log.topics[1].as_str())?);
    }
    Ok(agent_ids)
}

fn runner_slot_from_desired(
    desired: &BootstrapDesiredAgent,
    agent_id: Option<u64>,
) -> BootstrapRunnerSlot {
    BootstrapRunnerSlot {
        side: desired.side.to_string(),
        level_index: desired.level_index,
        agent_id,
        is_yes: desired.is_yes,
        price_bps: desired.price_bps,
        size: desired.size.to_string(),
        cadence: desired.cadence,
        expiry_window: desired.expiry_window,
    }
}

fn runner_slot_from_record(
    record: &BaseMarketBootstrapAgentRecord,
    config: &BaseMarketBootstrapConfigRecord,
) -> BootstrapRunnerSlot {
    BootstrapRunnerSlot {
        side: record.side.clone(),
        level_index: record.level_index,
        agent_id: record.agent_id,
        is_yes: record.side == "yes",
        price_bps: record.current_price_bps.unwrap_or(record.desired_price_bps),
        size: record
            .current_size
            .unwrap_or(record.desired_size)
            .to_string(),
        cadence: config.cadence_seconds,
        expiry_window: config.expiry_seconds,
    }
}

fn bootstrap_has_live_slots(records: &[BaseMarketBootstrapAgentRecord]) -> bool {
    records
        .iter()
        .any(|record| record.active && record.agent_id.is_some())
}

fn bootstrap_slot_records_map(
    records: &[BaseMarketBootstrapAgentRecord],
) -> HashMap<String, BaseMarketBootstrapAgentRecord> {
    records
        .iter()
        .cloned()
        .map(|record| {
            (
                bootstrap_slot_key(record.side.as_str(), record.level_index),
                record,
            )
        })
        .collect()
}

fn bootstrap_active_agent_ids(records: &[BaseMarketBootstrapAgentRecord]) -> Vec<u64> {
    records
        .iter()
        .filter(|record| record.active)
        .filter_map(|record| record.agent_id)
        .collect()
}

fn bootstrap_status_for_runtime(
    current: &BaseMarketBootstrapConfigRecord,
    approved: bool,
    available_balance: u64,
    has_live_slots: bool,
) -> &'static str {
    if current.status == BOOTSTRAP_STATUS_DISABLED {
        return BOOTSTRAP_STATUS_DISABLED;
    }
    if current.status == "graduated" {
        return "graduated";
    }

    let required_balance = bootstrap_seed_microusdc(current.seed_usdc);
    if !approved {
        if has_live_slots {
            BOOTSTRAP_STATUS_PAUSED
        } else {
            BOOTSTRAP_STATUS_PENDING_AUTHORIZATION
        }
    } else if available_balance < required_balance {
        if has_live_slots {
            BOOTSTRAP_STATUS_PAUSED
        } else {
            BOOTSTRAP_STATUS_PENDING_FUNDING
        }
    } else if has_live_slots {
        BOOTSTRAP_STATUS_ACTIVE
    } else {
        BOOTSTRAP_STATUS_PENDING_LAUNCH
    }
}

fn bootstrap_launch_inputs(
    desired: &[BootstrapDesiredAgent],
    existing: &HashMap<String, BaseMarketBootstrapAgentRecord>,
    market_id: u64,
) -> Vec<BootstrapAgentConfigInput> {
    desired
        .iter()
        .filter(|agent| {
            !existing
                .get(bootstrap_slot_key(agent.side, agent.level_index).as_str())
                .is_some_and(|record| record.active && record.agent_id.is_some())
        })
        .map(|agent| BootstrapAgentConfigInput {
            market_id,
            is_yes: agent.is_yes,
            price_bps: agent.price_bps,
            size: agent.size.to_string(),
            cadence: agent.cadence,
            expiry_window: agent.expiry_window,
        })
        .collect()
}

fn bootstrap_update_inputs(
    desired: &[BootstrapDesiredAgent],
    existing: &HashMap<String, BaseMarketBootstrapAgentRecord>,
) -> (Vec<BootstrapAgentUpdateInput>, Vec<BootstrapRunnerSlot>) {
    let mut updates = Vec::new();
    let mut slots = Vec::new();

    for agent in desired {
        let key = bootstrap_slot_key(agent.side, agent.level_index);
        let Some(record) = existing.get(key.as_str()) else {
            continue;
        };
        let Some(agent_id) = record.agent_id else {
            continue;
        };
        let current_price = record.current_price_bps.unwrap_or(record.desired_price_bps);
        let current_size = record.current_size.unwrap_or(record.desired_size);
        if !record.active || current_price != agent.price_bps || current_size != agent.size {
            updates.push(BootstrapAgentUpdateInput {
                agent_id,
                is_yes: agent.is_yes,
                price_bps: agent.price_bps,
                size: agent.size.to_string(),
                cadence: agent.cadence,
                expiry_window: agent.expiry_window,
            });
            slots.push(runner_slot_from_desired(agent, Some(agent_id)));
        }
    }

    (updates, slots)
}

fn bootstrap_active_for_market(
    config: &BaseMarketBootstrapConfigRecord,
    market: &BaseMarketSnapshot,
    now: u64,
) -> bool {
    config.liquidity_mode == BOOTSTRAP_LIQUIDITY_MODE_HYBRID
        && config.status == BOOTSTRAP_STATUS_ACTIVE
        && !market.resolved
        && market.close_time > now
}

fn apply_bootstrap_snapshot(
    snapshot: &mut BaseMarketSnapshot,
    config: Option<&BaseMarketBootstrapConfigRecord>,
) -> Result<(), ApiError> {
    snapshot.liquidity_mode = Some(BOOTSTRAP_LIQUIDITY_MODE_CLOB_ONLY.to_string());

    let Some(config) = config else {
        return Ok(());
    };

    snapshot.liquidity_mode = Some(config.liquidity_mode.clone());
    snapshot.bootstrap_status = Some(config.status.clone());
    snapshot.bootstrap_seed_usdc = Some(config.seed_usdc);
    snapshot.bootstrap_manager = config.manager.clone();
    snapshot.bootstrap_strategy = Some(config.strategy.clone());
    snapshot.bootstrap_levels = Some(config.levels);
    snapshot.bootstrap_initial_yes_bps = Some(config.initial_yes_bps);
    snapshot.bootstrap_base_spread_bps = Some(config.base_spread_bps);
    snapshot.bootstrap_step_bps = Some(config.step_bps);
    snapshot.bootstrap_cadence_seconds = Some(config.cadence_seconds);
    snapshot.bootstrap_expiry_seconds = Some(config.expiry_seconds);
    snapshot.bootstrap_graduated_at = parse_rfc3339_utc(config.graduated_at.as_ref());
    snapshot.bootstrap_launch_tx_hash = config.launch_tx_hash.clone();
    snapshot.bootstrap_last_reconciled_at = parse_rfc3339_utc(config.last_reconciled_at.as_ref());
    snapshot.bootstrap_last_error = config.last_error.clone();

    if config.liquidity_mode != BOOTSTRAP_LIQUIDITY_MODE_HYBRID {
        snapshot.bootstrap_active = Some(false);
        return Ok(());
    }

    let active = bootstrap_active_for_market(config, snapshot, now_seconds()?);
    snapshot.bootstrap_active = Some(active);

    if active {
        let yes_bps = bootstrap_reference_yes_bps(config);
        snapshot.yes_price = Some(price_from_bps(yes_bps));
        snapshot.no_price = Some(price_from_bps(PAR_PRICE_BPS - yes_bps));
    }

    Ok(())
}

fn validate_bootstrap_registration(
    body: &RegisterBaseMarketBootstrapRequest,
) -> Result<ValidatedBootstrapRegistration, ApiError> {
    let liquidity_mode = body.liquidity_mode.trim().to_ascii_lowercase();
    if liquidity_mode != BOOTSTRAP_LIQUIDITY_MODE_CLOB_ONLY
        && liquidity_mode != BOOTSTRAP_LIQUIDITY_MODE_HYBRID
    {
        return Err(ApiError::bad_request(
            "INVALID_LIQUIDITY_MODE",
            "liquidityMode must be either 'clob_only' or 'bootstrap_hybrid'",
        ));
    }

    let strategy = body.strategy.trim().to_ascii_lowercase();
    if strategy == BOOTSTRAP_STRATEGY_LMSR_EXPERIMENTAL
        || strategy == BOOTSTRAP_STRATEGY_PMM_EXPERIMENTAL
    {
        return Err(ApiError::bad_request(
            "BOOTSTRAP_STRATEGY_DISABLED",
            "only ladder_v1 is enabled in this build",
        ));
    }
    if strategy != BOOTSTRAP_STRATEGY_LADDER_V1 {
        return Err(ApiError::bad_request(
            "INVALID_BOOTSTRAP_STRATEGY",
            "strategy must be ladder_v1",
        ));
    }

    let initial_yes_bps = clamp_u64(body.initial_yes_bps, 1, PAR_PRICE_BPS - 1);
    let levels = clamp_u64(body.levels, 1, 12);
    let base_spread_bps = clamp_u64(body.base_spread_bps, 25, 1_000);
    let step_bps = clamp_u64(body.step_bps, 10, 1_000);
    let cadence_seconds = clamp_u64(body.cadence_seconds, 30, 3_600);
    let expiry_seconds = clamp_u64(body.expiry_seconds, cadence_seconds, 7_200);

    if liquidity_mode == BOOTSTRAP_LIQUIDITY_MODE_CLOB_ONLY {
        return Ok(ValidatedBootstrapRegistration {
            liquidity_mode,
            status: BOOTSTRAP_STATUS_DISABLED.to_string(),
            seed_usdc: 0.0,
            initial_yes_bps,
            strategy,
            levels,
            base_spread_bps,
            step_bps,
            cadence_seconds,
            expiry_seconds,
            organic_depth_window_bps: BOOTSTRAP_DEFAULT_DEPTH_WINDOW_BPS,
            target_depth_multiplier: BOOTSTRAP_DEFAULT_TARGET_DEPTH_MULTIPLIER,
            target_volume_multiplier: BOOTSTRAP_DEFAULT_TARGET_VOLUME_MULTIPLIER,
            max_age_seconds: BOOTSTRAP_DEFAULT_MAX_AGE_SECONDS,
            exposure_cap_bps: BOOTSTRAP_DEFAULT_EXPOSURE_CAP_BPS,
        });
    }

    if !body.seed_usdc.is_finite()
        || body.seed_usdc < BOOTSTRAP_MIN_SEED_USDC
        || body.seed_usdc > BOOTSTRAP_MAX_SEED_USDC
    {
        return Err(ApiError::bad_request(
            "INVALID_BOOTSTRAP_SEED",
            "seedUsdc must be between 50 and 1000000",
        ));
    }

    Ok(ValidatedBootstrapRegistration {
        liquidity_mode,
        status: BOOTSTRAP_STATUS_PENDING_LAUNCH.to_string(),
        seed_usdc: (body.seed_usdc * 100.0).round() / 100.0,
        initial_yes_bps,
        strategy,
        levels,
        base_spread_bps,
        step_bps,
        cadence_seconds,
        expiry_seconds,
        organic_depth_window_bps: BOOTSTRAP_DEFAULT_DEPTH_WINDOW_BPS,
        target_depth_multiplier: BOOTSTRAP_DEFAULT_TARGET_DEPTH_MULTIPLIER,
        target_volume_multiplier: BOOTSTRAP_DEFAULT_TARGET_VOLUME_MULTIPLIER,
        max_age_seconds: BOOTSTRAP_DEFAULT_MAX_AGE_SECONDS,
        exposure_cap_bps: BOOTSTRAP_DEFAULT_EXPOSURE_CAP_BPS,
    })
}

async fn verify_market_bootstrap_registration_tx(
    state: &AppState,
    market_id: u64,
    tx_hash: &str,
) -> Result<String, ApiError> {
    let tx_hash = normalize_required_bytes32(
        tx_hash,
        "INVALID_TX_HASH",
        "txHash must be a valid 0x-prefixed transaction hash",
    )?;
    let market_core = configured_address(
        &state.config.market_core_address,
        "MARKET_CORE_ADDRESS_NOT_CONFIGURED",
        "MARKET_CORE_ADDRESS must be configured for Base markets",
    )?;

    let receipt = state
        .evm_rpc
        .eth_get_transaction_receipt(tx_hash.as_str())
        .await
        .map_err(|_| {
            ApiError::bad_request("INVALID_TX_HASH", "unable to fetch transaction receipt")
        })?
        .ok_or_else(|| ApiError::bad_request("INVALID_TX_HASH", "transaction receipt not found"))?;
    let status = receipt
        .status
        .as_deref()
        .and_then(|value| parse_u64_hex(value).ok())
        .ok_or_else(|| {
            ApiError::bad_request("INVALID_TX_HASH", "transaction status unavailable")
        })?;
    if status != 1 {
        return Err(ApiError::bad_request(
            "INVALID_TX_HASH",
            "transaction reverted onchain",
        ));
    }

    let tx = state
        .evm_rpc
        .eth_get_transaction_by_hash(tx_hash.as_str())
        .await
        .map_err(|_| ApiError::bad_request("INVALID_TX_HASH", "unable to fetch transaction"))?
        .ok_or_else(|| ApiError::bad_request("INVALID_TX_HASH", "transaction not found"))?;

    let sender = tx
        .from
        .as_deref()
        .map(|value| {
            normalize_required_address(value, "INVALID_TX_HASH", "transaction sender unavailable")
        })
        .transpose()?
        .ok_or_else(|| {
            ApiError::bad_request("INVALID_TX_HASH", "transaction sender unavailable")
        })?;
    let target = tx
        .to
        .as_deref()
        .map(|value| {
            normalize_required_address(value, "INVALID_TX_HASH", "transaction target unavailable")
        })
        .transpose()?
        .ok_or_else(|| {
            ApiError::bad_request("INVALID_TX_HASH", "transaction target unavailable")
        })?;
    if target != market_core {
        return Err(ApiError::bad_request(
            "INVALID_TX_HASH",
            "transaction target does not match configured market core",
        ));
    }
    if !tx
        .input
        .to_ascii_lowercase()
        .starts_with(MARKET_CORE_CREATE_RICH_SELECTOR.trim_start_matches("0x"))
        && !tx
            .input
            .to_ascii_lowercase()
            .starts_with(MARKET_CORE_CREATE_RICH_SELECTOR)
    {
        return Err(ApiError::bad_request(
            "INVALID_TX_HASH",
            "transaction is not a createMarketRich call",
        ));
    }

    if let (Some(tx_block), Some(receipt_block)) =
        (tx.block_number.as_deref(), receipt.block_number.as_deref())
    {
        let tx_block = parse_u64_hex(tx_block).map_err(|_| {
            ApiError::bad_request("INVALID_TX_HASH", "transaction block is invalid")
        })?;
        let receipt_block = parse_u64_hex(receipt_block)
            .map_err(|_| ApiError::bad_request("INVALID_TX_HASH", "receipt block is invalid"))?;
        if tx_block != receipt_block {
            return Err(ApiError::bad_request(
                "INVALID_TX_HASH",
                "transaction block mismatch",
            ));
        }
    }

    let event_found = receipt.logs.iter().any(|log| {
        let Some(address) = log.address.as_deref() else {
            return false;
        };
        let Ok(log_address) =
            normalize_required_address(address, "INVALID_TX_HASH", "invalid receipt log address")
        else {
            return false;
        };

        log_address == market_core
            && log.topics.len() >= 2
            && log.topics[0].eq_ignore_ascii_case(MARKET_CREATED_TOPIC)
            && parse_u64_hex(log.topics[1].as_str()).ok() == Some(market_id)
    });
    if !event_found {
        return Err(ApiError::bad_request(
            "INVALID_TX_HASH",
            "transaction did not emit the expected MarketCreated event",
        ));
    }

    Ok(sender)
}

async fn maybe_refresh_bootstrap_state(
    state: &AppState,
    market: &BaseMarketSnapshot,
    config: BaseMarketBootstrapConfigRecord,
    organic_yes_bids: &BTreeMap<u64, LevelAggregate>,
    organic_no_bids: &BTreeMap<u64, LevelAggregate>,
) -> Result<BaseMarketBootstrapConfigRecord, ApiError> {
    if config.liquidity_mode != BOOTSTRAP_LIQUIDITY_MODE_HYBRID
        || config.status != BOOTSTRAP_STATUS_ACTIVE
    {
        return Ok(config);
    }

    let now = Utc::now();
    if market.resolved || market.close_time <= now.timestamp().max(0) as u64 {
        return Ok(state
            .db
            .graduate_base_market_bootstrap(config.market_id, "market_closed")
            .await?
            .unwrap_or(config));
    }

    if config.activated_at
        + ChronoDuration::seconds(config.max_age_seconds.min(i64::MAX as u64) as i64)
        <= now
    {
        return Ok(state
            .db
            .graduate_base_market_bootstrap(config.market_id, "max_age")
            .await?
            .unwrap_or(config));
    }

    if market
        .volume
        .is_some_and(|volume| volume >= config.seed_usdc * config.target_volume_multiplier)
    {
        return Ok(state
            .db
            .graduate_base_market_bootstrap(config.market_id, "volume")
            .await?
            .unwrap_or(config));
    }

    let synthetic = generate_bootstrap_synthetic_book(&config);
    let yes_mid = bootstrap_reference_yes_bps(&config);
    let no_mid = PAR_PRICE_BPS - yes_mid;
    let window_bps = config.organic_depth_window_bps;
    let organic_depth = sum_depth_near_mid(organic_yes_bids, yes_mid, window_bps)
        .saturating_add(sum_depth_near_mid(organic_no_bids, no_mid, window_bps));
    let bootstrap_depth = sum_depth_near_mid(&synthetic.yes_bids, yes_mid, window_bps)
        .saturating_add(sum_depth_near_mid(&synthetic.no_bids, no_mid, window_bps));

    if bootstrap_depth == 0 {
        return Ok(config);
    }

    let qualifies =
        (organic_depth as f64) >= (bootstrap_depth as f64 * config.target_depth_multiplier);
    if qualifies {
        if let Some(since) = config.depth_qualified_since {
            if now.signed_duration_since(since)
                >= ChronoDuration::hours(BOOTSTRAP_QUALIFY_DURATION_HOURS)
            {
                return Ok(state
                    .db
                    .graduate_base_market_bootstrap(config.market_id, "organic_depth")
                    .await?
                    .unwrap_or(config));
            }
        } else {
            state
                .db
                .set_base_market_bootstrap_depth_qualified_since(config.market_id, Some(now))
                .await?;
            let mut updated = config;
            updated.depth_qualified_since = Some(now);
            return Ok(updated);
        }
    } else if config.depth_qualified_since.is_some() {
        state
            .db
            .set_base_market_bootstrap_depth_qualified_since(config.market_id, None)
            .await?;
        let mut updated = config;
        updated.depth_qualified_since = None;
        return Ok(updated);
    }

    Ok(config)
}

async fn fetch_internal_market_snapshots(
    state: &AppState,
) -> Result<Vec<BaseMarketSnapshot>, ApiError> {
    let market_core = state.config.market_core_address.trim();
    if market_core.is_empty() {
        return Err(ApiError::bad_request(
            "MARKET_CORE_ADDRESS_NOT_CONFIGURED",
            "MARKET_CORE_ADDRESS must be configured for Base markets",
        ));
    }
    if !is_valid_evm_address(market_core) {
        return Err(ApiError::bad_request(
            "INVALID_MARKET_CORE_ADDRESS",
            "MARKET_CORE_ADDRESS must be a valid 0x EVM address",
        ));
    }

    let total_hex = state
        .evm_rpc
        .eth_call(market_core, MARKET_CORE_COUNT_SELECTOR)
        .await
        .map_err(map_evm_rpc_error)?;
    let total = parse_u64_hex(&total_hex)?;
    if total == 0 {
        return Ok(Vec::new());
    }
    let bootstrap_configs = state
        .db
        .list_base_market_bootstrap_configs()
        .await?
        .into_iter()
        .map(|entry| (entry.market_id, entry))
        .collect::<HashMap<_, _>>();

    let mut markets = Vec::with_capacity(total as usize);
    for index in 1..=total {
        let calldata = format!("{}{}", MARKET_CORE_MARKETS_SELECTOR, encode_u256_hex(index));
        let slot = state
            .evm_rpc
            .eth_call(market_core, &calldata)
            .await
            .map_err(map_evm_rpc_error)?;
        let mut snapshot = decode_market_snapshot(index, &slot)?;

        let metadata_calldata = format!(
            "{}{}",
            MARKET_CORE_METADATA_SELECTOR,
            encode_u256_hex(index)
        );
        if let Ok(payload) = state
            .evm_rpc
            .eth_call(market_core, &metadata_calldata)
            .await
        {
            if let Ok((question, description, category, resolution_source)) =
                decode_market_metadata_tuple(&payload)
            {
                snapshot.question = question;
                snapshot.description = description;
                snapshot.category = category;
                snapshot.resolution_source = resolution_source;
            }
        }
        snapshot.source = "internal_market_core".to_string();
        snapshot.provider = "internal".to_string();
        snapshot.is_external = false;
        snapshot.external_url = None;
        snapshot.chain_id = state.config.base_chain_id;
        snapshot.requires_credentials = false;
        snapshot.execution_users = true;
        snapshot.execution_agents = true;
        apply_bootstrap_snapshot(&mut snapshot, bootstrap_configs.get(&index))?;
        markets.push(snapshot);
    }

    Ok(markets)
}

async fn fetch_internal_market_snapshot_by_id(
    state: &AppState,
    market_id: u64,
) -> Result<BaseMarketSnapshot, ApiError> {
    if market_id == 0 {
        return Err(ApiError::bad_request(
            "INVALID_MARKET_ID",
            "market_id must be a positive integer",
        ));
    }

    let market_core = state.config.market_core_address.trim();
    if market_core.is_empty() {
        return Err(ApiError::bad_request(
            "MARKET_CORE_ADDRESS_NOT_CONFIGURED",
            "MARKET_CORE_ADDRESS must be configured for Base markets",
        ));
    }
    if !is_valid_evm_address(market_core) {
        return Err(ApiError::bad_request(
            "INVALID_MARKET_CORE_ADDRESS",
            "MARKET_CORE_ADDRESS must be a valid 0x EVM address",
        ));
    }

    let total_hex = state
        .evm_rpc
        .eth_call(market_core, MARKET_CORE_COUNT_SELECTOR)
        .await
        .map_err(map_evm_rpc_error)?;
    let total = parse_u64_hex(&total_hex)?;
    if market_id > total {
        return Err(ApiError::not_found("Base market"));
    }

    let calldata = format!(
        "{}{}",
        MARKET_CORE_MARKETS_SELECTOR,
        encode_u256_hex(market_id)
    );
    let slot = state
        .evm_rpc
        .eth_call(market_core, &calldata)
        .await
        .map_err(map_evm_rpc_error)?;
    let mut snapshot = decode_market_snapshot(market_id, &slot)?;

    let metadata_calldata = format!(
        "{}{}",
        MARKET_CORE_METADATA_SELECTOR,
        encode_u256_hex(market_id)
    );
    if let Ok(payload) = state
        .evm_rpc
        .eth_call(market_core, &metadata_calldata)
        .await
    {
        if let Ok((question, description, category, resolution_source)) =
            decode_market_metadata_tuple(&payload)
        {
            snapshot.question = question;
            snapshot.description = description;
            snapshot.category = category;
            snapshot.resolution_source = resolution_source;
        }
    }
    snapshot.source = "internal_market_core".to_string();
    snapshot.provider = "internal".to_string();
    snapshot.is_external = false;
    snapshot.external_url = None;
    snapshot.chain_id = state.config.base_chain_id;
    snapshot.requires_credentials = false;
    snapshot.execution_users = true;
    snapshot.execution_agents = true;
    let bootstrap_config = state.db.get_base_market_bootstrap(market_id).await?;
    apply_bootstrap_snapshot(&mut snapshot, bootstrap_config.as_ref())?;

    Ok(snapshot)
}

pub async fn get_base_markets(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
    query: web::Query<BaseMarketsQuery>,
) -> Result<impl Responder, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }

    let source = ExternalMarketSource::from_query(query.source.as_deref())?;
    let tradable = TradableFilter::from_query(query.tradable.as_deref())?;
    let limit = query.limit.unwrap_or(50).min(MAX_MARKETS_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0);
    let include_low_liquidity = query.include_low_liquidity.unwrap_or(false);
    let external_fetch_window = limit
        .saturating_add(offset)
        .clamp(1, MAX_EXTERNAL_MARKETS_FETCH_WINDOW);

    let mut markets = Vec::new();
    let mut warnings: Vec<BaseFeedWarning> = Vec::new();

    if matches!(
        source,
        ExternalMarketSource::All | ExternalMarketSource::Internal
    ) {
        match fetch_internal_market_snapshots(&state).await {
            Ok(internal_markets) => markets.extend(internal_markets),
            Err(err) if matches!(source, ExternalMarketSource::All) => {
                warnings.push(internal_feed_warning(&err));
            }
            Err(err) => return Err(err),
        }
    }

    if matches!(
        source,
        ExternalMarketSource::All
            | ExternalMarketSource::Limitless
            | ExternalMarketSource::Polymarket
    ) {
        if matches!(source, ExternalMarketSource::Limitless) {
            ensure_provider_action_allowed(
                &req,
                RailProvider::Limitless,
                ProviderRailAction::Feed,
            )?;
        }
        if matches!(source, ExternalMarketSource::Polymarket) {
            ensure_provider_action_allowed(
                &req,
                RailProvider::Polymarket,
                ProviderRailAction::Feed,
            )?;
        }

        let mut allow_limitless = true;
        let mut allow_polymarket = true;
        if matches!(source, ExternalMarketSource::All) {
            let decision =
                evaluate_provider_access(&req, RailProvider::Limitless, ProviderRailAction::Feed);
            if !decision.allowed {
                allow_limitless = false;
                warnings.push(restriction_warning("limitless", ProviderRailAction::Feed));
            } else if decision.would_block {
                warnings.push(BaseFeedWarning {
                    source: "limitless".to_string(),
                    message: "limitless feed is in observe-only regional policy state".to_string(),
                });
            }

            let polymarket_decision =
                evaluate_provider_access(&req, RailProvider::Polymarket, ProviderRailAction::Feed);
            if !polymarket_decision.allowed {
                allow_polymarket = false;
                warnings.push(restriction_warning("polymarket", ProviderRailAction::Feed));
            } else if polymarket_decision.would_block {
                warnings.push(BaseFeedWarning {
                    source: "polymarket".to_string(),
                    message: "polymarket feed is in observe-only regional policy state".to_string(),
                });
            }
        }

        let external_markets = external::fetch_markets(
            &state.config,
            &state.redis,
            source,
            tradable,
            external_fetch_window,
            0,
            ExternalMarketsRequest {
                include_low_liquidity,
                allow_limitless,
                allow_polymarket,
            },
        )
        .await?;
        markets.extend(external_markets.into_iter().map(from_external_market));
    }

    if !matches!(source, ExternalMarketSource::Internal) {
        markets.sort_by(|a, b| {
            b.close_time
                .cmp(&a.close_time)
                .then_with(|| a.id.cmp(&b.id))
        });
    }

    let total = markets.len() as u64;
    if total == 0 || offset >= total {
        return Ok(HttpResponse::Ok()
            .insert_header((
                "Cache-Control",
                "public, max-age=15, stale-while-revalidate=30",
            ))
            .insert_header((
                "Vary",
                "Accept-Encoding, CF-IPCountry, X-Vercel-IP-Country, X-Country-Code",
            ))
            .json(BaseMarketsResponse {
                markets: vec![],
                total,
                limit,
                offset,
                source: source_label(source).to_string(),
                warnings: if warnings.is_empty() {
                    None
                } else {
                    Some(warnings)
                },
            }));
    }

    let page = markets
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect::<Vec<_>>();

    Ok(HttpResponse::Ok()
        .insert_header((
            "Cache-Control",
            "public, max-age=15, stale-while-revalidate=30",
        ))
        .insert_header((
            "Vary",
            "Accept-Encoding, CF-IPCountry, X-Vercel-IP-Country, X-Country-Code",
        ))
        .json(BaseMarketsResponse {
            markets: page,
            total,
            limit,
            offset,
            source: source_label(source).to_string(),
            warnings: if warnings.is_empty() {
                None
            } else {
                Some(warnings)
            },
        }))
}

pub async fn get_base_market(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }

    let market_id_raw = path.into_inner();
    if is_external_market_id(&market_id_raw) {
        let external_id = ExternalMarketId::parse(market_id_raw.as_str())?;
        ensure_provider_action_allowed(
            &req,
            to_rail_provider(external_id.provider),
            ProviderRailAction::MarketData,
        )?;
        let market = external::fetch_market_by_id(&state.config, &external_id).await?;
        return Ok(HttpResponse::Ok().json(from_external_market(market)));
    }

    let market_id = market_id_raw.parse::<u64>().map_err(|_| {
        ApiError::bad_request(
            "INVALID_MARKET_ID",
            "market_id must be numeric or namespaced",
        )
    })?;
    let market = fetch_internal_market_snapshot_by_id(&state, market_id).await?;
    Ok(HttpResponse::Ok().json(market))
}

pub async fn register_base_market_bootstrap(
    state: web::Data<Arc<AppState>>,
    path: web::Path<u64>,
    body: web::Json<RegisterBaseMarketBootstrapRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let market_id = path.into_inner();
    if market_id == 0 {
        return Err(ApiError::bad_request(
            "INVALID_MARKET_ID",
            "market_id must be a positive integer",
        ));
    }

    let validated = validate_bootstrap_registration(&body)?;
    let manager = if validated.liquidity_mode == BOOTSTRAP_LIQUIDITY_MODE_HYBRID {
        let configured_manager = configured_bootstrap_operator(&state)?;
        if let Some(requested) = body.manager.as_ref() {
            let requested = normalize_required_address(
                requested.as_str(),
                "INVALID_MANAGER",
                "manager must be a valid 0x EVM address",
            )?;
            if requested != configured_manager {
                return Err(ApiError::bad_request(
                    "INVALID_MANAGER",
                    "manager must match BOOTSTRAP_OPERATOR_ADDRESS",
                ));
            }
        }
        Some(configured_manager)
    } else {
        None
    };
    let market = fetch_internal_market_snapshot_by_id(&state, market_id).await?;
    if market.resolved {
        return Err(ApiError::conflict(
            "MARKET_ALREADY_RESOLVED",
            "resolved markets cannot be registered for bootstrap liquidity",
        ));
    }

    let creator =
        verify_market_bootstrap_registration_tx(&state, market_id, body.tx_hash.as_str()).await?;
    let record = state
        .db
        .upsert_base_market_bootstrap(&BaseMarketBootstrapUpsert {
            market_id,
            creator: creator.as_str(),
            liquidity_mode: validated.liquidity_mode.as_str(),
            status: validated.status.as_str(),
            manager: manager.as_deref(),
            seed_usdc: validated.seed_usdc,
            initial_yes_bps: validated.initial_yes_bps,
            strategy: validated.strategy.as_str(),
            levels: validated.levels,
            base_spread_bps: validated.base_spread_bps,
            step_bps: validated.step_bps,
            cadence_seconds: validated.cadence_seconds,
            expiry_seconds: validated.expiry_seconds,
            organic_depth_window_bps: validated.organic_depth_window_bps,
            target_depth_multiplier: validated.target_depth_multiplier,
            target_volume_multiplier: validated.target_volume_multiplier,
            max_age_seconds: validated.max_age_seconds,
            inventory_skew_bps: 0,
            exposure_cap_bps: validated.exposure_cap_bps,
            activated_at: Some(Utc::now()),
            graduated_at: None,
            graduation_reason: None,
            create_tx_hash: Some(body.tx_hash.as_str()),
            launch_tx_hash: None,
            last_reconciled_at: None,
            last_error: None,
        })
        .await?;

    Ok(HttpResponse::Ok().json(record))
}

pub async fn update_base_market_bootstrap_runtime(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<u64>,
    body: web::Json<UpdateBaseMarketBootstrapRuntimeRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_admin_control(&req, &state)?;

    let market_id = path.into_inner();
    let current = state
        .db
        .get_base_market_bootstrap(market_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Base market bootstrap config"))?;
    if current.liquidity_mode != BOOTSTRAP_LIQUIDITY_MODE_HYBRID {
        return Err(ApiError::bad_request(
            "BOOTSTRAP_NOT_ACTIVE",
            "runtime updates require bootstrap_hybrid liquidity mode",
        ));
    }

    let next_skew = body.inventory_skew_bps.map(|value| {
        value.clamp(
            -(current.exposure_cap_bps.min(i32::MAX as u64) as i32),
            current.exposure_cap_bps.min(i32::MAX as u64) as i32,
        )
    });
    let updated = state
        .db
        .update_base_market_bootstrap_runtime(
            market_id, next_skew, None, None, None, None, None, false,
        )
        .await?
        .ok_or_else(|| ApiError::not_found("Base market bootstrap config"))?;

    Ok(HttpResponse::Ok().json(updated))
}

pub async fn get_bootstrap_operator_status(
    state: web::Data<Arc<AppState>>,
    query: web::Query<BootstrapOperatorStatusQuery>,
) -> Result<impl Responder, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }

    let owner = normalize_required_address(
        query.owner.as_str(),
        "INVALID_OWNER",
        "owner must be a valid 0x EVM address",
    )?;
    let operator = configured_bootstrap_operator(&state)?;
    let approved = fetch_manager_approval(&state, owner.as_str(), operator.as_str()).await?;

    Ok(HttpResponse::Ok().json(BootstrapOperatorStatusResponse {
        operator,
        owner,
        approved,
    }))
}

pub async fn bootstrap_runner_tick(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<BootstrapRunnerTickRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    if !state.config.evm_enabled
        || !state.config.evm_reads_enabled
        || !state.config.evm_writes_enabled
    {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM reads and writes must be enabled for bootstrap automation",
        ));
    }

    let operator = configured_bootstrap_operator(&state)?;
    let agent_runtime = configured_address(
        &state.config.agent_runtime_address,
        "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
        "AGENT_RUNTIME_ADDRESS must be configured for Base agents",
    )?;
    let now = now_seconds()?;
    let limit = body.limit.unwrap_or(100).clamp(1, 500) as usize;
    let configs = state.db.list_base_market_bootstrap_configs().await?;
    let mut actions = Vec::new();
    let mut scanned = 0_u64;

    'configs: for original in configs {
        if original.liquidity_mode != BOOTSTRAP_LIQUIDITY_MODE_HYBRID
            || original.status == BOOTSTRAP_STATUS_DISABLED
            || original.status == BOOTSTRAP_STATUS_ERROR
        {
            continue;
        }
        scanned = scanned.saturating_add(1);

        let market = fetch_internal_market_snapshot_by_id(&state, original.market_id).await?;
        let (organic_yes_bids, organic_no_bids) =
            collect_internal_order_levels(&state, original.market_id, now).await?;
        let mut config = maybe_refresh_bootstrap_state(
            &state,
            &market,
            original,
            &organic_yes_bids,
            &organic_no_bids,
        )
        .await?;
        let records = state
            .db
            .list_base_market_bootstrap_agents(config.market_id)
            .await?;

        let approved =
            fetch_manager_approval(&state, config.creator.as_str(), operator.as_str()).await?;
        let available_balance =
            fetch_vault_available_balance(&state, config.creator.as_str()).await?;
        let position =
            fetch_position_snapshot(&state, config.market_id, config.creator.as_str()).await?;
        let next_skew = bootstrap_inventory_skew_bps(&config, &position);
        let has_live_slots = bootstrap_has_live_slots(&records);
        let next_status =
            bootstrap_status_for_runtime(&config, approved, available_balance, has_live_slots);

        if config.inventory_skew_bps != next_skew
            || config.status != next_status
            || config.manager.as_deref() != Some(operator.as_str())
        {
            if let Some(updated) = state
                .db
                .update_base_market_bootstrap_runtime(
                    config.market_id,
                    Some(next_skew),
                    Some(next_status),
                    Some(operator.as_str()),
                    None,
                    Some(Utc::now()),
                    None,
                    false,
                )
                .await?
            {
                config = updated;
            }
        }

        let active_ids = bootstrap_active_agent_ids(&records);
        if matches!(
            config.status.as_str(),
            BOOTSTRAP_STATUS_PAUSED | "graduated"
        ) && !active_ids.is_empty()
        {
            let slots = records
                .iter()
                .filter(|record| record.active && record.agent_id.is_some())
                .map(|record| runner_slot_from_record(record, &config))
                .collect::<Vec<_>>();
            actions.push(BootstrapRunnerAction {
                kind: BOOTSTRAP_RUNNER_ACTION_DEACTIVATE.to_string(),
                market_id: config.market_id,
                config_status: config.status.clone(),
                prepared_write: prepared_write_response(
                    state.config.base_chain_id,
                    Some(operator.clone()),
                    agent_runtime.clone(),
                    encode_deactivate_agents_calldata(active_ids.as_slice()),
                    "deactivateAgents",
                ),
                slots,
            });
            if actions.len() >= limit {
                break 'configs;
            }
            continue;
        }

        if config.status != BOOTSTRAP_STATUS_ACTIVE
            && config.status != BOOTSTRAP_STATUS_PENDING_LAUNCH
        {
            continue;
        }

        let desired = bootstrap_desired_agents(&config);
        let existing = bootstrap_slot_records_map(&records);
        let missing_agents =
            bootstrap_launch_inputs(desired.as_slice(), &existing, config.market_id);
        if !missing_agents.is_empty() {
            let slots = desired
                .iter()
                .filter(|agent| {
                    !existing
                        .get(bootstrap_slot_key(agent.side, agent.level_index).as_str())
                        .is_some_and(|record| record.active && record.agent_id.is_some())
                })
                .map(|agent| runner_slot_from_desired(agent, None))
                .collect::<Vec<_>>();
            actions.push(BootstrapRunnerAction {
                kind: BOOTSTRAP_RUNNER_ACTION_LAUNCH.to_string(),
                market_id: config.market_id,
                config_status: config.status.clone(),
                prepared_write: prepared_write_response(
                    state.config.base_chain_id,
                    Some(operator.clone()),
                    agent_runtime.clone(),
                    encode_create_agents_for_calldata(
                        config.creator.as_str(),
                        operator.as_str(),
                        missing_agents.as_slice(),
                        config.strategy.as_str(),
                    )?,
                    "createAgentsFor",
                ),
                slots,
            });
            if actions.len() >= limit {
                break 'configs;
            }
            continue;
        }

        let (updates, update_slots) = bootstrap_update_inputs(desired.as_slice(), &existing);
        if !updates.is_empty() {
            actions.push(BootstrapRunnerAction {
                kind: BOOTSTRAP_RUNNER_ACTION_UPDATE.to_string(),
                market_id: config.market_id,
                config_status: config.status.clone(),
                prepared_write: prepared_write_response(
                    state.config.base_chain_id,
                    Some(operator.clone()),
                    agent_runtime.clone(),
                    encode_update_agents_calldata(updates.as_slice(), config.strategy.as_str())?,
                    "updateAgents",
                ),
                slots: update_slots,
            });
            if actions.len() >= limit {
                break 'configs;
            }
            continue;
        }

        for record in records.iter().filter(|record| record.active) {
            let Some(agent_id) = record.agent_id else {
                continue;
            };
            let calldata = format!(
                "{}{}",
                AGENT_RUNTIME_AGENTS_SELECTOR,
                encode_u256_hex(agent_id)
            );
            let slot = state
                .evm_rpc
                .eth_call(agent_runtime.as_str(), calldata.as_str())
                .await
                .map_err(map_evm_rpc_error)?;
            let Some(snapshot) = decode_agent_snapshot(agent_id, slot.as_str(), now)? else {
                continue;
            };
            if !snapshot.can_execute {
                continue;
            }

            actions.push(BootstrapRunnerAction {
                kind: BOOTSTRAP_RUNNER_ACTION_EXECUTE.to_string(),
                market_id: config.market_id,
                config_status: config.status.clone(),
                prepared_write: prepared_write_response(
                    state.config.base_chain_id,
                    Some(operator.clone()),
                    agent_runtime.clone(),
                    format!(
                        "{}{}",
                        AGENT_RUNTIME_EXECUTE_SELECTOR,
                        encode_u256_hex(agent_id)
                    ),
                    "executeAgent",
                ),
                slots: vec![runner_slot_from_record(record, &config)],
            });
            if actions.len() >= limit {
                break 'configs;
            }
        }
    }

    Ok(HttpResponse::Ok().json(BootstrapRunnerTickResponse { scanned, actions }))
}

pub async fn bootstrap_runner_report(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<BootstrapRunnerReportRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let tx_hash = normalize_required_bytes32(
        body.tx_hash.as_str(),
        "INVALID_TX_HASH",
        "txHash must be a valid 0x-prefixed transaction hash",
    )?;
    let agent_runtime = configured_address(
        &state.config.agent_runtime_address,
        "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
        "AGENT_RUNTIME_ADDRESS must be configured for Base agents",
    )?;
    let mut config = state
        .db
        .get_base_market_bootstrap(body.market_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Base market bootstrap config"))?;
    let receipt = state
        .evm_rpc
        .eth_get_transaction_receipt(tx_hash.as_str())
        .await
        .map_err(map_evm_rpc_error)?
        .ok_or_else(|| ApiError::bad_request("INVALID_TX_HASH", "transaction receipt not found"))?;
    let receipt_success = receipt
        .status
        .as_deref()
        .and_then(|value| parse_u64_hex(value).ok())
        .unwrap_or(0)
        == 1;
    let success = body.success && receipt_success;
    let now = Utc::now();

    if !success {
        let next_status = if body.kind == BOOTSTRAP_RUNNER_ACTION_LAUNCH {
            BOOTSTRAP_STATUS_ERROR
        } else {
            config.status.as_str()
        };
        let updated = state
            .db
            .update_base_market_bootstrap_runtime(
                body.market_id,
                None,
                Some(next_status),
                None,
                None,
                Some(now),
                body.error
                    .as_deref()
                    .or(Some("bootstrap runner transaction failed")),
                false,
            )
            .await?
            .ok_or_else(|| ApiError::not_found("Base market bootstrap config"))?;
        return Ok(HttpResponse::Ok().json(updated));
    }

    match body.kind.as_str() {
        BOOTSTRAP_RUNNER_ACTION_LAUNCH => {
            let agent_ids =
                extract_created_agent_ids(&receipt, agent_runtime.as_str(), body.market_id)?;
            if agent_ids.len() != body.slots.len() {
                return Err(ApiError::internal(
                    "bootstrap launch receipt did not match planned slots",
                ));
            }
            for (index, slot) in body.slots.iter().enumerate() {
                state
                    .db
                    .upsert_base_market_bootstrap_agent(&BaseMarketBootstrapAgentUpsert {
                        market_id: body.market_id,
                        side: slot.side.as_str(),
                        level_index: slot.level_index,
                        agent_id: Some(agent_ids[index]),
                        desired_price_bps: slot.price_bps,
                        desired_size: parse_u64_decimal(slot.size.as_str(), "size")?,
                        current_price_bps: Some(slot.price_bps),
                        current_size: Some(parse_u64_decimal(slot.size.as_str(), "size")?),
                        active: true,
                        created_tx_hash: Some(tx_hash.as_str()),
                        updated_tx_hash: None,
                        deactivated_tx_hash: None,
                        last_execute_tx_hash: None,
                        last_executed_at: None,
                        last_reconciled_at: Some(now),
                        last_error: None,
                    })
                    .await?;
            }
            config = state
                .db
                .update_base_market_bootstrap_runtime(
                    body.market_id,
                    None,
                    Some(BOOTSTRAP_STATUS_ACTIVE),
                    None,
                    Some(tx_hash.as_str()),
                    Some(now),
                    None,
                    true,
                )
                .await?
                .ok_or_else(|| ApiError::not_found("Base market bootstrap config"))?;
        }
        BOOTSTRAP_RUNNER_ACTION_UPDATE => {
            for slot in &body.slots {
                state
                    .db
                    .upsert_base_market_bootstrap_agent(&BaseMarketBootstrapAgentUpsert {
                        market_id: body.market_id,
                        side: slot.side.as_str(),
                        level_index: slot.level_index,
                        agent_id: slot.agent_id,
                        desired_price_bps: slot.price_bps,
                        desired_size: parse_u64_decimal(slot.size.as_str(), "size")?,
                        current_price_bps: Some(slot.price_bps),
                        current_size: Some(parse_u64_decimal(slot.size.as_str(), "size")?),
                        active: true,
                        created_tx_hash: None,
                        updated_tx_hash: Some(tx_hash.as_str()),
                        deactivated_tx_hash: None,
                        last_execute_tx_hash: None,
                        last_executed_at: None,
                        last_reconciled_at: Some(now),
                        last_error: None,
                    })
                    .await?;
            }
            config = state
                .db
                .update_base_market_bootstrap_runtime(
                    body.market_id,
                    None,
                    Some(BOOTSTRAP_STATUS_ACTIVE),
                    None,
                    None,
                    Some(now),
                    None,
                    true,
                )
                .await?
                .ok_or_else(|| ApiError::not_found("Base market bootstrap config"))?;
        }
        BOOTSTRAP_RUNNER_ACTION_DEACTIVATE => {
            for slot in &body.slots {
                state
                    .db
                    .upsert_base_market_bootstrap_agent(&BaseMarketBootstrapAgentUpsert {
                        market_id: body.market_id,
                        side: slot.side.as_str(),
                        level_index: slot.level_index,
                        agent_id: slot.agent_id,
                        desired_price_bps: slot.price_bps,
                        desired_size: parse_u64_decimal(slot.size.as_str(), "size")?,
                        current_price_bps: Some(slot.price_bps),
                        current_size: Some(parse_u64_decimal(slot.size.as_str(), "size")?),
                        active: false,
                        created_tx_hash: None,
                        updated_tx_hash: None,
                        deactivated_tx_hash: Some(tx_hash.as_str()),
                        last_execute_tx_hash: None,
                        last_executed_at: None,
                        last_reconciled_at: Some(now),
                        last_error: None,
                    })
                    .await?;
            }
            config = state
                .db
                .update_base_market_bootstrap_runtime(
                    body.market_id,
                    None,
                    Some(config.status.as_str()),
                    None,
                    None,
                    Some(now),
                    None,
                    true,
                )
                .await?
                .ok_or_else(|| ApiError::not_found("Base market bootstrap config"))?;
        }
        BOOTSTRAP_RUNNER_ACTION_EXECUTE => {
            for slot in &body.slots {
                state
                    .db
                    .upsert_base_market_bootstrap_agent(&BaseMarketBootstrapAgentUpsert {
                        market_id: body.market_id,
                        side: slot.side.as_str(),
                        level_index: slot.level_index,
                        agent_id: slot.agent_id,
                        desired_price_bps: slot.price_bps,
                        desired_size: parse_u64_decimal(slot.size.as_str(), "size")?,
                        current_price_bps: Some(slot.price_bps),
                        current_size: Some(parse_u64_decimal(slot.size.as_str(), "size")?),
                        active: true,
                        created_tx_hash: None,
                        updated_tx_hash: None,
                        deactivated_tx_hash: None,
                        last_execute_tx_hash: Some(tx_hash.as_str()),
                        last_executed_at: Some(now),
                        last_reconciled_at: Some(now),
                        last_error: None,
                    })
                    .await?;
            }
            config = state
                .db
                .update_base_market_bootstrap_runtime(
                    body.market_id,
                    None,
                    Some(BOOTSTRAP_STATUS_ACTIVE),
                    None,
                    None,
                    Some(now),
                    None,
                    true,
                )
                .await?
                .ok_or_else(|| ApiError::not_found("Base market bootstrap config"))?;
        }
        _ => {
            return Err(ApiError::bad_request(
                "INVALID_RUNNER_ACTION",
                "kind must be one of launch, update, deactivate, execute",
            ));
        }
    }

    Ok(HttpResponse::Ok().json(config))
}

pub async fn get_base_agents(
    state: web::Data<Arc<AppState>>,
    query: web::Query<BaseAgentsQuery>,
) -> Result<impl Responder, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }

    let agent_runtime = state.config.agent_runtime_address.trim();
    if agent_runtime.is_empty() {
        return Err(ApiError::bad_request(
            "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
            "AGENT_RUNTIME_ADDRESS must be configured for Base agents",
        ));
    }

    if !is_valid_evm_address(agent_runtime) {
        return Err(ApiError::bad_request(
            "INVALID_AGENT_RUNTIME_ADDRESS",
            "AGENT_RUNTIME_ADDRESS must be a valid 0x EVM address",
        ));
    }

    let owner_filter = match query.owner.as_ref() {
        Some(owner) if !owner.trim().is_empty() => Some(normalize_required_address(
            owner.as_str(),
            "INVALID_OWNER_ADDRESS",
            "owner must be a valid 0x EVM address",
        )?),
        _ => None,
    };
    let market_filter = query.market_id;
    let active_filter = query.active;

    let total_hex = state
        .evm_rpc
        .eth_call(agent_runtime, AGENT_RUNTIME_COUNT_SELECTOR)
        .await
        .map_err(map_evm_rpc_error)?;
    let total = parse_u64_hex(&total_hex)?;

    let limit = query.limit.unwrap_or(50).min(MAX_AGENTS_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0);
    if total == 0 {
        return Ok(HttpResponse::Ok().json(BaseAgentsResponse {
            agents: vec![],
            total: 0,
            limit,
            offset,
            source: "agent_runtime".to_string(),
        }));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::internal("System time error"))?
        .as_secs();

    let mut filtered = Vec::new();
    for index in (1..=total).rev() {
        let calldata = format!(
            "{}{}",
            AGENT_RUNTIME_AGENTS_SELECTOR,
            encode_u256_hex(index)
        );
        let slot = state
            .evm_rpc
            .eth_call(agent_runtime, &calldata)
            .await
            .map_err(map_evm_rpc_error)?;

        let Some(snapshot) = decode_agent_snapshot(index, &slot, now)? else {
            continue;
        };

        if let Some(owner) = owner_filter.as_ref() {
            if &snapshot.owner != owner {
                continue;
            }
        }
        if let Some(market_id) = market_filter {
            if snapshot.market_id != market_id.to_string() {
                continue;
            }
        }
        if let Some(active) = active_filter {
            if snapshot.active != active {
                continue;
            }
        }

        let enriched = enrich_agent_with_erc8004(&state, snapshot).await;
        filtered.push(enriched);
    }

    let total_filtered = filtered.len() as u64;
    if total_filtered == 0 || offset >= total_filtered {
        return Ok(HttpResponse::Ok().json(BaseAgentsResponse {
            agents: vec![],
            total: total_filtered,
            limit,
            offset,
            source: "agent_runtime".to_string(),
        }));
    }

    let end = (offset + limit).min(total_filtered) as usize;
    let agents = filtered[offset as usize..end].to_vec();

    Ok(HttpResponse::Ok().json(BaseAgentsResponse {
        agents,
        total: total_filtered,
        limit,
        offset,
        source: "agent_runtime".to_string(),
    }))
}

pub async fn get_base_payout_candidates(
    state: web::Data<Arc<AppState>>,
    query: web::Query<BasePayoutCandidatesQuery>,
) -> Result<impl Responder, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }

    let limit = query.limit.unwrap_or(1000).clamp(1, 5000);
    let rows = state
        .db
        .list_base_payout_candidates(limit as i64)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let candidates: Vec<BasePayoutCandidate> = rows
        .into_iter()
        .map(|(owner, market_id)| BasePayoutCandidate { owner, market_id })
        .collect();

    Ok(HttpResponse::Ok().json(BasePayoutCandidatesResponse {
        total: candidates.len() as u64,
        candidates,
        limit,
        source: "database".to_string(),
    }))
}

pub async fn report_matcher_cycle(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<MatcherReportRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_admin_control(&req, &state)?;
    let now = Utc::now().to_rfc3339();
    let attempted = body.attempted;
    let matched = body.matched;
    let failed = body.failed;
    let ratio = if attempted == 0 {
        1.0
    } else {
        matched as f64 / attempted as f64
    };

    let stats = MatcherRuntimeStats {
        attempted,
        matched,
        failed,
        backlog: body.backlog,
        tx_latency_ms: body.tx_latency_ms,
        success_ratio: ratio,
        last_tx_hash: body.last_tx_hash.clone(),
        last_cycle_at: Some(now.clone()),
    };

    state
        .redis
        .set(MATCHER_STATS_REDIS_KEY, &stats, Some(3600))
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let runtime = matcher_runtime_state(&state).await?;
    if runtime.updated_at.is_none() {
        let ready_state = MatcherRuntimeState {
            paused: false,
            reason: None,
            updated_at: Some(now),
        };
        state
            .redis
            .set(MATCHER_STATE_REDIS_KEY, &ready_state, Some(86400))
            .await
            .map_err(|err| ApiError::internal(&err.to_string()))?;
    }

    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

pub async fn get_matcher_health(
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let runtime = matcher_runtime_state(&state).await?;
    let stats = matcher_runtime_stats(&state).await?;

    Ok(HttpResponse::Ok().json(MatcherHealthResponse {
        running: state.config.matcher_enabled,
        paused: runtime.paused,
        reason: runtime.reason,
        backlog: stats.backlog,
        updated_at: stats.last_cycle_at.or(runtime.updated_at),
    }))
}

pub async fn get_matcher_stats(
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let stats = matcher_runtime_stats(&state).await?;
    Ok(HttpResponse::Ok().json(stats))
}

pub async fn pause_matcher(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<MatcherPauseRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_admin_control(&req, &state)?;
    let runtime = MatcherRuntimeState {
        paused: true,
        reason: body
            .reason
            .clone()
            .or_else(|| Some("paused_by_admin".to_string())),
        updated_at: Some(Utc::now().to_rfc3339()),
    };
    state
        .redis
        .set(MATCHER_STATE_REDIS_KEY, &runtime, Some(86400))
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(runtime))
}

pub async fn resume_matcher(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_admin_control(&req, &state)?;
    let runtime = MatcherRuntimeState {
        paused: false,
        reason: None,
        updated_at: Some(Utc::now().to_rfc3339()),
    };
    state
        .redis
        .set(MATCHER_STATE_REDIS_KEY, &runtime, Some(86400))
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(runtime))
}

pub async fn report_payout_job(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<PayoutReportRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_admin_control(&req, &state)?;
    let normalized_status = body.status.trim().to_ascii_lowercase();
    if !matches!(
        normalized_status.as_str(),
        "pending" | "processing" | "retry" | "failed" | "paid"
    ) {
        return Err(ApiError::bad_request(
            "INVALID_PAYOUT_STATUS",
            "status must be one of pending|processing|retry|failed|paid",
        ));
    }

    let wallet = normalize_required_address(
        body.wallet.as_str(),
        "INVALID_WALLET",
        "wallet must be a valid 0x EVM address",
    )?;

    state
        .db
        .update_payout_job_result(
            body.market_id,
            wallet.as_str(),
            normalized_status.as_str(),
            body.last_tx.as_deref(),
            body.last_error.as_deref(),
            body.retry_after_seconds,
        )
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

pub async fn get_payout_health(
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let seeded = state
        .db
        .seed_payout_jobs_from_positions(5_000)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let summary = state
        .db
        .payout_backlog_summary()
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(BasePayoutHealthResponse {
        seed_inserted: seeded,
        pending: summary.pending,
        processing: summary.processing,
        retry: summary.retry,
        failed: summary.failed,
        oldest_pending_seconds: summary.oldest_pending_seconds,
    }))
}

pub async fn get_payout_backlog(
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let summary = state
        .db
        .payout_backlog_summary()
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    Ok(HttpResponse::Ok().json(summary))
}

pub async fn get_payout_jobs(
    state: web::Data<Arc<AppState>>,
    query: web::Query<BasePayoutJobsQuery>,
) -> Result<impl Responder, ApiError> {
    let limit = query.limit.unwrap_or(100).clamp(1, 1_000);
    let offset = query.offset.unwrap_or(0);
    let (jobs, total) = state
        .db
        .list_payout_jobs(query.status.as_deref(), limit as i64, offset as i64)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(BasePayoutJobsResponse {
        jobs,
        total: total.max(0) as u64,
        limit,
        offset,
    }))
}

pub async fn get_indexer_health(state: web::Data<Arc<AppState>>) -> Result<HttpResponse, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }

    let latest_block = state
        .evm_rpc
        .eth_block_number()
        .await
        .map_err(map_evm_rpc_error)?;
    let cursor = state
        .db
        .get_chain_sync_cursor(INDEXER_CURSOR_KEY)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let last_indexed = cursor.as_ref().map(|entry| entry.last_block).unwrap_or(0);
    let confirmations = state.config.indexer_confirmations;
    let source_block = latest_block.saturating_sub(confirmations);
    let lag_blocks = source_block.saturating_sub(last_indexed);

    Ok(HttpResponse::Ok().json(IndexerHealthResponse {
        enabled: true,
        lag_blocks,
        latest_block,
        last_indexed_block: last_indexed,
        confirmations,
        source_block,
    }))
}

pub async fn get_indexer_lag(state: web::Data<Arc<AppState>>) -> Result<HttpResponse, ApiError> {
    get_indexer_health(state).await
}

pub async fn trigger_indexer_backfill(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<IndexerBackfillRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_admin_control(&req, &state)?;
    let latest_block = state
        .evm_rpc
        .eth_block_number()
        .await
        .map_err(map_evm_rpc_error)?;
    let from_block = body
        .from_block
        .unwrap_or_else(|| latest_block.saturating_sub(state.config.indexer_lookback_blocks));
    let cursor_block = from_block.saturating_sub(1);

    let meta = json!({
        "requested_at": Utc::now().to_rfc3339(),
        "mode": "backfill",
    });
    state
        .db
        .upsert_chain_sync_cursor(INDEXER_CURSOR_KEY, cursor_block, meta)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    state.evm_indexer.set_last_synced_block(cursor_block).await;

    Ok(HttpResponse::Accepted().json(json!({
        "ok": true,
        "fromBlock": from_block,
    })))
}

pub async fn get_base_agent(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }

    let agent_runtime = state.config.agent_runtime_address.trim();
    if agent_runtime.is_empty() {
        return Err(ApiError::bad_request(
            "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
            "AGENT_RUNTIME_ADDRESS must be configured for Base agents",
        ));
    }

    if !is_valid_evm_address(agent_runtime) {
        return Err(ApiError::bad_request(
            "INVALID_AGENT_RUNTIME_ADDRESS",
            "AGENT_RUNTIME_ADDRESS must be a valid 0x EVM address",
        ));
    }

    let agent_id_raw = path.into_inner();
    let agent_id = agent_id_raw.parse::<u64>().map_err(|_| {
        ApiError::bad_request("INVALID_AGENT_ID", "agent_id must be a positive integer")
    })?;
    if agent_id == 0 {
        return Err(ApiError::bad_request(
            "INVALID_AGENT_ID",
            "agent_id must be greater than zero",
        ));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::internal("System time error"))?
        .as_secs();
    let calldata = format!(
        "{}{}",
        AGENT_RUNTIME_AGENTS_SELECTOR,
        encode_u256_hex(agent_id)
    );
    let slot = state
        .evm_rpc
        .eth_call(agent_runtime, &calldata)
        .await
        .map_err(map_evm_rpc_error)?;

    let snapshot =
        decode_agent_snapshot(agent_id, &slot, now)?.ok_or_else(|| ApiError::not_found("Agent"))?;
    let snapshot = enrich_agent_with_erc8004(&state, snapshot).await;

    Ok(HttpResponse::Ok().json(snapshot))
}

pub async fn get_base_identity(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let wallet = normalize_required_address(
        path.as_str(),
        "INVALID_WALLET",
        "wallet must be a valid 0x EVM address",
    )?;
    let identity = fetch_erc8004_identity(&state, wallet.as_str()).await?;

    Ok(HttpResponse::Ok().json(BaseIdentityResponse {
        wallet,
        identity_id: identity.as_ref().map(|entry| entry.identity_id.to_string()),
        tier: identity.as_ref().map(|entry| entry.tier),
        active: identity.as_ref().map(|entry| entry.active),
        updated_at: identity.as_ref().map(|entry| entry.updated_at),
        source: "erc8004_identity_registry".to_string(),
    }))
}

pub async fn get_base_reputation(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let wallet = normalize_required_address(
        path.as_str(),
        "INVALID_WALLET",
        "wallet must be a valid 0x EVM address",
    )?;
    let reputation = fetch_erc8004_reputation(&state, wallet.as_str()).await?;

    Ok(HttpResponse::Ok().json(BaseReputationResponse {
        wallet,
        score_bps: reputation.as_ref().map(|entry| entry.score_bps),
        confidence_bps: reputation.as_ref().map(|entry| entry.confidence_bps),
        events: reputation.as_ref().map(|entry| entry.events),
        notional_microusdc: reputation
            .as_ref()
            .map(|entry| entry.notional_microusdc.to_string()),
        source: "erc8004_reputation_registry".to_string(),
    }))
}

pub async fn get_base_validation(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let request_hash = normalize_required_bytes32(
        path.as_str(),
        "INVALID_REQUEST_HASH",
        "request_hash must be a valid 0x-prefixed bytes32 value",
    )?;
    let validation = fetch_erc8004_validation(&state, request_hash.as_str()).await?;
    let responded = validation.responded();

    Ok(HttpResponse::Ok().json(BaseValidationResponse {
        request_hash,
        validator: validation.validator,
        agent_id: validation.agent_id.to_string(),
        response: validation.response,
        response_hash: validation.response_hash,
        tag: validation.tag,
        last_update: validation.last_update,
        responded,
        source: "erc8004_validation_registry".to_string(),
    }))
}

pub async fn get_base_orderbook(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<BaseOrderBookQuery>,
) -> Result<impl Responder, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }
    x402::ensure_payment_for_request(&state, &req, X402Resource::OrderBook).await?;

    let market_id_raw = path.into_inner();
    let outcome = match query.outcome.as_deref().unwrap_or("yes") {
        "yes" => "yes",
        "no" => "no",
        _ => {
            return Err(ApiError::bad_request(
                "INVALID_OUTCOME",
                "outcome must be either 'yes' or 'no'",
            ));
        }
    };
    let depth = query.depth.unwrap_or(20).min(MAX_ORDERBOOK_DEPTH);

    if is_external_market_id(&market_id_raw) {
        let external_id = ExternalMarketId::parse(market_id_raw.as_str())?;
        ensure_provider_action_allowed(
            &req,
            to_rail_provider(external_id.provider),
            ProviderRailAction::MarketData,
        )?;
        let snapshot =
            external::fetch_orderbook(&state.config, &state.redis, &external_id, outcome, depth)
                .await?;

        return Ok(HttpResponse::Ok().json(BaseOrderBookResponse {
            market_id: snapshot.market_id,
            outcome: snapshot.outcome,
            bids: snapshot
                .bids
                .into_iter()
                .map(|entry| BaseOrderBookLevel {
                    price: entry.price,
                    quantity: entry.quantity,
                    orders: entry.orders,
                })
                .collect(),
            asks: snapshot
                .asks
                .into_iter()
                .map(|entry| BaseOrderBookLevel {
                    price: entry.price,
                    quantity: entry.quantity,
                    orders: entry.orders,
                })
                .collect(),
            last_updated: snapshot.last_updated,
            source: snapshot.source,
            provider: snapshot.provider,
            chain_id: snapshot.chain_id,
            provider_market_ref: snapshot.provider_market_ref,
            is_synthetic: snapshot.is_synthetic,
        }));
    }

    let market_id = market_id_raw.parse::<u64>().map_err(|_| {
        ApiError::bad_request(
            "INVALID_MARKET_ID",
            "market_id must be numeric or namespaced",
        )
    })?;
    let outcome_is_yes = outcome == "yes";

    let order_book = state.config.order_book_address.trim();
    if order_book.is_empty() {
        return Err(ApiError::bad_request(
            "ORDER_BOOK_ADDRESS_NOT_CONFIGURED",
            "ORDER_BOOK_ADDRESS must be configured for Base order books",
        ));
    }

    if !is_valid_evm_address(order_book) {
        return Err(ApiError::bad_request(
            "INVALID_ORDER_BOOK_ADDRESS",
            "ORDER_BOOK_ADDRESS must be a valid 0x EVM address",
        ));
    }

    let total_hex = state
        .evm_rpc
        .eth_call(order_book, ORDER_BOOK_COUNT_SELECTOR)
        .await
        .map_err(map_evm_rpc_error)?;
    let total = parse_u64_hex(&total_hex)?;
    let start = if total > ORDERBOOK_SCAN_WINDOW {
        total - ORDERBOOK_SCAN_WINDOW + 1
    } else {
        1
    };
    let now = now_seconds()?;

    let mut yes_bid_levels: BTreeMap<u64, LevelAggregate> = BTreeMap::new();
    let mut no_bid_levels: BTreeMap<u64, LevelAggregate> = BTreeMap::new();

    if total > 0 {
        for order_id in (start..=total).rev() {
            let calldata = format!(
                "{}{}",
                ORDER_BOOK_ORDERS_SELECTOR,
                encode_u256_hex(order_id)
            );
            let payload = state
                .evm_rpc
                .eth_call(order_book, &calldata)
                .await
                .map_err(map_evm_rpc_error)?;
            let Some(order) = decode_order_snapshot(&payload)? else {
                continue;
            };

            if order.market_id != market_id
                || order.canceled
                || order.remaining == 0
                || order.expiry < now
                || !(1..PAR_PRICE_BPS).contains(&order.price_bps)
            {
                continue;
            }

            if order.is_yes {
                insert_level(&mut yes_bid_levels, order.price_bps, order.remaining);
            } else {
                insert_level(&mut no_bid_levels, order.price_bps, order.remaining);
            }
        }
    }

    let market_snapshot = fetch_internal_market_snapshot_by_id(&state, market_id).await?;
    let mut bootstrap_config = state.db.get_base_market_bootstrap(market_id).await?;
    if let Some(config) = bootstrap_config.take() {
        let refreshed = maybe_refresh_bootstrap_state(
            &state,
            &market_snapshot,
            config,
            &yes_bid_levels,
            &no_bid_levels,
        )
        .await?;

        if bootstrap_active_for_market(&refreshed, &market_snapshot, now) {
            let synthetic = generate_bootstrap_synthetic_book(&refreshed);
            merge_level_maps(&mut yes_bid_levels, &synthetic.yes_bids);
            merge_level_maps(&mut no_bid_levels, &synthetic.no_bids);
            bootstrap_config = Some(refreshed);
        } else {
            bootstrap_config = Some(refreshed);
        }
    }

    let (bids, asks) = if outcome_is_yes {
        (
            book_side_from_levels(&yes_bid_levels, depth),
            complementary_asks_from_levels(&no_bid_levels, depth),
        )
    } else {
        (
            book_side_from_levels(&no_bid_levels, depth),
            complementary_asks_from_levels(&yes_bid_levels, depth),
        )
    };
    let is_synthetic = bootstrap_config
        .as_ref()
        .is_some_and(|config| bootstrap_active_for_market(config, &market_snapshot, now));

    Ok(HttpResponse::Ok().json(BaseOrderBookResponse {
        market_id: market_id_raw,
        outcome: outcome.to_string(),
        bids,
        asks,
        last_updated: Utc::now().to_rfc3339(),
        source: "order_book_contract".to_string(),
        provider: "internal".to_string(),
        chain_id: state.config.base_chain_id,
        provider_market_ref: market_id.to_string(),
        is_synthetic,
    }))
}

pub async fn get_base_trades(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<BaseTradesQuery>,
) -> Result<impl Responder, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }
    x402::ensure_payment_for_request(&state, &req, X402Resource::Trades).await?;

    let market_id_raw = path.into_inner();
    let limit = query.limit.unwrap_or(50).min(MAX_TRADES_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0);

    let outcome_raw = query.outcome.as_deref();
    let outcome_filter = match outcome_raw {
        None => None,
        Some("yes") => Some(true),
        Some("no") => Some(false),
        Some(_) => {
            return Err(ApiError::bad_request(
                "INVALID_OUTCOME",
                "outcome must be either 'yes' or 'no'",
            ))
        }
    };

    if is_external_market_id(&market_id_raw) {
        let external_id = ExternalMarketId::parse(market_id_raw.as_str())?;
        ensure_provider_action_allowed(
            &req,
            to_rail_provider(external_id.provider),
            ProviderRailAction::MarketData,
        )?;
        let snapshot = external::fetch_trades(
            &state.config,
            &state.redis,
            &external_id,
            outcome_raw,
            limit,
            offset,
        )
        .await?;

        let trades = snapshot
            .trades
            .into_iter()
            .map(|entry| BaseTradeSnapshot {
                id: entry.id,
                market_id: entry.market_id,
                outcome: entry.outcome,
                price: entry.price,
                price_bps: entry.price_bps,
                quantity: entry.quantity,
                tx_hash: entry.tx_hash,
                block_number: entry.block_number,
                created_at: entry.created_at,
            })
            .collect::<Vec<_>>();

        return Ok(HttpResponse::Ok().json(BaseTradesResponse {
            trades,
            total: snapshot.total,
            limit: snapshot.limit,
            offset: snapshot.offset,
            has_more: snapshot.has_more,
            source: snapshot.source,
            provider: snapshot.provider,
            chain_id: snapshot.chain_id,
            provider_market_ref: snapshot.provider_market_ref,
            is_synthetic: snapshot.is_synthetic,
        }));
    }

    let market_id = market_id_raw.parse::<u64>().map_err(|_| {
        ApiError::bad_request(
            "INVALID_MARKET_ID",
            "market_id must be numeric or namespaced",
        )
    })?;

    let order_book = state.config.order_book_address.trim();
    if order_book.is_empty() {
        return Err(ApiError::bad_request(
            "ORDER_BOOK_ADDRESS_NOT_CONFIGURED",
            "ORDER_BOOK_ADDRESS must be configured for Base trades",
        ));
    }

    if !is_valid_evm_address(order_book) {
        return Err(ApiError::bad_request(
            "INVALID_ORDER_BOOK_ADDRESS",
            "ORDER_BOOK_ADDRESS must be a valid 0x EVM address",
        ));
    }

    let latest_block = state
        .evm_rpc
        .eth_block_number()
        .await
        .map_err(map_evm_rpc_error)?;
    if latest_block == 0 {
        return Ok(HttpResponse::Ok().json(BaseTradesResponse {
            trades: vec![],
            total: 0,
            limit,
            offset,
            has_more: false,
            source: "order_book_contract".to_string(),
            provider: "internal".to_string(),
            chain_id: state.config.base_chain_id,
            provider_market_ref: market_id.to_string(),
            is_synthetic: false,
        }));
    }

    let from_block = latest_block.saturating_sub(TRADES_BLOCK_SCAN_WINDOW);
    let _ = state
        .evm_indexer
        .sync(
            state.config.market_core_address.trim(),
            order_book,
            TRADES_BLOCK_SCAN_WINDOW,
            &[ORDER_FILLED_TOPIC],
            Some(latest_block),
        )
        .await;

    let indexed_logs = state.evm_indexer.logs_by_topic(ORDER_FILLED_TOPIC).await;
    let mut logs = indexed_logs
        .into_iter()
        .filter(|entry| {
            entry
                .block_number
                .as_deref()
                .and_then(|v| parse_u64_hex(v).ok())
                .map(|block| block >= from_block && block <= latest_block)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    if logs.is_empty() {
        logs = state
            .evm_rpc
            .eth_get_logs(order_book, ORDER_FILLED_TOPIC, from_block, latest_block)
            .await
            .map_err(map_evm_rpc_error)?;
    }

    let mut trades = Vec::new();
    let mut block_timestamp_cache: HashMap<u64, u64> = HashMap::new();
    for log in logs {
        let order_id = match log.topics.get(1) {
            Some(topic) => match parse_u64_hex(topic) {
                Ok(value) => value,
                Err(_) => continue,
            },
            None => continue,
        };

        let block_number = match log.block_number.as_deref() {
            Some(value) => match parse_u64_hex(value) {
                Ok(parsed) => parsed,
                Err(_) => continue,
            },
            None => continue,
        };
        let log_index = match log.log_index.as_deref() {
            Some(value) => parse_u64_hex(value).unwrap_or(0),
            None => 0,
        };

        let fill_size = match word_at(&log.data, 0).and_then(parse_u64_hex) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if fill_size == 0 {
            continue;
        }

        let calldata = format!(
            "{}{}",
            ORDER_BOOK_ORDERS_SELECTOR,
            encode_u256_hex(order_id)
        );
        let payload = state
            .evm_rpc
            .eth_call(order_book, &calldata)
            .await
            .map_err(map_evm_rpc_error)?;
        let Some(order) = decode_order_snapshot(&payload)? else {
            continue;
        };

        if order.market_id != market_id {
            continue;
        }
        if let Some(expected) = outcome_filter {
            if order.is_yes != expected {
                continue;
            }
        }
        if order.price_bps == 0 || order.price_bps >= 10_000 {
            continue;
        }

        let timestamp = if let Some(ts) = block_timestamp_cache.get(&block_number) {
            *ts
        } else {
            let ts = state
                .evm_rpc
                .eth_get_block_timestamp(block_number)
                .await
                .map_err(map_evm_rpc_error)?;
            block_timestamp_cache.insert(block_number, ts);
            ts
        };

        let tx_hash = log.transaction_hash.unwrap_or_default();
        let id = if tx_hash.is_empty() {
            format!("base-{}-{}", order_id, log_index)
        } else {
            format!("base-{}-{}", tx_hash, log_index)
        };

        trades.push(PendingTrade {
            id,
            order_id,
            block_number,
            log_index,
            tx_hash,
            quantity: fill_size,
            outcome: if order.is_yes {
                "yes".to_string()
            } else {
                "no".to_string()
            },
            price_bps: order.price_bps,
            created_at: unix_to_rfc3339(timestamp),
        });
    }

    trades.sort_by(|a, b| {
        b.block_number
            .cmp(&a.block_number)
            .then_with(|| b.log_index.cmp(&a.log_index))
            .then_with(|| b.order_id.cmp(&a.order_id))
    });

    let total = trades.len() as u64;
    if offset >= total {
        return Ok(HttpResponse::Ok().json(BaseTradesResponse {
            trades: vec![],
            total,
            limit,
            offset,
            has_more: false,
            source: "order_book_contract".to_string(),
            provider: "internal".to_string(),
            chain_id: state.config.base_chain_id,
            provider_market_ref: market_id.to_string(),
            is_synthetic: false,
        }));
    }

    let end = (offset + limit).min(total);
    let mut page = Vec::new();
    for entry in trades
        .into_iter()
        .skip(offset as usize)
        .take((end - offset) as usize)
    {
        page.push(BaseTradeSnapshot {
            id: entry.id,
            market_id: market_id_raw.clone(),
            outcome: entry.outcome,
            price: (entry.price_bps as f64) / 10_000.0,
            price_bps: entry.price_bps,
            quantity: entry.quantity,
            tx_hash: entry.tx_hash,
            block_number: entry.block_number,
            created_at: entry.created_at,
        });
    }

    Ok(HttpResponse::Ok().json(BaseTradesResponse {
        trades: page,
        total,
        limit,
        offset,
        has_more: end < total,
        source: "order_book_contract".to_string(),
        provider: "internal".to_string(),
        chain_id: state.config.base_chain_id,
        provider_market_ref: market_id.to_string(),
        is_synthetic: false,
    }))
}

pub async fn prepare_create_market_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareCreateMarketWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let market_core = configured_address(
        &state.config.market_core_address,
        "MARKET_CORE_ADDRESS_NOT_CONFIGURED",
        "MARKET_CORE_ADDRESS must be configured for write operations",
    )?;

    let resolver = normalize_required_address(
        body.resolver.as_str(),
        "INVALID_RESOLVER",
        "resolver must be a valid 0x EVM address",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;

    let question = body.question.trim();
    if question.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_QUESTION",
            "question must not be empty",
        ));
    }
    if question.len() > MAX_MARKET_TEXT_LENGTH {
        return Err(ApiError::bad_request(
            "QUESTION_TOO_LONG",
            "question exceeds max length",
        ));
    }

    let description = body.description.as_deref().unwrap_or("").trim();
    let category = body.category.as_deref().unwrap_or("").trim();
    let resolution_source = body.resolution_source.as_deref().unwrap_or("").trim();
    if description.len() > MAX_MARKET_TEXT_LENGTH
        || category.len() > MAX_MARKET_TEXT_LENGTH
        || resolution_source.len() > MAX_MARKET_TEXT_LENGTH
    {
        return Err(ApiError::bad_request(
            "MARKET_TEXT_TOO_LONG",
            "description/category/resolutionSource exceeds max length",
        ));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::internal("System time error"))?
        .as_secs();
    if body.close_time <= now {
        return Err(ApiError::bad_request(
            "INVALID_CLOSE_TIME",
            "closeTime must be in the future",
        ));
    }

    let data = encode_create_market_rich_calldata(
        question,
        description,
        category,
        resolution_source,
        body.close_time,
        &resolver,
    )?;

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        market_core,
        data,
        "createMarketRich",
    )))
}

pub async fn prepare_place_order_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PreparePlaceOrderWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let order_book = configured_address(
        &state.config.order_book_address,
        "ORDER_BOOK_ADDRESS_NOT_CONFIGURED",
        "ORDER_BOOK_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;

    let is_yes = match body.outcome.as_str() {
        "yes" => true,
        "no" => false,
        _ => {
            return Err(ApiError::bad_request(
                "INVALID_OUTCOME",
                "outcome must be either 'yes' or 'no'",
            ))
        }
    };

    if body.price_bps == 0 || body.price_bps >= 10_000 {
        return Err(ApiError::bad_request(
            "INVALID_PRICE_BPS",
            "priceBps must be between 1 and 9999",
        ));
    }

    let size = parse_u128_decimal(&body.size, "size")?;
    if size == 0 {
        return Err(ApiError::bad_request(
            "INVALID_SIZE",
            "size must be greater than zero",
        ));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::internal("System time error"))?
        .as_secs();
    if body.expiry <= now {
        return Err(ApiError::bad_request(
            "INVALID_EXPIRY",
            "expiry must be in the future",
        ));
    }

    let data = format!(
        "{}{}{}{}{}{}",
        ORDER_BOOK_PLACE_SELECTOR,
        encode_u256_hex(body.market_id),
        encode_bool_word(is_yes),
        encode_u256_hex_u128(body.price_bps as u128),
        encode_u256_hex_u128(size),
        encode_u256_hex(body.expiry),
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        order_book,
        data,
        "placeOrder",
    )))
}

pub async fn prepare_cancel_order_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareCancelOrderWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let order_book = configured_address(
        &state.config.order_book_address,
        "ORDER_BOOK_ADDRESS_NOT_CONFIGURED",
        "ORDER_BOOK_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;

    let data = format!(
        "{}{}",
        ORDER_BOOK_CANCEL_SELECTOR,
        encode_u256_hex(body.order_id)
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        order_book,
        data,
        "cancelOrder",
    )))
}

pub async fn prepare_claim_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareClaimWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let order_book = configured_address(
        &state.config.order_book_address,
        "ORDER_BOOK_ADDRESS_NOT_CONFIGURED",
        "ORDER_BOOK_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;

    let data = format!(
        "{}{}",
        ORDER_BOOK_CLAIM_SELECTOR,
        encode_u256_hex(body.market_id)
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        order_book,
        data,
        "claim",
    )))
}

pub async fn prepare_resolve_market_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareResolveMarketWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let market_core = configured_address(
        &state.config.market_core_address,
        "MARKET_CORE_ADDRESS_NOT_CONFIGURED",
        "MARKET_CORE_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;

    let data = format!(
        "{}{}{}",
        MARKET_CORE_RESOLVE_SELECTOR,
        encode_u256_hex(body.market_id),
        encode_bool_word(body.outcome)
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        market_core,
        data,
        "resolveMarket",
    )))
}

pub async fn prepare_claim_for_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareClaimForWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let order_book = configured_address(
        &state.config.order_book_address,
        "ORDER_BOOK_ADDRESS_NOT_CONFIGURED",
        "ORDER_BOOK_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let user = normalize_required_address(
        body.user.as_str(),
        "INVALID_USER_ADDRESS",
        "user must be a valid 0x EVM address",
    )?;

    let user_word = encode_address_word(user.as_str())?;
    let data = format!(
        "{}{}{}",
        ORDER_BOOK_CLAIM_FOR_SELECTOR,
        user_word,
        encode_u256_hex(body.market_id)
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        order_book,
        data,
        "claimFor",
    )))
}

pub async fn prepare_match_orders_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareMatchOrdersWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let order_book = configured_address(
        &state.config.order_book_address,
        "ORDER_BOOK_ADDRESS_NOT_CONFIGURED",
        "ORDER_BOOK_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;

    if body.first_order_id == body.second_order_id {
        return Err(ApiError::bad_request(
            "INVALID_MATCH_PAIR",
            "firstOrderId and secondOrderId must differ",
        ));
    }
    let fill_size = parse_u128_decimal(&body.fill_size, "fillSize")?;
    if fill_size == 0 {
        return Err(ApiError::bad_request(
            "INVALID_FILL_SIZE",
            "fillSize must be greater than zero",
        ));
    }

    let data = format!(
        "{}{}{}{}",
        ORDER_BOOK_MATCH_SELECTOR,
        encode_u256_hex(body.first_order_id),
        encode_u256_hex(body.second_order_id),
        encode_u256_hex_u128(fill_size),
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        order_book,
        data,
        "matchOrders",
    )))
}

pub async fn prepare_create_agent_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareCreateAgentWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let agent_runtime = configured_address(
        &state.config.agent_runtime_address,
        "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
        "AGENT_RUNTIME_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;

    if body.price_bps == 0 || body.price_bps >= 10_000 {
        return Err(ApiError::bad_request(
            "INVALID_PRICE_BPS",
            "priceBps must be between 1 and 9999",
        ));
    }
    let size = parse_u128_decimal(&body.size, "size")?;
    if size == 0 {
        return Err(ApiError::bad_request(
            "INVALID_SIZE",
            "size must be greater than zero",
        ));
    }
    if body.cadence == 0 || body.expiry_window == 0 {
        return Err(ApiError::bad_request(
            "INVALID_AGENT_TIMING",
            "cadence and expiryWindow must be greater than zero",
        ));
    }
    if body.strategy.len() > MAX_MARKET_TEXT_LENGTH {
        return Err(ApiError::bad_request(
            "STRATEGY_TOO_LONG",
            "strategy exceeds max length",
        ));
    }

    let data = encode_create_agent_calldata(
        body.market_id,
        body.is_yes,
        body.price_bps,
        size,
        body.cadence,
        body.expiry_window,
        body.strategy.as_str(),
    )?;

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        agent_runtime,
        data,
        "createAgent",
    )))
}

pub async fn prepare_execute_agent_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareExecuteAgentWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let agent_runtime = configured_address(
        &state.config.agent_runtime_address,
        "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
        "AGENT_RUNTIME_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let data = format!(
        "{}{}",
        AGENT_RUNTIME_EXECUTE_SELECTOR,
        encode_u256_hex(body.agent_id)
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        agent_runtime,
        data,
        "executeAgent",
    )))
}

pub async fn prepare_set_manager_approval_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareSetManagerApprovalWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let agent_runtime = configured_address(
        &state.config.agent_runtime_address,
        "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
        "AGENT_RUNTIME_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let manager = normalize_required_address(
        body.manager.as_str(),
        "INVALID_MANAGER",
        "manager must be a valid 0x EVM address",
    )?;
    let data = encode_set_manager_approval_calldata(manager.as_str(), body.approved)?;

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        agent_runtime,
        data,
        "setManagerApproval",
    )))
}

pub async fn prepare_bootstrap_create_agents_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareBootstrapCreateAgentsWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let agent_runtime = configured_address(
        &state.config.agent_runtime_address,
        "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
        "AGENT_RUNTIME_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let owner = normalize_required_address(
        body.owner.as_str(),
        "INVALID_OWNER",
        "owner must be a valid 0x EVM address",
    )?;
    let manager = normalize_required_address(
        body.manager.as_str(),
        "INVALID_MANAGER",
        "manager must be a valid 0x EVM address",
    )?;
    let strategy = body.strategy.trim();
    if strategy.is_empty() || strategy.len() > MAX_MARKET_TEXT_LENGTH {
        return Err(ApiError::bad_request(
            "INVALID_STRATEGY",
            "strategy must be present and within length limits",
        ));
    }
    if body.agents.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_AGENTS",
            "agents must contain at least one config",
        ));
    }

    for config in &body.agents {
        let _ = parse_u128_decimal(config.size.as_str(), "size")?;
        if config.price_bps == 0 || config.price_bps >= 10_000 {
            return Err(ApiError::bad_request(
                "INVALID_PRICE_BPS",
                "priceBps must be between 1 and 9999",
            ));
        }
        if config.cadence == 0 || config.expiry_window == 0 {
            return Err(ApiError::bad_request(
                "INVALID_AGENT_TIMING",
                "cadence and expiryWindow must be greater than zero",
            ));
        }
    }

    let data = encode_create_agents_for_calldata(
        owner.as_str(),
        manager.as_str(),
        &body.agents,
        strategy,
    )?;

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        agent_runtime,
        data,
        "createAgentsFor",
    )))
}

pub async fn prepare_update_agents_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareUpdateAgentsWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let agent_runtime = configured_address(
        &state.config.agent_runtime_address,
        "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
        "AGENT_RUNTIME_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let strategy = body.strategy.trim();
    if strategy.is_empty() || strategy.len() > MAX_MARKET_TEXT_LENGTH {
        return Err(ApiError::bad_request(
            "INVALID_STRATEGY",
            "strategy must be present and within length limits",
        ));
    }
    if body.updates.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_UPDATES",
            "updates must contain at least one agent update",
        ));
    }
    for update in &body.updates {
        let _ = parse_u128_decimal(update.size.as_str(), "size")?;
        if update.price_bps == 0 || update.price_bps >= 10_000 {
            return Err(ApiError::bad_request(
                "INVALID_PRICE_BPS",
                "priceBps must be between 1 and 9999",
            ));
        }
        if update.cadence == 0 || update.expiry_window == 0 {
            return Err(ApiError::bad_request(
                "INVALID_AGENT_TIMING",
                "cadence and expiryWindow must be greater than zero",
            ));
        }
    }

    let data = encode_update_agents_calldata(&body.updates, strategy)?;

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        agent_runtime,
        data,
        "updateAgents",
    )))
}

pub async fn prepare_deactivate_agents_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareDeactivateAgentsWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let agent_runtime = configured_address(
        &state.config.agent_runtime_address,
        "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
        "AGENT_RUNTIME_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    if body.agent_ids.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_AGENT_IDS",
            "agentIds must contain at least one agent id",
        ));
    }
    let data = encode_deactivate_agents_calldata(&body.agent_ids);

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        agent_runtime,
        data,
        "deactivateAgents",
    )))
}

pub async fn prepare_set_agent_manager_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareSetAgentManagerWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let agent_runtime = configured_address(
        &state.config.agent_runtime_address,
        "AGENT_RUNTIME_ADDRESS_NOT_CONFIGURED",
        "AGENT_RUNTIME_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let manager = normalize_required_address(
        body.manager.as_str(),
        "INVALID_MANAGER",
        "manager must be a valid 0x EVM address",
    )?;
    let data = encode_set_agent_manager_calldata(body.agent_id, manager.as_str())?;

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        agent_runtime,
        data,
        "setAgentManager",
    )))
}

pub async fn prepare_erc8004_register_identity_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareErc8004RegisterIdentityWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    if body.tier > ERC8004_MAX_TIER {
        return Err(ApiError::bad_request(
            "INVALID_TIER",
            "tier must be between 0 and 100",
        ));
    }

    let registry = configured_address(
        &state.config.erc8004_identity_registry_address,
        "ERC8004_IDENTITY_REGISTRY_NOT_CONFIGURED",
        "ERC8004_IDENTITY_REGISTRY_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let wallet = normalize_required_address(
        body.wallet.as_str(),
        "INVALID_WALLET",
        "wallet must be a valid 0x EVM address",
    )?;
    let data = format!(
        "{}{}{}",
        ERC8004_IDENTITY_REGISTER_SELECTOR,
        encode_address_word(wallet.as_str())?,
        encode_u256_hex_u128(body.tier as u128),
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        registry,
        data,
        "registerIdentity",
    )))
}

pub async fn prepare_erc8004_set_tier_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareErc8004SetTierWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    if body.tier > ERC8004_MAX_TIER {
        return Err(ApiError::bad_request(
            "INVALID_TIER",
            "tier must be between 0 and 100",
        ));
    }

    let registry = configured_address(
        &state.config.erc8004_identity_registry_address,
        "ERC8004_IDENTITY_REGISTRY_NOT_CONFIGURED",
        "ERC8004_IDENTITY_REGISTRY_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let wallet = normalize_required_address(
        body.wallet.as_str(),
        "INVALID_WALLET",
        "wallet must be a valid 0x EVM address",
    )?;
    let data = format!(
        "{}{}{}",
        ERC8004_IDENTITY_SET_TIER_SELECTOR,
        encode_address_word(wallet.as_str())?,
        encode_u256_hex_u128(body.tier as u128),
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        registry,
        data,
        "setIdentityTier",
    )))
}

pub async fn prepare_erc8004_set_active_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareErc8004SetActiveWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let registry = configured_address(
        &state.config.erc8004_identity_registry_address,
        "ERC8004_IDENTITY_REGISTRY_NOT_CONFIGURED",
        "ERC8004_IDENTITY_REGISTRY_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let wallet = normalize_required_address(
        body.wallet.as_str(),
        "INVALID_WALLET",
        "wallet must be a valid 0x EVM address",
    )?;
    let data = format!(
        "{}{}{}",
        ERC8004_IDENTITY_SET_ACTIVE_SELECTOR,
        encode_address_word(wallet.as_str())?,
        encode_bool_word(body.active),
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        registry,
        data,
        "setIdentityActive",
    )))
}

pub async fn prepare_erc8004_submit_outcome_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareErc8004SubmitOutcomeWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    if body.confidence_weight_bps > 10_000 {
        return Err(ApiError::bad_request(
            "INVALID_CONFIDENCE_WEIGHT",
            "confidenceWeightBps must be between 0 and 10000",
        ));
    }

    let registry = configured_address(
        &state.config.erc8004_reputation_registry_address,
        "ERC8004_REPUTATION_REGISTRY_NOT_CONFIGURED",
        "ERC8004_REPUTATION_REGISTRY_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let wallet = normalize_required_address(
        body.wallet.as_str(),
        "INVALID_WALLET",
        "wallet must be a valid 0x EVM address",
    )?;
    let notional = parse_u128_decimal(body.notional_microusdc.as_str(), "notionalMicrousdc")?;
    let data = format!(
        "{}{}{}{}{}",
        ERC8004_REPUTATION_SUBMIT_OUTCOME_SELECTOR,
        encode_address_word(wallet.as_str())?,
        encode_bool_word(body.success),
        encode_u256_hex_u128(notional),
        encode_u256_hex_u128(body.confidence_weight_bps as u128),
    );

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        registry,
        data,
        "submitOutcome",
    )))
}

pub async fn prepare_erc8004_validation_request_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareErc8004ValidationRequestWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    let registry = configured_address(
        &state.config.erc8004_validation_registry_address,
        "ERC8004_VALIDATION_REGISTRY_NOT_CONFIGURED",
        "ERC8004_VALIDATION_REGISTRY_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let validator = normalize_required_address(
        body.validator.as_str(),
        "INVALID_VALIDATOR",
        "validator must be a valid 0x EVM address",
    )?;
    let agent_id = parse_u128_decimal(body.agent_id.as_str(), "agentId")?;
    let request_uri = body.request_uri.trim();
    let request_hash = match body.request_hash.as_ref() {
        Some(raw) if !raw.trim().is_empty() => normalize_required_bytes32(
            raw.as_str(),
            "INVALID_REQUEST_HASH",
            "requestHash must be a valid 0x-prefixed bytes32 value",
        )?,
        _ => {
            let mut hasher = Keccak256::new();
            hasher.update(request_uri.as_bytes());
            format!("0x{}", hex::encode(hasher.finalize()))
        }
    };

    let data = encode_validation_request_calldata(
        validator.as_str(),
        agent_id,
        request_uri,
        request_hash.as_str(),
    )?;

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        registry,
        data,
        "validationRequest",
    )))
}

pub async fn prepare_erc8004_validation_response_write(
    state: web::Data<Arc<AppState>>,
    body: web::Json<PrepareErc8004ValidationResponseWriteRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    if body.response > 100 {
        return Err(ApiError::bad_request(
            "INVALID_VALIDATION_RESPONSE",
            "response must be between 0 and 100",
        ));
    }

    let registry = configured_address(
        &state.config.erc8004_validation_registry_address,
        "ERC8004_VALIDATION_REGISTRY_NOT_CONFIGURED",
        "ERC8004_VALIDATION_REGISTRY_ADDRESS must be configured for write operations",
    )?;
    let from = normalize_optional_address(body.from.as_ref())?;
    let request_hash = normalize_required_bytes32(
        body.request_hash.as_str(),
        "INVALID_REQUEST_HASH",
        "requestHash must be a valid 0x-prefixed bytes32 value",
    )?;
    let response_hash = normalize_required_bytes32(
        body.response_hash.as_str(),
        "INVALID_RESPONSE_HASH",
        "responseHash must be a valid 0x-prefixed bytes32 value",
    )?;
    let tag = normalize_required_bytes32(
        body.tag.as_str(),
        "INVALID_TAG",
        "tag must be a valid 0x-prefixed bytes32 value",
    )?;

    let data = encode_validation_response_calldata(
        request_hash.as_str(),
        body.response,
        body.response_uri.as_str(),
        response_hash.as_str(),
        tag.as_str(),
    )?;

    Ok(HttpResponse::Ok().json(prepared_write_response(
        state.config.base_chain_id,
        from,
        registry,
        data,
        "validationResponse",
    )))
}

pub async fn relay_raw_transaction(
    state: web::Data<Arc<AppState>>,
    body: web::Json<RelayRawTransactionRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_evm_writes_enabled(&state)?;

    if !is_valid_hex_payload(body.raw_tx.as_str()) {
        return Err(ApiError::bad_request(
            "INVALID_RAW_TX",
            "rawTx must be a valid 0x-prefixed hex string",
        ));
    }

    let tx_hash = state
        .evm_rpc
        .eth_send_raw_transaction(body.raw_tx.as_str())
        .await
        .map_err(map_evm_rpc_error)?;

    Ok(HttpResponse::Ok().json(RelayRawTransactionResponse {
        chain_id: state.config.base_chain_id,
        tx_hash,
    }))
}

async fn matcher_runtime_state(state: &AppState) -> Result<MatcherRuntimeState, ApiError> {
    match state
        .redis
        .get::<MatcherRuntimeState>(MATCHER_STATE_REDIS_KEY)
        .await
    {
        Ok(Some(runtime)) => Ok(runtime),
        Ok(None) => Ok(MatcherRuntimeState::default()),
        Err(err) => Err(ApiError::internal(&err.to_string())),
    }
}

async fn matcher_runtime_stats(state: &AppState) -> Result<MatcherRuntimeStats, ApiError> {
    match state
        .redis
        .get::<MatcherRuntimeStats>(MATCHER_STATS_REDIS_KEY)
        .await
    {
        Ok(Some(stats)) => Ok(stats),
        Ok(None) => Ok(MatcherRuntimeStats::default()),
        Err(err) => Err(ApiError::internal(&err.to_string())),
    }
}

fn ensure_admin_control(req: &HttpRequest, state: &AppState) -> Result<(), ApiError> {
    let expected = state.config.admin_control_key.trim();
    if expected.is_empty() {
        return Err(ApiError::forbidden(
            "admin control key is not configured for this environment",
        ));
    }

    let provided = req
        .headers()
        .get("x-admin-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or("");
    if provided != expected {
        return Err(ApiError::unauthorized("invalid admin key"));
    }

    Ok(())
}

fn is_valid_evm_address(address: &str) -> bool {
    address.len() == 42
        && address.starts_with("0x")
        && address[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn is_valid_bytes32(value: &str) -> bool {
    value.len() == 66
        && value.starts_with("0x")
        && value[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn is_valid_hex_payload(value: &str) -> bool {
    value.len() >= 4
        && value.starts_with("0x")
        && value.len() % 2 == 0
        && value[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn ensure_evm_writes_enabled(state: &Arc<AppState>) -> Result<(), ApiError> {
    if !state.config.evm_enabled || !state.config.evm_writes_enabled {
        return Err(ApiError::bad_request(
            "EVM_WRITES_DISABLED",
            "EVM write operations are disabled",
        ));
    }
    Ok(())
}

fn configured_address(address: &str, code: &str, message: &str) -> Result<String, ApiError> {
    let trimmed = address.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(code, message));
    }
    normalize_required_address(trimmed, code, message)
}

fn normalize_required_address(
    address: &str,
    code: &str,
    message: &str,
) -> Result<String, ApiError> {
    let trimmed = address.trim();
    if !is_valid_evm_address(trimmed) {
        return Err(ApiError::bad_request(code, message));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn normalize_required_bytes32(value: &str, code: &str, message: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if !is_valid_bytes32(trimmed) {
        return Err(ApiError::bad_request(code, message));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn normalize_optional_address(address: Option<&String>) -> Result<Option<String>, ApiError> {
    match address {
        None => Ok(None),
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if !is_valid_evm_address(trimmed) {
                return Err(ApiError::bad_request(
                    "INVALID_FROM_ADDRESS",
                    "from must be a valid 0x EVM address",
                ));
            }
            Ok(Some(trimmed.to_ascii_lowercase()))
        }
    }
}

fn parse_u8_hex(value: &str) -> Result<u8, ApiError> {
    let parsed = parse_u64_hex(value)?;
    if parsed > u8::MAX as u64 {
        return Err(ApiError::internal("RPC value out of range for u8"));
    }

    Ok(parsed as u8)
}

fn parse_u64_hex(value: &str) -> Result<u64, ApiError> {
    let trimmed = value.trim_start_matches("0x");
    if trimmed.is_empty() {
        return Err(ApiError::internal("Invalid RPC hex result"));
    }

    let normalized = trimmed.trim_start_matches('0');
    if normalized.is_empty() {
        return Ok(0);
    }
    if normalized.len() > 16 {
        return Err(ApiError::internal("RPC value out of range for u64"));
    }

    u64::from_str_radix(normalized, 16).map_err(|_| ApiError::internal("Invalid RPC hex result"))
}

fn parse_u128_hex(value: &str) -> Result<u128, ApiError> {
    let trimmed = value.trim_start_matches("0x");
    if trimmed.is_empty() {
        return Err(ApiError::internal("Invalid RPC hex result"));
    }

    let normalized = trimmed.trim_start_matches('0');
    if normalized.is_empty() {
        return Ok(0);
    }
    if normalized.len() > 32 {
        return Err(ApiError::internal("RPC value out of range for u128"));
    }

    u128::from_str_radix(normalized, 16).map_err(|_| ApiError::internal("Invalid RPC hex result"))
}

fn parse_bool_word(word: &str) -> Result<bool, ApiError> {
    Ok(parse_u64_hex(word)? != 0)
}

fn encode_u256_hex(value: u64) -> String {
    format!("{:064x}", value)
}

fn encode_u256_hex_u128(value: u128) -> String {
    format!("{:064x}", value)
}

fn encode_bool_word(value: bool) -> String {
    if value {
        format!("{:064x}", 1)
    } else {
        format!("{:064x}", 0)
    }
}

fn encode_address_word(value: &str) -> Result<String, ApiError> {
    if !is_valid_evm_address(value) {
        return Err(ApiError::bad_request(
            "INVALID_ADDRESS",
            "address must be a valid 0x EVM address",
        ));
    }
    Ok(format!("{:0>64}", value[2..].to_ascii_lowercase()))
}

fn encode_bytes32_word(value: &str) -> Result<String, ApiError> {
    if !is_valid_bytes32(value) {
        return Err(ApiError::bad_request(
            "INVALID_BYTES32",
            "value must be a valid 0x-prefixed bytes32 string",
        ));
    }
    Ok(value.trim_start_matches("0x").to_ascii_lowercase())
}

fn encode_dynamic_string_tail(value: &str) -> String {
    let encoded = hex::encode(value.as_bytes());
    let padded_len = if encoded.is_empty() {
        0
    } else {
        ((encoded.len() + 63) / 64) * 64
    };
    let mut padded = encoded;
    if padded.len() < padded_len {
        padded.push_str(&"0".repeat(padded_len - padded.len()));
    }
    format!("{}{}", encode_u256_hex_u128(value.len() as u128), padded)
}

fn encode_create_market_rich_calldata(
    question: &str,
    description: &str,
    category: &str,
    resolution_source: &str,
    close_time: u64,
    resolver: &str,
) -> Result<String, ApiError> {
    let question_tail = encode_dynamic_string_tail(question);
    let description_tail = encode_dynamic_string_tail(description);
    let category_tail = encode_dynamic_string_tail(category);
    let source_tail = encode_dynamic_string_tail(resolution_source);
    let resolver_word = encode_address_word(resolver)?;

    let head_len_bytes = 32usize * 6usize;
    let question_offset = head_len_bytes;
    let description_offset = question_offset + (question_tail.len() / 2);
    let category_offset = description_offset + (description_tail.len() / 2);
    let source_offset = category_offset + (category_tail.len() / 2);

    Ok(format!(
        "{}{}{}{}{}{}{}{}{}{}",
        MARKET_CORE_CREATE_RICH_SELECTOR,
        encode_u256_hex_u128(question_offset as u128),
        encode_u256_hex_u128(description_offset as u128),
        encode_u256_hex_u128(category_offset as u128),
        encode_u256_hex_u128(source_offset as u128),
        encode_u256_hex(close_time),
        resolver_word,
        question_tail,
        description_tail,
        format!("{}{}", category_tail, source_tail),
    ))
}

fn encode_create_agent_calldata(
    market_id: u64,
    is_yes: bool,
    price_bps: u64,
    size: u128,
    cadence: u64,
    expiry_window: u64,
    strategy: &str,
) -> Result<String, ApiError> {
    let strategy_tail = encode_dynamic_string_tail(strategy);
    let head_len_bytes = 32usize * 7usize;

    Ok(format!(
        "{}{}{}{}{}{}{}{}{}",
        AGENT_RUNTIME_CREATE_SELECTOR,
        encode_u256_hex(market_id),
        encode_bool_word(is_yes),
        encode_u256_hex_u128(price_bps as u128),
        encode_u256_hex_u128(size),
        encode_u256_hex(cadence),
        encode_u256_hex(expiry_window),
        encode_u256_hex_u128(head_len_bytes as u128),
        strategy_tail,
    ))
}

fn encode_agent_config_array_tail(
    configs: &[BootstrapAgentConfigInput],
) -> Result<String, ApiError> {
    let mut encoded = encode_u256_hex_u128(configs.len() as u128);
    for config in configs {
        let size = parse_u128_decimal(config.size.as_str(), "size")?;
        encoded.push_str(
            format!(
                "{}{}{}{}{}{}",
                encode_u256_hex(config.market_id),
                encode_bool_word(config.is_yes),
                encode_u256_hex_u128(config.price_bps as u128),
                encode_u256_hex_u128(size),
                encode_u256_hex(config.cadence),
                encode_u256_hex(config.expiry_window),
            )
            .as_str(),
        );
    }
    Ok(encoded)
}

fn encode_agent_update_array_tail(
    updates: &[BootstrapAgentUpdateInput],
) -> Result<String, ApiError> {
    let mut encoded = encode_u256_hex_u128(updates.len() as u128);
    for update in updates {
        let size = parse_u128_decimal(update.size.as_str(), "size")?;
        encoded.push_str(
            format!(
                "{}{}{}{}{}{}",
                encode_u256_hex(update.agent_id),
                encode_bool_word(update.is_yes),
                encode_u256_hex_u128(update.price_bps as u128),
                encode_u256_hex_u128(size),
                encode_u256_hex(update.cadence),
                encode_u256_hex(update.expiry_window),
            )
            .as_str(),
        );
    }
    Ok(encoded)
}

fn encode_u64_array_tail(values: &[u64]) -> String {
    let mut encoded = encode_u256_hex_u128(values.len() as u128);
    for value in values {
        encoded.push_str(encode_u256_hex(*value).as_str());
    }
    encoded
}

fn encode_set_manager_approval_calldata(manager: &str, approved: bool) -> Result<String, ApiError> {
    Ok(format!(
        "{}{}{}",
        AGENT_RUNTIME_SET_MANAGER_APPROVAL_SELECTOR,
        encode_address_word(manager)?,
        encode_bool_word(approved),
    ))
}

fn encode_create_agents_for_calldata(
    owner: &str,
    manager: &str,
    configs: &[BootstrapAgentConfigInput],
    strategy: &str,
) -> Result<String, ApiError> {
    let configs_tail = encode_agent_config_array_tail(configs)?;
    let strategy_tail = encode_dynamic_string_tail(strategy);
    let head_len_bytes = 32usize * 4usize;
    let strategy_offset = head_len_bytes + (configs_tail.len() / 2);

    Ok(format!(
        "{}{}{}{}{}{}{}",
        AGENT_RUNTIME_CREATE_FOR_SELECTOR,
        encode_address_word(owner)?,
        encode_address_word(manager)?,
        encode_u256_hex_u128(head_len_bytes as u128),
        encode_u256_hex_u128(strategy_offset as u128),
        configs_tail,
        strategy_tail,
    ))
}

fn encode_update_agents_calldata(
    updates: &[BootstrapAgentUpdateInput],
    strategy: &str,
) -> Result<String, ApiError> {
    let updates_tail = encode_agent_update_array_tail(updates)?;
    let strategy_tail = encode_dynamic_string_tail(strategy);
    let head_len_bytes = 32usize * 2usize;
    let strategy_offset = head_len_bytes + (updates_tail.len() / 2);

    Ok(format!(
        "{}{}{}{}{}",
        AGENT_RUNTIME_UPDATE_BATCH_SELECTOR,
        encode_u256_hex_u128(head_len_bytes as u128),
        encode_u256_hex_u128(strategy_offset as u128),
        updates_tail,
        strategy_tail,
    ))
}

fn encode_deactivate_agents_calldata(agent_ids: &[u64]) -> String {
    let ids_tail = encode_u64_array_tail(agent_ids);
    format!(
        "{}{}{}",
        AGENT_RUNTIME_DEACTIVATE_BATCH_SELECTOR,
        encode_u256_hex_u128(32),
        ids_tail,
    )
}

fn encode_set_agent_manager_calldata(agent_id: u64, manager: &str) -> Result<String, ApiError> {
    Ok(format!(
        "{}{}{}",
        AGENT_RUNTIME_SET_MANAGER_SELECTOR,
        encode_u256_hex(agent_id),
        encode_address_word(manager)?,
    ))
}

fn encode_validation_request_calldata(
    validator: &str,
    agent_id: u128,
    request_uri: &str,
    request_hash: &str,
) -> Result<String, ApiError> {
    let request_uri_tail = encode_dynamic_string_tail(request_uri);
    let head_len_bytes = 32usize * 4usize;

    Ok(format!(
        "{}{}{}{}{}{}",
        ERC8004_VALIDATION_REQUEST_SELECTOR,
        encode_address_word(validator)?,
        encode_u256_hex_u128(agent_id),
        encode_u256_hex_u128(head_len_bytes as u128),
        encode_bytes32_word(request_hash)?,
        request_uri_tail,
    ))
}

fn encode_validation_response_calldata(
    request_hash: &str,
    response: u8,
    response_uri: &str,
    response_hash: &str,
    tag: &str,
) -> Result<String, ApiError> {
    let response_uri_tail = encode_dynamic_string_tail(response_uri);
    let head_len_bytes = 32usize * 5usize;

    Ok(format!(
        "{}{}{}{}{}{}{}",
        ERC8004_VALIDATION_RESPONSE_SELECTOR,
        encode_bytes32_word(request_hash)?,
        encode_u256_hex_u128(response as u128),
        encode_u256_hex_u128(head_len_bytes as u128),
        encode_bytes32_word(response_hash)?,
        encode_bytes32_word(tag)?,
        response_uri_tail,
    ))
}

fn parse_u128_decimal(value: &str, field: &str) -> Result<u128, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_NUMERIC_FIELD",
            &format!("{} is required", field),
        ));
    }
    trimmed.parse::<u128>().map_err(|_| {
        ApiError::bad_request(
            "INVALID_NUMERIC_FIELD",
            &format!("{} must be an unsigned integer string", field),
        )
    })
}

fn parse_u64_decimal(value: &str, field: &str) -> Result<u64, ApiError> {
    let parsed = parse_u128_decimal(value, field)?;
    if parsed > u64::MAX as u128 {
        return Err(ApiError::bad_request(
            "INVALID_NUMERIC_FIELD",
            &format!("{} exceeds supported range", field),
        ));
    }
    Ok(parsed as u64)
}

fn prepared_write_response(
    chain_id: u64,
    from: Option<String>,
    to: String,
    data: String,
    method: &str,
) -> PreparedEvmWriteResponse {
    PreparedEvmWriteResponse {
        chain_id,
        from,
        to,
        data: format!("0x{}", data.trim_start_matches("0x")),
        value: "0x0".to_string(),
        method: method.to_string(),
    }
}

fn word_at(data: &str, index: usize) -> Result<&str, ApiError> {
    if !data.starts_with("0x") {
        return Err(ApiError::internal("Invalid RPC hex result"));
    }

    let start = 2 + (index * 64);
    let end = start + 64;
    if data.len() < end {
        return Err(ApiError::internal("Invalid market slot payload"));
    }
    Ok(&data[start..end])
}

fn decode_market_metadata_tuple(
    payload: &str,
) -> Result<(String, String, String, String), ApiError> {
    Ok((
        decode_abi_string_at_offset(payload, word_at(payload, 0)?)?,
        decode_abi_string_at_offset(payload, word_at(payload, 1)?)?,
        decode_abi_string_at_offset(payload, word_at(payload, 2)?)?,
        decode_abi_string_at_offset(payload, word_at(payload, 3)?)?,
    ))
}

fn decode_abi_string_at_offset(payload: &str, offset_word: &str) -> Result<String, ApiError> {
    let offset = parse_u64_hex(offset_word)? as usize;
    if !payload.starts_with("0x") {
        return Err(ApiError::internal("Invalid ABI payload"));
    }

    let head = 2 + (offset * 2);
    if payload.len() < head + 64 {
        return Err(ApiError::internal("Invalid ABI payload"));
    }
    let len_word = &payload[head..head + 64];
    let length = parse_u64_hex(len_word)? as usize;
    let data_start = head + 64;
    let data_end = data_start + (length * 2);
    if payload.len() < data_end {
        return Err(ApiError::internal("Invalid ABI payload"));
    }

    let raw = &payload[data_start..data_end];
    let bytes = hex::decode(raw).map_err(|_| ApiError::internal("Invalid ABI payload"))?;
    String::from_utf8(bytes).map_err(|_| ApiError::internal("Invalid UTF-8 market metadata"))
}

fn decode_market_snapshot(index: u64, slot: &str) -> Result<BaseMarketSnapshot, ApiError> {
    let question_hash = format!("0x{}", word_at(slot, 0)?);
    let close_time = parse_u64_hex(word_at(slot, 1)?)?;
    let resolve_time = parse_u64_hex(word_at(slot, 2)?)?;
    let resolver_word = word_at(slot, 3)?;
    let resolver = format!("0x{}", &resolver_word[24..]).to_ascii_lowercase();
    let resolved = parse_bool_word(word_at(slot, 4)?)?;
    let outcome_true = parse_bool_word(word_at(slot, 5)?)?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::internal("System time error"))?
        .as_secs();

    let status = if resolved {
        "resolved".to_string()
    } else if close_time <= now {
        "closed".to_string()
    } else {
        "active".to_string()
    };

    Ok(BaseMarketSnapshot {
        id: index.to_string(),
        question_hash,
        question: String::new(),
        description: String::new(),
        category: String::new(),
        resolution_source: String::new(),
        resolver,
        close_time,
        resolve_time,
        resolved,
        outcome: if resolved {
            Some(if outcome_true {
                "yes".to_string()
            } else {
                "no".to_string()
            })
        } else {
            None
        },
        status,
        source: "internal_market_core".to_string(),
        provider: "internal".to_string(),
        is_external: false,
        external_url: None,
        chain_id: 8453,
        requires_credentials: false,
        execution_users: true,
        execution_agents: true,
        outcomes: Vec::new(),
        yes_price: None,
        no_price: None,
        volume: None,
        liquidity_mode: Some(BOOTSTRAP_LIQUIDITY_MODE_CLOB_ONLY.to_string()),
        bootstrap_status: None,
        bootstrap_active: None,
        bootstrap_seed_usdc: None,
        bootstrap_manager: None,
        bootstrap_strategy: None,
        bootstrap_levels: None,
        bootstrap_initial_yes_bps: None,
        bootstrap_base_spread_bps: None,
        bootstrap_step_bps: None,
        bootstrap_cadence_seconds: None,
        bootstrap_expiry_seconds: None,
        bootstrap_graduated_at: None,
        bootstrap_launch_tx_hash: None,
        bootstrap_last_reconciled_at: None,
        bootstrap_last_error: None,
    })
}

fn decode_agent_snapshot(
    index: u64,
    slot: &str,
    now: u64,
) -> Result<Option<BaseAgentSnapshot>, ApiError> {
    let Some(raw) = decode_agent_slot(slot)? else {
        return Ok(None);
    };

    let next_execution_at = if raw.last_executed_at == 0 {
        0
    } else {
        raw.last_executed_at.saturating_add(raw.cadence)
    };
    let can_execute = raw.active && (raw.last_executed_at == 0 || now >= next_execution_at);
    let status = if !raw.active {
        "inactive"
    } else if can_execute {
        "ready"
    } else {
        "cooldown"
    };

    Ok(Some(BaseAgentSnapshot {
        id: index.to_string(),
        owner: raw.owner,
        manager: raw.manager,
        market_id: raw.market_id.to_string(),
        is_yes: raw.is_yes,
        price_bps: raw.price_bps,
        size: raw.size.to_string(),
        cadence: raw.cadence,
        expiry_window: raw.expiry_window,
        last_executed_at: raw.last_executed_at,
        next_execution_at,
        can_execute,
        active: raw.active,
        status: status.to_string(),
        strategy: raw.strategy,
        identity_id: None,
        identity_tier: None,
        identity_active: None,
        identity_updated_at: None,
        reputation_score_bps: None,
        reputation_confidence_bps: None,
        reputation_events: None,
        reputation_notional_microusdc: None,
    }))
}

async fn enrich_agent_with_erc8004(
    state: &Arc<AppState>,
    mut snapshot: BaseAgentSnapshot,
) -> BaseAgentSnapshot {
    if let Ok(Some(identity)) = fetch_erc8004_identity(state, snapshot.owner.as_str()).await {
        snapshot.identity_id = Some(identity.identity_id.to_string());
        snapshot.identity_tier = Some(identity.tier);
        snapshot.identity_active = Some(identity.active);
        snapshot.identity_updated_at = Some(identity.updated_at);
    }
    if let Ok(Some(reputation)) = fetch_erc8004_reputation(state, snapshot.owner.as_str()).await {
        snapshot.reputation_score_bps = Some(reputation.score_bps);
        snapshot.reputation_confidence_bps = Some(reputation.confidence_bps);
        snapshot.reputation_events = Some(reputation.events);
        snapshot.reputation_notional_microusdc = Some(reputation.notional_microusdc.to_string());
    }
    snapshot
}

async fn fetch_erc8004_identity(
    state: &Arc<AppState>,
    wallet: &str,
) -> Result<Option<Erc8004Identity>, ApiError> {
    let registry = state.config.erc8004_identity_registry_address.trim();
    if registry.is_empty() {
        return Ok(None);
    }
    if !is_valid_evm_address(registry) {
        return Err(ApiError::bad_request(
            "INVALID_ERC8004_IDENTITY_REGISTRY",
            "ERC8004_IDENTITY_REGISTRY_ADDRESS must be a valid 0x EVM address",
        ));
    }

    let calldata = format!(
        "{}{}",
        ERC8004_IDENTITY_PROFILE_SELECTOR,
        encode_address_word(wallet)?
    );
    let payload = state
        .evm_rpc
        .eth_call(registry, calldata.as_str())
        .await
        .map_err(map_evm_rpc_error)?;

    let identity_id = parse_u128_hex(word_at(payload.as_str(), 0)?)?;
    let tier = parse_u8_hex(word_at(payload.as_str(), 1)?)?;
    let active = parse_bool_word(word_at(payload.as_str(), 2)?)?;
    let updated_at = parse_u64_hex(word_at(payload.as_str(), 3)?)?;

    if identity_id == 0 {
        return Ok(None);
    }

    Ok(Some(Erc8004Identity {
        identity_id,
        tier,
        active,
        updated_at,
    }))
}

async fn fetch_erc8004_reputation(
    state: &Arc<AppState>,
    wallet: &str,
) -> Result<Option<Erc8004Reputation>, ApiError> {
    let registry = state.config.erc8004_reputation_registry_address.trim();
    if registry.is_empty() {
        return Ok(None);
    }
    if !is_valid_evm_address(registry) {
        return Err(ApiError::bad_request(
            "INVALID_ERC8004_REPUTATION_REGISTRY",
            "ERC8004_REPUTATION_REGISTRY_ADDRESS must be a valid 0x EVM address",
        ));
    }

    let calldata = format!(
        "{}{}",
        ERC8004_REPUTATION_OF_SELECTOR,
        encode_address_word(wallet)?
    );
    let payload = state
        .evm_rpc
        .eth_call(registry, calldata.as_str())
        .await
        .map_err(map_evm_rpc_error)?;

    let score_raw = parse_u64_hex(word_at(payload.as_str(), 0)?)?;
    let confidence_raw = parse_u64_hex(word_at(payload.as_str(), 1)?)?;
    if score_raw > u32::MAX as u64 || confidence_raw > u32::MAX as u64 {
        return Err(ApiError::internal("ERC8004 reputation value out of range"));
    }
    let score_bps = score_raw as u32;
    let confidence_bps = confidence_raw as u32;
    let events = parse_u64_hex(word_at(payload.as_str(), 2)?)?;
    let notional_microusdc = parse_u128_hex(word_at(payload.as_str(), 3)?)?;

    if events == 0 && notional_microusdc == 0 {
        return Ok(None);
    }

    Ok(Some(Erc8004Reputation {
        score_bps,
        confidence_bps,
        events,
        notional_microusdc,
    }))
}

async fn fetch_erc8004_validation(
    state: &Arc<AppState>,
    request_hash: &str,
) -> Result<Erc8004Validation, ApiError> {
    let registry = configured_address(
        state.config.erc8004_validation_registry_address.as_str(),
        "ERC8004_VALIDATION_REGISTRY_NOT_CONFIGURED",
        "ERC8004_VALIDATION_REGISTRY_ADDRESS must be configured for read operations",
    )?;

    let calldata = format!(
        "{}{}",
        ERC8004_VALIDATION_STATUS_SELECTOR,
        encode_bytes32_word(request_hash)?,
    );
    let payload = state
        .evm_rpc
        .eth_call(registry.as_str(), calldata.as_str())
        .await
        .map_err(map_evm_rpc_error)?;

    let validator_word = word_at(payload.as_str(), 0)?;
    let validator = format!("0x{}", &validator_word[24..]).to_ascii_lowercase();
    let agent_id = parse_u128_hex(word_at(payload.as_str(), 1)?)?;
    let response = parse_u8_hex(word_at(payload.as_str(), 2)?)?;
    let response_hash = format!("0x{}", word_at(payload.as_str(), 3)?);
    let tag = format!("0x{}", word_at(payload.as_str(), 4)?);
    let last_update = parse_u64_hex(word_at(payload.as_str(), 5)?)?;

    Ok(Erc8004Validation {
        validator,
        agent_id,
        response,
        response_hash,
        tag,
        last_update,
    })
}

fn decode_agent_slot(slot: &str) -> Result<Option<BaseRawAgent>, ApiError> {
    let owner_word = word_at(slot, 0)?;
    if owner_word.chars().all(|c| c == '0') {
        return Ok(None);
    }

    Ok(Some(BaseRawAgent {
        owner: format!("0x{}", &owner_word[24..]).to_ascii_lowercase(),
        manager: {
            let manager_word = word_at(slot, 1)?;
            if manager_word.chars().all(|c| c == '0') {
                None
            } else {
                Some(format!("0x{}", &manager_word[24..]).to_ascii_lowercase())
            }
        },
        market_id: parse_u64_hex(word_at(slot, 2)?)?,
        is_yes: parse_bool_word(word_at(slot, 3)?)?,
        price_bps: parse_u64_hex(word_at(slot, 4)?)?,
        size: parse_u128_hex(word_at(slot, 5)?)?,
        cadence: parse_u64_hex(word_at(slot, 6)?)?,
        expiry_window: parse_u64_hex(word_at(slot, 7)?)?,
        last_executed_at: parse_u64_hex(word_at(slot, 8)?)?,
        active: parse_bool_word(word_at(slot, 9)?)?,
        strategy: decode_abi_string_at_offset(slot, word_at(slot, 10)?)?,
    }))
}

fn decode_order_snapshot(slot: &str) -> Result<Option<BaseRawOrder>, ApiError> {
    let maker_word = word_at(slot, 0)?;
    if maker_word.chars().all(|c| c == '0') {
        return Ok(None);
    }

    Ok(Some(BaseRawOrder {
        market_id: parse_u64_hex(word_at(slot, 1)?)?,
        is_yes: parse_bool_word(word_at(slot, 2)?)?,
        price_bps: parse_u64_hex(word_at(slot, 3)?)?,
        remaining: parse_u64_hex(word_at(slot, 5)?)?,
        expiry: parse_u64_hex(word_at(slot, 6)?)?,
        canceled: parse_bool_word(word_at(slot, 7)?)?,
    }))
}

fn map_evm_rpc_error(err: anyhow::Error) -> ApiError {
    ApiError::internal(&format!("Base RPC request failed: {}", err))
}

fn unix_to_rfc3339(timestamp: u64) -> String {
    Utc.timestamp_opt(timestamp as i64, 0)
        .single()
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| Utc::now().to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bootstrap_config() -> BaseMarketBootstrapConfigRecord {
        let now = Utc::now();
        BaseMarketBootstrapConfigRecord {
            market_id: 42,
            creator: "0x71c7656ec7ab88b098defb751b7401b5f6d8976f".to_string(),
            liquidity_mode: BOOTSTRAP_LIQUIDITY_MODE_HYBRID.to_string(),
            status: BOOTSTRAP_STATUS_ACTIVE.to_string(),
            manager: Some("0x0000000000000000000000000000000000000042".to_string()),
            seed_usdc: 100.0,
            initial_yes_bps: 5_000,
            strategy: BOOTSTRAP_STRATEGY_LADDER_V1.to_string(),
            levels: BOOTSTRAP_DEFAULT_LEVELS,
            base_spread_bps: BOOTSTRAP_DEFAULT_BASE_SPREAD_BPS,
            step_bps: BOOTSTRAP_DEFAULT_STEP_BPS,
            cadence_seconds: BOOTSTRAP_DEFAULT_CADENCE_SECONDS,
            expiry_seconds: BOOTSTRAP_DEFAULT_EXPIRY_SECONDS,
            organic_depth_window_bps: BOOTSTRAP_DEFAULT_DEPTH_WINDOW_BPS,
            target_depth_multiplier: BOOTSTRAP_DEFAULT_TARGET_DEPTH_MULTIPLIER,
            target_volume_multiplier: BOOTSTRAP_DEFAULT_TARGET_VOLUME_MULTIPLIER,
            max_age_seconds: BOOTSTRAP_DEFAULT_MAX_AGE_SECONDS,
            inventory_skew_bps: 0,
            exposure_cap_bps: BOOTSTRAP_DEFAULT_EXPOSURE_CAP_BPS,
            depth_qualified_since: None,
            activated_at: now,
            graduated_at: None,
            graduation_reason: None,
            create_tx_hash: None,
            launch_tx_hash: None,
            last_reconciled_at: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn test_validate_bootstrap_registration_rejects_disabled_strategies() {
        let request = RegisterBaseMarketBootstrapRequest {
            tx_hash: "0x1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            liquidity_mode: BOOTSTRAP_LIQUIDITY_MODE_HYBRID.to_string(),
            seed_usdc: 100.0,
            initial_yes_bps: 5_000,
            manager: Some("0x0000000000000000000000000000000000000042".to_string()),
            strategy: BOOTSTRAP_STRATEGY_PMM_EXPERIMENTAL.to_string(),
            levels: 5,
            base_spread_bps: 150,
            step_bps: 100,
            cadence_seconds: 300,
            expiry_seconds: 900,
        };

        let error = validate_bootstrap_registration(&request).unwrap_err();
        assert_eq!(error.code, "BOOTSTRAP_STRATEGY_DISABLED");
    }

    #[test]
    fn test_generate_bootstrap_synthetic_book_is_symmetric_without_skew() {
        let config = sample_bootstrap_config();
        let synthetic = generate_bootstrap_synthetic_book(&config);
        let yes_prices = synthetic.yes_bids.keys().copied().collect::<Vec<_>>();
        let no_prices = synthetic.no_bids.keys().copied().collect::<Vec<_>>();

        assert_eq!(yes_prices, vec![4_450, 4_550, 4_650, 4_750, 4_850]);
        assert_eq!(no_prices, vec![4_450, 4_550, 4_650, 4_750, 4_850]);
        assert_eq!(
            synthetic
                .yes_bids
                .values()
                .map(|level| level.quantity)
                .sum::<u64>(),
            50_000_000
        );
        assert_eq!(
            synthetic
                .no_bids
                .values()
                .map(|level| level.quantity)
                .sum::<u64>(),
            50_000_000
        );
    }

    #[test]
    fn test_generate_bootstrap_synthetic_book_suppresses_crowded_side_at_cap() {
        let mut config = sample_bootstrap_config();
        config.inventory_skew_bps = config.exposure_cap_bps as i32;

        let synthetic = generate_bootstrap_synthetic_book(&config);

        assert!(synthetic.yes_bids.is_empty());
        assert_eq!(
            synthetic
                .no_bids
                .values()
                .map(|level| level.quantity)
                .sum::<u64>(),
            100_000_000
        );
    }

    #[test]
    fn test_merge_level_maps_aggregates_quantities() {
        let mut organic = BTreeMap::from([(
            4_900,
            LevelAggregate {
                quantity: 10,
                orders: 1,
            },
        )]);
        let synthetic = BTreeMap::from([
            (
                4_900,
                LevelAggregate {
                    quantity: 25,
                    orders: 1,
                },
            ),
            (
                4_800,
                LevelAggregate {
                    quantity: 40,
                    orders: 1,
                },
            ),
        ]);

        merge_level_maps(&mut organic, &synthetic);

        assert_eq!(organic.get(&4_900).unwrap().quantity, 35);
        assert_eq!(organic.get(&4_900).unwrap().orders, 2);
        assert_eq!(organic.get(&4_800).unwrap().quantity, 40);
    }

    #[test]
    fn test_bootstrap_side_budget_bps_respects_exposure_cap() {
        let mut config = sample_bootstrap_config();
        config.inventory_skew_bps = -((config.exposure_cap_bps / 2) as i32);

        let (yes_budget_bps, no_budget_bps) = bootstrap_side_budget_bps(&config);

        assert_eq!(yes_budget_bps, 7_500);
        assert_eq!(no_budget_bps, 2_500);
    }

    #[test]
    fn test_is_valid_evm_address() {
        assert!(is_valid_evm_address(
            "0x71C7656EC7ab88b098defB751B7401B5f6d8976F"
        ));
        assert!(!is_valid_evm_address("0x123"));
        assert!(!is_valid_evm_address(
            "71C7656EC7ab88b098defB751B7401B5f6d8976F"
        ));
    }

    #[test]
    fn test_parse_u8_hex() {
        assert_eq!(parse_u8_hex("0x12").unwrap(), 0x12);
        assert_eq!(
            parse_u8_hex("0x0000000000000000000000000000000000000000000000000000000000000006")
                .unwrap(),
            6
        );
        assert!(parse_u8_hex("0x100").is_err());
        assert!(parse_u8_hex("0x").is_err());
    }

    #[test]
    fn test_parse_u64_hex() {
        assert_eq!(parse_u64_hex("0x0").unwrap(), 0);
        assert_eq!(parse_u64_hex("0x2a").unwrap(), 42);
        assert_eq!(
            parse_u64_hex("0x00000000000000000000000000000000000000000000000000000000000000ff")
                .unwrap(),
            255
        );
    }

    #[test]
    fn test_parse_u128_hex() {
        assert_eq!(parse_u128_hex("0x0").unwrap(), 0);
        assert_eq!(parse_u128_hex("0x2a").unwrap(), 42);
        assert_eq!(
            parse_u128_hex("0x000000000000000000000000000000000000000000000000000000000000ffff")
                .unwrap(),
            65_535
        );
    }

    #[test]
    fn test_encode_u256_hex() {
        let encoded = encode_u256_hex(42);
        assert_eq!(encoded.len(), 64);
        assert!(encoded.ends_with("2a"));
    }

    #[test]
    fn test_is_valid_hex_payload() {
        assert!(is_valid_hex_payload("0x1234"));
        assert!(!is_valid_hex_payload("0x123"));
        assert!(!is_valid_hex_payload("1234"));
    }

    #[test]
    fn test_decode_market_metadata_tuple() {
        let q = encode_dynamic_string_tail("question?");
        let d = encode_dynamic_string_tail("description");
        let c = encode_dynamic_string_tail("crypto");
        let s = encode_dynamic_string_tail("source");

        let head = format!(
            "{}{}{}{}",
            encode_u256_hex_u128(128),
            encode_u256_hex_u128(128 + q.len() as u128 / 2),
            encode_u256_hex_u128(128 + q.len() as u128 / 2 + d.len() as u128 / 2),
            encode_u256_hex_u128(
                128 + q.len() as u128 / 2 + d.len() as u128 / 2 + c.len() as u128 / 2
            ),
        );
        let payload = format!("0x{}{}{}{}{}", head, q, d, c, s);
        let decoded = decode_market_metadata_tuple(&payload).unwrap();
        assert_eq!(decoded.0, "question?");
        assert_eq!(decoded.1, "description");
        assert_eq!(decoded.2, "crypto");
        assert_eq!(decoded.3, "source");
    }

    #[test]
    fn test_decode_market_snapshot() {
        let question_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let close_time = format!("{:064x}", 1u64);
        let resolve_time = format!("{:064x}", 2u64);
        let resolver = "00000000000000000000000071c7656ec7ab88b098defb751b7401b5f6d8976f";
        let resolved = format!("{:064x}", 1u64);
        let outcome = format!("{:064x}", 1u64);

        let payload = format!(
            "0x{}{}{}{}{}{}",
            question_hash, close_time, resolve_time, resolver, resolved, outcome
        );

        let decoded = decode_market_snapshot(7, &payload).unwrap();
        assert_eq!(decoded.id, "7");
        assert_eq!(
            decoded.question_hash,
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(
            decoded.resolver,
            "0x71c7656ec7ab88b098defb751b7401b5f6d8976f"
        );
        assert_eq!(decoded.status, "resolved");
        assert_eq!(decoded.outcome.as_deref(), Some("yes"));
    }

    #[test]
    fn test_decode_agent_snapshot() {
        let owner = "00000000000000000000000039e4939df3763e342db531a2a58867bc26a22b98";
        let manager = "0000000000000000000000000000000000000000000000000000000000000000";
        let market_id = format!("{:064x}", 12u64);
        let is_yes = format!("{:064x}", 1u64);
        let price_bps = format!("{:064x}", 5500u64);
        let size = format!("{:064x}", 100_000u128);
        let cadence = format!("{:064x}", 30u64);
        let expiry_window = format!("{:064x}", 1800u64);
        let last_executed_at = format!("{:064x}", 100u64);
        let active = format!("{:064x}", 1u64);
        let strategy_offset = encode_u256_hex_u128((32 * 11) as u128);
        let strategy = encode_dynamic_string_tail("momentum-v1");

        let payload = format!(
            "0x{}{}{}{}{}{}{}{}{}{}{}{}",
            owner,
            manager,
            market_id,
            is_yes,
            price_bps,
            size,
            cadence,
            expiry_window,
            last_executed_at,
            active,
            strategy_offset,
            strategy
        );

        let decoded = decode_agent_snapshot(5, &payload, 120).unwrap().unwrap();
        assert_eq!(decoded.id, "5");
        assert_eq!(decoded.owner, "0x39e4939df3763e342db531a2a58867bc26a22b98");
        assert_eq!(decoded.market_id, "12");
        assert!(decoded.is_yes);
        assert_eq!(decoded.price_bps, 5500);
        assert_eq!(decoded.size, "100000");
        assert_eq!(decoded.next_execution_at, 130);
        assert_eq!(decoded.status, "cooldown");
        assert!(!decoded.can_execute);
        assert_eq!(decoded.strategy, "momentum-v1");
    }

    #[test]
    fn test_decode_order_snapshot() {
        let maker = "00000000000000000000000071c7656ec7ab88b098defb751b7401b5f6d8976f";
        let market_id = format!("{:064x}", 5u64);
        let is_yes = format!("{:064x}", 1u64);
        let price_bps = format!("{:064x}", 6300u64);
        let size = format!("{:064x}", 100u64);
        let remaining = format!("{:064x}", 25u64);
        let expiry = format!("{:064x}", 1_800_000_000u64);
        let canceled = format!("{:064x}", 0u64);

        let payload = format!(
            "0x{}{}{}{}{}{}{}{}",
            maker, market_id, is_yes, price_bps, size, remaining, expiry, canceled
        );
        let decoded = decode_order_snapshot(&payload).unwrap().unwrap();

        assert_eq!(decoded.market_id, 5);
        assert!(decoded.is_yes);
        assert_eq!(decoded.price_bps, 6300);
        assert_eq!(decoded.remaining, 25);
        assert_eq!(decoded.expiry, 1_800_000_000);
        assert!(!decoded.canceled);
    }

    #[test]
    fn test_decode_order_snapshot_empty_slot() {
        let maker = "0000000000000000000000000000000000000000000000000000000000000000";
        let payload = format!(
            "0x{}{}{}{}{}{}{}{}",
            maker,
            format!("{:064x}", 0u64),
            format!("{:064x}", 0u64),
            format!("{:064x}", 0u64),
            format!("{:064x}", 0u64),
            format!("{:064x}", 0u64),
            format!("{:064x}", 0u64),
            format!("{:064x}", 0u64)
        );
        assert!(decode_order_snapshot(&payload).unwrap().is_none());
    }

    #[test]
    fn test_unix_to_rfc3339() {
        let value = unix_to_rfc3339(1_700_000_000);
        assert!(value.starts_with("2023-"));
    }

    #[test]
    fn test_internal_feed_warning_for_rate_limit() {
        let warning = internal_feed_warning(&ApiError::internal(
            "Base RPC request failed: Base RPC returned non-success status: 429 Too Many Requests",
        ));

        assert_eq!(warning.source, "internal");
        assert_eq!(
            warning.message,
            "internal Base feed temporarily rate limited"
        );
    }

    #[test]
    fn test_internal_feed_warning_for_generic_failure() {
        let warning = internal_feed_warning(&ApiError::internal(
            "Base RPC request failed: connection reset",
        ));

        assert_eq!(warning.source, "internal");
        assert_eq!(warning.message, "internal Base feed unavailable");
    }
}
