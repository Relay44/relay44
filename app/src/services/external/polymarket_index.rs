use std::env;
use std::sync::OnceLock;
use std::time::Duration;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::api::ApiError;
use crate::services::external::paper::simulate_fill;

use super::types::{
    clamp_probability, price_to_bps, ExternalMarketSnapshot, ExternalOrderBookLevel,
    ExternalOrderBookSnapshot, ExternalTradeSnapshot, ExternalTradesSnapshot,
};

const POLYMARKET_PROVIDER: &str = "polymarket";
const POLYMARKET_SOURCE: &str = "external_polymarket";
const POLYMARKET_CHAIN_ID: u64 = 137;

static INDEX_POOL: OnceLock<Result<PgPool, String>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolymarketIndexLane {
    PublicTape,
    UserFills,
}

impl PolymarketIndexLane {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PublicTape => "public_tape",
            Self::UserFills => "user_fills",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolymarketTradeLifecycleStatus {
    Matched,
    Mined,
    Confirmed,
    Retrying,
    Failed,
}

impl PolymarketTradeLifecycleStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Matched => "MATCHED",
            Self::Mined => "MINED",
            Self::Confirmed => "CONFIRMED",
            Self::Retrying => "RETRYING",
            Self::Failed => "FAILED",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PolymarketReferenceCandidates {
    pub tx_hashes: Vec<String>,
    pub provider_order_refs: Vec<String>,
    pub builder_trade_refs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PolymarketIndexStateRecord {
    pub lane: String,
    pub market_id: String,
    pub provider_market_ref: String,
    pub index_status: String,
    pub indexed_from: Option<DateTime<Utc>>,
    pub indexed_through: Option<DateTime<Utc>>,
    pub is_partial_backfill: bool,
    pub last_error: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PolymarketPublicTradeUpsert {
    pub provider_trade_id: String,
    pub market_id: String,
    pub provider_market_ref: String,
    pub market_category: Option<String>,
    pub outcome: String,
    pub side: Option<String>,
    pub price: f64,
    pub quantity: f64,
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub token_id: Option<String>,
    pub maker: Option<String>,
    pub taker: Option<String>,
    pub match_time: DateTime<Utc>,
    pub raw_payload: Value,
}

#[derive(Debug, Clone)]
pub struct PolymarketUserTradeEventUpsert {
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
    pub external_order_id: Option<String>,
    pub owner: Option<String>,
    pub market_id: String,
    pub provider_market_ref: Option<String>,
    pub provider_order_id: Option<String>,
    pub builder_trade_id: Option<String>,
    pub taker_hash: Option<String>,
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub outcome: Option<String>,
    pub side: Option<String>,
    pub price: Option<f64>,
    pub requested_quantity: Option<f64>,
    pub filled_quantity: Option<f64>,
    pub fee_usdc: Option<f64>,
    pub lifecycle_status: PolymarketTradeLifecycleStatus,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub raw_payload: Value,
    pub observed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketUserTradeEventRecord {
    pub id: String,
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
    pub external_order_id: Option<String>,
    pub owner: Option<String>,
    pub market_id: String,
    pub provider_market_ref: Option<String>,
    pub provider_order_id: Option<String>,
    pub builder_trade_id: Option<String>,
    pub taker_hash: Option<String>,
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub outcome: Option<String>,
    pub side: Option<String>,
    pub price: Option<f64>,
    pub price_bps: Option<u64>,
    pub requested_quantity: Option<f64>,
    pub filled_quantity: Option<f64>,
    pub fee_usdc: f64,
    pub lifecycle_status: String,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub raw_payload: Value,
    pub matched_at: Option<DateTime<Utc>>,
    pub mined_at: Option<DateTime<Utc>>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketPublicTradeRecord {
    pub provider_trade_id: String,
    pub market_id: String,
    pub provider_market_ref: String,
    pub market_category: Option<String>,
    pub outcome: String,
    pub side: Option<String>,
    pub price: f64,
    pub price_bps: u64,
    pub quantity: f64,
    pub maker: Option<String>,
    pub taker: Option<String>,
    pub tx_hash: Option<String>,
    pub block_number: Option<u64>,
    pub match_time: DateTime<Utc>,
    pub ingested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketPublicTradesPage {
    pub items: Vec<PolymarketPublicTradeRecord>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketOrderbookHistoryRecord {
    pub id: String,
    pub market_id: String,
    pub provider_market_ref: String,
    pub outcome: String,
    pub depth: u64,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub mid_price: Option<f64>,
    pub bids: Vec<ExternalOrderBookLevel>,
    pub asks: Vec<ExternalOrderBookLevel>,
    pub source: String,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketWalletScoreRecord {
    pub wallet: String,
    pub market_category: Option<String>,
    pub window_hours: u64,
    pub trade_count: u64,
    pub markets_traded: u64,
    pub recency_score: f64,
    pub consistency_score: f64,
    pub specialization_score: f64,
    pub crowding_penalty: f64,
    pub edge_persistence_score: f64,
    pub composite_score: f64,
    pub last_trade_at: Option<DateTime<Utc>>,
    pub metrics: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyReplayRunRecord {
    pub id: String,
    pub created_by: Option<String>,
    pub strategy: String,
    pub baseline: Option<String>,
    pub status: String,
    pub market_id: Option<String>,
    pub market_category: Option<String>,
    pub target_wallet: Option<String>,
    pub delay_ms: u64,
    pub window_hours: u64,
    pub input_params: Value,
    pub summary: Value,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyReplayFillRecord {
    pub id: String,
    pub replay_run_id: String,
    pub event_time: DateTime<Utc>,
    pub market_id: String,
    pub outcome: String,
    pub side: String,
    pub target_wallet: Option<String>,
    pub followed_trade_id: Option<String>,
    pub requested_quantity: f64,
    pub filled_quantity: f64,
    pub price: f64,
    pub mark_price: f64,
    pub fee_usdc: f64,
    pub pnl_usdc: f64,
    pub slippage_ticks: f64,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StrategyReplayRequest {
    pub created_by: Option<String>,
    pub strategy: String,
    pub baseline: Option<String>,
    pub market_id: Option<String>,
    pub market_category: Option<String>,
    pub target_wallet: Option<String>,
    pub delay_ms: u64,
    pub window_hours: u64,
    pub follow_ratio: f64,
    pub markout_minutes: u64,
    pub max_trades: u64,
}

fn pool() -> Result<&'static PgPool, ApiError> {
    match INDEX_POOL.get_or_init(|| {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL is required for polymarket indexing".to_string())?;
        PgPoolOptions::new()
            .max_connections(4)
            .min_connections(0)
            .acquire_timeout(Duration::from_secs(5))
            .connect_lazy(&database_url)
            .map_err(|err| format!("failed to initialize polymarket index pool: {err}"))
    }) {
        Ok(pool) => Ok(pool),
        Err(message) => Err(ApiError::internal(message)),
    }
}

fn trim_to_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn lifecycle_rank(status: &str) -> i32 {
    match status {
        "MATCHED" => 1,
        "MINED" => 2,
        "RETRYING" => 3,
        "FAILED" => 4,
        "CONFIRMED" => 5,
        _ => 0,
    }
}

fn merged_lifecycle_status(current: &str, incoming: PolymarketTradeLifecycleStatus) -> String {
    let incoming = incoming.as_str();
    if lifecycle_rank(incoming) >= lifecycle_rank(current) {
        incoming.to_string()
    } else {
        current.to_string()
    }
}

fn push_reference(target: &mut Vec<String>, value: Option<&str>) {
    if let Some(value) = trim_to_option(value) {
        target.push(value);
    }
}

fn dedupe_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

pub fn reference_candidates_from_payload(payload: &Value) -> PolymarketReferenceCandidates {
    let mut refs = PolymarketReferenceCandidates::default();
    for key in ["txHash", "transactionHash", "tx_hash"] {
        push_reference(
            &mut refs.tx_hashes,
            payload.get(key).and_then(Value::as_str),
        );
    }
    for key in [
        "providerOrderId",
        "provider_order_id",
        "orderId",
        "orderID",
        "order_id",
        "taker_order_id",
        "takerOrderId",
        "takerHash",
        "taker_hash",
        "orderHash",
        "order_hash",
    ] {
        push_reference(
            &mut refs.provider_order_refs,
            payload.get(key).and_then(Value::as_str),
        );
    }
    for key in [
        "builderTradeId",
        "builder_trade_id",
        "tradeId",
        "trade_id",
        "id",
    ] {
        push_reference(
            &mut refs.builder_trade_refs,
            payload.get(key).and_then(Value::as_str),
        );
    }
    if let Some(order) = payload.get("order").and_then(Value::as_object) {
        for key in [
            "txHash",
            "transactionHash",
            "transaction_hash",
            "tx_hash",
            "providerOrderId",
            "provider_order_id",
            "orderId",
            "orderID",
            "order_id",
            "taker_order_id",
            "takerOrderId",
            "takerHash",
            "taker_hash",
            "orderHash",
            "order_hash",
        ] {
            push_reference(
                if key.starts_with("tx") || key == "transactionHash" {
                    &mut refs.tx_hashes
                } else {
                    &mut refs.provider_order_refs
                },
                order.get(key).and_then(Value::as_str),
            );
        }
        for key in [
            "builderTradeId",
            "builder_trade_id",
            "tradeId",
            "trade_id",
            "id",
        ] {
            push_reference(
                &mut refs.builder_trade_refs,
                order.get(key).and_then(Value::as_str),
            );
        }
    }
    dedupe_strings(&mut refs.tx_hashes);
    dedupe_strings(&mut refs.provider_order_refs);
    dedupe_strings(&mut refs.builder_trade_refs);
    refs
}

pub fn reference_candidates_for_reconciliation(
    provider_order_id: &str,
    provider_payload: &Value,
    submit_payload: &Value,
) -> PolymarketReferenceCandidates {
    let mut refs = reference_candidates_from_payload(provider_payload);
    let submit_refs = reference_candidates_from_payload(submit_payload);
    refs.tx_hashes.extend(submit_refs.tx_hashes);
    refs.provider_order_refs
        .extend(submit_refs.provider_order_refs);
    refs.builder_trade_refs
        .extend(submit_refs.builder_trade_refs);
    push_reference(&mut refs.provider_order_refs, Some(provider_order_id));
    dedupe_strings(&mut refs.tx_hashes);
    dedupe_strings(&mut refs.provider_order_refs);
    dedupe_strings(&mut refs.builder_trade_refs);
    refs
}

fn reference_score(
    event: &PolymarketUserTradeEventRecord,
    refs: &PolymarketReferenceCandidates,
) -> (u8, DateTime<Utc>) {
    if let Some(tx_hash) = event.tx_hash.as_deref() {
        if refs
            .tx_hashes
            .iter()
            .any(|value| value.eq_ignore_ascii_case(tx_hash))
        {
            return (3, event.confirmed_at.unwrap_or(event.updated_at));
        }
    }
    for candidate in [
        event.provider_order_id.as_deref(),
        event.taker_hash.as_deref(),
    ] {
        if let Some(candidate) = candidate {
            if refs
                .provider_order_refs
                .iter()
                .any(|value| value.eq_ignore_ascii_case(candidate))
            {
                return (2, event.confirmed_at.unwrap_or(event.updated_at));
            }
        }
    }
    if let Some(builder_trade_id) = event.builder_trade_id.as_deref() {
        if refs
            .builder_trade_refs
            .iter()
            .any(|value| value.eq_ignore_ascii_case(builder_trade_id))
        {
            return (1, event.confirmed_at.unwrap_or(event.updated_at));
        }
    }
    (0, event.confirmed_at.unwrap_or(event.updated_at))
}

fn match_existing_event<'a>(
    events: &'a [PolymarketUserTradeEventRecord],
    upsert: &PolymarketUserTradeEventUpsert,
) -> Option<&'a PolymarketUserTradeEventRecord> {
    if let Some(run_id) = upsert.run_id.as_deref() {
        if let Some(found) = events
            .iter()
            .find(|event| event.run_id.as_deref() == Some(run_id))
        {
            return Some(found);
        }
    }
    if let Some(external_order_id) = upsert.external_order_id.as_deref() {
        if let Some(found) = events
            .iter()
            .find(|event| event.external_order_id.as_deref() == Some(external_order_id))
        {
            return Some(found);
        }
    }

    let refs = PolymarketReferenceCandidates {
        tx_hashes: trim_to_option(upsert.tx_hash.as_deref())
            .into_iter()
            .collect(),
        provider_order_refs: [
            trim_to_option(upsert.provider_order_id.as_deref()),
            trim_to_option(upsert.taker_hash.as_deref()),
        ]
        .into_iter()
        .flatten()
        .collect(),
        builder_trade_refs: trim_to_option(upsert.builder_trade_id.as_deref())
            .into_iter()
            .collect(),
    };

    events
        .iter()
        .filter_map(|event| {
            let (score, timestamp) = reference_score(event, &refs);
            (score > 0).then_some((score, timestamp, event))
        })
        .max_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)))
        .map(|(_, _, event)| event)
}

fn parse_index_state_row(
    row: sqlx::postgres::PgRow,
) -> Result<PolymarketIndexStateRecord, ApiError> {
    Ok(PolymarketIndexStateRecord {
        lane: row
            .try_get("lane")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        market_id: row
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider_market_ref: row
            .try_get("provider_market_ref")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        index_status: row
            .try_get("index_status")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        indexed_from: row
            .try_get("indexed_from")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        indexed_through: row
            .try_get("indexed_through")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        is_partial_backfill: row
            .try_get("is_partial_backfill")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        last_error: row
            .try_get("last_error")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn parse_user_trade_event_row(
    row: sqlx::postgres::PgRow,
) -> Result<PolymarketUserTradeEventRecord, ApiError> {
    let block_number: Option<i64> = row
        .try_get("block_number")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let price_bps: Option<i64> = row
        .try_get("price_bps")
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    let attempt_count: i32 = row
        .try_get("attempt_count")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(PolymarketUserTradeEventRecord {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        agent_id: row
            .try_get("agent_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        run_id: row
            .try_get("run_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        external_order_id: row
            .try_get("external_order_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        owner: row
            .try_get("owner")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        market_id: row
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider_market_ref: row
            .try_get("provider_market_ref")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider_order_id: row
            .try_get("provider_order_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        builder_trade_id: row
            .try_get("builder_trade_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        taker_hash: row
            .try_get("taker_hash")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        tx_hash: row
            .try_get("tx_hash")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        block_number: block_number.map(|value| value.max(0) as u64),
        outcome: row
            .try_get("outcome")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        side: row
            .try_get("side")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        price: row
            .try_get("price")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        price_bps: price_bps.map(|value| value.max(0) as u64),
        requested_quantity: row
            .try_get("requested_quantity")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        filled_quantity: row
            .try_get("filled_quantity")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        fee_usdc: row
            .try_get("fee_usdc")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        lifecycle_status: row
            .try_get("lifecycle_status")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        attempt_count: attempt_count.max(0) as u32,
        last_error: row
            .try_get("last_error")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        raw_payload: row
            .try_get("raw_payload")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        matched_at: row
            .try_get("matched_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        mined_at: row
            .try_get("mined_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        confirmed_at: row
            .try_get("confirmed_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        created_at: row
            .try_get("created_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn empty_public_snapshot(
    _market_id: &str,
    provider_market_ref: &str,
    outcome: Option<&str>,
    limit: u64,
    offset: u64,
    index_state: Option<&PolymarketIndexStateRecord>,
) -> ExternalTradesSnapshot {
    let default_status = if outcome.is_some() {
        "pending"
    } else {
        "empty"
    };
    ExternalTradesSnapshot {
        trades: Vec::new(),
        total: 0,
        limit,
        offset,
        has_more: false,
        source: POLYMARKET_SOURCE.to_string(),
        provider: POLYMARKET_PROVIDER.to_string(),
        chain_id: POLYMARKET_CHAIN_ID,
        provider_market_ref: provider_market_ref.to_string(),
        is_synthetic: false,
        index_status: Some(
            index_state
                .map(|state| state.index_status.clone())
                .unwrap_or_else(|| default_status.to_string()),
        ),
        indexed_from: index_state
            .and_then(|state| state.indexed_from)
            .map(|value| value.to_rfc3339()),
        indexed_through: index_state
            .and_then(|state| state.indexed_through)
            .map(|value| value.to_rfc3339()),
        is_partial_backfill: index_state
            .map(|state| state.is_partial_backfill)
            .unwrap_or(true),
    }
}

pub async fn upsert_index_state(
    lane: PolymarketIndexLane,
    market_id: &str,
    provider_market_ref: &str,
    index_status: &str,
    indexed_from: Option<DateTime<Utc>>,
    indexed_through: Option<DateTime<Utc>>,
    is_partial_backfill: bool,
    last_error: Option<&str>,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO polymarket_index_state (
            lane, market_id, provider_market_ref, index_status,
            indexed_from, indexed_through, is_partial_backfill, last_error, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW())
         ON CONFLICT (lane, market_id) DO UPDATE SET
             provider_market_ref = EXCLUDED.provider_market_ref,
             index_status = EXCLUDED.index_status,
             indexed_from = EXCLUDED.indexed_from,
             indexed_through = EXCLUDED.indexed_through,
             is_partial_backfill = EXCLUDED.is_partial_backfill,
             last_error = EXCLUDED.last_error,
             updated_at = NOW()",
    )
    .bind(lane.as_str())
    .bind(market_id)
    .bind(provider_market_ref)
    .bind(index_status)
    .bind(indexed_from)
    .bind(indexed_through)
    .bind(is_partial_backfill)
    .bind(last_error)
    .execute(pool()?)
    .await
    .map_err(|err| {
        ApiError::internal(&format!("failed to upsert polymarket index state: {err}"))
    })?;

    Ok(())
}

pub async fn upsert_public_trades(trades: &[PolymarketPublicTradeUpsert]) -> Result<u64, ApiError> {
    if trades.is_empty() {
        return Ok(0);
    }

    let mut inserted = 0_u64;
    let pool = pool()?;
    for trade in trades {
        sqlx::query(
            "INSERT INTO polymarket_public_trades (
                id, provider_trade_id, market_id, provider_market_ref, outcome, side,
                market_category, price, price_bps, quantity, tx_hash, block_number, token_id, maker, taker,
                match_time, raw_payload, ingested_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,NOW())
             ON CONFLICT (provider_trade_id) DO UPDATE SET
                 market_id = EXCLUDED.market_id,
                 provider_market_ref = EXCLUDED.provider_market_ref,
                 outcome = EXCLUDED.outcome,
                 side = EXCLUDED.side,
                 market_category = EXCLUDED.market_category,
                 price = EXCLUDED.price,
                 price_bps = EXCLUDED.price_bps,
                 quantity = EXCLUDED.quantity,
                 tx_hash = EXCLUDED.tx_hash,
                 block_number = EXCLUDED.block_number,
                 token_id = EXCLUDED.token_id,
                 maker = EXCLUDED.maker,
                 taker = EXCLUDED.taker,
                 match_time = EXCLUDED.match_time,
                 raw_payload = EXCLUDED.raw_payload,
                 ingested_at = NOW()",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(trade.provider_trade_id.as_str())
        .bind(trade.market_id.as_str())
        .bind(trade.provider_market_ref.as_str())
        .bind(trade.outcome.as_str())
        .bind(trade.side.as_deref())
        .bind(trade.market_category.as_deref())
        .bind(clamp_probability(trade.price))
        .bind(price_to_bps(trade.price) as i64)
        .bind(trade.quantity.max(0.0))
        .bind(trade.tx_hash.as_deref())
        .bind(trade.block_number.map(|value| value as i64))
        .bind(trade.token_id.as_deref())
        .bind(trade.maker.as_deref())
        .bind(trade.taker.as_deref())
        .bind(trade.match_time)
        .bind(&trade.raw_payload)
        .execute(pool)
        .await
        .map_err(|err| {
            ApiError::internal(&format!("failed to upsert polymarket public trade: {err}"))
        })?;
        inserted += 1;
    }

    Ok(inserted)
}

pub async fn fetch_public_trades(
    provider_market_ref: &str,
    outcome: Option<&str>,
    limit: u64,
    offset: u64,
) -> Result<ExternalTradesSnapshot, ApiError> {
    let safe_limit = limit.clamp(1, 200);
    let market_id = format!("polymarket:{}", provider_market_ref.trim());
    let pool = pool()?;

    let index_state = sqlx::query(
        "SELECT lane, market_id, provider_market_ref, index_status, indexed_from, indexed_through,
                is_partial_backfill, last_error, updated_at
         FROM polymarket_index_state
         WHERE lane = $1 AND market_id = $2",
    )
    .bind(PolymarketIndexLane::PublicTape.as_str())
    .bind(market_id.as_str())
    .fetch_optional(pool)
    .await
    .map_err(|err| ApiError::internal(&format!("failed to load polymarket index state: {err}")))?
    .map(parse_index_state_row)
    .transpose()?;

    let total_row = if let Some(outcome) = outcome {
        sqlx::query(
            "SELECT COUNT(*) AS total
             FROM polymarket_public_trades
             WHERE market_id = $1 AND outcome = $2",
        )
        .bind(market_id.as_str())
        .bind(outcome)
        .fetch_one(pool)
        .await
    } else {
        sqlx::query(
            "SELECT COUNT(*) AS total
             FROM polymarket_public_trades
             WHERE market_id = $1",
        )
        .bind(market_id.as_str())
        .fetch_one(pool)
        .await
    }
    .map_err(|err| {
        ApiError::internal(&format!("failed to count polymarket public trades: {err}"))
    })?;
    let total: i64 = total_row
        .try_get("total")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    if total <= 0 {
        return Ok(empty_public_snapshot(
            market_id.as_str(),
            provider_market_ref,
            outcome,
            safe_limit,
            offset,
            index_state.as_ref(),
        ));
    }

    let rows = if let Some(outcome) = outcome {
        sqlx::query(
            "SELECT provider_trade_id, outcome, price, price_bps, quantity, tx_hash, block_number, match_time
             FROM polymarket_public_trades
             WHERE market_id = $1 AND outcome = $2
             ORDER BY match_time DESC, block_number DESC NULLS LAST, provider_trade_id DESC
             LIMIT $3 OFFSET $4",
        )
        .bind(market_id.as_str())
        .bind(outcome)
        .bind(safe_limit as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT provider_trade_id, outcome, price, price_bps, quantity, tx_hash, block_number, match_time
             FROM polymarket_public_trades
             WHERE market_id = $1
             ORDER BY match_time DESC, block_number DESC NULLS LAST, provider_trade_id DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(market_id.as_str())
        .bind(safe_limit as i64)
        .bind(offset as i64)
        .fetch_all(pool)
        .await
    }
    .map_err(|err| ApiError::internal(&format!("failed to load polymarket public trades: {err}")))?;

    let trades = rows
        .into_iter()
        .map(|row| {
            let quantity: f64 = row
                .try_get("quantity")
                .map_err(|err| ApiError::internal(&err.to_string()))?;
            let block_number: Option<i64> = row
                .try_get("block_number")
                .map_err(|err| ApiError::internal(&err.to_string()))?;
            let price_bps: i64 = row
                .try_get("price_bps")
                .map_err(|err| ApiError::internal(&err.to_string()))?;
            let match_time: DateTime<Utc> = row
                .try_get("match_time")
                .map_err(|err| ApiError::internal(&err.to_string()))?;
            Ok(ExternalTradeSnapshot {
                id: format!(
                    "polymarket:{}",
                    row.try_get::<String, _>("provider_trade_id")
                        .map_err(|err| ApiError::internal(&err.to_string()))?
                ),
                market_id: market_id.clone(),
                outcome: row
                    .try_get("outcome")
                    .map_err(|err| ApiError::internal(&err.to_string()))?,
                price: row
                    .try_get("price")
                    .map_err(|err| ApiError::internal(&err.to_string()))?,
                price_bps: price_bps.max(0) as u64,
                quantity: quantity.round().clamp(0.0, u64::MAX as f64) as u64,
                tx_hash: row
                    .try_get("tx_hash")
                    .map_err(|err| ApiError::internal(&err.to_string()))?,
                block_number: block_number.unwrap_or_default().max(0) as u64,
                created_at: match_time.to_rfc3339(),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    Ok(ExternalTradesSnapshot {
        has_more: (offset + safe_limit) < total.max(0) as u64,
        trades,
        total: total.max(0) as u64,
        limit: safe_limit,
        offset,
        source: POLYMARKET_SOURCE.to_string(),
        provider: POLYMARKET_PROVIDER.to_string(),
        chain_id: POLYMARKET_CHAIN_ID,
        provider_market_ref: provider_market_ref.to_string(),
        is_synthetic: false,
        index_status: Some(
            index_state
                .as_ref()
                .map(|state| state.index_status.clone())
                .unwrap_or_else(|| "partial".to_string()),
        ),
        indexed_from: index_state
            .as_ref()
            .and_then(|state| state.indexed_from)
            .map(|value| value.to_rfc3339()),
        indexed_through: index_state
            .as_ref()
            .and_then(|state| state.indexed_through)
            .map(|value| value.to_rfc3339()),
        is_partial_backfill: index_state
            .as_ref()
            .map(|state| state.is_partial_backfill)
            .unwrap_or(true),
    })
}

async fn load_candidate_user_trade_events(
    market_id: &str,
) -> Result<Vec<PolymarketUserTradeEventRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT
            id, agent_id, run_id, external_order_id, owner, market_id, provider_market_ref,
            provider_order_id, builder_trade_id, taker_hash, tx_hash, block_number,
            outcome, side, price, price_bps, requested_quantity, filled_quantity, fee_usdc,
            lifecycle_status, attempt_count, last_error, raw_payload, matched_at, mined_at,
            confirmed_at, created_at, updated_at
         FROM polymarket_user_trade_events
         WHERE market_id = $1",
    )
    .bind(market_id)
    .fetch_all(pool()?)
    .await
    .map_err(|err| {
        ApiError::internal(&format!(
            "failed to load polymarket user trade events: {err}"
        ))
    })?;

    rows.into_iter().map(parse_user_trade_event_row).collect()
}

pub async fn upsert_user_trade_event(
    upsert: &PolymarketUserTradeEventUpsert,
) -> Result<PolymarketUserTradeEventRecord, ApiError> {
    let pool = pool()?;
    let candidates = load_candidate_user_trade_events(upsert.market_id.as_str()).await?;
    let existing = match_existing_event(&candidates, upsert);
    let now = upsert.observed_at.unwrap_or_else(Utc::now);

    let id = existing
        .map(|event| event.id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let current_status = existing
        .map(|event| event.lifecycle_status.as_str())
        .unwrap_or("MATCHED");
    let lifecycle_status = merged_lifecycle_status(current_status, upsert.lifecycle_status);
    let matched_at = existing.and_then(|event| event.matched_at).or_else(|| {
        matches!(
            upsert.lifecycle_status,
            PolymarketTradeLifecycleStatus::Matched
                | PolymarketTradeLifecycleStatus::Mined
                | PolymarketTradeLifecycleStatus::Confirmed
        )
        .then_some(now)
    });
    let mined_at = existing.and_then(|event| event.mined_at).or_else(|| {
        matches!(
            upsert.lifecycle_status,
            PolymarketTradeLifecycleStatus::Mined | PolymarketTradeLifecycleStatus::Confirmed
        )
        .then_some(now)
    });
    let confirmed_at = existing.and_then(|event| event.confirmed_at).or_else(|| {
        (upsert.lifecycle_status == PolymarketTradeLifecycleStatus::Confirmed).then_some(now)
    });
    let raw_payload = if existing.is_some() {
        json!({
            "latest": upsert.raw_payload,
            "history": [
                existing.map(|event| event.raw_payload.clone()).unwrap_or_else(|| json!({})),
                upsert.raw_payload.clone()
            ]
        })
    } else {
        upsert.raw_payload.clone()
    };

    sqlx::query(
        "INSERT INTO polymarket_user_trade_events (
            id, agent_id, run_id, external_order_id, owner, market_id, provider_market_ref,
            provider_order_id, builder_trade_id, taker_hash, tx_hash, block_number,
            outcome, side, price, price_bps, requested_quantity, filled_quantity, fee_usdc,
            lifecycle_status, attempt_count, last_error, raw_payload, matched_at, mined_at,
            confirmed_at, created_at, updated_at
        ) VALUES (
            $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,
            $20,$21,$22,$23,$24,$25,$26,
            COALESCE((SELECT created_at FROM polymarket_user_trade_events WHERE id = $1), NOW()),
            NOW()
        )
        ON CONFLICT (id) DO UPDATE SET
            agent_id = EXCLUDED.agent_id,
            run_id = EXCLUDED.run_id,
            external_order_id = EXCLUDED.external_order_id,
            owner = EXCLUDED.owner,
            market_id = EXCLUDED.market_id,
            provider_market_ref = EXCLUDED.provider_market_ref,
            provider_order_id = EXCLUDED.provider_order_id,
            builder_trade_id = EXCLUDED.builder_trade_id,
            taker_hash = EXCLUDED.taker_hash,
            tx_hash = EXCLUDED.tx_hash,
            block_number = EXCLUDED.block_number,
            outcome = EXCLUDED.outcome,
            side = EXCLUDED.side,
            price = EXCLUDED.price,
            price_bps = EXCLUDED.price_bps,
            requested_quantity = EXCLUDED.requested_quantity,
            filled_quantity = EXCLUDED.filled_quantity,
            fee_usdc = EXCLUDED.fee_usdc,
            lifecycle_status = EXCLUDED.lifecycle_status,
            attempt_count = EXCLUDED.attempt_count,
            last_error = EXCLUDED.last_error,
            raw_payload = EXCLUDED.raw_payload,
            matched_at = COALESCE(polymarket_user_trade_events.matched_at, EXCLUDED.matched_at),
            mined_at = COALESCE(polymarket_user_trade_events.mined_at, EXCLUDED.mined_at),
            confirmed_at = COALESCE(polymarket_user_trade_events.confirmed_at, EXCLUDED.confirmed_at),
            updated_at = NOW()",
    )
    .bind(id.as_str())
    .bind(upsert.agent_id.as_deref().or(existing.and_then(|event| event.agent_id.as_deref())))
    .bind(upsert.run_id.as_deref().or(existing.and_then(|event| event.run_id.as_deref())))
    .bind(
        upsert
            .external_order_id
            .as_deref()
            .or(existing.and_then(|event| event.external_order_id.as_deref())),
    )
    .bind(upsert.owner.as_deref().or(existing.and_then(|event| event.owner.as_deref())))
    .bind(upsert.market_id.as_str())
    .bind(
        upsert
            .provider_market_ref
            .as_deref()
            .or(existing.and_then(|event| event.provider_market_ref.as_deref())),
    )
    .bind(
        upsert
            .provider_order_id
            .as_deref()
            .or(existing.and_then(|event| event.provider_order_id.as_deref())),
    )
    .bind(
        upsert
            .builder_trade_id
            .as_deref()
            .or(existing.and_then(|event| event.builder_trade_id.as_deref())),
    )
    .bind(
        upsert
            .taker_hash
            .as_deref()
            .or(existing.and_then(|event| event.taker_hash.as_deref())),
    )
    .bind(upsert.tx_hash.as_deref().or(existing.and_then(|event| event.tx_hash.as_deref())))
    .bind(
        upsert
            .block_number
            .map(|value| value as i64)
            .or(existing.and_then(|event| event.block_number.map(|value| value as i64))),
    )
    .bind(upsert.outcome.as_deref().or(existing.and_then(|event| event.outcome.as_deref())))
    .bind(upsert.side.as_deref().or(existing.and_then(|event| event.side.as_deref())))
    .bind(upsert.price.or(existing.and_then(|event| event.price)))
    .bind(
        upsert
            .price
            .map(price_to_bps)
            .or(existing.and_then(|event| event.price_bps))
            .map(|value| value as i64),
    )
    .bind(
        upsert
            .requested_quantity
            .or(existing.and_then(|event| event.requested_quantity)),
    )
    .bind(
        upsert
            .filled_quantity
            .or(existing.and_then(|event| event.filled_quantity)),
    )
    .bind(
        upsert
            .fee_usdc
            .unwrap_or_else(|| existing.map(|event| event.fee_usdc).unwrap_or(0.0)),
    )
    .bind(lifecycle_status.as_str())
    .bind(
        if upsert.attempt_count > 0 {
            upsert.attempt_count as i32
        } else {
            existing.map(|event| event.attempt_count as i32).unwrap_or(0)
                + matches!(
                    upsert.lifecycle_status,
                    PolymarketTradeLifecycleStatus::Retrying | PolymarketTradeLifecycleStatus::Failed
                ) as i32
        },
    )
    .bind(
        upsert
            .last_error
            .as_deref()
            .or(existing.and_then(|event| event.last_error.as_deref())),
    )
    .bind(raw_payload)
    .bind(matched_at)
    .bind(mined_at)
    .bind(confirmed_at)
    .execute(pool)
    .await
    .map_err(|err| ApiError::internal(&format!("failed to upsert polymarket user trade event: {err}")))?;

    let row = sqlx::query(
        "SELECT
            id, agent_id, run_id, external_order_id, owner, market_id, provider_market_ref,
            provider_order_id, builder_trade_id, taker_hash, tx_hash, block_number,
            outcome, side, price, price_bps, requested_quantity, filled_quantity, fee_usdc,
            lifecycle_status, attempt_count, last_error, raw_payload, matched_at, mined_at,
            confirmed_at, created_at, updated_at
         FROM polymarket_user_trade_events
         WHERE id = $1",
    )
    .bind(id.as_str())
    .fetch_one(pool)
    .await
    .map_err(|err| {
        ApiError::internal(&format!(
            "failed to reload polymarket user trade event: {err}"
        ))
    })?;

    parse_user_trade_event_row(row)
}

pub async fn load_confirmed_user_fill(
    market_id: &str,
    run_id: &str,
    external_order_id: &str,
    refs: &PolymarketReferenceCandidates,
) -> Result<Option<PolymarketUserTradeEventRecord>, ApiError> {
    let candidates = load_candidate_user_trade_events(market_id).await?;
    let best = candidates
        .into_iter()
        .filter(|event| event.lifecycle_status == "CONFIRMED")
        .filter(|event| {
            event.run_id.as_deref() == Some(run_id)
                || event.external_order_id.as_deref() == Some(external_order_id)
                || reference_score(event, refs).0 > 0
        })
        .max_by(|left, right| {
            let left_score = reference_score(left, refs);
            let right_score = reference_score(right, refs);
            left_score
                .0
                .cmp(&right_score.0)
                .then_with(|| left_score.1.cmp(&right_score.1))
        });

    Ok(best)
}

fn parse_snapshot_levels(raw: Value) -> Result<Vec<ExternalOrderBookLevel>, ApiError> {
    serde_json::from_value(raw).map_err(|err| ApiError::internal(&err.to_string()))
}

fn parse_public_trade_record(
    row: sqlx::postgres::PgRow,
) -> Result<PolymarketPublicTradeRecord, ApiError> {
    Ok(PolymarketPublicTradeRecord {
        provider_trade_id: row
            .try_get("provider_trade_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        market_id: row
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider_market_ref: row
            .try_get("provider_market_ref")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        market_category: row.try_get("market_category").ok(),
        outcome: row
            .try_get("outcome")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        side: row.try_get("side").ok(),
        price: row
            .try_get("price")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        price_bps: row.try_get::<i64, _>("price_bps").unwrap_or(0).max(0) as u64,
        quantity: row
            .try_get("quantity")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        maker: row.try_get("maker").ok(),
        taker: row.try_get("taker").ok(),
        tx_hash: row.try_get("tx_hash").ok(),
        block_number: row
            .try_get::<Option<i64>, _>("block_number")
            .ok()
            .flatten()
            .map(|value| value.max(0) as u64),
        match_time: row
            .try_get("match_time")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        ingested_at: row
            .try_get("ingested_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn parse_orderbook_history_row(
    row: sqlx::postgres::PgRow,
) -> Result<PolymarketOrderbookHistoryRecord, ApiError> {
    Ok(PolymarketOrderbookHistoryRecord {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        market_id: row
            .try_get("market_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        provider_market_ref: row
            .try_get("provider_market_ref")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        outcome: row
            .try_get("outcome")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        depth: row.try_get::<i32, _>("depth").unwrap_or(0).max(0) as u64,
        best_bid: row.try_get("best_bid").ok(),
        best_ask: row.try_get("best_ask").ok(),
        mid_price: row.try_get("mid_price").ok(),
        bids: parse_snapshot_levels(row.try_get("bids").unwrap_or_else(|_| json!([])))?,
        asks: parse_snapshot_levels(row.try_get("asks").unwrap_or_else(|_| json!([])))?,
        source: row
            .try_get("source")
            .unwrap_or_else(|_| POLYMARKET_SOURCE.to_string()),
        captured_at: row
            .try_get("captured_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn parse_wallet_score_row(
    row: sqlx::postgres::PgRow,
) -> Result<PolymarketWalletScoreRecord, ApiError> {
    Ok(PolymarketWalletScoreRecord {
        wallet: row
            .try_get("wallet")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        market_category: row.try_get("market_category").ok(),
        window_hours: row.try_get::<i32, _>("window_hours").unwrap_or(0).max(0) as u64,
        trade_count: row.try_get::<i64, _>("trade_count").unwrap_or(0).max(0) as u64,
        markets_traded: row.try_get::<i64, _>("markets_traded").unwrap_or(0).max(0) as u64,
        recency_score: row.try_get("recency_score").unwrap_or(0.0),
        consistency_score: row.try_get("consistency_score").unwrap_or(0.0),
        specialization_score: row.try_get("specialization_score").unwrap_or(0.0),
        crowding_penalty: row.try_get("crowding_penalty").unwrap_or(0.0),
        edge_persistence_score: row.try_get("edge_persistence_score").unwrap_or(0.0),
        composite_score: row.try_get("composite_score").unwrap_or(0.0),
        last_trade_at: row.try_get("last_trade_at").ok(),
        metrics: row.try_get("metrics").unwrap_or_else(|_| json!({})),
        updated_at: row
            .try_get("updated_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn parse_strategy_replay_run_row(
    row: sqlx::postgres::PgRow,
) -> Result<StrategyReplayRunRecord, ApiError> {
    Ok(StrategyReplayRunRecord {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        created_by: row.try_get("created_by").ok(),
        strategy: row
            .try_get("strategy")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        baseline: row.try_get("baseline").ok(),
        status: row
            .try_get("status")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        market_id: row.try_get("market_id").ok(),
        market_category: row.try_get("market_category").ok(),
        target_wallet: row.try_get("target_wallet").ok(),
        delay_ms: row.try_get::<i32, _>("delay_ms").unwrap_or(0).max(0) as u64,
        window_hours: row.try_get::<i32, _>("window_hours").unwrap_or(0).max(0) as u64,
        input_params: row.try_get("input_params").unwrap_or_else(|_| json!({})),
        summary: row.try_get("summary").unwrap_or_else(|_| json!({})),
        created_at: row
            .try_get("created_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        completed_at: row.try_get("completed_at").ok(),
        updated_at: row
            .try_get("updated_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn parse_strategy_replay_fill_row(
    row: sqlx::postgres::PgRow,
) -> Result<StrategyReplayFillRecord, ApiError> {
    Ok(StrategyReplayFillRecord {
        id: row
            .try_get("id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        replay_run_id: row
            .try_get("replay_run_id")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        event_time: row
            .try_get("event_time")
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
        target_wallet: row.try_get("target_wallet").ok(),
        followed_trade_id: row.try_get("followed_trade_id").ok(),
        requested_quantity: row.try_get("requested_quantity").unwrap_or(0.0),
        filled_quantity: row.try_get("filled_quantity").unwrap_or(0.0),
        price: row.try_get("price").unwrap_or(0.0),
        mark_price: row.try_get("mark_price").unwrap_or(0.0),
        fee_usdc: row.try_get("fee_usdc").unwrap_or(0.0),
        pnl_usdc: row.try_get("pnl_usdc").unwrap_or(0.0),
        slippage_ticks: row.try_get("slippage_ticks").unwrap_or(0.0),
        metadata: row.try_get("metadata").unwrap_or_else(|_| json!({})),
        created_at: row
            .try_get("created_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

fn normalize_category(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn normalize_replay_strategy(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace('_', "-")
}

fn round5(value: f64) -> f64 {
    (value * 100_000.0).round() / 100_000.0
}

fn polymarket_fee_rate(category: Option<&str>) -> f64 {
    match normalize_category(category).as_deref() {
        Some("crypto") => 0.0072,
        Some("sports") => 0.003,
        Some("finance") | Some("politics") | Some("mentions") | Some("tech") => 0.004,
        Some("economics") | Some("culture") | Some("weather") => 0.005,
        Some("geopolitics") => 0.0,
        _ => 0.005,
    }
}

fn polymarket_taker_fee_usdc(category: Option<&str>, price: f64, quantity: f64) -> f64 {
    let p = clamp_probability(price);
    let fee = quantity.max(0.0) * polymarket_fee_rate(category) * p * (1.0 - p);
    round5(fee.max(0.0))
}

fn infer_tick_size(levels: &[ExternalOrderBookLevel]) -> Option<f64> {
    let mut prices = levels
        .iter()
        .map(|level| clamp_probability(level.price))
        .filter(|price| *price > 0.0)
        .collect::<Vec<_>>();
    prices.sort_by(|left, right| left.total_cmp(right));
    prices
        .windows(2)
        .filter_map(|window| {
            let delta = (window[1] - window[0]).abs();
            (delta > 0.0).then_some(delta)
        })
        .min_by(|left, right| left.total_cmp(right))
}

fn replay_tick_size(snapshot: &PolymarketOrderbookHistoryRecord) -> f64 {
    infer_tick_size(&snapshot.bids)
        .into_iter()
        .chain(infer_tick_size(&snapshot.asks))
        .min_by(|left, right| left.total_cmp(right))
        .unwrap_or(0.01)
}

fn synthetic_market_snapshot(
    snapshot: &PolymarketOrderbookHistoryRecord,
    category: Option<&str>,
    outcome: &str,
    fallback_price: f64,
) -> ExternalMarketSnapshot {
    let outcome_mid = snapshot
        .mid_price
        .or_else(|| match (snapshot.best_bid, snapshot.best_ask) {
            (Some(bid), Some(ask)) => Some((bid + ask) / 2.0),
            (Some(bid), None) => Some(bid),
            (None, Some(ask)) => Some(ask),
            _ => None,
        })
        .unwrap_or_else(|| clamp_probability(fallback_price));
    let (yes_price, no_price) = if outcome.eq_ignore_ascii_case("no") {
        (
            clamp_probability(1.0 - outcome_mid),
            clamp_probability(outcome_mid),
        )
    } else {
        (
            clamp_probability(outcome_mid),
            clamp_probability(1.0 - outcome_mid),
        )
    };

    ExternalMarketSnapshot {
        id: snapshot.market_id.clone(),
        question: snapshot.market_id.clone(),
        description: "replay".to_string(),
        category: category.unwrap_or("other").to_string(),
        status: "active".to_string(),
        close_time: 0,
        resolved: false,
        outcome: None,
        yes_price,
        no_price,
        volume: 0.0,
        source: POLYMARKET_SOURCE.to_string(),
        provider: POLYMARKET_PROVIDER.to_string(),
        is_external: true,
        external_url: String::new(),
        chain_id: POLYMARKET_CHAIN_ID,
        requires_credentials: false,
        execution_users: true,
        execution_agents: true,
        outcomes: Vec::new(),
        provider_market_ref: snapshot.provider_market_ref.clone(),
    }
}

fn synthetic_orderbook_snapshot(
    snapshot: &PolymarketOrderbookHistoryRecord,
) -> ExternalOrderBookSnapshot {
    ExternalOrderBookSnapshot {
        market_id: snapshot.market_id.clone(),
        outcome: snapshot.outcome.clone(),
        bids: snapshot.bids.clone(),
        asks: snapshot.asks.clone(),
        last_updated: snapshot.captured_at.to_rfc3339(),
        source: snapshot.source.clone(),
        provider: POLYMARKET_PROVIDER.to_string(),
        chain_id: POLYMARKET_CHAIN_ID,
        provider_market_ref: snapshot.provider_market_ref.clone(),
        is_synthetic: false,
    }
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|left, right| left.total_cmp(right));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[mid - 1] + values[mid]) / 2.0)
    } else {
        Some(values[mid])
    }
}

fn max_drawdown(points: &[f64]) -> f64 {
    let mut peak = 0.0;
    let mut equity = 0.0;
    let mut worst = 0.0;
    for value in points {
        equity += *value;
        if equity > peak {
            peak = equity;
        }
        let drawdown = peak - equity;
        if drawdown > worst {
            worst = drawdown;
        }
    }
    worst
}

pub async fn record_orderbook_snapshot(
    snapshot: &ExternalOrderBookSnapshot,
) -> Result<(), ApiError> {
    if !snapshot.provider.eq_ignore_ascii_case(POLYMARKET_PROVIDER) {
        return Ok(());
    }

    let captured_at = DateTime::parse_from_rfc3339(snapshot.last_updated.as_str())
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let best_bid = snapshot
        .bids
        .iter()
        .map(|level| clamp_probability(level.price))
        .max_by(|left, right| left.total_cmp(right));
    let best_ask = snapshot
        .asks
        .iter()
        .map(|level| clamp_probability(level.price))
        .min_by(|left, right| left.total_cmp(right));
    let mid_price = match (best_bid, best_ask) {
        (Some(bid), Some(ask)) => Some(clamp_probability((bid + ask) / 2.0)),
        (Some(bid), None) => Some(clamp_probability(bid)),
        (None, Some(ask)) => Some(clamp_probability(ask)),
        _ => None,
    };

    sqlx::query(
        "INSERT INTO polymarket_public_orderbook_snapshots (
            id, market_id, provider_market_ref, outcome, depth, best_bid, best_ask, mid_price,
            bids, asks, source, captured_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(snapshot.market_id.as_str())
    .bind(snapshot.provider_market_ref.as_str())
    .bind(snapshot.outcome.as_str())
    .bind(snapshot.bids.len().max(snapshot.asks.len()) as i32)
    .bind(best_bid)
    .bind(best_ask)
    .bind(mid_price)
    .bind(serde_json::to_value(&snapshot.bids).map_err(|err| ApiError::internal(&err.to_string()))?)
    .bind(serde_json::to_value(&snapshot.asks).map_err(|err| ApiError::internal(&err.to_string()))?)
    .bind(snapshot.source.as_str())
    .bind(captured_at)
    .execute(pool()?)
    .await
    .map_err(|err| ApiError::internal(&format!("failed to persist orderbook snapshot: {err}")))?;

    Ok(())
}

pub async fn fetch_orderbook_history(
    market_id: Option<&str>,
    outcome: Option<&str>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    limit: u64,
) -> Result<Vec<PolymarketOrderbookHistoryRecord>, ApiError> {
    let safe_limit = limit.clamp(1, 500);
    let mut query = QueryBuilder::new(
        "SELECT id, market_id, provider_market_ref, outcome, depth, best_bid, best_ask, mid_price,
                bids, asks, source, captured_at
         FROM polymarket_public_orderbook_snapshots
         WHERE TRUE",
    );

    if let Some(market_id) = market_id {
        query.push(" AND market_id = ").push_bind(market_id);
    }
    if let Some(outcome) = outcome {
        query.push(" AND outcome = ").push_bind(outcome);
    }
    if let Some(from) = from {
        query.push(" AND captured_at >= ").push_bind(from);
    }
    if let Some(to) = to {
        query.push(" AND captured_at <= ").push_bind(to);
    }

    query
        .push(" ORDER BY captured_at DESC, id DESC LIMIT ")
        .push_bind(safe_limit as i64);

    let rows =
        query.build().fetch_all(pool()?).await.map_err(|err| {
            ApiError::internal(&format!("failed to load orderbook history: {err}"))
        })?;

    rows.into_iter().map(parse_orderbook_history_row).collect()
}

pub async fn query_public_trades(
    market_id: Option<&str>,
    wallet: Option<&str>,
    limit: u64,
    offset: u64,
) -> Result<PolymarketPublicTradesPage, ApiError> {
    let safe_limit = limit.clamp(1, 500);
    let normalized_wallet = wallet
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());

    let mut count_query =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM polymarket_public_trades WHERE TRUE");
    let mut query = QueryBuilder::new(
        "SELECT provider_trade_id, market_id, provider_market_ref, market_category, outcome, side,
                price, price_bps, quantity, maker, taker, tx_hash, block_number, match_time, ingested_at
         FROM polymarket_public_trades
         WHERE TRUE",
    );

    if let Some(market_id) = market_id {
        count_query.push(" AND market_id = ").push_bind(market_id);
        query.push(" AND market_id = ").push_bind(market_id);
    }
    if let Some(wallet) = normalized_wallet.as_deref() {
        count_query.push(" AND (LOWER(COALESCE(maker, '')) = ");
        count_query.push_bind(wallet);
        count_query.push(" OR LOWER(COALESCE(taker, '')) = ");
        count_query.push_bind(wallet);
        count_query.push(")");

        query.push(" AND (LOWER(COALESCE(maker, '')) = ");
        query.push_bind(wallet);
        query.push(" OR LOWER(COALESCE(taker, '')) = ");
        query.push_bind(wallet);
        query.push(")");
    }

    query
        .push(" ORDER BY match_time DESC, provider_trade_id DESC LIMIT ")
        .push_bind(safe_limit as i64)
        .push(" OFFSET ")
        .push_bind(offset as i64);

    let total_row = count_query
        .build()
        .fetch_one(pool()?)
        .await
        .map_err(|err| ApiError::internal(&format!("failed to count public trades: {err}")))?;
    let total = total_row.try_get::<i64, _>("total").unwrap_or(0).max(0) as u64;

    let rows = query
        .build()
        .fetch_all(pool()?)
        .await
        .map_err(|err| ApiError::internal(&format!("failed to load public trades: {err}")))?;

    Ok(PolymarketPublicTradesPage {
        items: rows
            .into_iter()
            .map(parse_public_trade_record)
            .collect::<Result<Vec<_>, ApiError>>()?,
        total,
        limit: safe_limit,
        offset,
    })
}

pub async fn compute_wallet_scores(
    market_category: Option<&str>,
    window_hours: u64,
    limit: u64,
) -> Result<Vec<PolymarketWalletScoreRecord>, ApiError> {
    let safe_limit = limit.clamp(1, 250);
    let safe_window_hours = window_hours.clamp(1, 24 * 30);
    let cutoff = Utc::now() - ChronoDuration::hours(safe_window_hours as i64);
    let normalized_category = normalize_category(market_category);

    let rows = if let Some(category) = normalized_category.as_deref() {
        sqlx::query(
            "SELECT market_id, market_category, quantity, match_time, maker, taker
             FROM polymarket_public_trades
             WHERE match_time >= $1
               AND market_category = $2
             ORDER BY match_time DESC",
        )
        .bind(cutoff)
        .bind(category)
        .fetch_all(pool()?)
        .await
    } else {
        sqlx::query(
            "SELECT market_id, market_category, quantity, match_time, maker, taker
             FROM polymarket_public_trades
             WHERE match_time >= $1
             ORDER BY match_time DESC",
        )
        .bind(cutoff)
        .fetch_all(pool()?)
        .await
    }
    .map_err(|err| ApiError::internal(&format!("failed to load wallet score inputs: {err}")))?;

    #[derive(Default)]
    struct WalletAccumulator {
        trade_count: u64,
        quantities: Vec<f64>,
        markets: HashMap<String, u64>,
        categories: HashMap<String, u64>,
        last_trade_at: Option<DateTime<Utc>>,
    }

    let mut market_popularity = HashMap::<String, u64>::new();
    let mut wallets = HashMap::<String, WalletAccumulator>::new();

    for row in rows {
        let market_id: String = row.try_get("market_id").unwrap_or_default();
        let category: Option<String> = row.try_get("market_category").ok();
        let quantity = row.try_get::<f64, _>("quantity").unwrap_or(0.0).max(0.0);
        let match_time: DateTime<Utc> = row.try_get("match_time").unwrap_or_else(|_| Utc::now());
        *market_popularity.entry(market_id.clone()).or_default() += 1;

        let mut seen = HashSet::new();
        for wallet in [
            row.try_get::<Option<String>, _>("maker").ok().flatten(),
            row.try_get::<Option<String>, _>("taker").ok().flatten(),
        ]
        .into_iter()
        .flatten()
        {
            let normalized = wallet.trim().to_ascii_lowercase();
            if normalized.is_empty() || !seen.insert(normalized.clone()) {
                continue;
            }
            let entry = wallets.entry(normalized).or_default();
            entry.trade_count += 1;
            entry.quantities.push(quantity);
            *entry.markets.entry(market_id.clone()).or_default() += 1;
            if let Some(category) = category.clone() {
                *entry.categories.entry(category).or_default() += 1;
            }
            entry.last_trade_at = Some(
                entry
                    .last_trade_at
                    .map_or(match_time, |current| current.max(match_time)),
            );
        }
    }

    let max_market_popularity = market_popularity.values().copied().max().unwrap_or(1) as f64;
    let now = Utc::now();
    let mut scored = Vec::new();

    for (wallet, acc) in wallets {
        if acc.trade_count == 0 {
            continue;
        }
        let last_trade_at = acc.last_trade_at;
        let hours_since_last = last_trade_at
            .map(|value| (now - value).num_minutes().max(0) as f64 / 60.0)
            .unwrap_or(safe_window_hours as f64);
        let recency_score = (1.0 - (hours_since_last / safe_window_hours as f64)).clamp(0.0, 1.0);
        let avg_quantity = acc.quantities.iter().sum::<f64>() / acc.quantities.len() as f64;
        let variance = if acc.quantities.len() <= 1 {
            0.0
        } else {
            acc.quantities
                .iter()
                .map(|value| (value - avg_quantity).powi(2))
                .sum::<f64>()
                / acc.quantities.len() as f64
        };
        let stddev = variance.sqrt();
        let consistency_score = if avg_quantity <= f64::EPSILON {
            0.0
        } else {
            (1.0 - (stddev / avg_quantity).min(1.0)).clamp(0.0, 1.0)
        };
        let top_market_trades = acc.markets.values().copied().max().unwrap_or(0) as f64;
        let specialization_score = if acc.trade_count == 0 {
            0.0
        } else {
            (top_market_trades / acc.trade_count as f64).clamp(0.0, 1.0)
        };
        let crowding_penalty = if acc.markets.is_empty() {
            0.0
        } else {
            acc.markets
                .keys()
                .map(|market| {
                    *market_popularity.get(market).unwrap_or(&0) as f64 / max_market_popularity
                })
                .sum::<f64>()
                / acc.markets.len() as f64
        };
        let edge_persistence_score = 0.5;
        let composite_score = (0.30 * recency_score
            + 0.25 * consistency_score
            + 0.20 * specialization_score
            + 0.25 * edge_persistence_score
            - 0.15 * crowding_penalty)
            .clamp(0.0, 1.0);

        let dominant_category = acc
            .categories
            .iter()
            .max_by(|left, right| left.1.cmp(right.1))
            .map(|(category, _)| category.clone())
            .or_else(|| normalized_category.clone());
        let metrics = json!({
            "avgQuantity": avg_quantity,
            "quantityStddev": stddev,
            "topMarketShare": specialization_score,
            "crowdingPenalty": crowding_penalty,
            "windowCutoff": cutoff.to_rfc3339(),
        });

        sqlx::query(
            "INSERT INTO polymarket_wallet_scores (
                wallet, market_category, window_hours, trade_count, markets_traded,
                recency_score, consistency_score, specialization_score, crowding_penalty,
                edge_persistence_score, composite_score, last_trade_at, metrics, updated_at
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,NOW())
             ON CONFLICT (wallet) DO UPDATE SET
                market_category = EXCLUDED.market_category,
                window_hours = EXCLUDED.window_hours,
                trade_count = EXCLUDED.trade_count,
                markets_traded = EXCLUDED.markets_traded,
                recency_score = EXCLUDED.recency_score,
                consistency_score = EXCLUDED.consistency_score,
                specialization_score = EXCLUDED.specialization_score,
                crowding_penalty = EXCLUDED.crowding_penalty,
                edge_persistence_score = EXCLUDED.edge_persistence_score,
                composite_score = EXCLUDED.composite_score,
                last_trade_at = EXCLUDED.last_trade_at,
                metrics = EXCLUDED.metrics,
                updated_at = NOW()",
        )
        .bind(wallet.as_str())
        .bind(dominant_category.as_deref())
        .bind(safe_window_hours as i32)
        .bind(acc.trade_count as i64)
        .bind(acc.markets.len() as i64)
        .bind(recency_score)
        .bind(consistency_score)
        .bind(specialization_score)
        .bind(crowding_penalty)
        .bind(edge_persistence_score)
        .bind(composite_score)
        .bind(last_trade_at)
        .bind(metrics.clone())
        .execute(pool()?)
        .await
        .map_err(|err| ApiError::internal(&format!("failed to upsert wallet score: {err}")))?;

        scored.push(PolymarketWalletScoreRecord {
            wallet,
            market_category: dominant_category,
            window_hours: safe_window_hours,
            trade_count: acc.trade_count,
            markets_traded: acc.markets.len() as u64,
            recency_score,
            consistency_score,
            specialization_score,
            crowding_penalty,
            edge_persistence_score,
            composite_score,
            last_trade_at,
            metrics,
            updated_at: now,
        });
    }

    scored.sort_by(|left, right| {
        right
            .composite_score
            .partial_cmp(&left.composite_score)
            .unwrap_or(Ordering::Equal)
    });
    scored.truncate(safe_limit as usize);
    Ok(scored)
}

async fn load_orderbook_snapshot_after(
    market_id: &str,
    outcome: &str,
    at_or_after: DateTime<Utc>,
) -> Result<Option<PolymarketOrderbookHistoryRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT id, market_id, provider_market_ref, outcome, depth, best_bid, best_ask, mid_price,
                bids, asks, source, captured_at
         FROM polymarket_public_orderbook_snapshots
         WHERE market_id = $1
           AND outcome = $2
           AND captured_at >= $3
         ORDER BY captured_at ASC, id ASC
         LIMIT 1",
    )
    .bind(market_id)
    .bind(outcome)
    .bind(at_or_after)
    .fetch_optional(pool()?)
    .await
    .map_err(|err| ApiError::internal(&format!("failed to load replay snapshot: {err}")))?;

    row.map(parse_orderbook_history_row).transpose()
}

async fn load_latest_orderbook_snapshot(
    market_id: &str,
    outcome: &str,
) -> Result<Option<PolymarketOrderbookHistoryRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT id, market_id, provider_market_ref, outcome, depth, best_bid, best_ask, mid_price,
                bids, asks, source, captured_at
         FROM polymarket_public_orderbook_snapshots
         WHERE market_id = $1
           AND outcome = $2
         ORDER BY captured_at DESC, id DESC
         LIMIT 1",
    )
    .bind(market_id)
    .bind(outcome)
    .fetch_optional(pool()?)
    .await
    .map_err(|err| ApiError::internal(&format!("failed to load latest replay snapshot: {err}")))?;

    row.map(parse_orderbook_history_row).transpose()
}

pub async fn run_strategy_replay(
    request: &StrategyReplayRequest,
) -> Result<StrategyReplayRunRecord, ApiError> {
    let strategy = normalize_replay_strategy(request.strategy.as_str());
    if strategy != "wallet-follow-v2" {
        return Err(ApiError::bad_request(
            "UNSUPPORTED_REPLAY_STRATEGY",
            "only wallet_follow_v2 replay is implemented",
        ));
    }

    let target_wallet = request
        .target_wallet
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            ApiError::bad_request(
                "INVALID_REPLAY_TARGET",
                "wallet_follow_v2 replay requires targetWallet",
            )
        })?;
    let safe_delay_ms = request.delay_ms.min(60_000);
    let safe_window_hours = request.window_hours.clamp(1, 24 * 30);
    let safe_follow_ratio = request.follow_ratio.clamp(0.01, 1.0);
    let safe_markout_minutes = request.markout_minutes.clamp(1, 24 * 60);
    let safe_max_trades = request.max_trades.clamp(1, 5_000);
    let cutoff = Utc::now() - ChronoDuration::hours(safe_window_hours as i64);
    let run_id = Uuid::new_v4().to_string();
    let normalized_category = normalize_category(request.market_category.as_deref());

    let mut query = QueryBuilder::new(
        "SELECT provider_trade_id, market_id, provider_market_ref, market_category, outcome, side,
                price, price_bps, quantity, maker, taker, tx_hash, block_number, match_time, ingested_at
         FROM polymarket_public_trades
         WHERE match_time >= ",
    );
    query.push_bind(cutoff);
    query.push(" AND (LOWER(COALESCE(maker, '')) = ");
    query.push_bind(target_wallet.as_str());
    query.push(" OR LOWER(COALESCE(taker, '')) = ");
    query.push_bind(target_wallet.as_str());
    query.push(")");
    if let Some(market_id) = request.market_id.as_deref() {
        query.push(" AND market_id = ").push_bind(market_id);
    }
    if let Some(category) = normalized_category.as_deref() {
        query.push(" AND market_category = ").push_bind(category);
    }
    query
        .push(" ORDER BY match_time ASC, provider_trade_id ASC LIMIT ")
        .push_bind(safe_max_trades as i64);

    let trade_rows = query
        .build()
        .fetch_all(pool()?)
        .await
        .map_err(|err| ApiError::internal(&format!("failed to load replay trades: {err}")))?;
    let trades = trade_rows
        .into_iter()
        .map(parse_public_trade_record)
        .collect::<Result<Vec<_>, ApiError>>()?;

    sqlx::query(
        "INSERT INTO strategy_replay_runs (
            id, created_by, strategy, baseline, status, market_id, market_category, target_wallet,
            delay_ms, window_hours, input_params, summary, created_at, completed_at, updated_at
         ) VALUES ($1,$2,$3,$4,'running',$5,$6,$7,$8,$9,$10,'{}'::jsonb,NOW(),NULL,NOW())",
    )
    .bind(run_id.as_str())
    .bind(request.created_by.as_deref())
    .bind(strategy.as_str())
    .bind(request.baseline.as_deref())
    .bind(request.market_id.as_deref())
    .bind(normalized_category.as_deref())
    .bind(target_wallet.as_str())
    .bind(safe_delay_ms as i32)
    .bind(safe_window_hours as i32)
    .bind(json!({
        "followRatio": safe_follow_ratio,
        "markoutMinutes": safe_markout_minutes,
        "maxTrades": safe_max_trades,
        "requestedMarketId": request.market_id,
        "requestedMarketCategory": normalized_category,
    }))
    .execute(pool()?)
    .await
    .map_err(|err| ApiError::internal(&format!("failed to create replay run: {err}")))?;

    let mut fills = Vec::<StrategyReplayFillRecord>::new();
    let mut pnl_points = Vec::<f64>::new();
    let mut slippage_points = Vec::<f64>::new();

    for trade in trades {
        let side = trade
            .side
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_else(|| "buy".to_string());
        let follow_at = trade.match_time + ChronoDuration::milliseconds(safe_delay_ms as i64);
        let Some(entry_snapshot) = load_orderbook_snapshot_after(
            trade.market_id.as_str(),
            trade.outcome.as_str(),
            follow_at,
        )
        .await?
        .or(load_latest_orderbook_snapshot(
            trade.market_id.as_str(),
            trade.outcome.as_str(),
        )
        .await?) else {
            continue;
        };

        let requested_quantity = trade.quantity.max(0.0) * safe_follow_ratio;
        let market = synthetic_market_snapshot(
            &entry_snapshot,
            trade.market_category.as_deref(),
            trade.outcome.as_str(),
            trade.price,
        );
        let orderbook = synthetic_orderbook_snapshot(&entry_snapshot);
        let mut fill = simulate_fill(
            &market,
            &orderbook,
            trade.outcome.as_str(),
            side.as_str(),
            requested_quantity,
            0,
            None,
        );
        fill.fee_usdc = polymarket_taker_fee_usdc(
            trade.market_category.as_deref(),
            fill.average_price,
            fill.filled_quantity,
        );

        let markout_at =
            entry_snapshot.captured_at + ChronoDuration::minutes(safe_markout_minutes as i64);
        let mark_snapshot = load_orderbook_snapshot_after(
            trade.market_id.as_str(),
            trade.outcome.as_str(),
            markout_at,
        )
        .await?
        .unwrap_or_else(|| entry_snapshot.clone());
        let mark_price = mark_snapshot.mid_price.unwrap_or(fill.mark_price);
        let pnl_usdc = if side == "sell" {
            (fill.average_price - mark_price) * fill.filled_quantity - fill.fee_usdc
        } else {
            (mark_price - fill.average_price) * fill.filled_quantity - fill.fee_usdc
        };
        let slippage_ticks = if replay_tick_size(&entry_snapshot) > 0.0 {
            ((fill.average_price - trade.price).abs() / replay_tick_size(&entry_snapshot)).abs()
        } else {
            0.0
        };

        let record = StrategyReplayFillRecord {
            id: Uuid::new_v4().to_string(),
            replay_run_id: run_id.clone(),
            event_time: trade.match_time,
            market_id: trade.market_id.clone(),
            outcome: trade.outcome.clone(),
            side: side.clone(),
            target_wallet: Some(target_wallet.clone()),
            followed_trade_id: Some(trade.provider_trade_id.clone()),
            requested_quantity,
            filled_quantity: fill.filled_quantity,
            price: fill.average_price,
            mark_price,
            fee_usdc: fill.fee_usdc,
            pnl_usdc,
            slippage_ticks,
            metadata: json!({
                "sourcePrice": trade.price,
                "marketCategory": trade.market_category,
                "entrySnapshotAt": entry_snapshot.captured_at.to_rfc3339(),
                "markSnapshotAt": mark_snapshot.captured_at.to_rfc3339(),
                "usedOrderbookDepth": fill.used_orderbook_depth,
                "partialFill": fill.partial_fill,
            }),
            created_at: Utc::now(),
        };

        sqlx::query(
            "INSERT INTO strategy_replay_fills (
                id, replay_run_id, event_time, market_id, outcome, side, target_wallet,
                followed_trade_id, requested_quantity, filled_quantity, price, mark_price, fee_usdc,
                pnl_usdc, slippage_ticks, metadata, created_at
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)",
        )
        .bind(record.id.as_str())
        .bind(record.replay_run_id.as_str())
        .bind(record.event_time)
        .bind(record.market_id.as_str())
        .bind(record.outcome.as_str())
        .bind(record.side.as_str())
        .bind(record.target_wallet.as_deref())
        .bind(record.followed_trade_id.as_deref())
        .bind(record.requested_quantity)
        .bind(record.filled_quantity)
        .bind(record.price)
        .bind(record.mark_price)
        .bind(record.fee_usdc)
        .bind(record.pnl_usdc)
        .bind(record.slippage_ticks)
        .bind(record.metadata.clone())
        .bind(record.created_at)
        .execute(pool()?)
        .await
        .map_err(|err| ApiError::internal(&format!("failed to persist replay fill: {err}")))?;

        pnl_points.push(record.pnl_usdc);
        slippage_points.push(record.slippage_ticks);
        fills.push(record);
    }

    let net_pnl_usdc = fills.iter().map(|fill| fill.pnl_usdc).sum::<f64>();
    let gross_volume_usdc = fills
        .iter()
        .map(|fill| fill.filled_quantity * fill.price)
        .sum::<f64>();
    let total_fees_usdc = fills.iter().map(|fill| fill.fee_usdc).sum::<f64>();
    let fill_count = fills
        .iter()
        .filter(|fill| fill.filled_quantity > 0.0)
        .count() as u64;
    let mut slippage_copy = slippage_points.clone();
    let summary = json!({
        "tradesObserved": fills.len(),
        "fills": fill_count,
        "fillRate": if fills.is_empty() { 0.0 } else { fill_count as f64 / fills.len() as f64 },
        "grossVolumeUsdc": gross_volume_usdc,
        "feesUsdc": total_fees_usdc,
        "netPnlUsdc": net_pnl_usdc,
        "maxDrawdownUsdc": max_drawdown(&pnl_points),
        "p50SlippageTicks": median(slippage_copy.as_mut_slice()),
        "delayMs": safe_delay_ms,
        "markoutMinutes": safe_markout_minutes,
    });

    sqlx::query(
        "UPDATE strategy_replay_runs
         SET status = 'completed',
             summary = $2,
             completed_at = NOW(),
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(run_id.as_str())
    .bind(summary)
    .execute(pool()?)
    .await
    .map_err(|err| ApiError::internal(&format!("failed to finalize replay run: {err}")))?;

    load_strategy_replay_run(run_id.as_str())
        .await?
        .ok_or_else(|| ApiError::internal("replay run missing after completion"))
}

pub async fn load_strategy_replay_run(
    replay_run_id: &str,
) -> Result<Option<StrategyReplayRunRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT id, created_by, strategy, baseline, status, market_id, market_category,
                target_wallet, delay_ms, window_hours, input_params, summary,
                created_at, completed_at, updated_at
         FROM strategy_replay_runs
         WHERE id = $1",
    )
    .bind(replay_run_id)
    .fetch_optional(pool()?)
    .await
    .map_err(|err| ApiError::internal(&format!("failed to load replay run: {err}")))?;

    row.map(parse_strategy_replay_run_row).transpose()
}

pub async fn load_strategy_replay_fills(
    replay_run_id: &str,
) -> Result<Vec<StrategyReplayFillRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, replay_run_id, event_time, market_id, outcome, side, target_wallet,
                followed_trade_id, requested_quantity, filled_quantity, price, mark_price,
                fee_usdc, pnl_usdc, slippage_ticks, metadata, created_at
         FROM strategy_replay_fills
         WHERE replay_run_id = $1
         ORDER BY event_time DESC, id DESC",
    )
    .bind(replay_run_id)
    .fetch_all(pool()?)
    .await
    .map_err(|err| ApiError::internal(&format!("failed to load replay fills: {err}")))?;

    rows.into_iter()
        .map(parse_strategy_replay_fill_row)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event(id: &str) -> PolymarketUserTradeEventRecord {
        let now = Utc::now();
        PolymarketUserTradeEventRecord {
            id: id.to_string(),
            agent_id: Some("agent-1".to_string()),
            run_id: Some("run-1".to_string()),
            external_order_id: Some("order-1".to_string()),
            owner: Some("0xabc".to_string()),
            market_id: "polymarket:1".to_string(),
            provider_market_ref: Some("1".to_string()),
            provider_order_id: Some("provider-1".to_string()),
            builder_trade_id: Some("builder-1".to_string()),
            taker_hash: Some("taker-1".to_string()),
            tx_hash: Some("0xtx".to_string()),
            block_number: Some(123),
            outcome: Some("yes".to_string()),
            side: Some("buy".to_string()),
            price: Some(0.61),
            price_bps: Some(6100),
            requested_quantity: Some(15.0),
            filled_quantity: Some(12.0),
            fee_usdc: 0.25,
            lifecycle_status: "CONFIRMED".to_string(),
            attempt_count: 1,
            last_error: None,
            raw_payload: json!({"id": id}),
            matched_at: Some(now),
            mined_at: Some(now),
            confirmed_at: Some(now),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn payload_reference_extraction_captures_polymarket_keys() {
        let payload = json!({
            "txHash": "0xaaa",
            "takerHash": "taker-1",
            "builderTradeId": "builder-1",
            "order": {
                "orderId": "provider-1"
            }
        });

        let refs = reference_candidates_from_payload(&payload);

        assert_eq!(refs.tx_hashes, vec!["0xaaa".to_string()]);
        assert!(refs.provider_order_refs.contains(&"provider-1".to_string()));
        assert!(refs.provider_order_refs.contains(&"taker-1".to_string()));
        assert_eq!(refs.builder_trade_refs, vec!["builder-1".to_string()]);
    }

    #[test]
    fn reference_matching_prefers_tx_hash_over_other_refs() {
        let tx_event = sample_event("tx");
        let mut provider_event = sample_event("provider");
        provider_event.tx_hash = Some("0xother".to_string());
        provider_event.provider_order_id = Some("provider-1".to_string());
        provider_event.confirmed_at = tx_event
            .confirmed_at
            .map(|value| value - chrono::Duration::minutes(1));

        let refs = PolymarketReferenceCandidates {
            tx_hashes: vec!["0xtx".to_string()],
            provider_order_refs: vec!["provider-1".to_string()],
            builder_trade_refs: Vec::new(),
        };

        let tx_score = reference_score(&tx_event, &refs);
        let provider_score = reference_score(&provider_event, &refs);

        assert!(tx_score.0 > provider_score.0);
    }

    #[test]
    fn lifecycle_merge_never_regresses_confirmed_status() {
        assert_eq!(
            merged_lifecycle_status("CONFIRMED", PolymarketTradeLifecycleStatus::Retrying),
            "CONFIRMED"
        );
        assert_eq!(
            merged_lifecycle_status("MATCHED", PolymarketTradeLifecycleStatus::Mined),
            "MINED"
        );
    }

    #[test]
    fn upsert_matching_prefers_internal_run_or_order_ids() {
        let mut event = sample_event("existing");
        event.tx_hash = None;
        event.provider_order_id = None;
        event.builder_trade_id = None;
        event.taker_hash = None;

        let upsert = PolymarketUserTradeEventUpsert {
            agent_id: Some("agent-1".to_string()),
            run_id: Some("run-1".to_string()),
            external_order_id: Some("order-1".to_string()),
            owner: Some("0xabc".to_string()),
            market_id: "polymarket:1".to_string(),
            provider_market_ref: Some("1".to_string()),
            provider_order_id: Some("provider-2".to_string()),
            builder_trade_id: None,
            taker_hash: None,
            tx_hash: None,
            block_number: None,
            outcome: Some("yes".to_string()),
            side: Some("buy".to_string()),
            price: Some(0.61),
            requested_quantity: Some(15.0),
            filled_quantity: Some(12.0),
            fee_usdc: Some(0.25),
            lifecycle_status: PolymarketTradeLifecycleStatus::Matched,
            attempt_count: 0,
            last_error: None,
            raw_payload: json!({}),
            observed_at: Some(Utc::now()),
        };

        let events = [event];
        let found = match_existing_event(&events, &upsert).unwrap();
        assert_eq!(found.id, "existing");
    }

    #[test]
    fn empty_snapshot_reports_partial_backfill_without_state() {
        let snapshot = empty_public_snapshot("polymarket:123", "123", Some("yes"), 50, 0, None);

        assert_eq!(snapshot.index_status.as_deref(), Some("pending"));
        assert!(snapshot.is_partial_backfill);
        assert_eq!(snapshot.trades.len(), 0);
        assert_eq!(snapshot.provider_market_ref, "123");
    }
}
