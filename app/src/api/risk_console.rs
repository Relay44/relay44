use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::api::auth::extract_authenticated_user;
use crate::api::ApiError;
use crate::AppState;

fn ensure_risk_console_enabled(state: &AppState) -> Result<(), ApiError> {
    if !state.config.risk_console_enabled {
        return Err(ApiError::bad_request("RISK_CONSOLE_DISABLED", "risk console is disabled"));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotQuery {
    pub days: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplianceExportQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub event_type: Option<String>,
    pub limit: Option<i64>,
}

/// GET /v1/risk/portfolio — current portfolio risk snapshot.
pub async fn get_portfolio(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_risk_console_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    // Get latest snapshot.
    let latest: Option<(
        f64, f64, f64, i32, f64, f64, Option<f64>, Option<f64>, f64, f64, String,
    )> = sqlx::query_as(
        "SELECT total_value_usdc, unrealized_pnl_usdc, realized_pnl_usdc, \
         open_positions, gross_exposure_usdc, max_single_position_pct, \
         var_95_usdc, var_99_usdc, drawdown_from_peak_pct, peak_value_usdc, \
         snapshot_at::text \
         FROM portfolio_snapshots WHERE owner = $1 \
         ORDER BY snapshot_at DESC LIMIT 1",
    )
    .bind(user.wallet_address.as_str())
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    match latest {
        Some(r) => Ok(HttpResponse::Ok().json(json!({
            "totalValueUsdc": r.0,
            "unrealizedPnlUsdc": r.1,
            "realizedPnlUsdc": r.2,
            "openPositions": r.3,
            "grossExposureUsdc": r.4,
            "maxSinglePositionPct": r.5,
            "var95Usdc": r.6,
            "var99Usdc": r.7,
            "drawdownFromPeakPct": r.8,
            "peakValueUsdc": r.9,
            "snapshotAt": r.10,
        }))),
        None => Ok(HttpResponse::Ok().json(json!({
            "totalValueUsdc": 0,
            "openPositions": 0,
            "message": "no portfolio data yet",
        }))),
    }
}

/// GET /v1/risk/history — portfolio value time series.
pub async fn get_portfolio_history(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<SnapshotQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_risk_console_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;
    let days = query.days.unwrap_or(30).min(365);
    let limit = query.limit.unwrap_or(100).min(1000);

    let rows: Vec<(f64, f64, f64, i32, f64, Option<f64>, f64, String)> = sqlx::query_as(
        "SELECT total_value_usdc, unrealized_pnl_usdc, realized_pnl_usdc, \
         open_positions, gross_exposure_usdc, var_95_usdc, \
         drawdown_from_peak_pct, snapshot_at::text \
         FROM portfolio_snapshots \
         WHERE owner = $1 AND snapshot_at >= NOW() - make_interval(days => $2) \
         ORDER BY snapshot_at DESC LIMIT $3",
    )
    .bind(user.wallet_address.as_str())
    .bind(days as i32)
    .bind(limit)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let points: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "totalValueUsdc": r.0,
                "unrealizedPnl": r.1,
                "realizedPnl": r.2,
                "openPositions": r.3,
                "grossExposure": r.4,
                "var95": r.5,
                "drawdownPct": r.6,
                "snapshotAt": r.7,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "history": points })))
}

/// GET /v1/risk/drawdown — drawdown analysis.
pub async fn get_drawdown(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_risk_console_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    let stats: Option<(f64, f64, f64, f64, String)> = sqlx::query_as(
        "SELECT MAX(drawdown_from_peak_pct) as max_dd, \
         AVG(drawdown_from_peak_pct) as avg_dd, \
         MAX(peak_value_usdc) as peak, \
         MIN(total_value_usdc) as trough, \
         MAX(snapshot_at)::text as latest \
         FROM portfolio_snapshots WHERE owner = $1",
    )
    .bind(user.wallet_address.as_str())
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    match stats {
        Some(r) => Ok(HttpResponse::Ok().json(json!({
            "maxDrawdownPct": r.0,
            "avgDrawdownPct": r.1,
            "peakValueUsdc": r.2,
            "troughValueUsdc": r.3,
            "latestSnapshot": r.4,
        }))),
        None => Ok(HttpResponse::Ok().json(json!({
            "maxDrawdownPct": 0,
            "message": "no drawdown data yet",
        }))),
    }
}

/// GET /v1/risk/compliance/export — export compliance events.
pub async fn export_compliance(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ComplianceExportQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_risk_console_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;
    let limit = query.limit.unwrap_or(500).min(5000);

    // Build dynamic WHERE clause for optional filters.
    let from_ts = query.from.as_deref().unwrap_or("1970-01-01");
    let to_ts = query.to.as_deref().unwrap_or("2100-01-01");

    let rows: Vec<(
        i32, String, Option<i64>, Option<String>, Option<String>,
        Option<f64>, Option<String>, Option<String>, Option<String>,
        serde_json::Value, String,
    )> = if let Some(event_type) = &query.event_type {
        sqlx::query_as(
            "SELECT id, event_type, market_id, market_slug, side, amount_usdc, \
             counterparty, provider, tx_hash, metadata, created_at::text \
             FROM compliance_events \
             WHERE owner = $1 AND event_type = $2 \
               AND created_at >= $3::timestamptz AND created_at <= $4::timestamptz \
             ORDER BY created_at DESC LIMIT $5",
        )
        .bind(user.wallet_address.as_str())
        .bind(event_type)
        .bind(from_ts)
        .bind(to_ts)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    } else {
        sqlx::query_as(
            "SELECT id, event_type, market_id, market_slug, side, amount_usdc, \
             counterparty, provider, tx_hash, metadata, created_at::text \
             FROM compliance_events \
             WHERE owner = $1 \
               AND created_at >= $2::timestamptz AND created_at <= $3::timestamptz \
             ORDER BY created_at DESC LIMIT $4",
        )
        .bind(user.wallet_address.as_str())
        .bind(from_ts)
        .bind(to_ts)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    };

    let events: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.0,
                "eventType": r.1,
                "marketId": r.2,
                "marketSlug": r.3,
                "side": r.4,
                "amountUsdc": r.5,
                "counterparty": r.6,
                "provider": r.7,
                "txHash": r.8,
                "metadata": r.9,
                "createdAt": r.10,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({
        "events": events,
        "count": events.len(),
    })))
}
