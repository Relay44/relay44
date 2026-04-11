use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;

use super::rate_limit::{check_rate_limit_by_user, RateLimitTier};
use super::ApiError;
use crate::AppState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a short, URL-safe referral code: 8 hex chars derived from random bytes.
fn generate_referral_code() -> String {
    let bytes: [u8; 4] = rand::random();
    hex::encode(bytes)
}

// ---------------------------------------------------------------------------
// POST /v1/referrals/generate — create or return existing referral code
// ---------------------------------------------------------------------------

pub async fn generate_code(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = crate::api::auth::extract_authenticated_user(&req, &state).await?;
    check_rate_limit_by_user(&user.wallet_address, &state.redis, RateLimitTier::Write).await?;
    let wallet = user.wallet_address.to_lowercase();
    let pool = state.db.pool();

    // Check if user already has a referral code (look for any referral they created)
    let existing = sqlx::query(
        "SELECT DISTINCT referral_code FROM referrals WHERE LOWER(referrer_wallet) = $1 LIMIT 1",
    )
    .bind(&wallet)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = existing {
        let code: String = row.get("referral_code");
        return Ok(HttpResponse::Ok().json(json!({
            "code": code,
            "created": false,
        })));
    }

    // Generate a unique code, retry on collision
    let mut code = generate_referral_code();
    for _ in 0..5 {
        let collision = sqlx::query("SELECT 1 FROM referrals WHERE referral_code = $1 LIMIT 1")
            .bind(&code)
            .fetch_optional(pool)
            .await?;

        if collision.is_none() {
            break;
        }
        code = generate_referral_code();
    }

    // Insert a self-referral placeholder row to "claim" the code for this wallet.
    // The referee_wallet is set to a sentinel that won't conflict with real wallets.
    // We store the code association via a dedicated lookup: just store an initial row
    // with status='code_owner' so the code is tied to this wallet.
    // Actually, let's keep it simpler: store the code in its own lightweight table-less
    // approach — we'll just return the code and only persist rows on actual referrals.
    // BUT: we need to persist the code<->wallet mapping so GET /code works later.
    //
    // Strategy: use a single-row insert with a sentinel referee_wallet that is the
    // wallet itself (self-referral is blocked at the /apply level, not here).
    // We'll use a special status 'code_owner' to distinguish from real referrals.
    sqlx::query(
        "INSERT INTO referrals (referrer_wallet, referee_wallet, referral_code, status)
         VALUES ($1, $1, $2, 'code_owner')
         ON CONFLICT (referee_wallet) DO NOTHING",
    )
    .bind(&wallet)
    .bind(&code)
    .execute(pool)
    .await?;

    Ok(HttpResponse::Created().json(json!({
        "code": code,
        "created": true,
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/referrals/code — get the authenticated user's referral code
// ---------------------------------------------------------------------------

pub async fn get_code(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = crate::api::auth::extract_authenticated_user(&req, &state).await?;
    let wallet = user.wallet_address.to_lowercase();
    let pool = state.db.pool();

    let row = sqlx::query(
        "SELECT DISTINCT referral_code FROM referrals WHERE LOWER(referrer_wallet) = $1 LIMIT 1",
    )
    .bind(&wallet)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let code: String = r.get("referral_code");
            Ok(HttpResponse::Ok().json(json!({ "code": code })))
        }
        None => Ok(HttpResponse::Ok().json(json!({ "code": serde_json::Value::Null }))),
    }
}

// ---------------------------------------------------------------------------
// POST /v1/referrals/apply — apply a referral code (authenticated)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ApplyReferralRequest {
    pub code: String,
}

pub async fn apply_code(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<ApplyReferralRequest>,
) -> Result<impl Responder, ApiError> {
    let user = crate::api::auth::extract_authenticated_user(&req, &state).await?;
    check_rate_limit_by_user(&user.wallet_address, &state.redis, RateLimitTier::Write).await?;
    let wallet = user.wallet_address.to_lowercase();
    let pool = state.db.pool();

    let code = body.code.trim().to_lowercase();
    if code.is_empty() || code.len() > 32 {
        return Err(ApiError::bad_request(
            "INVALID_CODE",
            "Referral code must be between 1 and 32 characters",
        ));
    }

    // Look up the referrer who owns this code
    let referrer_row = sqlx::query(
        "SELECT LOWER(referrer_wallet) AS referrer_wallet
         FROM referrals
         WHERE referral_code = $1
         LIMIT 1",
    )
    .bind(&code)
    .fetch_optional(pool)
    .await?;

    let referrer_wallet: String = match referrer_row {
        Some(r) => r.get("referrer_wallet"),
        None => {
            return Err(ApiError::not_found("Referral code"));
        }
    };

    // Can't self-refer
    if referrer_wallet == wallet {
        return Err(ApiError::bad_request(
            "SELF_REFERRAL",
            "You cannot use your own referral code",
        ));
    }

    // Check if this referee already used any referral code
    let already_referred = sqlx::query(
        "SELECT 1 FROM referrals
         WHERE LOWER(referee_wallet) = $1 AND status != 'code_owner'
         LIMIT 1",
    )
    .bind(&wallet)
    .fetch_optional(pool)
    .await?;

    if already_referred.is_some() {
        return Err(ApiError::conflict(
            "ALREADY_REFERRED",
            "You have already been referred",
        ));
    }

    // Insert the referral
    sqlx::query(
        "INSERT INTO referrals (referrer_wallet, referee_wallet, referral_code, status)
         VALUES ($1, $2, $3, 'active')",
    )
    .bind(&referrer_wallet)
    .bind(&wallet)
    .bind(&code)
    .execute(pool)
    .await
    .map_err(|e| {
        // Handle unique constraint violation on referee_wallet
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.code().map_or(false, |c| c == "23505") {
                return ApiError::conflict("ALREADY_REFERRED", "You have already been referred");
            }
        }
        ApiError::from(e)
    })?;

    log::info!(
        "Referral applied: referee={}, referrer={}, code={}",
        wallet,
        referrer_wallet,
        code
    );

    Ok(HttpResponse::Ok().json(json!({
        "ok": true,
        "referrer": referrer_wallet,
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/referrals/stats — referral stats for the authenticated user
// ---------------------------------------------------------------------------

pub async fn get_stats(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = crate::api::auth::extract_authenticated_user(&req, &state).await?;
    let wallet = user.wallet_address.to_lowercase();
    let pool = state.db.pool();

    // Count of active referrals (people this user referred)
    let count_row = sqlx::query(
        "SELECT
            COUNT(*)::BIGINT AS total_referrals,
            COALESCE(SUM(CASE WHEN rewarded THEN 1 ELSE 0 END), 0)::BIGINT AS rewarded_count
         FROM referrals
         WHERE LOWER(referrer_wallet) = $1 AND status = 'active'",
    )
    .bind(&wallet)
    .fetch_one(pool)
    .await?;

    let total_referrals: i64 = count_row.get("total_referrals");
    let rewarded_count: i64 = count_row.get("rewarded_count");

    // Check if this user was referred by someone
    let referred_by = sqlx::query(
        "SELECT LOWER(referrer_wallet) AS referrer_wallet
         FROM referrals
         WHERE LOWER(referee_wallet) = $1 AND status = 'active'
         LIMIT 1",
    )
    .bind(&wallet)
    .fetch_optional(pool)
    .await?;

    let referred_by_wallet: Option<String> = referred_by.map(|r| r.get("referrer_wallet"));

    // List of referees
    let referee_rows = sqlx::query(
        "SELECT LOWER(referee_wallet) AS referee_wallet, created_at, rewarded
         FROM referrals
         WHERE LOWER(referrer_wallet) = $1 AND status = 'active'
         ORDER BY created_at DESC
         LIMIT 100",
    )
    .bind(&wallet)
    .fetch_all(pool)
    .await?;

    let referees: Vec<serde_json::Value> = referee_rows
        .iter()
        .map(|row| {
            json!({
                "wallet": row.get::<String, _>("referee_wallet"),
                "createdAt": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
                "rewarded": row.get::<bool, _>("rewarded"),
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({
        "totalReferrals": total_referrals,
        "rewardedCount": rewarded_count,
        "pendingRewards": total_referrals - rewarded_count,
        "referredBy": referred_by_wallet,
        "referees": referees,
    })))
}
