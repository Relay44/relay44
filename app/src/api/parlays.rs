use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::auth::extract_authenticated_user;
use crate::api::ApiError;
use crate::AppState;

fn ensure_parlays_enabled(state: &AppState) -> Result<(), ApiError> {
    if !state.config.parlays_enabled {
        return Err(ApiError::bad_request("PARLAYS_DISABLED", "parlays are disabled"));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateParlayRequest {
    pub stake_usdc: f64,
    pub legs: Vec<ParlayLegRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParlayLegRequest {
    pub market_slug: String,
    pub market_id: Option<i64>,
    pub outcome_yes: bool,
    pub odds_bps: i32,
}

/// POST /v1/parlays — create a new parlay bet.
pub async fn create_parlay(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateParlayRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_parlays_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    if body.legs.len() < 2 || body.legs.len() > 10 {
        return Err(ApiError::bad_request(
            "INVALID_LEG_COUNT",
            "parlay must have 2-10 legs",
        ));
    }
    if body.stake_usdc <= 0.0 {
        return Err(ApiError::bad_request("INVALID_STAKE", "stake must be positive"));
    }

    // Reject duplicate markets within the same parlay.
    let mut seen_slugs = std::collections::HashSet::new();
    for leg in &body.legs {
        if !seen_slugs.insert(&leg.market_slug) {
            return Err(ApiError::bad_request(
                "DUPLICATE_MARKET",
                "each leg must reference a different market",
            ));
        }
    }

    // Validate odds and compute potential payout.
    for leg in &body.legs {
        if leg.odds_bps < 10000 || leg.odds_bps > 1_000_000 {
            return Err(ApiError::bad_request(
                "INVALID_ODDS",
                "odds_bps must be between 10000 (1x) and 1000000 (100x)",
            ));
        }
    }

    let mut payout_multiplier = 1.0_f64;
    for leg in &body.legs {
        payout_multiplier *= leg.odds_bps as f64 / 10_000.0;
    }
    let potential_payout = body.stake_usdc * payout_multiplier;

    let id = Uuid::new_v4().to_string();
    let leg_count = body.legs.len() as i32;

    let mut tx = state
        .db
        .pool()
        .begin()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    sqlx::query(
        "INSERT INTO parlays (id, owner, stake_usdc, leg_count) VALUES ($1, $2, $3, $4)",
    )
    .bind(&id)
    .bind(user.wallet_address.as_str())
    .bind(body.stake_usdc)
    .bind(leg_count)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    for (i, leg) in body.legs.iter().enumerate() {
        sqlx::query(
            "INSERT INTO parlay_legs (parlay_id, leg_index, market_slug, market_id, outcome_yes, odds_bps) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&id)
        .bind(i as i32)
        .bind(&leg.market_slug)
        .bind(leg.market_id)
        .bind(leg.outcome_yes)
        .bind(leg.odds_bps)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({
        "id": id,
        "stakeUsdc": body.stake_usdc,
        "legCount": leg_count,
        "potentialPayout": potential_payout,
        "payoutMultiplier": payout_multiplier,
        "status": "active",
    })))
}

/// GET /v1/parlays — list user's parlays.
pub async fn list_parlays(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_parlays_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    let rows: Vec<(String, f64, i32, i32, bool, Option<f64>, String, String)> = sqlx::query_as(
        "SELECT id, stake_usdc, leg_count, resolved_count, all_won, \
         payout_usdc, status, created_at::text \
         FROM parlays WHERE owner = $1 \
         ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user.wallet_address.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let items: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.0,
                "stakeUsdc": r.1,
                "legCount": r.2,
                "resolvedCount": r.3,
                "allWon": r.4,
                "payoutUsdc": r.5,
                "status": r.6,
                "createdAt": r.7,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "parlays": items })))
}

/// GET /v1/parlays/{parlay_id} — get parlay details with legs.
pub async fn get_parlay(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    parlay_id: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    ensure_parlays_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    let parlay: Option<(String, f64, i32, i32, bool, Option<f64>, String, String)> =
        sqlx::query_as(
            "SELECT id, stake_usdc, leg_count, resolved_count, all_won, \
             payout_usdc, status, created_at::text \
             FROM parlays WHERE id = $1 AND owner = $2",
        )
        .bind(parlay_id.as_str())
        .bind(user.wallet_address.as_str())
        .fetch_optional(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let parlay = parlay.ok_or_else(|| ApiError::not_found("Parlay"))?;

    let legs: Vec<(i32, String, Option<i64>, bool, i32, bool, Option<bool>)> = sqlx::query_as(
        "SELECT leg_index, market_slug, market_id, outcome_yes, odds_bps, resolved, won \
         FROM parlay_legs WHERE parlay_id = $1 ORDER BY leg_index",
    )
    .bind(parlay_id.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let leg_items: Vec<_> = legs
        .iter()
        .map(|l| {
            json!({
                "legIndex": l.0,
                "marketSlug": l.1,
                "marketId": l.2,
                "outcomeYes": l.3,
                "oddsBps": l.4,
                "resolved": l.5,
                "won": l.6,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({
        "id": parlay.0,
        "stakeUsdc": parlay.1,
        "legCount": parlay.2,
        "resolvedCount": parlay.3,
        "allWon": parlay.4,
        "payoutUsdc": parlay.5,
        "status": parlay.6,
        "createdAt": parlay.7,
        "legs": leg_items,
    })))
}

/// POST /v1/parlays/{parlay_id}/resolve — resolve a leg (operator endpoint).
pub async fn resolve_leg(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    parlay_id: web::Path<String>,
    body: web::Json<ResolveLegRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_parlays_enabled(&state)?;
    crate::api::compliance::ensure_admin_public(&req, &state)?;

    if body.leg_index < 0 {
        return Err(ApiError::bad_request("INVALID_LEG_INDEX", "leg_index must be non-negative"));
    }

    let mut tx = state
        .db
        .pool()
        .begin()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let updated = sqlx::query(
        "UPDATE parlay_legs SET resolved = true, won = $1, resolved_at = NOW() \
         WHERE parlay_id = $2 AND leg_index = $3 AND resolved = false",
    )
    .bind(body.won)
    .bind(parlay_id.as_str())
    .bind(body.leg_index)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    if updated.rows_affected() == 0 {
        return Err(ApiError::bad_request(
            "ALREADY_RESOLVED",
            "leg already resolved or not found",
        ));
    }

    // Update parlay resolved count and all_won.
    sqlx::query(
        "UPDATE parlays SET \
         resolved_count = resolved_count + 1, \
         all_won = all_won AND $1, \
         updated_at = NOW() \
         WHERE id = $2",
    )
    .bind(body.won)
    .bind(parlay_id.as_str())
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Check if fully resolved → auto-settle.
    let parlay: (i32, i32, bool, f64) = sqlx::query_as(
        "SELECT leg_count, resolved_count, all_won, stake_usdc FROM parlays WHERE id = $1",
    )
    .bind(parlay_id.as_str())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let (leg_count, resolved_count, all_won, stake) = parlay;
    let mut payout = None;

    if resolved_count >= leg_count {
        if all_won {
            // Compute payout from leg odds.
            let legs: Vec<(i32,)> = sqlx::query_as(
                "SELECT odds_bps FROM parlay_legs WHERE parlay_id = $1 ORDER BY leg_index",
            )
            .bind(parlay_id.as_str())
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?;

            let mut total = stake;
            for (odds,) in &legs {
                total = total * (*odds as f64) / 10_000.0;
            }
            payout = Some(total);
        }

        sqlx::query(
            "UPDATE parlays SET status = 'settled', payout_usdc = $1, settled_at = NOW() \
             WHERE id = $2 AND status != 'settled'",
        )
        .bind(payout)
        .bind(parlay_id.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({
        "ok": true,
        "legIndex": body.leg_index,
        "won": body.won,
        "fullyResolved": resolved_count >= leg_count,
        "payout": payout,
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveLegRequest {
    pub leg_index: i32,
    pub won: bool,
}
