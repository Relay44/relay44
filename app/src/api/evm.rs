use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha3::{Digest, Keccak256};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::ApiError;
use crate::services::database::PayoutJobRecord;
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
const ORDER_BOOK_COUNT_SELECTOR: &str = "0x2453ffa8";
const ORDER_BOOK_ORDERS_SELECTOR: &str = "0xa85c38ef";
const ORDER_BOOK_PLACE_SELECTOR: &str = "0xa8dd6515";
const ORDER_BOOK_CANCEL_SELECTOR: &str = "0x514fcac7";
const ORDER_BOOK_CLAIM_SELECTOR: &str = "0x379607f5";
const ORDER_BOOK_CLAIM_FOR_SELECTOR: &str = "0x0de05659";
const ORDER_BOOK_MATCH_SELECTOR: &str = "0xc6437097";
const AGENT_RUNTIME_COUNT_SELECTOR: &str = "0xb7dc1284";
const AGENT_RUNTIME_AGENTS_SELECTOR: &str = "0x513856c8";
const AGENT_RUNTIME_CREATE_SELECTOR: &str = "0x325993ba";
const AGENT_RUNTIME_EXECUTE_SELECTOR: &str = "0xe2a343a5";
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

#[derive(Serialize)]
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

#[derive(Default)]
struct LevelAggregate {
    quantity: u64,
    orders: u64,
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
    if total == 0 {
        return Ok(HttpResponse::Ok().json(BaseOrderBookResponse {
            market_id: market_id_raw,
            outcome: outcome.to_string(),
            bids: vec![],
            asks: vec![],
            last_updated: Utc::now().to_rfc3339(),
            source: "order_book_contract".to_string(),
            provider: "internal".to_string(),
            chain_id: state.config.base_chain_id,
            provider_market_ref: market_id.to_string(),
            is_synthetic: false,
        }));
    }

    let start = if total > ORDERBOOK_SCAN_WINDOW {
        total - ORDERBOOK_SCAN_WINDOW + 1
    } else {
        1
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::internal("System time error"))?
        .as_secs();

    let mut bid_levels: BTreeMap<u64, LevelAggregate> = BTreeMap::new();
    let mut ask_levels: BTreeMap<u64, LevelAggregate> = BTreeMap::new();

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
            || order.price_bps == 0
            || order.price_bps >= 10_000
        {
            continue;
        }

        if order.is_yes == outcome_is_yes {
            let level = bid_levels.entry(order.price_bps).or_default();
            level.quantity += order.remaining;
            level.orders += 1;
        } else {
            let ask_price_bps = 10_000 - order.price_bps;
            if ask_price_bps == 0 || ask_price_bps >= 10_000 {
                continue;
            }
            let level = ask_levels.entry(ask_price_bps).or_default();
            level.quantity += order.remaining;
            level.orders += 1;
        }
    }

    let bids = bid_levels
        .into_iter()
        .rev()
        .take(depth as usize)
        .map(|(price_bps, level)| BaseOrderBookLevel {
            price: (price_bps as f64) / 10_000.0,
            quantity: level.quantity as f64,
            orders: level.orders,
        })
        .collect::<Vec<_>>();

    let asks = ask_levels
        .into_iter()
        .take(depth as usize)
        .map(|(price_bps, level)| BaseOrderBookLevel {
            price: (price_bps as f64) / 10_000.0,
            quantity: level.quantity as f64,
            orders: level.orders,
        })
        .collect::<Vec<_>>();

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
        is_synthetic: false,
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
