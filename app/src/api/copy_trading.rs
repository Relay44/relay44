use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;

use crate::api::auth::extract_jwt_user;
use crate::api::rate_limit::{check_rate_limit_by_user, RateLimitTier};
use crate::api::validation::validate_wallet_address;
use crate::api::ApiError;
use crate::AppState;

/// Maximum number of active copy trading subscriptions per user
const MAX_ACTIVE_SUBSCRIPTIONS: i64 = 10;

// ── Types ────────────────────────────────────────────────────────────

/// Subscription status: 0=active, 1=paused, 2=cancelled
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionStatus(pub i32);

impl SubscriptionStatus {
    pub const ACTIVE: Self = Self(0);
    pub const PAUSED: Self = Self(1);
    pub const CANCELLED: Self = Self(2);

    pub fn is_active(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyTradingSubscription {
    pub id: String,
    pub subscriber: String,
    pub target_wallet: String,
    pub agent_id: Option<String>,
    pub allocation_usdc: f64,
    pub max_position_usdc: f64,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

// ── Request / Response ───────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeRequest {
    pub target_wallet: String,
    pub allocation_usdc: Option<f64>,
    pub max_position_usdc: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubscriptionRequest {
    pub active: Option<bool>,
    pub allocation_usdc: Option<f64>,
    pub max_position_usdc: Option<f64>,
}

#[derive(Deserialize)]
pub struct PaginationParams {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// Maximum pagination offset to prevent resource exhaustion on large skips
const MAX_PAGINATION_OFFSET: u64 = 10_000;

// ── Handlers ─────────────────────────────────────────────────────────

/// POST /copy-trading/subscribe
pub async fn subscribe(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<SubscribeRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let subscriber = user.wallet_address.to_ascii_lowercase();

    // Rate limit: use Write tier (30/min)
    check_rate_limit_by_user(&subscriber, &state.redis, RateLimitTier::Write).await?;

    let target = body.target_wallet.trim().to_ascii_lowercase();

    // Validate: target wallet address format
    validate_wallet_address(&target)?;

    // Validate: numeric inputs must be finite (reject NaN / Infinity)
    if let Some(v) = body.allocation_usdc {
        if !v.is_finite() {
            return Err(ApiError::bad_request(
                "INVALID_ALLOCATION",
                "allocation_usdc must be a finite number",
            ));
        }
    }
    if let Some(v) = body.max_position_usdc {
        if !v.is_finite() {
            return Err(ApiError::bad_request(
                "INVALID_MAX_POSITION",
                "max_position_usdc must be a finite number",
            ));
        }
    }

    // Validate: can't copy yourself
    if subscriber == target {
        return Err(ApiError::bad_request(
            "CANNOT_COPY_SELF",
            "You cannot copy your own trades",
        ));
    }

    // Validate: target wallet must exist in users table
    let target_exists = sqlx::query(
        "SELECT 1 FROM users WHERE LOWER(wallet) = LOWER($1)",
    )
    .bind(&target)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    if target_exists.is_none() {
        return Err(ApiError::bad_request(
            "TARGET_NOT_FOUND",
            "Target wallet is not a registered user",
        ));
    }

    // Validate: max 10 active subscriptions
    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM copy_trading_subscriptions WHERE subscriber = $1 AND active = true",
    )
    .bind(&subscriber)
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    if active_count >= MAX_ACTIVE_SUBSCRIPTIONS {
        return Err(ApiError::bad_request(
            "MAX_SUBSCRIPTIONS_REACHED",
            &format!(
                "Maximum of {} active copy trading subscriptions reached",
                MAX_ACTIVE_SUBSCRIPTIONS
            ),
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let allocation = body.allocation_usdc.unwrap_or(50.0).clamp(1.0, 100_000.0);
    let max_position = body
        .max_position_usdc
        .unwrap_or(20.0)
        .clamp(1.0, 50_000.0);

    let row = sqlx::query(
        r#"
        INSERT INTO copy_trading_subscriptions (id, subscriber, target_wallet, allocation_usdc, max_position_usdc, active)
        VALUES ($1, LOWER($2), LOWER($3), $4, $5, true)
        ON CONFLICT (subscriber, target_wallet) DO UPDATE
            SET active = true,
                allocation_usdc = EXCLUDED.allocation_usdc,
                max_position_usdc = EXCLUDED.max_position_usdc,
                updated_at = NOW()
        RETURNING id, subscriber, target_wallet, agent_id, allocation_usdc, max_position_usdc, active, created_at, updated_at
        "#,
    )
    .bind(&id)
    .bind(&subscriber)
    .bind(&target)
    .bind(allocation)
    .bind(max_position)
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let sub = map_subscription_row(&row);
    Ok(HttpResponse::Created().json(sub))
}

/// DELETE /copy-trading/subscribe/{subscription_id}
pub async fn unsubscribe(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let subscriber = user.wallet_address.to_ascii_lowercase();
    let subscription_id = path.into_inner();

    // Rate limit: use Write tier (30/min)
    check_rate_limit_by_user(&subscriber, &state.redis, RateLimitTier::Write).await?;

    let result = sqlx::query(
        r#"
        UPDATE copy_trading_subscriptions
        SET active = false, updated_at = NOW()
        WHERE id = $1 AND subscriber = LOWER($2) AND active = true
        RETURNING id
        "#,
    )
    .bind(&subscription_id)
    .bind(&subscriber)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    if result.is_none() {
        return Err(ApiError::not_found("Subscription"));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({ "ok": true })))
}

/// GET /copy-trading/subscriptions
pub async fn list_subscriptions(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<PaginationParams>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let subscriber = user.wallet_address.to_ascii_lowercase();
    let limit = query.limit.unwrap_or(50).clamp(1, 100) as i64;
    let offset = query.offset.unwrap_or(0).min(MAX_PAGINATION_OFFSET) as i64;

    let rows = sqlx::query(
        r#"
        SELECT id, subscriber, target_wallet, agent_id, allocation_usdc, max_position_usdc, active, created_at, updated_at
        FROM copy_trading_subscriptions
        WHERE subscriber = LOWER($1) AND active = true
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(&subscriber)
    .bind(limit)
    .bind(offset)
    .fetch_all(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let subscriptions: Vec<CopyTradingSubscription> =
        rows.iter().map(map_subscription_row).collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "data": subscriptions,
        "limit": limit,
        "offset": offset,
    })))
}

/// GET /copy-trading/subscribers
pub async fn get_subscriber_count(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let wallet = user.wallet_address.to_ascii_lowercase();

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM copy_trading_subscriptions WHERE target_wallet = LOWER($1) AND active = true",
    )
    .bind(&wallet)
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "copySubscriberCount": count,
    })))
}

/// PUT /copy-trading/subscribe/{id}
pub async fn update_subscription(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<UpdateSubscriptionRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let subscriber = user.wallet_address.to_ascii_lowercase();
    let subscription_id = path.into_inner();

    // Rate limit: use Write tier (30/min)
    check_rate_limit_by_user(&subscriber, &state.redis, RateLimitTier::Write).await?;

    // Validate: numeric inputs must be finite (reject NaN / Infinity)
    if let Some(v) = body.allocation_usdc {
        if !v.is_finite() {
            return Err(ApiError::bad_request(
                "INVALID_ALLOCATION",
                "allocation_usdc must be a finite number",
            ));
        }
    }
    if let Some(v) = body.max_position_usdc {
        if !v.is_finite() {
            return Err(ApiError::bad_request(
                "INVALID_MAX_POSITION",
                "max_position_usdc must be a finite number",
            ));
        }
    }

    // Fetch existing subscription to verify ownership
    let existing = sqlx::query(
        "SELECT id FROM copy_trading_subscriptions WHERE id = $1 AND subscriber = LOWER($2)",
    )
    .bind(&subscription_id)
    .bind(&subscriber)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    if existing.is_none() {
        return Err(ApiError::not_found("Subscription"));
    }

    // Build dynamic update
    let active = body.active;
    let allocation = body.allocation_usdc.map(|v| v.clamp(1.0, 100_000.0));
    let max_position = body.max_position_usdc.map(|v| v.clamp(1.0, 50_000.0));

    let row = sqlx::query(
        r#"
        UPDATE copy_trading_subscriptions
        SET active = COALESCE($3, active),
            allocation_usdc = COALESCE($4, allocation_usdc),
            max_position_usdc = COALESCE($5, max_position_usdc),
            updated_at = NOW()
        WHERE id = $1 AND subscriber = LOWER($2)
        RETURNING id, subscriber, target_wallet, agent_id, allocation_usdc, max_position_usdc, active, created_at, updated_at
        "#,
    )
    .bind(&subscription_id)
    .bind(&subscriber)
    .bind(active)
    .bind(allocation)
    .bind(max_position)
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let sub = map_subscription_row(&row);
    Ok(HttpResponse::Ok().json(sub))
}

/// GET /copy-trading/subscribe/{id}/history
pub async fn get_subscription_history(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<PaginationParams>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let subscriber = user.wallet_address.to_ascii_lowercase();
    let subscription_id = path.into_inner();
    let limit = query.limit.unwrap_or(50).clamp(1, 100) as i64;
    let offset = query.offset.unwrap_or(0).min(MAX_PAGINATION_OFFSET) as i64;

    // Verify ownership
    let existing = sqlx::query(
        "SELECT agent_id FROM copy_trading_subscriptions WHERE id = $1 AND subscriber = LOWER($2)",
    )
    .bind(&subscription_id)
    .bind(&subscriber)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    let row = match existing {
        Some(r) => r,
        None => return Err(ApiError::not_found("Subscription")),
    };

    // Copy trading execution history will be populated once the execution
    // engine is wired up. For now, verify ownership and return empty list.
    let _agent_id: Option<String> = row.try_get("agent_id").ok().flatten();
    let trades: Vec<serde_json::Value> = Vec::new();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "data": trades,
        "limit": limit,
        "offset": offset,
    })))
}

// ── Helpers ──────────────────────────────────────────────────────────

fn map_subscription_row(row: &sqlx::postgres::PgRow) -> CopyTradingSubscription {
    CopyTradingSubscription {
        id: row.get("id"),
        subscriber: row.get("subscriber"),
        target_wallet: row.get("target_wallet"),
        agent_id: row.try_get("agent_id").ok().flatten(),
        allocation_usdc: row.get("allocation_usdc"),
        max_position_usdc: row.get("max_position_usdc"),
        active: row.get("active"),
        created_at: row
            .get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .to_rfc3339(),
        updated_at: row
            .get::<chrono::DateTime<chrono::Utc>, _>("updated_at")
            .to_rfc3339(),
    }
}
