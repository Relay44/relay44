use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::api::auth::extract_authenticated_user;
use crate::api::ApiError;
use crate::services::smart_router;
use crate::AppState;

fn ensure_routing_enabled(state: &AppState) -> Result<(), ApiError> {
    if !state.config.routing_enabled {
        return Err(ApiError::bad_request("ROUTING_DISABLED", "smart routing is disabled"));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteOrderRequest {
    pub market_slug: String,
    pub outcome: String,
    pub side: String,
    pub quantity: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArbQuery {
    pub market_slug: Option<String>,
    pub min_spread_bps: Option<i32>,
    pub limit: Option<i64>,
}

/// POST /v1/routing/quote — get best execution venue for an order.
pub async fn route_order(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<RouteOrderRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_routing_enabled(&state)?;
    let _ = extract_authenticated_user(&req, &state).await?;

    if body.side != "buy" && body.side != "sell" {
        return Err(ApiError::bad_request("INVALID_SIDE", "side must be buy or sell"));
    }
    if body.quantity <= 0.0 {
        return Err(ApiError::bad_request("INVALID_QUANTITY", "quantity must be positive"));
    }

    let decision = smart_router::route_order(&state, &body.market_slug, &body.outcome, &body.side, body.quantity)
        .await
        .map_err(|e| ApiError::bad_request("ROUTING_FAILED", &e))?;

    Ok(HttpResponse::Ok().json(json!({
        "chosen": decision.chosen,
        "alternatives": decision.alternatives,
        "savingsBps": decision.savings_bps,
    })))
}

/// GET /v1/routing/arbitrage — list detected arbitrage opportunities.
pub async fn list_arbitrage(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ArbQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_routing_enabled(&state)?;
    let _ = extract_authenticated_user(&req, &state).await?;

    let min_spread = query.min_spread_bps.unwrap_or(10);
    let limit = query.limit.unwrap_or(50).min(200);

    let rows: Vec<(
        i32, String, String, String, f64, String, f64, i32, f64, String, String,
    )> = if let Some(slug) = &query.market_slug {
        sqlx::query_as(
            "SELECT id, market_slug, outcome, buy_provider, buy_price::float8, \
             sell_provider, sell_price::float8, spread_bps, max_size_usdc::float8, \
             status, created_at::text \
             FROM arbitrage_opportunities \
             WHERE market_slug = $1 AND spread_bps >= $2 \
             ORDER BY created_at DESC LIMIT $3",
        )
        .bind(slug)
        .bind(min_spread)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    } else {
        sqlx::query_as(
            "SELECT id, market_slug, outcome, buy_provider, buy_price::float8, \
             sell_provider, sell_price::float8, spread_bps, max_size_usdc::float8, \
             status, created_at::text \
             FROM arbitrage_opportunities \
             WHERE spread_bps >= $1 \
             ORDER BY created_at DESC LIMIT $2",
        )
        .bind(min_spread)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    };

    let items: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.0,
                "marketSlug": r.1,
                "outcome": r.2,
                "buyProvider": r.3,
                "buyPrice": r.4,
                "sellProvider": r.5,
                "sellPrice": r.6,
                "spreadBps": r.7,
                "maxSizeUsdc": r.8,
                "status": r.9,
                "createdAt": r.10,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "opportunities": items })))
}

/// POST /v1/routing/venues — manage venue links for a market (operator only).
pub async fn upsert_venue_link(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<UpsertVenueLinkRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_routing_enabled(&state)?;
    crate::api::compliance::ensure_admin_public(&req, &state)?;

    let row = sqlx::query_as::<_, (i32,)>(
        "INSERT INTO market_venue_links (market_slug, provider, provider_market_id, fee_bps) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (market_slug, provider) \
         DO UPDATE SET provider_market_id = EXCLUDED.provider_market_id, \
                       fee_bps = EXCLUDED.fee_bps, \
                       active = true, \
                       updated_at = NOW() \
         RETURNING id",
    )
    .bind(&body.market_slug)
    .bind(&body.provider)
    .bind(&body.provider_market_id)
    .bind(body.fee_bps.unwrap_or(0))
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({ "id": row.0, "ok": true })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertVenueLinkRequest {
    pub market_slug: String,
    pub provider: String,
    pub provider_market_id: String,
    pub fee_bps: Option<i32>,
}
