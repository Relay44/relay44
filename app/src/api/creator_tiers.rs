use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::api::auth::extract_authenticated_user;
use crate::api::ApiError;
use crate::AppState;

fn ensure_creator_tiers_enabled(state: &AppState) -> Result<(), ApiError> {
    if !state.config.creator_tiers_enabled {
        return Err(ApiError::bad_request("CREATOR_TIERS_DISABLED", "creator tiers are disabled"));
    }
    Ok(())
}

/// GET /v1/creator/tiers — list available tiers.
pub async fn list_tiers(
    _req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_creator_tiers_enabled(&state)?;
    let rows: Vec<(String, String, f64, i32, i32, bool)> = sqlx::query_as(
        "SELECT id, name, max_seed_usdc, platform_take_bps, max_markets, priority_placement \
         FROM creator_tiers ORDER BY max_seed_usdc ASC",
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let tiers: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.0,
                "name": r.1,
                "maxSeedUsdc": r.2,
                "platformTakeBps": r.3,
                "maxMarkets": r.4,
                "priorityPlacement": r.5,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "tiers": tiers })))
}

/// GET /v1/creator/profile — get or create creator profile.
pub async fn get_profile(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_creator_tiers_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    // Upsert creator profile.
    let row: (String, f64, f64, f64, i32, i32, f64, String) = sqlx::query_as(
        "INSERT INTO creator_profiles (owner) VALUES ($1) \
         ON CONFLICT (owner) DO UPDATE SET updated_at = NOW() \
         RETURNING tier_id, total_seed_deployed, total_pnl_usdc, \
         total_platform_fees_usdc, markets_created, markets_graduated, \
         staking_amount_usdc, updated_at::text",
    )
    .bind(user.wallet_address.as_str())
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Get tier details.
    let tier: Option<(String, f64, i32, i32, bool)> = sqlx::query_as(
        "SELECT name, max_seed_usdc, platform_take_bps, max_markets, priority_placement \
         FROM creator_tiers WHERE id = $1",
    )
    .bind(&row.0)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({
        "tierId": row.0,
        "tier": tier.as_ref().map(|t| json!({
            "name": t.0,
            "maxSeedUsdc": t.1,
            "platformTakeBps": t.2,
            "maxMarkets": t.3,
            "priorityPlacement": t.4,
        })),
        "totalSeedDeployed": row.1,
        "totalPnlUsdc": row.2,
        "totalPlatformFeesUsdc": row.3,
        "marketsCreated": row.4,
        "marketsGraduated": row.5,
        "stakingAmountUsdc": row.6,
        "updatedAt": row.7,
    })))
}

/// POST /v1/creator/upgrade — request tier upgrade.
pub async fn upgrade_tier(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<UpgradeTierRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_creator_tiers_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    // Validate tier exists and is an upgrade.
    let target: Option<(String, f64)> = sqlx::query_as(
        "SELECT id, max_seed_usdc FROM creator_tiers WHERE id = $1",
    )
    .bind(&body.tier_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let (tier_id, _) = target.ok_or_else(|| ApiError::not_found("Tier"))?;

    // Get current tier.
    let current: Option<(String,)> = sqlx::query_as(
        "SELECT tier_id FROM creator_profiles WHERE owner = $1",
    )
    .bind(user.wallet_address.as_str())
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let current_tier = current.map(|c| c.0).unwrap_or_else(|| "starter".to_string());

    let tier_order = ["starter", "pro", "institutional"];
    let current_idx = tier_order.iter().position(|t| *t == current_tier).unwrap_or(0);
    let target_idx = tier_order.iter().position(|t| *t == tier_id).unwrap_or(0);

    if target_idx <= current_idx {
        return Err(ApiError::bad_request(
            "NOT_AN_UPGRADE",
            "target tier must be higher than current tier",
        ));
    }

    sqlx::query(
        "UPDATE creator_profiles SET tier_id = $1, updated_at = NOW() WHERE owner = $2",
    )
    .bind(&tier_id)
    .bind(user.wallet_address.as_str())
    .execute(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({ "ok": true, "tierId": tier_id })))
}

/// GET /v1/creator/fees — get fee ledger.
pub async fn list_fees(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_creator_tiers_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    let rows: Vec<(i32, Option<i64>, String, f64, String, i32, String)> = sqlx::query_as(
        "SELECT id, market_id, fee_type, amount_usdc, tier_id, take_bps, created_at::text \
         FROM platform_fee_ledger \
         WHERE creator_owner = $1 \
         ORDER BY created_at DESC LIMIT 100",
    )
    .bind(user.wallet_address.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let fees: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.0,
                "marketId": r.1,
                "feeType": r.2,
                "amountUsdc": r.3,
                "tierId": r.4,
                "takeBps": r.5,
                "createdAt": r.6,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "fees": fees })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeTierRequest {
    pub tier_id: String,
}
