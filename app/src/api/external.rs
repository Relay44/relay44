use actix_web::{web, HttpRequest, HttpResponse, Responder};
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine as _;
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use sha3::{Digest, Keccak256};
use sqlx::Row;
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::auth::{extract_authenticated_user, extract_jwt_user, AuthenticatedUserWithRole};
use crate::api::jwt::{check_role, UserRole};
use crate::api::ApiError;
use crate::config::ExternalExecutionMode;
use crate::services::external;
use crate::services::external::credentials::{decrypt_json, encrypt_json, mask_secret};
use crate::services::external::paper::{realized_pnl, simulate_fill, unrealized_pnl};
use crate::services::external::types::{ExternalMarketId, ExternalProvider};
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

struct PolymarketCredentials {
    api_key: String,
    api_secret: String,
    api_passphrase: String,
    funder: String,
    signature_type: u8,
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
pub struct SubmitExternalOrderRequest {
    pub intent_id: String,
    pub signed_order: Value,
    pub credential_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelExternalOrderRequest {
    pub provider: String,
    pub provider_order_id: String,
    pub credential_id: Option<String>,
    pub payload: Option<Value>,
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
    pub credential_id: Option<String>,
    pub active: Option<bool>,
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
    pub credential_id: Option<String>,
    pub active: Option<bool>,
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
    pub credential_id: Option<String>,
    pub active: bool,
    pub last_executed_at: Option<String>,
    pub next_execution_at: String,
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

#[derive(Debug, Clone)]
struct StoredCredential {
    id: String,
    owner: String,
    payload: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct ExternalAgentRecord {
    pub(crate) id: String,
    pub(crate) owner: String,
    pub(crate) name: String,
    pub(crate) provider: ExternalProvider,
    pub(crate) market_id: String,
    pub(crate) outcome: String,
    pub(crate) side: String,
    pub(crate) price: f64,
    pub(crate) quantity: f64,
    pub(crate) cadence_seconds: i64,
    pub(crate) strategy: String,
    pub(crate) credential_id: Option<String>,
    pub(crate) active: bool,
    pub(crate) next_execution_at: chrono::DateTime<Utc>,
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

fn to_rail_provider(provider: ExternalProvider) -> RailProvider {
    match provider {
        ExternalProvider::Limitless => RailProvider::Limitless,
        ExternalProvider::Polymarket => RailProvider::Polymarket,
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

fn requires_live_credentials(state: &AppState) -> bool {
    execution_mode(state) == ExternalExecutionMode::Live
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
    let user = extract_jwt_user(req, state)?;
    check_role(user.role, UserRole::Admin)?;
    Ok(())
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
                credential_id,
                active,
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
                entry.credential_id,
                entry.active,
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
                credential_id TEXT,
                active BOOLEAN,
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
                credential_id = EXCLUDED.credential_id,
                active = EXCLUDED.active,
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
    payload
        .get("orderId")
        .or_else(|| payload.get("id"))
        .or_else(|| payload.get("order_id"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
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

    Ok(ExternalAgentRecord {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        owner: row
            .try_get("owner")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
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
        credential_id: row.try_get("credential_id").ok(),
        active: row
            .try_get("active")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        next_execution_at: row
            .try_get("next_execution_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
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

pub(crate) async fn load_external_agent_for_owner(
    state: &AppState,
    agent_id: &str,
    owner: &str,
) -> Result<ExternalAgentRecord, ApiError> {
    let row = sqlx::query(
        "SELECT id, owner, name, provider, market_id, outcome, side, price, quantity,
                cadence_seconds, strategy, credential_id, active, last_executed_at, next_execution_at
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

async fn load_due_external_agents(
    state: &AppState,
    limit: i64,
) -> Result<Vec<ExternalAgentRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, owner, name, provider, market_id, outcome, side, price, quantity,
                cadence_seconds, strategy, credential_id, active, last_executed_at, next_execution_at
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

fn polymarket_request_body(payload: &Value) -> Result<String, ApiError> {
    serde_json::to_string(payload).map_err(|err| ApiError::internal(&err.to_string()))
}

fn polymarket_l2_signature(
    api_secret: &str,
    method: &str,
    path: &str,
    body: &str,
    timestamp: &str,
) -> Result<String, ApiError> {
    let decoded_secret = URL_SAFE.decode(api_secret.trim()).map_err(|_| {
        ApiError::bad_request("INVALID_CREDENTIALS", "polymarket apiSecret is invalid")
    })?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&decoded_secret).map_err(|_| {
        ApiError::bad_request("INVALID_CREDENTIALS", "polymarket apiSecret is invalid")
    })?;
    mac.update(format!("{}{}{}{}", timestamp, method, path, body).as_bytes());
    Ok(URL_SAFE.encode(mac.finalize().into_bytes()))
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
    }
}

async fn build_limitless_submit_payload(
    state: &AppState,
    credential: &StoredCredential,
    market_id: &str,
    provider_market_ref: &str,
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

    Ok(json!({
        "order": order,
        "orderType": "GTC",
        "marketSlug": market_slug,
        "ownerId": profile.id,
    }))
}

async fn submit_polymarket_order(
    state: &AppState,
    credential: &StoredCredential,
    signed_order: &Value,
) -> Result<Value, ApiError> {
    let credentials = polymarket_credentials(credential)?;
    let owner = normalize_evm_wallet(credential.owner.as_str())?.to_ascii_lowercase();
    let body = polymarket_request_body(signed_order)?;
    let path = "/order";
    let timestamp = Utc::now().timestamp().to_string();
    let signature = polymarket_l2_signature(
        credentials.api_secret.as_str(),
        "POST",
        path,
        body.as_str(),
        timestamp.as_str(),
    )?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let response = client
        .post(format!(
            "{}{}",
            state.config.polymarket_clob_api_base.trim_end_matches('/'),
            path
        ))
        .header("POLY_ADDRESS", owner)
        .header("POLY_API_KEY", credentials.api_key)
        .header("POLY_PASSPHRASE", credentials.api_passphrase)
        .header("POLY_SIGNATURE", signature)
        .header("POLY_TIMESTAMP", timestamp)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|err| ApiError::internal(&format!("polymarket submit failed: {}", err)))?;

    let status = response.status();
    let payload = response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| json!({ "ok": status.is_success() }));

    if !status.is_success() {
        let message = payload
            .get("errorMsg")
            .or_else(|| payload.get("message"))
            .or_else(|| payload.get("error"))
            .and_then(|value| value.as_str())
            .unwrap_or("polymarket order submission failed");
        return Err(ApiError::bad_request("POLYMARKET_SUBMIT_FAILED", message));
    }

    Ok(payload)
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
            let credentials = polymarket_credentials(credential)?;
            let owner = normalize_evm_wallet(credential.owner.as_str())?.to_ascii_lowercase();
            let body = payload.unwrap_or_else(|| json!({ "orderId": provider_order_id }));
            let body_string = polymarket_request_body(&body)?;
            let path = "/order";
            let timestamp = Utc::now().timestamp().to_string();
            let signature = polymarket_l2_signature(
                credentials.api_secret.as_str(),
                "DELETE",
                path,
                body_string.as_str(),
                timestamp.as_str(),
            )?;

            let response = client
                .delete(format!(
                    "{}{}",
                    state.config.polymarket_clob_api_base.trim_end_matches('/'),
                    path
                ))
                .header("POLY_ADDRESS", owner)
                .header("POLY_API_KEY", credentials.api_key)
                .header("POLY_PASSPHRASE", credentials.api_passphrase)
                .header("POLY_SIGNATURE", signature)
                .header("POLY_TIMESTAMP", timestamp)
                .header("Content-Type", "application/json")
                .body(body_string)
                .send()
                .await
                .map_err(|err| ApiError::internal(&format!("polymarket cancel failed: {}", err)))?;

            let status = response.status();
            let payload = response
                .json::<Value>()
                .await
                .unwrap_or_else(|_| json!({ "ok": status.is_success() }));

            if !status.is_success() {
                return Err(ApiError::bad_request(
                    "POLYMARKET_CANCEL_FAILED",
                    payload
                        .get("errorMsg")
                        .or_else(|| payload.get("message"))
                        .or_else(|| payload.get("error"))
                        .and_then(|value| value.as_str())
                        .unwrap_or("polymarket cancel failed"),
                ));
            }

            Ok(payload)
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

fn parse_external_agent(row: sqlx::postgres::PgRow) -> Result<ExternalAgentResponse, ApiError> {
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

    Ok(ExternalAgentResponse {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        owner: row
            .try_get("owner")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
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
        strategy: row
            .try_get("strategy")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        credential_id: row.try_get("credential_id").ok(),
        active: row
            .try_get("active")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        last_executed_at: last_executed_at.map(|entry| entry.to_rfc3339()),
        next_execution_at: next_execution_at.to_rfc3339(),
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    })
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
    let fill = simulate_fill(
        market,
        orderbook,
        agent.outcome.as_str(),
        agent.side.as_str(),
        agent.quantity,
        state.config.paper_fee_bps,
    );

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
        "quantity": agent.quantity,
        "price": agent.price
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
            "fill": fill
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
            "holdUntil": hold_until.to_rfc3339()
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
    let market = external::fetch_market_by_id(&state.config, &market_id).await?;
    let market_closed = market_is_closed_for_paper_entry(&market, now);

    if let Some(position) = load_open_paper_position(state, agent.id.as_str()).await? {
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

async fn execute_live_agent(
    state: &AppState,
    agent: &ExternalAgentRecord,
    signed_order_override: Option<Value>,
) -> Result<AgentExecutionOutcome, ApiError> {
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

    let submit_payload =
        submit_to_provider(state, agent.provider, &credential, &signed_order).await?;
    let provider_order_id = provider_order_id_from_payload(&submit_payload);
    let now = Utc::now();
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
    .bind(&signed_order)
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
            "response": submit_payload
        }),
    )
    .await?;

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
    match execution_mode(state) {
        ExternalExecutionMode::Paper => execute_paper_agent(state, agent).await,
        ExternalExecutionMode::Live => {
            execute_live_agent(state, agent, signed_order_override).await
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

    let row = sqlx::query(
        "SELECT id, provider, market_id, provider_market_ref, credential_id, typed_data, status
         FROM external_order_intents
         WHERE id = $1 AND owner = $2",
    )
    .bind(body.intent_id.as_str())
    .bind(user.wallet_address.as_str())
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .ok_or_else(|| ApiError::not_found("External order intent"))?;

    let provider_raw: String = row
        .try_get("provider")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let provider = normalize_provider(provider_raw.as_str())?;
    ensure_provider_action_allowed(&req, provider, ProviderRailAction::TradeOpen)?;

    let credential_id = body
        .credential_id
        .as_deref()
        .map(ToOwned::to_owned)
        .or_else(|| row.try_get::<String, _>("credential_id").ok());

    let credential = load_credential(
        &state,
        user.wallet_address.as_str(),
        provider,
        credential_id.as_deref(),
    )
    .await?;
    ensure_provider_credential_ready(&state, provider, &credential).await?;

    let market_id: String = row
        .try_get("market_id")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let provider_market_ref: String = row
        .try_get("provider_market_ref")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let typed_data: Value = row
        .try_get("typed_data")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let provider_payload = match provider {
        ExternalProvider::Limitless => {
            build_limitless_submit_payload(
                &state,
                &credential,
                market_id.as_str(),
                provider_market_ref.as_str(),
                &typed_data,
                &body.signed_order,
            )
            .await?
        }
        ExternalProvider::Polymarket => {
            build_polymarket_submit_payload(&credential, &typed_data, &body.signed_order)?
        }
    };

    let provider_response =
        submit_to_provider(&state, provider, &credential, &provider_payload).await;
    let now = Utc::now();
    let order_id = Uuid::new_v4().to_string();

    let (status, payload, error_message, provider_order_id) = match provider_response {
        Ok(payload) => {
            let provider_order_id = payload
                .get("orderId")
                .or_else(|| payload.get("id"))
                .or_else(|| payload.get("order_id"))
                .or_else(|| payload.get("order").and_then(|value| value.get("id")))
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            ("submitted".to_string(), payload, None, provider_order_id)
        }
        Err(err) => (
            "failed".to_string(),
            json!({ "error": err.message }),
            Some(err.message),
            String::new(),
        ),
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
    .bind(market_id.as_str())
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
        market_id: row
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider_order_id,
        status,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
        response_payload: payload,
        error_message: None,
    }))
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

    let response_payload = cancel_on_provider(
        &state,
        provider,
        &credential,
        body.provider_order_id.as_str(),
        body.payload.clone(),
    )
    .await?;

    sqlx::query(
        "UPDATE external_orders
         SET status = 'cancelled', response_payload = $1, updated_at = NOW()
         WHERE owner = $2 AND provider = $3 AND provider_order_id = $4",
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
        "SELECT id, owner, name, provider, market_id, outcome, side, price, quantity, cadence_seconds,
                strategy, credential_id, active, last_executed_at, next_execution_at, created_at, updated_at
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
        agents.push(parse_external_agent(row)?);
    }

    Ok(HttpResponse::Ok().json(ExternalAgentsListResponse {
        agents,
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
    let provider = normalize_provider(body.provider.as_str())?;
    ensure_provider_action_allowed(&req, provider, ProviderRailAction::TradeOpen)?;
    let outcome = normalize_outcome(body.outcome.as_str())?;
    let side = normalize_side(body.side.as_str())?;

    if body.name.trim().is_empty() {
        return Err(ApiError::bad_request("INVALID_NAME", "name is required"));
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

    let namespaced_market_id = normalize_namespaced_market_id(provider, body.market_id.as_str());
    let parsed_market_id = ExternalMarketId::parse(namespaced_market_id.as_str())?;
    let market = external::fetch_market_by_id(&state.config, &parsed_market_id).await?;
    if !market.execution_agents {
        return Err(ApiError::bad_request(
            "MARKET_NOT_EXECUTABLE",
            "market is not executable for external agents",
        ));
    }

    let credential_id = if requires_live_credentials(&state) || body.credential_id.is_some() {
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
            outcome, side, price, quantity, cadence_seconds, strategy,
            credential_id, active, next_execution_at, created_at, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,NOW(),NOW(),NOW())
        RETURNING id, owner, name, provider, market_id, outcome, side, price, quantity,
                  cadence_seconds, strategy, credential_id, active, last_executed_at,
                  next_execution_at, created_at, updated_at",
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
    .bind(body.strategy.trim())
    .bind(credential_id.as_deref())
    .bind(body.active.unwrap_or(true))
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(parse_external_agent(row)?))
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
            "SELECT id, owner, provider, market_id, outcome, side, price, quantity, cadence_seconds, strategy, credential_id, active
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
            "SELECT id, owner, provider, market_id, outcome, side, price, quantity, cadence_seconds, strategy, credential_id, active
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
    let next_name = body.name.as_deref().unwrap_or("").trim().to_string();
    let next_active = body
        .active
        .unwrap_or_else(|| current.try_get("active").unwrap_or(true));

    let credential_id = if let Some(id) = body.credential_id.as_deref() {
        let credential = load_credential(&state, owner.as_str(), provider, Some(id)).await?;
        ensure_provider_credential_ready(&state, provider, &credential).await?;
        Some(credential.id)
    } else {
        current.try_get::<String, _>("credential_id").ok()
    };

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
                 credential_id = $9,
                 active = $10,
                 updated_at = NOW()
             WHERE id = $1
             RETURNING id, owner, name, provider, market_id, outcome, side, price, quantity,
                       cadence_seconds, strategy, credential_id, active, last_executed_at,
                       next_execution_at, created_at, updated_at",
        )
        .bind(agent_id.as_str())
        .bind(next_name)
        .bind(next_outcome)
        .bind(next_side)
        .bind(next_price)
        .bind(next_quantity)
        .bind(next_cadence as i64)
        .bind(next_strategy)
        .bind(credential_id.as_deref())
        .bind(next_active)
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
                 credential_id = $10,
                 active = $11,
                 updated_at = NOW()
             WHERE id = $1 AND owner = $2
             RETURNING id, owner, name, provider, market_id, outcome, side, price, quantity,
                       cadence_seconds, strategy, credential_id, active, last_executed_at,
                       next_execution_at, created_at, updated_at",
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
        .bind(credential_id.as_deref())
        .bind(next_active)
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
    };

    Ok(HttpResponse::Ok().json(parse_external_agent(row)?))
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

    Ok(HttpResponse::Ok().json(json!({
        "ok": outcome.executed,
        "mode": execution_mode(&state).as_str(),
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
                "paper_skipped",
                None,
                Some(reason.as_str()),
                &json!({
                    "mode": execution_mode(&state).as_str(),
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
                } else if let Some(reason) = outcome.skip_reason.as_deref() {
                    increment_skip_reason(&mut skips_by_reason, reason);
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
                        "mode": execution_mode(&state).as_str(),
                        "error": {
                            "code": err.code,
                            "message": err.message,
                            "details": err.details
                        }
                    }),
                )
                .await?;
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

    let strategy_rows = if let Some(owner) = owner_filter.as_ref() {
        sqlx::query(
            "SELECT strategy,
                    COUNT(*) AS agents,
                    COUNT(*) FILTER (WHERE active) AS active_agents
             FROM external_agents
             WHERE owner = $1
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
             GROUP BY strategy
             ORDER BY strategy ASC",
        )
        .fetch_all(state.db.pool())
        .await
    }
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let position_strategy_rows = if let Some(owner) = owner_filter.as_ref() {
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

    let fill_strategy_rows = if let Some(owner) = owner_filter.as_ref() {
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

    let outcome_strategy_rows = if let Some(owner) = owner_filter.as_ref() {
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

    let mut strategy_map = BTreeMap::new();
    for row in strategy_rows {
        let strategy = row
            .try_get::<String, _>("strategy")
            .unwrap_or_else(|_| "unclassified".to_string());
        strategy_map.insert(
            strategy.clone(),
            ExternalAgentStrategyPerformance {
                strategy,
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
            },
        );
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
                });
        entry.open_positions = row.try_get::<i64, _>("open_positions").unwrap_or(0).max(0) as u64;
        entry.closed_positions = row
            .try_get::<i64, _>("closed_positions")
            .unwrap_or(0)
            .max(0) as u64;
        entry.unrealized_pnl_usdc = row.try_get::<f64, _>("unrealized_pnl_usdc").unwrap_or(0.0);
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
                });
        entry.fills = row.try_get::<i64, _>("fills").unwrap_or(0).max(0) as u64;
        entry.volume_usdc = row.try_get::<f64, _>("volume_usdc").unwrap_or(0.0);
        entry.fees_usdc = row.try_get::<f64, _>("fees_usdc").unwrap_or(0.0);
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
                });
        entry.realized_pnl_usdc = row.try_get::<f64, _>("realized_pnl_usdc").unwrap_or(0.0);
        entry.win_rate = row.try_get::<f64, _>("win_rate").unwrap_or(0.0);
    }

    let mut strategies = strategy_map.into_values().collect::<Vec<_>>();
    for entry in &mut strategies {
        entry.net_pnl_usdc = entry.realized_pnl_usdc + entry.unrealized_pnl_usdc;
    }

    let volume_timeline_rows = if let Some(owner) = owner_filter.as_ref() {
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

    let realized_timeline_rows = if let Some(owner) = owner_filter.as_ref() {
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

    let unrealized_timeline_rows = if let Some(owner) = owner_filter.as_ref() {
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
}
