use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

use crate::api::jwt::{check_role, UserRole};
use crate::api::auth::extract_jwt_user;
use crate::api::validation::validate_pagination;
use crate::api::ApiError;
use crate::AppState;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ALLOWED_PERIODS: &[&str] = &["daily", "weekly", "monthly", "all_time"];
const ALLOWED_METRICS: &[&str] = &["pnl", "volume", "trades", "win_rate"];

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardQuery {
    pub period: Option<String>,
    pub metric: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankQuery {
    pub period: Option<String>,
    pub metric: Option<String>,
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn validate_period(period: &str) -> Result<(), ApiError> {
    if !ALLOWED_PERIODS.contains(&period) {
        return Err(ApiError::bad_request(
            "INVALID_PERIOD",
            &format!(
                "Period must be one of: {}",
                ALLOWED_PERIODS.join(", ")
            ),
        ));
    }
    Ok(())
}

fn validate_metric(metric: &str) -> Result<(), ApiError> {
    if !ALLOWED_METRICS.contains(&metric) {
        return Err(ApiError::bad_request(
            "INVALID_METRIC",
            &format!(
                "Metric must be one of: {}",
                ALLOWED_METRICS.join(", ")
            ),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn snapshot_row_to_json(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "rank": row.get::<i32, _>("rank"),
        "wallet": row.get::<String, _>("wallet_address"),
        "value": row.get::<f64, _>("value"),
        "change": row.try_get::<f64, _>("change").ok(),
        "previousRank": row.try_get::<i32, _>("previous_rank").ok().and_then(|v| Some(v)),
    })
}

// ---------------------------------------------------------------------------
// GET /v1/leaderboard
// ---------------------------------------------------------------------------

pub async fn get_leaderboard(
    state: web::Data<Arc<AppState>>,
    query: web::Query<LeaderboardQuery>,
) -> Result<impl Responder, ApiError> {
    let (limit, offset) = validate_pagination(query.limit, query.offset)?;

    let period = query.period.as_deref().unwrap_or("all_time");
    let metric = query.metric.as_deref().unwrap_or("pnl");

    validate_period(period)?;
    validate_metric(metric)?;

    let pool = state.db.pool();

    // Find latest snapshot_time for this period+metric
    let latest_row = sqlx::query(
        "SELECT snapshot_time FROM leaderboard_snapshots
         WHERE period = $1 AND metric = $2
         ORDER BY snapshot_time DESC
         LIMIT 1"
    )
    .bind(period)
    .bind(metric)
    .fetch_optional(pool)
    .await?;

    let latest_row = match latest_row {
        Some(r) => r,
        None => {
            return Ok(HttpResponse::Ok().json(json!({
                "period": period,
                "metric": metric,
                "entries": [],
                "updatedAt": serde_json::Value::Null,
                "total": 0,
            })));
        }
    };

    let snapshot_time: chrono::DateTime<Utc> = latest_row.get("snapshot_time");

    // Count total entries for this snapshot
    let count_row = sqlx::query(
        "SELECT COUNT(*) AS total FROM leaderboard_snapshots
         WHERE period = $1 AND metric = $2 AND snapshot_time = $3"
    )
    .bind(period)
    .bind(metric)
    .bind(snapshot_time)
    .fetch_one(pool)
    .await?;

    let total: i64 = count_row.get("total");

    // Fetch paginated entries ordered by rank
    let rows = sqlx::query(
        "SELECT wallet_address, value, rank, previous_rank, change
         FROM leaderboard_snapshots
         WHERE period = $1 AND metric = $2 AND snapshot_time = $3
         ORDER BY rank ASC
         LIMIT $4 OFFSET $5"
    )
    .bind(period)
    .bind(metric)
    .bind(snapshot_time)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let entries: Vec<Value> = rows.iter().map(snapshot_row_to_json).collect();

    Ok(HttpResponse::Ok().json(json!({
        "period": period,
        "metric": metric,
        "entries": entries,
        "updatedAt": snapshot_time.to_rfc3339(),
        "total": total,
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/leaderboard/rank/{wallet}
// ---------------------------------------------------------------------------

pub async fn get_user_rank(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<RankQuery>,
) -> Result<impl Responder, ApiError> {
    let wallet = path.into_inner();

    let period = query.period.as_deref().unwrap_or("all_time");
    let metric = query.metric.as_deref().unwrap_or("pnl");

    validate_period(period)?;
    validate_metric(metric)?;

    let pool = state.db.pool();

    // Find latest snapshot_time for this period+metric
    let latest_row = sqlx::query(
        "SELECT snapshot_time FROM leaderboard_snapshots
         WHERE period = $1 AND metric = $2
         ORDER BY snapshot_time DESC
         LIMIT 1"
    )
    .bind(period)
    .bind(metric)
    .fetch_optional(pool)
    .await?;

    let snapshot_time: chrono::DateTime<Utc> = match latest_row {
        Some(r) => r.get("snapshot_time"),
        None => return Err(ApiError::not_found("Leaderboard")),
    };

    // Look up user's row in this snapshot
    let row = sqlx::query(
        "SELECT wallet_address, value, rank, previous_rank, change
         FROM leaderboard_snapshots
         WHERE period = $1 AND metric = $2 AND snapshot_time = $3 AND wallet_address = $4"
    )
    .bind(period)
    .bind(metric)
    .bind(snapshot_time)
    .bind(&wallet)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(HttpResponse::Ok().json(json!({
            "wallet": r.get::<String, _>("wallet_address"),
            "rank": r.get::<i32, _>("rank"),
            "value": r.get::<f64, _>("value"),
            "period": period,
            "metric": metric,
        }))),
        None => Err(ApiError::not_found("Rank")),
    }
}

// ---------------------------------------------------------------------------
// POST /v1/leaderboard/compute  (admin only)
// ---------------------------------------------------------------------------

pub async fn compute_leaderboard(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let pool = state.db.pool();
    let now = Utc::now();

    let mut total_entries: i64 = 0;
    let computed_periods: Vec<&str> = ALLOWED_PERIODS.to_vec();

    for period in &computed_periods {
        let time_filter = match *period {
            "daily" => Some("AND t.created_at >= NOW() - INTERVAL '1 day'"),
            "weekly" => Some("AND t.created_at >= NOW() - INTERVAL '7 days'"),
            "monthly" => Some("AND t.created_at >= NOW() - INTERVAL '30 days'"),
            "all_time" => None,
            _ => None,
        };

        let time_clause = time_filter.unwrap_or("");

        let paper_time_filter = match *period {
            "daily" => Some("AND p.created_at >= NOW() - INTERVAL '1 day'"),
            "weekly" => Some("AND p.created_at >= NOW() - INTERVAL '7 days'"),
            "monthly" => Some("AND p.created_at >= NOW() - INTERVAL '30 days'"),
            "all_time" => None,
            _ => None,
        };
        let paper_time_clause = paper_time_filter.unwrap_or("");

        let outcome_time_filter = match *period {
            "daily" => Some("AND o.closed_at >= NOW() - INTERVAL '1 day'"),
            "weekly" => Some("AND o.closed_at >= NOW() - INTERVAL '7 days'"),
            "monthly" => Some("AND o.closed_at >= NOW() - INTERVAL '30 days'"),
            "all_time" => None,
            _ => None,
        };
        let outcome_time_clause = outcome_time_filter.unwrap_or("");

        // --- PnL ---
        // Combines on-chain trades with paper outcome realized PnL
        let pnl_entries = compute_metric_from_trades(
            pool, period, "pnl", &now, time_clause,
            &format!(
                "SELECT wallet, SUM(pnl) AS value FROM (
                    SELECT buyer AS wallet,
                           -1.0 * CAST(price AS DOUBLE PRECISION) * CAST(quantity AS DOUBLE PRECISION) AS pnl
                    FROM trades t WHERE 1=1 {time_clause}
                    UNION ALL
                    SELECT seller AS wallet,
                           CAST(price AS DOUBLE PRECISION) * CAST(quantity AS DOUBLE PRECISION) AS pnl
                    FROM trades t WHERE 1=1 {time_clause}
                    UNION ALL
                    SELECT o.owner AS wallet, o.realized_pnl_usdc AS pnl
                    FROM paper_outcomes o WHERE 1=1 {outcome_time_clause}
                    UNION ALL
                    SELECT o.owner AS wallet, o.realized_pnl_usdc AS pnl
                    FROM external_outcomes o WHERE 1=1 {outcome_time_clause}
                ) sub
                GROUP BY wallet
                ORDER BY value DESC",
                time_clause = time_clause,
                outcome_time_clause = outcome_time_clause,
            ),
        ).await?;
        total_entries += pnl_entries;

        // --- Volume ---
        let volume_entries = compute_metric_from_trades(
            pool, period, "volume", &now, time_clause,
            &format!(
                "SELECT wallet, SUM(vol) AS value FROM (
                    SELECT buyer AS wallet,
                           CAST(price AS DOUBLE PRECISION) * CAST(quantity AS DOUBLE PRECISION) AS vol
                    FROM trades t WHERE 1=1 {time_clause}
                    UNION ALL
                    SELECT seller AS wallet,
                           CAST(price AS DOUBLE PRECISION) * CAST(quantity AS DOUBLE PRECISION) AS vol
                    FROM trades t WHERE 1=1 {time_clause}
                    UNION ALL
                    SELECT p.owner AS wallet, p.notional_usdc AS vol
                    FROM paper_fills p WHERE 1=1 {paper_time_clause}
                    UNION ALL
                    SELECT p.owner AS wallet, p.notional_usdc AS vol
                    FROM external_fills p WHERE 1=1 {paper_time_clause}
                ) sub
                GROUP BY wallet
                ORDER BY value DESC",
                time_clause = time_clause,
                paper_time_clause = paper_time_clause,
            ),
        ).await?;
        total_entries += volume_entries;

        // --- Trades ---
        let trades_entries = compute_metric_from_trades(
            pool, period, "trades", &now, time_clause,
            &format!(
                "SELECT wallet, COUNT(*)::DOUBLE PRECISION AS value FROM (
                    SELECT buyer AS wallet FROM trades t WHERE 1=1 {time_clause}
                    UNION ALL
                    SELECT seller AS wallet FROM trades t WHERE 1=1 {time_clause}
                    UNION ALL
                    SELECT p.owner AS wallet FROM paper_fills p WHERE 1=1 {paper_time_clause}
                    UNION ALL
                    SELECT p.owner AS wallet FROM external_fills p WHERE 1=1 {paper_time_clause}
                ) sub
                GROUP BY wallet
                ORDER BY value DESC",
                time_clause = time_clause,
                paper_time_clause = paper_time_clause,
            ),
        ).await?;
        total_entries += trades_entries;

        // --- Win Rate ---
        // Uses closed outcomes: win = positive gross PnL
        let win_rate_entries = compute_metric_from_trades(
            pool, period, "win_rate", &now, time_clause,
            &format!(
                "SELECT wallet,
                        CASE WHEN COUNT(*) = 0 THEN 0
                             ELSE SUM(CASE WHEN is_win THEN 1 ELSE 0 END)::DOUBLE PRECISION / COUNT(*)::DOUBLE PRECISION
                        END AS value
                FROM (
                    SELECT buyer AS wallet,
                           (CAST(price AS DOUBLE PRECISION) < 0.5) AS is_win
                    FROM trades t WHERE 1=1 {time_clause}
                    UNION ALL
                    SELECT seller AS wallet,
                           (CAST(price AS DOUBLE PRECISION) >= 0.5) AS is_win
                    FROM trades t WHERE 1=1 {time_clause}
                    UNION ALL
                    SELECT o.owner AS wallet, (o.gross_pnl_usdc > 0) AS is_win
                    FROM paper_outcomes o WHERE 1=1 {outcome_time_clause}
                    UNION ALL
                    SELECT o.owner AS wallet, (o.gross_pnl_usdc > 0) AS is_win
                    FROM external_outcomes o WHERE 1=1 {outcome_time_clause}
                ) sub
                GROUP BY wallet
                HAVING COUNT(*) >= 5
                ORDER BY value DESC",
                time_clause = time_clause,
                outcome_time_clause = outcome_time_clause,
            ),
        ).await?;
        total_entries += win_rate_entries;
    }

    Ok(HttpResponse::Ok().json(json!({
        "computed": true,
        "periods": computed_periods,
        "entriesPerMetric": total_entries,
        "timestamp": now.to_rfc3339(),
    })))
}

/// Runs an aggregation query, assigns ranks, looks up previous ranks, and inserts
/// new snapshot rows. Returns the number of entries inserted.
async fn compute_metric_from_trades(
    pool: &sqlx::PgPool,
    period: &str,
    metric: &str,
    snapshot_time: &chrono::DateTime<Utc>,
    _time_clause: &str,
    agg_query: &str,
) -> Result<i64, ApiError> {
    // Fetch aggregated values
    let rows = sqlx::query(agg_query)
        .fetch_all(pool)
        .await?;

    if rows.is_empty() {
        return Ok(0);
    }

    // Get previous snapshot time for change tracking
    let prev_snapshot = sqlx::query(
        "SELECT snapshot_time FROM leaderboard_snapshots
         WHERE period = $1 AND metric = $2 AND snapshot_time < $3
         ORDER BY snapshot_time DESC
         LIMIT 1"
    )
    .bind(period)
    .bind(metric)
    .bind(snapshot_time)
    .fetch_optional(pool)
    .await?;

    let prev_time: Option<chrono::DateTime<Utc>> = prev_snapshot.map(|r| r.get("snapshot_time"));

    let mut inserted: i64 = 0;

    for (idx, row) in rows.iter().enumerate() {
        let wallet: String = row.get("wallet");
        let value: f64 = row.get("value");
        let rank = (idx + 1) as i32;

        // Look up previous rank
        let (previous_rank, change) = if let Some(pt) = prev_time {
            let prev_row = sqlx::query(
                "SELECT rank, value FROM leaderboard_snapshots
                 WHERE period = $1 AND metric = $2 AND snapshot_time = $3 AND wallet_address = $4"
            )
            .bind(period)
            .bind(metric)
            .bind(pt)
            .bind(&wallet)
            .fetch_optional(pool)
            .await?;

            match prev_row {
                Some(pr) => {
                    let prev_r: i32 = pr.get("rank");
                    let prev_v: f64 = pr.get("value");
                    (Some(prev_r), Some(value - prev_v))
                }
                None => (None, None),
            }
        } else {
            (None, None)
        };

        sqlx::query(
            "INSERT INTO leaderboard_snapshots
                (wallet_address, period, metric, value, rank, previous_rank, change, snapshot_time)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (wallet_address, period, metric, snapshot_time)
             DO UPDATE SET value = EXCLUDED.value,
                           rank = EXCLUDED.rank,
                           previous_rank = EXCLUDED.previous_rank,
                           change = EXCLUDED.change"
        )
        .bind(&wallet)
        .bind(period)
        .bind(metric)
        .bind(value)
        .bind(rank)
        .bind(previous_rank)
        .bind(change)
        .bind(snapshot_time)
        .execute(pool)
        .await?;

        inserted += 1;
    }

    Ok(inserted)
}
