use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::api::auth::extract_authenticated_user;
use crate::api::ApiError;
use crate::services::polymarket_scanner;
use crate::AppState;

fn ensure_scanner_enabled(state: &AppState) -> Result<(), ApiError> {
    if !state.config.polymarket_enabled {
        return Err(ApiError::bad_request(
            "PM_SCANNER_DISABLED",
            "polymarket scanner is disabled",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpportunitiesQuery {
    pub opportunity_type: Option<String>,
    pub category: Option<String>,
    pub min_score: Option<f64>,
    pub min_liquidity: Option<f64>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
}

/// GET /v1/pm-scanner/opportunities — list scored opportunities.
pub async fn list_opportunities(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<OpportunitiesQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_scanner_enabled(&state)?;
    let _ = extract_authenticated_user(&req, &state).await?;

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let sort = query
        .sort
        .as_deref()
        .map(polymarket_scanner::OpportunitySort::parse)
        .unwrap_or_default();

    let filter = polymarket_scanner::OpportunityFilter {
        opportunity_type: query.opportunity_type.as_deref(),
        category: query.category.as_deref(),
        min_score: query.min_score,
        min_liquidity: query.min_liquidity,
        sort,
        limit,
    };

    let opportunities = polymarket_scanner::list_opportunities(&state, filter)
        .await
        .map_err(|e| ApiError::internal(&e))?;

    Ok(HttpResponse::Ok().json(json!({
        "opportunities": opportunities,
        "count": opportunities.len(),
    })))
}

/// POST /v1/pm-scanner/scan — trigger a manual scan cycle.
pub async fn trigger_scan(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_scanner_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    // Admin-only manual trigger
    if !state
        .config
        .admin_wallets
        .contains(&user.wallet_address.to_ascii_lowercase())
    {
        return Err(ApiError::forbidden("manual scan requires admin"));
    }

    let opportunities = polymarket_scanner::run_scan(&state)
        .await
        .map_err(|e| ApiError::internal(&e))?;

    let longshots = opportunities
        .iter()
        .filter(|o| o.opportunity_type.starts_with("longshot"))
        .count();
    let near_certs = opportunities
        .iter()
        .filter(|o| o.opportunity_type.starts_with("near_certainty"))
        .count();
    let spreads = opportunities
        .iter()
        .filter(|o| o.opportunity_type == "spread_capture")
        .count();

    Ok(HttpResponse::Ok().json(json!({
        "scanned": true,
        "totalOpportunities": opportunities.len(),
        "longshots": longshots,
        "nearCertainties": near_certs,
        "spreadCaptures": spreads,
    })))
}

/// GET /v1/pm-scanner/calibration — view calibration bucket data.
pub async fn get_calibration(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_scanner_enabled(&state)?;
    let _ = extract_authenticated_user(&req, &state).await?;

    let pool = state.db.pool();
    let rows = sqlx::query_as::<_, (serde_json::Value,)>(
        r#"
        SELECT json_build_object(
            'priceBucketLow', price_bucket_low,
            'priceBucketHigh', price_bucket_high,
            'category', category,
            'totalPositions', total_positions,
            'wins', wins,
            'actualWinRate', actual_win_rate,
            'impliedProbability', implied_probability,
            'mispricingPct', mispricing_pct,
            'lastUpdatedAt', last_updated_at
        )
        FROM calibration_buckets
        ORDER BY category, price_bucket_low
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let buckets: Vec<serde_json::Value> = rows.into_iter().map(|(v,)| v).collect();

    Ok(HttpResponse::Ok().json(json!({
        "calibrationBuckets": buckets,
        "count": buckets.len(),
    })))
}

/// GET /v1/pm-scanner/runs — view recent scan history.
pub async fn list_scan_runs(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_scanner_enabled(&state)?;
    let _ = extract_authenticated_user(&req, &state).await?;

    let pool = state.db.pool();
    let rows = sqlx::query_as::<_, (serde_json::Value,)>(
        r#"
        SELECT json_build_object(
            'id', id,
            'startedAt', started_at,
            'completedAt', completed_at,
            'marketsScanned', markets_scanned,
            'opportunitiesFound', opportunities_found,
            'longshotsFound', longshots_found,
            'nearCertaintiesFound', near_certainties_found,
            'spreadCapturesFound', spread_captures_found,
            'error', error
        )
        FROM polymarket_scanner_runs
        ORDER BY started_at DESC
        LIMIT 50
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let runs: Vec<serde_json::Value> = rows.into_iter().map(|(v,)| v).collect();

    Ok(HttpResponse::Ok().json(json!({
        "runs": runs,
        "count": runs.len(),
    })))
}
