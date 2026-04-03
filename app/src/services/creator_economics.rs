use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::Row;
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

use crate::api::evm::fetch_internal_market_snapshot_by_id;
use crate::api::ApiError;
use crate::models::MarketStatus;
use crate::models::Position;
use crate::services::database::{
    BaseMarketBootstrapConfigRecord, BootstrapFillEventUpsert, CreatorMarketEconomicsDailyRecord,
    CreatorMarketEconomicsDailyUpsert,
};
use crate::AppState;

const BOOTSTRAP_FILL_SOURCE: &str = "internal_orderbook";
const MIRROR_STALE_SECONDS: u64 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeseriesWindow {
    Days7,
    Days30,
    Days90,
}

impl TimeseriesWindow {
    pub fn days(self) -> i64 {
        match self {
            Self::Days7 => 7,
            Self::Days30 => 30,
            Self::Days90 => 90,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Days7 => "7d",
            Self::Days30 => "30d",
            Self::Days90 => "90d",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ApiError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "7d" => Ok(Self::Days7),
            "30d" => Ok(Self::Days30),
            "90d" => Ok(Self::Days90),
            _ => Err(ApiError::bad_request(
                "INVALID_WINDOW",
                "window must be one of: 7d, 30d, 90d",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEconomicsOverviewResponse {
    pub creator: String,
    pub active_seeded_markets: u64,
    pub total_seed_deployed_usdc: f64,
    pub current_capital_value_usdc: f64,
    pub net_liquidity_pnl_usdc: f64,
    pub subsidy_burn_usdc: f64,
    pub realized_resolution_pnl_usdc: f64,
    pub graduation_success_rate: f64,
    pub stale_error_mirror_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEconomicsMarketResponse {
    pub market_id: u64,
    pub market_question: String,
    pub status: String,
    pub liquidity_mode: String,
    pub bootstrap_status: String,
    pub seed_usdc: f64,
    pub available_usdc: f64,
    pub reserved_usdc: f64,
    pub inventory_yes_usdc: f64,
    pub inventory_no_usdc: f64,
    pub inventory_net_usdc: f64,
    pub current_capital_value_usdc: f64,
    pub net_liquidity_pnl_usdc: f64,
    pub subsidy_burn_usdc: f64,
    pub roi_bps: f64,
    pub cumulative_bootstrap_fills_usdc: f64,
    pub organic_replacement_ratio: f64,
    pub graduation_state: String,
    pub graduation_reason: Option<String>,
    pub mirror_freshness_seconds: Option<u64>,
    pub mirror_pending_hedges: u64,
    pub mirror_error_count: u64,
    pub mirror_links_with_errors: u64,
    pub realized_resolution_pnl_usdc: f64,
    pub graduated_at: Option<String>,
    pub last_reconciled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorMarketEconomicsResponse {
    pub market: CreatorEconomicsMarketResponse,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEconomicsTimeseriesPoint {
    pub day: String,
    pub cumulative_bootstrap_fills_usdc: f64,
    pub subsidy_burn_usdc: f64,
    pub inventory_mark_value_usdc: f64,
    pub organic_replacement_ratio: f64,
    pub mirror_freshness_seconds: Option<u64>,
    pub mirror_pending_hedges: u64,
    pub mirror_error_count: u64,
    pub graduation_retention_24h: Option<f64>,
    pub graduation_retention_7d: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEconomicsTimeseriesResponse {
    pub window: String,
    pub market_id: u64,
    pub creator: String,
    pub points: Vec<CreatorEconomicsTimeseriesPoint>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEconomicsMaterializeMarketResponse {
    pub market_id: u64,
    pub creator: String,
    pub day: String,
    pub backfilled_fill_events: u64,
    pub materialized_rows: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEconomicsMaterializeResponse {
    pub day: String,
    pub processed_markets: u64,
    pub backfilled_fill_events: u64,
    pub materialized_rows: u64,
    pub markets: Vec<CreatorEconomicsMaterializeMarketResponse>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEconomicsMaterializerRunResponse {
    pub scanned_markets: u64,
    pub materialized_markets: u64,
    pub rows_loaded: u64,
    pub window_days: u64,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorEconomicsMaterializerHealthResponse {
    pub status: String,
    pub tracked_markets: u64,
    pub markets_with_today_row: u64,
    pub stale_markets: u64,
    pub max_lag_days: u64,
    pub latest_materialized_day: Option<String>,
    pub last_materialized_at: Option<String>,
}

#[derive(Debug, Clone)]
struct MirrorMetrics {
    freshness_seconds: Option<u64>,
    pending_hedges: u64,
    error_count: u64,
    links_with_errors: u64,
}

#[derive(Debug, Clone)]
struct FillTradeSide {
    order_id: String,
    owner: String,
    side: String,
    price_bps: u64,
    quantity: u64,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct FillTradeJoin {
    trade_id: String,
    market_id: u64,
    outcome: String,
    price: f64,
    quantity: f64,
    occurred_at: DateTime<Utc>,
    buy: FillTradeSide,
    sell: FillTradeSide,
}

#[derive(Debug, Clone)]
struct BootstrapAgentMatch {
    agent_id: u64,
    side: String,
    price_bps: u64,
    size: u64,
}

#[derive(Debug, Clone)]
struct BootstrapFillDraft {
    id: String,
    market_id: u64,
    creator: String,
    trade_id: String,
    source: String,
    agent_id: u64,
    maker_order_id: String,
    outcome: String,
    side: String,
    price: f64,
    quantity: f64,
    notional_usdc: f64,
    occurred_at: DateTime<Utc>,
    raw: serde_json::Value,
}

#[derive(Debug, Clone)]
struct MarketContext {
    config: BaseMarketBootstrapConfigRecord,
    market_question: String,
    status: String,
    available_usdc: f64,
    reserved_usdc: f64,
    inventory_yes_usdc: f64,
    inventory_no_usdc: f64,
    inventory_net_usdc: f64,
    inventory_mark_value_usdc: f64,
    current_capital_value_usdc: f64,
    net_liquidity_pnl_usdc: f64,
    subsidy_burn_usdc: f64,
    roi_bps: f64,
    cumulative_bootstrap_fills_usdc: f64,
    realized_resolution_pnl_usdc: f64,
    mirror: MirrorMetrics,
}

#[derive(Debug, Clone)]
struct MaterializedMarketRows {
    backfilled_fill_events: u64,
    rows: Vec<CreatorMarketEconomicsDailyRecord>,
}

#[derive(Debug, Clone)]
struct CreatorMarketView {
    question: String,
    status: String,
    resolved: bool,
    yes_price: f64,
    no_price: f64,
}

fn normalize_wallet(value: &str) -> Result<String, ApiError> {
    let wallet = value.trim().to_ascii_lowercase();
    if wallet.len() != 42
        || !wallet.starts_with("0x")
        || !wallet[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "wallet must be a valid 0x EVM address",
        ));
    }
    Ok(wallet)
}

fn market_id_string(market_id: u64) -> String {
    market_id.to_string()
}

fn market_status_label(status: MarketStatus) -> &'static str {
    match status {
        MarketStatus::Active => "active",
        MarketStatus::Paused => "paused",
        MarketStatus::Closed => "closed",
        MarketStatus::Resolved => "resolved",
        MarketStatus::Cancelled => "cancelled",
    }
}

fn is_creator_owned_bootstrap(config: &BaseMarketBootstrapConfigRecord, creator: &str) -> bool {
    config.liquidity_mode == "bootstrap_hybrid" && config.creator.eq_ignore_ascii_case(creator)
}

fn date_range(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    if end < start {
        return Vec::new();
    }

    let mut days = Vec::new();
    let mut current = start;
    while current <= end {
        days.push(current);
        current = current
            .checked_add_signed(ChronoDuration::days(1))
            .unwrap_or(end.succ_opt().unwrap_or(end));
    }
    days
}

async fn load_creator_market_view(
    state: &AppState,
    market_id: u64,
) -> Result<CreatorMarketView, ApiError> {
    let market_id_str = market_id_string(market_id);
    if let Some(market) = state
        .db
        .get_market(market_id_str.as_str())
        .await
        .map_err(ApiError::from)?
    {
        return Ok(CreatorMarketView {
            question: market.question,
            status: market_status_label(market.status).to_string(),
            resolved: market.status == MarketStatus::Resolved,
            yes_price: market.yes_price,
            no_price: market.no_price,
        });
    }

    let snapshot = fetch_internal_market_snapshot_by_id(state, market_id).await?;
    let yes_price = snapshot
        .yes_price
        .or_else(|| snapshot.outcomes.first().map(|outcome| outcome.probability))
        .unwrap_or(0.0);
    let no_price = snapshot
        .no_price
        .or_else(|| snapshot.outcomes.get(1).map(|outcome| outcome.probability))
        .unwrap_or((1.0 - yes_price).clamp(0.0, 1.0));

    Ok(CreatorMarketView {
        question: snapshot.question,
        status: snapshot.status,
        resolved: snapshot.resolved,
        yes_price,
        no_price,
    })
}

fn compute_capital_value(
    available_usdc: f64,
    reserved_usdc: f64,
    inventory_mark_value_usdc: f64,
) -> f64 {
    available_usdc + reserved_usdc + inventory_mark_value_usdc
}

fn compute_net_liquidity_pnl(seed_usdc: f64, capital_value_usdc: f64) -> f64 {
    capital_value_usdc - seed_usdc
}

fn compute_subsidy_burn(seed_usdc: f64, capital_value_usdc: f64) -> f64 {
    (seed_usdc - capital_value_usdc).max(0.0)
}

fn compute_roi_bps(seed_usdc: f64, net_liquidity_pnl_usdc: f64) -> f64 {
    if seed_usdc <= 0.0 {
        0.0
    } else {
        net_liquidity_pnl_usdc * 10_000.0 / seed_usdc
    }
}

fn compute_inventory_mark_value(position: Option<&Position>, yes_price: f64, no_price: f64) -> f64 {
    let Some(position) = position else {
        return 0.0;
    };

    position.yes_balance as f64 * yes_price + position.no_balance as f64 * no_price
}

fn compute_inventory_marked_side_values(
    position: Option<&Position>,
    yes_price: f64,
    no_price: f64,
) -> (f64, f64, f64) {
    let Some(position) = position else {
        return (0.0, 0.0, 0.0);
    };

    let yes_value = position.yes_balance as f64 * yes_price;
    let no_value = position.no_balance as f64 * no_price;
    (yes_value, no_value, yes_value - no_value)
}

fn mirror_stale(mirror: &MirrorMetrics) -> bool {
    mirror.error_count > 0
        || mirror
            .freshness_seconds
            .is_some_and(|value| value > MIRROR_STALE_SECONDS)
}

fn classification_matches(
    creator: &str,
    trade: &FillTradeJoin,
    agent: &BootstrapAgentMatch,
) -> bool {
    if trade.outcome.trim().eq_ignore_ascii_case(&agent.side) {
        let buy_is_creator = trade.buy.owner.eq_ignore_ascii_case(creator);
        let sell_is_creator = trade.sell.owner.eq_ignore_ascii_case(creator);

        if buy_is_creator == sell_is_creator {
            return false;
        }

        let maker = if buy_is_creator {
            &trade.buy
        } else {
            &trade.sell
        };
        if maker.price_bps != agent.price_bps || maker.quantity != agent.size {
            return false;
        }

        let taker = if buy_is_creator {
            &trade.sell
        } else {
            &trade.buy
        };
        if maker.created_at >= taker.created_at {
            return false;
        }

        return true;
    }

    false
}

fn classify_bootstrap_fill(
    creator: &str,
    trade: &FillTradeJoin,
    agents: &[BootstrapAgentMatch],
) -> Option<BootstrapFillDraft> {
    let buy_is_creator = trade.buy.owner.eq_ignore_ascii_case(creator);
    let sell_is_creator = trade.sell.owner.eq_ignore_ascii_case(creator);
    if buy_is_creator == sell_is_creator {
        return None;
    }

    let maker = if buy_is_creator {
        &trade.buy
    } else {
        &trade.sell
    };
    let taker = if buy_is_creator {
        &trade.sell
    } else {
        &trade.buy
    };
    if maker.created_at >= taker.created_at {
        return None;
    }

    let mut matches = agents
        .iter()
        .filter(|agent| classification_matches(creator, trade, agent))
        .collect::<Vec<_>>();

    if matches.len() != 1 {
        return None;
    }

    let agent = matches.swap_remove(0);
    let side = if buy_is_creator { "buy" } else { "sell" };
    let notional_usdc = trade.price * trade.quantity;

    Some(BootstrapFillDraft {
        id: Uuid::new_v4().to_string(),
        market_id: trade.market_id,
        creator: creator.to_string(),
        trade_id: trade.trade_id.clone(),
        source: BOOTSTRAP_FILL_SOURCE.to_string(),
        agent_id: agent.agent_id,
        maker_order_id: maker.order_id.clone(),
        outcome: trade.outcome.clone(),
        side: side.to_string(),
        price: trade.price,
        quantity: trade.quantity,
        notional_usdc,
        occurred_at: trade.occurred_at,
        raw: json!({
            "tradeId": trade.trade_id,
            "marketId": trade.market_id,
            "outcome": trade.outcome,
            "price": trade.price,
            "quantity": trade.quantity,
            "makerOrderId": maker.order_id,
            "makerSide": side,
            "takerOrderId": taker.order_id,
            "bootstrapAgentId": agent.agent_id,
            "bootstrapAgentPriceBps": agent.price_bps,
            "bootstrapAgentSize": agent.size,
        }),
    })
}

async fn fetch_mirror_metrics(state: &AppState, market_id: u64) -> Result<MirrorMetrics, ApiError> {
    let row = sqlx::query(
        "SELECT
            COUNT(*)::bigint AS link_count,
            COUNT(*) FILTER (WHERE active)::bigint AS active_link_count,
            COUNT(*) FILTER (WHERE mirror_error IS NOT NULL)::bigint AS mirror_error_count,
            COUNT(*) FILTER (WHERE hedge_error IS NOT NULL)::bigint AS hedge_error_count,
            MAX(last_mirror_at) AS last_mirror_at,
            MAX(last_hedge_at) AS last_hedge_at
         FROM mirror_market_links
         WHERE internal_market_id = $1",
    )
    .bind(market_id as i64)
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&format!("failed to load mirror metrics: {err}")))?;

    let last_mirror_at = row
        .try_get::<Option<DateTime<Utc>>, _>("last_mirror_at")
        .ok()
        .flatten();
    let freshness_seconds = last_mirror_at.map(|value| {
        let diff = Utc::now().signed_duration_since(value);
        diff.to_std()
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
    });
    let pending_hedges = sqlx::query(
        "SELECT COUNT(*)::bigint AS cnt
         FROM mirror_hedge_log log
         INNER JOIN mirror_market_links link ON link.id = log.mirror_link_id
         WHERE link.internal_market_id = $1 AND log.hedge_status = 'pending'",
    )
    .bind(market_id as i64)
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&format!("failed to load mirror hedge backlog: {err}")))?;

    Ok(MirrorMetrics {
        freshness_seconds,
        pending_hedges: pending_hedges.get::<i64, _>("cnt").max(0) as u64,
        error_count: row.get::<i64, _>("mirror_error_count").max(0) as u64
            + row.get::<i64, _>("hedge_error_count").max(0) as u64,
        links_with_errors: row.get::<i64, _>("mirror_error_count").max(0) as u64,
    })
}

async fn fetch_market_context(
    state: &AppState,
    creator: &str,
    config: &BaseMarketBootstrapConfigRecord,
) -> Result<MarketContext, ApiError> {
    let market = load_creator_market_view(state, config.market_id).await?;
    let position = state
        .db
        .get_position(creator, market_id_string(config.market_id).as_str())
        .await
        .map_err(ApiError::from)?;

    let mirror = fetch_mirror_metrics(state, config.market_id).await?;
    let inventory_mark_value_usdc =
        compute_inventory_mark_value(position.as_ref(), market.yes_price, market.no_price);
    let (inventory_yes_usdc, inventory_no_usdc, inventory_net_usdc) =
        compute_inventory_marked_side_values(position.as_ref(), market.yes_price, market.no_price);
    let current_capital_value_usdc = compute_capital_value(
        config.available_usdc,
        config.reserved_usdc,
        inventory_mark_value_usdc,
    );
    let net_liquidity_pnl_usdc =
        compute_net_liquidity_pnl(config.seed_usdc, current_capital_value_usdc);
    let subsidy_burn_usdc = compute_subsidy_burn(config.seed_usdc, current_capital_value_usdc);
    let roi_bps = compute_roi_bps(config.seed_usdc, net_liquidity_pnl_usdc);
    let realized_resolution_pnl_usdc = if market.resolved {
        position
            .as_ref()
            .map(|entry| entry.realized_pnl)
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let cumulative_bootstrap_fills_usdc = state
        .db
        .list_bootstrap_fill_events_for_creator_market(creator, config.market_id)
        .await
        .map_err(ApiError::from)?
        .iter()
        .map(|event| event.notional_usdc)
        .sum::<f64>();

    Ok(MarketContext {
        config: config.clone(),
        market_question: market.question,
        status: market.status,
        available_usdc: config.available_usdc,
        reserved_usdc: config.reserved_usdc,
        inventory_yes_usdc,
        inventory_no_usdc,
        inventory_net_usdc,
        inventory_mark_value_usdc,
        current_capital_value_usdc,
        net_liquidity_pnl_usdc,
        subsidy_burn_usdc,
        roi_bps,
        cumulative_bootstrap_fills_usdc,
        realized_resolution_pnl_usdc,
        mirror,
    })
}

async fn backfill_bootstrap_fill_events_for_market(
    state: &AppState,
    creator: &str,
    config: &BaseMarketBootstrapConfigRecord,
    agents: &[BootstrapAgentMatch],
) -> Result<u64, ApiError> {
    let market_id = market_id_string(config.market_id);
    let rows = sqlx::query(
        r#"
        SELECT
            t.id AS trade_id,
            CAST(t.market_id AS BIGINT) AS market_id,
            t.outcome AS outcome,
            t.price AS price,
            t.quantity AS quantity,
            t.created_at AS occurred_at,
            bo.id AS buy_order_id,
            bo.owner AS buy_owner,
            bo.side AS buy_side,
            bo.price_bps AS buy_price_bps,
            bo.quantity AS buy_quantity,
            bo.created_at AS buy_created_at,
            so.id AS sell_order_id,
            so.owner AS sell_owner,
            so.side AS sell_side,
            so.price_bps AS sell_price_bps,
            so.quantity AS sell_quantity,
            so.created_at AS sell_created_at
        FROM trades t
        INNER JOIN orders bo ON bo.id = t.buy_order_id
        INNER JOIN orders so ON so.id = t.sell_order_id
        WHERE t.market_id = $1
        ORDER BY t.created_at ASC, t.id ASC
        "#,
    )
    .bind(market_id.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&format!("failed to load trade history: {err}")))?;

    let existing_trade_ids = state
        .db
        .list_bootstrap_fill_events_for_creator_market(creator, config.market_id)
        .await
        .map_err(ApiError::from)?
        .into_iter()
        .map(|event| event.trade_id)
        .collect::<HashSet<_>>();

    let mut inserted = 0_u64;
    let mut seen_trade_ids = existing_trade_ids;
    for row in rows {
        let trade = FillTradeJoin {
            trade_id: row.try_get("trade_id").unwrap_or_default(),
            market_id: row
                .try_get::<i64, _>("market_id")
                .unwrap_or_default()
                .max(0) as u64,
            outcome: row
                .try_get::<i16, _>("outcome")
                .ok()
                .map(|value| if value == 0 { "yes" } else { "no" }.to_string())
                .unwrap_or_else(|| "yes".to_string()),
            price: row.try_get("price").unwrap_or(0.0),
            quantity: row.try_get("quantity").unwrap_or(0.0),
            occurred_at: row
                .try_get::<DateTime<Utc>, _>("occurred_at")
                .unwrap_or_else(|_| Utc::now()),
            buy: FillTradeSide {
                order_id: row.try_get("buy_order_id").unwrap_or_default(),
                owner: row.try_get("buy_owner").unwrap_or_default(),
                side: "buy".to_string(),
                price_bps: row.try_get::<i16, _>("buy_price_bps").unwrap_or(0).max(0) as u64,
                quantity: row.try_get::<i64, _>("buy_quantity").unwrap_or(0).max(0) as u64,
                created_at: row
                    .try_get::<DateTime<Utc>, _>("buy_created_at")
                    .unwrap_or_else(|_| Utc::now()),
            },
            sell: FillTradeSide {
                order_id: row.try_get("sell_order_id").unwrap_or_default(),
                owner: row.try_get("sell_owner").unwrap_or_default(),
                side: "sell".to_string(),
                price_bps: row.try_get::<i16, _>("sell_price_bps").unwrap_or(0).max(0) as u64,
                quantity: row.try_get::<i64, _>("sell_quantity").unwrap_or(0).max(0) as u64,
                created_at: row
                    .try_get::<DateTime<Utc>, _>("sell_created_at")
                    .unwrap_or_else(|_| Utc::now()),
            },
        };

        if let Some(draft) = classify_bootstrap_fill(creator, &trade, agents) {
            let is_new = seen_trade_ids.insert(draft.trade_id.clone());
            state
                .db
                .upsert_bootstrap_fill_event(&BootstrapFillEventUpsert {
                    id: draft.id.as_str(),
                    market_id: draft.market_id,
                    creator: draft.creator.as_str(),
                    trade_id: draft.trade_id.as_str(),
                    source: draft.source.as_str(),
                    agent_id: Some(draft.agent_id),
                    maker_order_id: draft.maker_order_id.as_str(),
                    outcome: draft.outcome.as_str(),
                    side: draft.side.as_str(),
                    price: draft.price,
                    quantity: draft.quantity,
                    notional_usdc: draft.notional_usdc,
                    occurred_at: draft.occurred_at,
                    raw: &draft.raw,
                })
                .await
                .map_err(ApiError::from)?;
            if is_new {
                inserted = inserted.saturating_add(1);
            }
        }
    }

    Ok(inserted)
}

async fn materialize_daily_rows_for_market(
    state: &AppState,
    creator: &str,
    config: &BaseMarketBootstrapConfigRecord,
    window_days: i64,
) -> Result<MaterializedMarketRows, ApiError> {
    let agents = state
        .db
        .list_base_market_bootstrap_agents(config.market_id)
        .await
        .map_err(ApiError::from)?
        .into_iter()
        .filter(|record| record.active && record.agent_id.is_some())
        .map(|record| BootstrapAgentMatch {
            agent_id: record.agent_id.unwrap_or_default(),
            side: record.side,
            price_bps: record.current_price_bps.unwrap_or(record.desired_price_bps),
            size: record.current_size.unwrap_or(record.desired_size),
        })
        .collect::<Vec<_>>();

    let backfilled_fill_events =
        backfill_bootstrap_fill_events_for_market(state, creator, config, &agents).await?;

    let events = state
        .db
        .list_bootstrap_fill_events_for_creator_market(creator, config.market_id)
        .await
        .map_err(ApiError::from)?;

    let market = load_creator_market_view(state, config.market_id).await?;
    let position = state
        .db
        .get_position(creator, market_id_string(config.market_id).as_str())
        .await
        .map_err(ApiError::from)?;
    let mirror = fetch_mirror_metrics(state, config.market_id).await?;

    let end_day = Utc::now().date_naive();
    let start_day = end_day
        .checked_sub_signed(chrono::Duration::days(window_days.saturating_sub(1)))
        .unwrap_or(end_day);
    let mut by_day = BTreeMap::<NaiveDate, f64>::new();
    for event in &events {
        let day = event.occurred_at.date_naive();
        *by_day.entry(day).or_default() += event.notional_usdc;
    }

    let mut cumulative = 0.0;
    for day in date_range(start_day, end_day) {
        if let Some(value) = by_day.get(&day) {
            cumulative += *value;
        }

        if day != end_day {
            continue;
        }

        let inventory_mark_value_usdc =
            compute_inventory_mark_value(position.as_ref(), market.yes_price, market.no_price);
        let current_capital_value_usdc = compute_capital_value(
            config.available_usdc,
            config.reserved_usdc,
            inventory_mark_value_usdc,
        );
        let net_liquidity_pnl_usdc =
            compute_net_liquidity_pnl(config.seed_usdc, current_capital_value_usdc);
        let subsidy_burn_usdc = compute_subsidy_burn(config.seed_usdc, current_capital_value_usdc);
        let roi_bps = compute_roi_bps(config.seed_usdc, net_liquidity_pnl_usdc);
        let realized_resolution_pnl_usdc = if market.resolved {
            position
                .as_ref()
                .map(|entry| entry.realized_pnl)
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let (inventory_yes_usdc, inventory_no_usdc, _) = compute_inventory_marked_side_values(
            position.as_ref(),
            market.yes_price,
            market.no_price,
        );

        state
            .db
            .upsert_creator_market_economics_daily(&CreatorMarketEconomicsDailyUpsert {
                market_id: config.market_id,
                creator,
                day,
                seed_usdc: config.seed_usdc,
                available_usdc: config.available_usdc,
                reserved_usdc: config.reserved_usdc,
                inventory_yes: inventory_yes_usdc,
                inventory_no: inventory_no_usdc,
                inventory_mark_value_usdc,
                cumulative_bootstrap_fills_usdc: cumulative,
                net_liquidity_pnl_usdc,
                subsidy_burn_usdc,
                roi_bps,
                realized_resolution_pnl_usdc,
                organic_depth_ratio: config.organic_depth_ratio,
                graduated: config.status.eq_ignore_ascii_case("graduated"),
                graduation_retention_24h: None,
                graduation_retention_7d: None,
                mirror_freshness_seconds: mirror.freshness_seconds,
                mirror_pending_hedges: mirror.pending_hedges,
                mirror_error_count: mirror.error_count,
            })
            .await
            .map_err(ApiError::from)?;
    }

    let rows = state
        .db
        .list_creator_market_economics_daily_for_market(
            creator,
            config.market_id,
            Some(start_day),
            Some(end_day),
        )
        .await
        .map_err(ApiError::from)?;

    Ok(MaterializedMarketRows {
        backfilled_fill_events,
        rows,
    })
}

fn to_market_response(context: &MarketContext) -> CreatorEconomicsMarketResponse {
    CreatorEconomicsMarketResponse {
        market_id: context.config.market_id,
        market_question: context.market_question.clone(),
        status: context.status.clone(),
        liquidity_mode: context.config.liquidity_mode.clone(),
        bootstrap_status: context.config.status.clone(),
        seed_usdc: context.config.seed_usdc,
        available_usdc: context.available_usdc,
        reserved_usdc: context.reserved_usdc,
        inventory_yes_usdc: context.inventory_yes_usdc,
        inventory_no_usdc: context.inventory_no_usdc,
        inventory_net_usdc: context.inventory_net_usdc,
        current_capital_value_usdc: context.current_capital_value_usdc,
        net_liquidity_pnl_usdc: context.net_liquidity_pnl_usdc,
        subsidy_burn_usdc: context.subsidy_burn_usdc,
        roi_bps: context.roi_bps,
        cumulative_bootstrap_fills_usdc: context.cumulative_bootstrap_fills_usdc,
        organic_replacement_ratio: context.config.organic_depth_ratio,
        graduation_state: context.config.status.clone(),
        graduation_reason: context.config.graduation_reason.clone(),
        mirror_freshness_seconds: context.mirror.freshness_seconds,
        mirror_pending_hedges: context.mirror.pending_hedges,
        mirror_error_count: context.mirror.error_count,
        mirror_links_with_errors: context.mirror.links_with_errors,
        realized_resolution_pnl_usdc: context.realized_resolution_pnl_usdc,
        graduated_at: context.config.graduated_at.map(|value| value.to_rfc3339()),
        last_reconciled_at: context
            .config
            .last_reconciled_at
            .map(|value| value.to_rfc3339()),
    }
}

async fn load_creator_contexts(
    state: &AppState,
    creator: &str,
) -> Result<Vec<MarketContext>, ApiError> {
    let configs = state
        .db
        .list_base_market_bootstrap_configs_for_creator(creator)
        .await
        .map_err(ApiError::from)?;

    let mut contexts = Vec::new();
    for config in configs {
        if !is_creator_owned_bootstrap(&config, creator) || config.seed_usdc <= 0.0 {
            continue;
        }

        let _materialized = materialize_daily_rows_for_market(state, creator, &config, 1).await?;
        contexts.push(fetch_market_context(state, creator, &config).await?);
    }

    Ok(contexts)
}

fn normalize_materializer_window_days(window_days: i64) -> i64 {
    window_days.clamp(1, 90)
}

pub async fn materialize_creator_market_rows(
    state: &AppState,
    creator_filter: Option<&str>,
    market_id_filter: Option<u64>,
    window_days: i64,
    limit: Option<usize>,
) -> Result<CreatorEconomicsMaterializerRunResponse, ApiError> {
    let creator_filter = creator_filter.map(normalize_wallet).transpose()?;
    let mut configs = state
        .db
        .list_base_market_bootstrap_configs()
        .await
        .map_err(ApiError::from)?
        .into_iter()
        .filter(|config| config.seed_usdc > 0.0)
        .filter(|config| config.liquidity_mode == "bootstrap_hybrid")
        .collect::<Vec<_>>();
    configs.sort_by_key(|config| (config.creator.clone(), config.market_id));

    let window_days = normalize_materializer_window_days(window_days);
    let limit = limit.unwrap_or(usize::MAX);
    let mut scanned_markets = 0_u64;
    let mut materialized_markets = 0_u64;
    let mut rows_loaded = 0_u64;
    let mut failures = Vec::new();

    for config in configs {
        if let Some(expected_creator) = creator_filter.as_ref() {
            if !config.creator.eq_ignore_ascii_case(expected_creator) {
                continue;
            }
        }

        if market_id_filter.is_some_and(|market_id| config.market_id != market_id) {
            continue;
        }

        if scanned_markets as usize >= limit {
            break;
        }

        scanned_markets = scanned_markets.saturating_add(1);
        match materialize_daily_rows_for_market(
            state,
            config.creator.as_str(),
            &config,
            window_days,
        )
        .await
        {
            Ok(rows) => {
                materialized_markets = materialized_markets.saturating_add(1);
                rows_loaded = rows_loaded.saturating_add(rows.rows.len() as u64);
            }
            Err(error) => {
                failures.push(format!(
                    "{}:{} {}",
                    config.creator, config.market_id, error.message
                ));
            }
        }
    }

    Ok(CreatorEconomicsMaterializerRunResponse {
        scanned_markets,
        materialized_markets,
        rows_loaded,
        window_days: window_days as u64,
        failures,
    })
}

pub async fn creator_materializer_health(
    state: &AppState,
) -> Result<CreatorEconomicsMaterializerHealthResponse, ApiError> {
    let configs = state
        .db
        .list_base_market_bootstrap_configs()
        .await
        .map_err(ApiError::from)?
        .into_iter()
        .filter(|config| config.seed_usdc > 0.0 && config.liquidity_mode == "bootstrap_hybrid")
        .collect::<Vec<_>>();

    if configs.is_empty() {
        return Ok(CreatorEconomicsMaterializerHealthResponse {
            status: "idle".to_string(),
            tracked_markets: 0,
            markets_with_today_row: 0,
            stale_markets: 0,
            max_lag_days: 0,
            latest_materialized_day: None,
            last_materialized_at: None,
        });
    }

    let rows = sqlx::query(
        r#"
        SELECT
            LOWER(creator) AS creator,
            market_id,
            MAX(day) AS latest_day,
            MAX(updated_at) AS last_materialized_at
        FROM creator_market_economics_daily
        GROUP BY LOWER(creator), market_id
        "#,
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| {
        ApiError::internal(&format!(
            "failed to load creator materializer health: {err}"
        ))
    })?;

    let today = Utc::now().date_naive();
    let mut latest_by_market =
        BTreeMap::<(String, u64), (Option<NaiveDate>, Option<DateTime<Utc>>)>::new();
    for row in rows {
        let creator = row.try_get::<String, _>("creator").unwrap_or_default();
        let market_id = row
            .try_get::<i64, _>("market_id")
            .unwrap_or_default()
            .max(0) as u64;
        let latest_day = row
            .try_get::<Option<NaiveDate>, _>("latest_day")
            .ok()
            .flatten();
        let last_materialized_at = row
            .try_get::<Option<DateTime<Utc>>, _>("last_materialized_at")
            .ok()
            .flatten();
        latest_by_market.insert((creator, market_id), (latest_day, last_materialized_at));
    }

    let mut markets_with_today_row = 0_u64;
    let mut stale_markets = 0_u64;
    let mut max_lag_days = 0_i64;
    let mut latest_materialized_day = None;
    let mut last_materialized_at = None;

    for config in configs.iter() {
        let key = (config.creator.to_ascii_lowercase(), config.market_id);
        let (latest_day, market_last_materialized_at) =
            latest_by_market.get(&key).cloned().unwrap_or((None, None));

        if let Some(day) = latest_day {
            if day >= today {
                markets_with_today_row = markets_with_today_row.saturating_add(1);
            } else {
                stale_markets = stale_markets.saturating_add(1);
            }
            let lag_days = today.signed_duration_since(day).num_days().max(0);
            max_lag_days = max_lag_days.max(lag_days);
            latest_materialized_day = latest_materialized_day
                .map(|current: NaiveDate| current.max(day))
                .or(Some(day));
        } else {
            stale_markets = stale_markets.saturating_add(1);
            max_lag_days = max_lag_days.max(999);
        }

        if let Some(updated_at) = market_last_materialized_at {
            last_materialized_at = last_materialized_at
                .map(|current: DateTime<Utc>| current.max(updated_at))
                .or(Some(updated_at));
        }
    }

    let status = if stale_markets == 0 {
        "healthy"
    } else if markets_with_today_row > 0 {
        "stale"
    } else {
        "error"
    };

    Ok(CreatorEconomicsMaterializerHealthResponse {
        status: status.to_string(),
        tracked_markets: configs.len() as u64,
        markets_with_today_row,
        stale_markets,
        max_lag_days: max_lag_days.max(0) as u64,
        latest_materialized_day: latest_materialized_day.map(|value| value.to_string()),
        last_materialized_at: last_materialized_at.map(|value| value.to_rfc3339()),
    })
}

pub async fn creator_overview(
    state: &AppState,
    creator: &str,
) -> Result<CreatorEconomicsOverviewResponse, ApiError> {
    let creator = normalize_wallet(creator)?;
    let contexts = load_creator_contexts(state, creator.as_str()).await?;

    let active_seeded_markets = contexts
        .iter()
        .filter(|context| context.config.seed_usdc > 0.0)
        .count() as u64;
    let total_seed_deployed_usdc = contexts
        .iter()
        .map(|context| context.config.seed_usdc)
        .sum();
    let current_capital_value_usdc = contexts
        .iter()
        .map(|context| context.current_capital_value_usdc)
        .sum();
    let net_liquidity_pnl_usdc = contexts
        .iter()
        .map(|context| context.net_liquidity_pnl_usdc)
        .sum();
    let subsidy_burn_usdc = contexts
        .iter()
        .map(|context| context.subsidy_burn_usdc)
        .sum();
    let realized_resolution_pnl_usdc = contexts
        .iter()
        .map(|context| context.realized_resolution_pnl_usdc)
        .sum();
    let graduated_count = contexts
        .iter()
        .filter(|context| context.config.status.eq_ignore_ascii_case("graduated"))
        .count() as f64;
    let graduation_success_rate = if active_seeded_markets == 0 {
        0.0
    } else {
        graduated_count / active_seeded_markets as f64
    };
    let stale_error_mirror_count = contexts
        .iter()
        .filter(|context| mirror_stale(&context.mirror))
        .count() as u64;

    Ok(CreatorEconomicsOverviewResponse {
        creator,
        active_seeded_markets,
        total_seed_deployed_usdc,
        current_capital_value_usdc,
        net_liquidity_pnl_usdc,
        subsidy_burn_usdc,
        realized_resolution_pnl_usdc,
        graduation_success_rate,
        stale_error_mirror_count,
    })
}

pub async fn creator_markets(
    state: &AppState,
    creator: &str,
) -> Result<Vec<CreatorEconomicsMarketResponse>, ApiError> {
    let creator = normalize_wallet(creator)?;
    let contexts = load_creator_contexts(state, creator.as_str()).await?;
    Ok(contexts.iter().map(to_market_response).collect())
}

pub async fn creator_market(
    state: &AppState,
    creator: &str,
    market_id: u64,
) -> Result<CreatorEconomicsMarketResponse, ApiError> {
    let creator = normalize_wallet(creator)?;
    let configs = state
        .db
        .list_base_market_bootstrap_configs_for_creator(creator.as_str())
        .await
        .map_err(ApiError::from)?;
    let Some(config) = configs.into_iter().find(|config| {
        config.market_id == market_id && is_creator_owned_bootstrap(config, creator.as_str())
    }) else {
        return Err(ApiError::not_found("Creator market"));
    };

    let _materialized =
        materialize_daily_rows_for_market(state, creator.as_str(), &config, 1).await?;
    let context = fetch_market_context(state, creator.as_str(), &config).await?;
    Ok(to_market_response(&context))
}

pub async fn creator_market_timeseries(
    state: &AppState,
    creator: &str,
    market_id: u64,
    window: TimeseriesWindow,
) -> Result<CreatorEconomicsTimeseriesResponse, ApiError> {
    let creator = normalize_wallet(creator)?;
    let configs = state
        .db
        .list_base_market_bootstrap_configs_for_creator(creator.as_str())
        .await
        .map_err(ApiError::from)?;
    let Some(config) = configs.into_iter().find(|config| {
        config.market_id == market_id && is_creator_owned_bootstrap(config, creator.as_str())
    }) else {
        return Err(ApiError::not_found("Creator market"));
    };

    let materialized =
        materialize_daily_rows_for_market(state, creator.as_str(), &config, window.days()).await?;
    let points = materialized
        .rows
        .into_iter()
        .map(|row| CreatorEconomicsTimeseriesPoint {
            day: row.day.to_string(),
            cumulative_bootstrap_fills_usdc: row.cumulative_bootstrap_fills_usdc,
            subsidy_burn_usdc: row.subsidy_burn_usdc,
            inventory_mark_value_usdc: row.inventory_mark_value_usdc,
            organic_replacement_ratio: row.organic_depth_ratio,
            mirror_freshness_seconds: row.mirror_freshness_seconds,
            mirror_pending_hedges: row.mirror_pending_hedges,
            mirror_error_count: row.mirror_error_count,
            graduation_retention_24h: row.graduation_retention_24h,
            graduation_retention_7d: row.graduation_retention_7d,
        })
        .collect();

    Ok(CreatorEconomicsTimeseriesResponse {
        window: window.as_str().to_string(),
        market_id,
        creator,
        points,
    })
}

pub async fn materialize_creator_economics(
    state: &AppState,
    owner: Option<&str>,
    market_id: Option<u64>,
    limit: Option<usize>,
) -> Result<CreatorEconomicsMaterializeResponse, ApiError> {
    let configs = match owner {
        Some(owner) => {
            let creator = normalize_wallet(owner)?;
            state
                .db
                .list_base_market_bootstrap_configs_for_creator(creator.as_str())
                .await
                .map_err(ApiError::from)?
        }
        None => state
            .db
            .list_base_market_bootstrap_configs()
            .await
            .map_err(ApiError::from)?,
    };

    let safe_limit = limit.unwrap_or(200).clamp(1, 1_000);
    let today = Utc::now().date_naive().to_string();
    let mut markets = Vec::new();
    let mut backfilled_fill_events = 0_u64;
    let mut materialized_rows = 0_u64;

    for config in configs
        .into_iter()
        .filter(|config| config.liquidity_mode == "bootstrap_hybrid" && config.seed_usdc > 0.0)
        .filter(|config| {
            market_id
                .map(|value| value == config.market_id)
                .unwrap_or(true)
        })
        .take(safe_limit)
    {
        let creator = normalize_wallet(config.creator.as_str())?;
        let materialized =
            materialize_daily_rows_for_market(state, creator.as_str(), &config, 1).await?;

        backfilled_fill_events =
            backfilled_fill_events.saturating_add(materialized.backfilled_fill_events);
        materialized_rows = materialized_rows.saturating_add(materialized.rows.len() as u64);
        markets.push(CreatorEconomicsMaterializeMarketResponse {
            market_id: config.market_id,
            creator,
            day: today.clone(),
            backfilled_fill_events: materialized.backfilled_fill_events,
            materialized_rows: materialized.rows.len() as u64,
        });
    }

    Ok(CreatorEconomicsMaterializeResponse {
        day: today,
        processed_markets: markets.len() as u64,
        backfilled_fill_events,
        materialized_rows,
        markets,
    })
}

pub fn test_classify_bootstrap_fill(
    creator: &str,
    trade: FillTradeJoin,
    agents: &[BootstrapAgentMatch],
) -> Option<BootstrapFillDraft> {
    classify_bootstrap_fill(creator, &trade, agents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn dt(offset: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(offset, 0)
            .single()
            .expect("valid timestamp")
    }

    #[test]
    fn classifies_single_bootstrap_maker_fill() {
        let trade = FillTradeJoin {
            trade_id: "trade-1".to_string(),
            market_id: 12,
            outcome: "yes".to_string(),
            price: 0.57,
            quantity: 10.0,
            occurred_at: dt(10),
            buy: FillTradeSide {
                order_id: "buy-1".to_string(),
                owner: "0xcreator".to_string(),
                side: "buy".to_string(),
                price_bps: 5700,
                quantity: 10,
                created_at: dt(1),
            },
            sell: FillTradeSide {
                order_id: "sell-1".to_string(),
                owner: "0xtaker".to_string(),
                side: "sell".to_string(),
                price_bps: 5700,
                quantity: 10,
                created_at: dt(9),
            },
        };
        let agents = vec![BootstrapAgentMatch {
            agent_id: 7,
            side: "yes".to_string(),
            price_bps: 5700,
            size: 10,
        }];

        let draft = classify_bootstrap_fill("0xcreator", &trade, &agents).expect("classified");
        assert_eq!(draft.agent_id, 7);
        assert_eq!(draft.side, "buy");
        assert_eq!(draft.maker_order_id, "buy-1");
        assert!((draft.notional_usdc - 5.7).abs() < 1e-9);
    }

    #[test]
    fn skips_ambiguous_creator_orders() {
        let trade = FillTradeJoin {
            trade_id: "trade-2".to_string(),
            market_id: 12,
            outcome: "yes".to_string(),
            price: 0.57,
            quantity: 10.0,
            occurred_at: dt(10),
            buy: FillTradeSide {
                order_id: "buy-1".to_string(),
                owner: "0xcreator".to_string(),
                side: "buy".to_string(),
                price_bps: 5700,
                quantity: 10,
                created_at: dt(9),
            },
            sell: FillTradeSide {
                order_id: "sell-1".to_string(),
                owner: "0xother".to_string(),
                side: "sell".to_string(),
                price_bps: 5700,
                quantity: 10,
                created_at: dt(8),
            },
        };
        let agents = vec![
            BootstrapAgentMatch {
                agent_id: 7,
                side: "yes".to_string(),
                price_bps: 5700,
                size: 10,
            },
            BootstrapAgentMatch {
                agent_id: 8,
                side: "yes".to_string(),
                price_bps: 5700,
                size: 10,
            },
        ];

        assert!(classify_bootstrap_fill("0xcreator", &trade, &agents).is_none());
    }

    #[test]
    fn computes_values_and_burn_consistently() {
        let capital = compute_capital_value(80.0, 20.0, 50.0);
        let pnl = compute_net_liquidity_pnl(100.0, capital);
        let burn = compute_subsidy_burn(100.0, capital);
        let roi = compute_roi_bps(100.0, pnl);

        assert_eq!(capital, 150.0);
        assert_eq!(pnl, 50.0);
        assert_eq!(burn, 0.0);
        assert_eq!(roi, 5_000.0);
    }

    #[test]
    fn computes_marked_inventory_values_from_position() {
        let position = Position {
            market_id: "12".to_string(),
            market_question: "Will relay44 ship creator economics?".to_string(),
            owner: "0xcreator".to_string(),
            yes_balance: 10,
            no_balance: 4,
            avg_yes_cost: 0.0,
            avg_no_cost: 0.0,
            current_yes_price: 0.62,
            current_no_price: 0.38,
            unrealized_pnl: 0.0,
            total_deposited: 0,
            total_withdrawn: 0,
            open_order_count: 0,
            total_trades: 0,
            realized_pnl: 7.5,
            created_at: Utc::now(),
        };

        let (yes_value, no_value, net_value) =
            compute_inventory_marked_side_values(Some(&position), 0.62, 0.38);

        assert!((yes_value - 6.2).abs() < 1e-9);
        assert!((no_value - 1.52).abs() < 1e-9);
        assert!((net_value - 4.68).abs() < 1e-9);
    }
}
