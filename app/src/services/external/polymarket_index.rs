use std::env;
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use uuid::Uuid;

use crate::api::ApiError;

use super::types::{
    clamp_probability, price_to_bps, ExternalTradeSnapshot, ExternalTradesSnapshot,
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
                price, price_bps, quantity, tx_hash, block_number, token_id, maker, taker,
                match_time, raw_payload, ingested_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,NOW())
             ON CONFLICT (provider_trade_id) DO UPDATE SET
                 market_id = EXCLUDED.market_id,
                 provider_market_ref = EXCLUDED.provider_market_ref,
                 outcome = EXCLUDED.outcome,
                 side = EXCLUDED.side,
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
