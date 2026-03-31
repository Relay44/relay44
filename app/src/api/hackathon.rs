use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::auth::{extract_authenticated_user, extract_jwt_user};
use crate::api::jwt::{check_role, UserRole};
use crate::api::validation::{sanitize_string, validate_pagination};
use crate::api::ApiError;
use crate::AppState;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ALLOWED_STATUSES: &[&str] = &["upcoming", "active", "completed", "cancelled"];
const ALLOWED_SCORING_METHODS: &[&str] = &["net_pnl"];
const MAX_RULES_JSON_BYTES: usize = 65_536; // 64 KB
const MAX_AGENTS_PER_WALLET: i64 = 3;
const SNAPSHOT_LOCK_TTL_SECS: u64 = 600;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListHackathonsQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateHackathonRequest {
    pub name: String,
    pub description: Option<String>,
    pub prize_pool_usdc: Option<f64>,
    pub start_time: String,
    pub end_time: String,
    pub scoring_method: Option<String>,
    pub rules_json: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHackathonRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub prize_pool_usdc: Option<f64>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub status: Option<String>,
    pub rules_json: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterForHackathonRequest {
    pub identity_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkAgentRequest {
    pub agent_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotsQuery {
    pub wallet_address: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn validate_status_param(status: &str) -> Result<(), ApiError> {
    if !ALLOWED_STATUSES.contains(&status) {
        return Err(ApiError::bad_request(
            "INVALID_STATUS",
            &format!(
                "Status must be one of: {}",
                ALLOWED_STATUSES.join(", ")
            ),
        ));
    }
    Ok(())
}

fn validate_prize(prize: f64) -> Result<(), ApiError> {
    if prize.is_nan() || prize.is_infinite() || prize < 0.0 {
        return Err(ApiError::bad_request(
            "INVALID_PRIZE",
            "Prize pool must be a non-negative number",
        ));
    }
    Ok(())
}

fn validate_rules_json(rules: &Value) -> Result<(), ApiError> {
    let serialized = serde_json::to_string(rules).unwrap_or_default();
    if serialized.len() > MAX_RULES_JSON_BYTES {
        return Err(ApiError::bad_request(
            "RULES_TOO_LARGE",
            "Rules JSON exceeds 64KB limit",
        ));
    }
    Ok(())
}

/// Validate status transition state machine.
/// Allowed: upcoming→active, active→completed, any→cancelled
fn validate_status_transition(current: &str, next: &str) -> Result<(), ApiError> {
    validate_status_param(next)?;
    let valid = match (current, next) {
        (_, "cancelled") => true,
        ("upcoming", "active") => true,
        ("active", "completed") => true,
        _ => false,
    };
    if !valid {
        return Err(ApiError::bad_request(
            "INVALID_TRANSITION",
            &format!("Cannot transition from '{}' to '{}'", current, next),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn hackathon_row_to_json(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id": row.get::<String, _>("id"),
        "name": row.get::<String, _>("name"),
        "description": row.get::<String, _>("description"),
        "prizePoolUsdc": row.get::<f64, _>("prize_pool_usdc"),
        "startTime": row.get::<chrono::DateTime<Utc>, _>("start_time").to_rfc3339(),
        "endTime": row.get::<chrono::DateTime<Utc>, _>("end_time").to_rfc3339(),
        "status": row.get::<String, _>("status"),
        "scoringMethod": row.get::<String, _>("scoring_method"),
        "createdBy": row.get::<String, _>("created_by"),
        "rulesJson": row.get::<Value, _>("rules_json"),
        "participantCount": row.try_get::<i64, _>("participant_count").unwrap_or(0),
        "agentCount": row.try_get::<i64, _>("agent_count").unwrap_or(0),
        "createdAt": row.get::<chrono::DateTime<Utc>, _>("created_at").to_rfc3339(),
        "updatedAt": row.get::<chrono::DateTime<Utc>, _>("updated_at").to_rfc3339(),
    })
}

fn hackathon_with_counts_sql() -> &'static str {
    "SELECT h.*,
        (SELECT COUNT(*) FROM hackathon_registrations r
         WHERE r.hackathon_id = h.id AND r.status = 'active') AS participant_count,
        (SELECT COUNT(*) FROM hackathon_agents a
         WHERE a.hackathon_id = h.id) AS agent_count
     FROM hackathons h"
}

// ---------------------------------------------------------------------------
// GET /v1/hackathons
// ---------------------------------------------------------------------------

pub async fn list_hackathons(
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListHackathonsQuery>,
) -> Result<impl Responder, ApiError> {
    let (limit, offset) = validate_pagination(query.limit, query.offset)?;
    let pool = state.db.pool();

    if let Some(ref status) = query.status {
        validate_status_param(status)?;
    }

    let rows = if let Some(ref status) = query.status {
        sqlx::query(&format!(
            "{} WHERE h.status = $1 ORDER BY h.start_time DESC LIMIT $2 OFFSET $3",
            hackathon_with_counts_sql()
        ))
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(&format!(
            "{} ORDER BY h.start_time DESC LIMIT $1 OFFSET $2",
            hackathon_with_counts_sql()
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    };

    let hackathons: Vec<Value> = rows.iter().map(hackathon_row_to_json).collect();

    let total_row = if let Some(ref status) = query.status {
        sqlx::query("SELECT COUNT(*) AS cnt FROM hackathons WHERE status = $1")
            .bind(status)
            .fetch_one(pool)
            .await?
    } else {
        sqlx::query("SELECT COUNT(*) AS cnt FROM hackathons")
            .fetch_one(pool)
            .await?
    };
    let total: i64 = total_row.get("cnt");

    Ok(HttpResponse::Ok().json(json!({
        "hackathons": hackathons,
        "total": total,
        "limit": limit,
        "offset": offset,
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/hackathons/{id}
// ---------------------------------------------------------------------------

pub async fn get_hackathon(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let hackathon_id = path.into_inner();
    let pool = state.db.pool();

    let row = sqlx::query(&format!(
        "{} WHERE h.id = $1",
        hackathon_with_counts_sql()
    ))
    .bind(&hackathon_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Hackathon"))?;

    Ok(HttpResponse::Ok().json(hackathon_row_to_json(&row)))
}

// ---------------------------------------------------------------------------
// POST /v1/hackathons  (admin only)
// ---------------------------------------------------------------------------

pub async fn create_hackathon(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateHackathonRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let name = sanitize_string(&body.name, 256);
    if name.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_NAME",
            "Name must be 1-256 characters",
        ));
    }

    let description = sanitize_string(
        body.description.as_deref().unwrap_or(""),
        4000,
    );

    let prize_pool = body.prize_pool_usdc.unwrap_or(0.0);
    validate_prize(prize_pool)?;

    let scoring_method = body.scoring_method.as_deref().unwrap_or("net_pnl");
    if !ALLOWED_SCORING_METHODS.contains(&scoring_method) {
        return Err(ApiError::bad_request(
            "INVALID_SCORING_METHOD",
            &format!(
                "Scoring method must be one of: {}",
                ALLOWED_SCORING_METHODS.join(", ")
            ),
        ));
    }

    let start_time = chrono::DateTime::parse_from_rfc3339(&body.start_time)
        .map_err(|_| ApiError::bad_request("INVALID_DATE", "Invalid start_time format (RFC3339)"))?
        .with_timezone(&Utc);

    let end_time = chrono::DateTime::parse_from_rfc3339(&body.end_time)
        .map_err(|_| ApiError::bad_request("INVALID_DATE", "Invalid end_time format (RFC3339)"))?
        .with_timezone(&Utc);

    if end_time <= start_time {
        return Err(ApiError::bad_request(
            "INVALID_DATE",
            "end_time must be after start_time",
        ));
    }

    let rules = body.rules_json.clone().unwrap_or_else(|| json!({}));
    validate_rules_json(&rules)?;

    let hackathon_id = format!("hack_{}", Uuid::new_v4().simple());
    let pool = state.db.pool();

    sqlx::query(
        "INSERT INTO hackathons (id, name, description, prize_pool_usdc, start_time, end_time,
                                 status, scoring_method, created_by, rules_json)
         VALUES ($1, $2, $3, $4, $5, $6, 'upcoming', $7, $8, $9)",
    )
    .bind(&hackathon_id)
    .bind(&name)
    .bind(&description)
    .bind(prize_pool)
    .bind(start_time)
    .bind(end_time)
    .bind(scoring_method)
    .bind(&user.wallet_address)
    .bind(&rules)
    .execute(pool)
    .await?;

    log::info!(
        "Hackathon created: id={}, name={}, by={}",
        hackathon_id, name, user.wallet_address
    );

    let row = sqlx::query(
        "SELECT h.*, 0::bigint AS participant_count, 0::bigint AS agent_count
         FROM hackathons h WHERE h.id = $1",
    )
    .bind(&hackathon_id)
    .fetch_one(pool)
    .await?;

    Ok(HttpResponse::Created().json(hackathon_row_to_json(&row)))
}

// ---------------------------------------------------------------------------
// PATCH /v1/hackathons/{id}  (admin only)
// ---------------------------------------------------------------------------

pub async fn update_hackathon(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<UpdateHackathonRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let hackathon_id = path.into_inner();
    let pool = state.db.pool();

    let existing = sqlx::query("SELECT id, status FROM hackathons WHERE id = $1")
        .bind(&hackathon_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Hackathon"))?;

    let current_status: String = existing.get("status");

    // Validate status transition if status is being changed
    if let Some(ref new_status) = body.status {
        validate_status_transition(&current_status, new_status)?;
    }

    // Validate prize if provided
    if let Some(prize) = body.prize_pool_usdc {
        validate_prize(prize)?;
    }

    // Validate rules JSON size if provided
    if let Some(ref rules) = body.rules_json {
        validate_rules_json(rules)?;
    }

    // Validate date ordering if either date is being changed
    if body.start_time.is_some() || body.end_time.is_some() {
        if let (Some(ref st), Some(ref et)) = (&body.start_time, &body.end_time) {
            let s = chrono::DateTime::parse_from_rfc3339(st)
                .map_err(|_| ApiError::bad_request("INVALID_DATE", "Invalid start_time format"))?;
            let e = chrono::DateTime::parse_from_rfc3339(et)
                .map_err(|_| ApiError::bad_request("INVALID_DATE", "Invalid end_time format"))?;
            if e <= s {
                return Err(ApiError::bad_request(
                    "INVALID_DATE",
                    "end_time must be after start_time",
                ));
            }
        }
    }

    let mut sets: Vec<String> = Vec::new();
    let mut idx: usize = 1;
    let mut binds: Vec<String> = Vec::new();

    macro_rules! push_field {
        ($field:expr, $col:expr) => {
            if let Some(ref val) = $field {
                idx += 1;
                sets.push(format!("{} = ${}", $col, idx));
                binds.push(val.clone());
            }
        };
    }

    if let Some(ref name) = body.name {
        let sanitized = sanitize_string(name, 256);
        if sanitized.is_empty() {
            return Err(ApiError::bad_request("INVALID_NAME", "Name cannot be empty"));
        }
        idx += 1;
        sets.push(format!("name = ${}", idx));
        binds.push(sanitized);
    }

    if let Some(ref desc) = body.description {
        let sanitized = sanitize_string(desc, 4000);
        idx += 1;
        sets.push(format!("description = ${}", idx));
        binds.push(sanitized);
    }

    push_field!(body.status, "status");
    push_field!(body.start_time, "start_time");
    push_field!(body.end_time, "end_time");

    if let Some(prize) = body.prize_pool_usdc {
        idx += 1;
        sets.push(format!("prize_pool_usdc = ${}", idx));
        binds.push(prize.to_string());
    }

    if let Some(ref rules) = body.rules_json {
        idx += 1;
        sets.push(format!("rules_json = ${}::jsonb", idx));
        binds.push(rules.to_string());
    }

    if sets.is_empty() {
        return Err(ApiError::bad_request("NO_FIELDS", "No fields to update"));
    }

    sets.push("updated_at = NOW()".to_string());

    let sql = format!(
        "UPDATE hackathons SET {} WHERE id = $1",
        sets.join(", ")
    );

    let mut query = sqlx::query(&sql).bind(&hackathon_id);
    for val in &binds {
        query = query.bind(val);
    }
    query.execute(pool).await?;

    log::info!(
        "Hackathon updated: id={}, fields={:?}, by={}",
        hackathon_id,
        sets.iter()
            .filter(|s| !s.starts_with("updated_at"))
            .collect::<Vec<_>>(),
        user.wallet_address
    );

    let row = sqlx::query(&format!(
        "{} WHERE h.id = $1",
        hackathon_with_counts_sql()
    ))
    .bind(&hackathon_id)
    .fetch_one(pool)
    .await?;

    Ok(HttpResponse::Ok().json(hackathon_row_to_json(&row)))
}

// ---------------------------------------------------------------------------
// POST /v1/hackathons/{id}/register
// ---------------------------------------------------------------------------

pub async fn register_for_hackathon(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<RegisterForHackathonRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_authenticated_user(&req, &state).await?;
    let hackathon_id = path.into_inner();
    let pool = state.db.pool();

    // Validate hackathon exists and is open for registration
    let hackathon = sqlx::query("SELECT id, status FROM hackathons WHERE id = $1")
        .bind(&hackathon_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Hackathon"))?;

    let status: String = hackathon.get("status");
    if status != "upcoming" && status != "active" {
        return Err(ApiError::bad_request(
            "REGISTRATION_CLOSED",
            "This hackathon is not open for registration",
        ));
    }

    // Sanitize optional identity_id
    let identity_id = body
        .identity_id
        .as_deref()
        .map(|id| sanitize_string(id, 256))
        .filter(|id| !id.is_empty());

    // Atomic insert — ON CONFLICT eliminates TOCTOU race
    let result = sqlx::query(
        "INSERT INTO hackathon_registrations (hackathon_id, wallet_address, identity_id, status)
         VALUES ($1, $2, $3, 'active')
         ON CONFLICT (hackathon_id, wallet_address) DO NOTHING",
    )
    .bind(&hackathon_id)
    .bind(&user.wallet_address)
    .bind(&identity_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::conflict(
            "ALREADY_REGISTERED",
            "You are already registered for this hackathon",
        ));
    }

    log::info!(
        "Hackathon registration: hackathon={}, wallet={}",
        hackathon_id, user.wallet_address
    );

    Ok(HttpResponse::Created().json(json!({
        "hackathonId": hackathon_id,
        "walletAddress": user.wallet_address,
        "identityId": identity_id,
        "status": "active",
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/hackathons/{id}/registrations
// ---------------------------------------------------------------------------

pub async fn list_registrations(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<RegistrationsQuery>,
) -> Result<impl Responder, ApiError> {
    let hackathon_id = path.into_inner();
    let (limit, offset) = validate_pagination(query.limit, query.offset)?;
    let pool = state.db.pool();

    let rows = sqlx::query(
        "SELECT r.*,
            (SELECT COUNT(*) FROM hackathon_agents a
             WHERE a.hackathon_id = r.hackathon_id AND a.wallet_address = r.wallet_address) AS agent_count
         FROM hackathon_registrations r
         WHERE r.hackathon_id = $1
         ORDER BY r.registered_at ASC
         LIMIT $2 OFFSET $3",
    )
    .bind(&hackathon_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let registrations: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "hackathonId": row.get::<String, _>("hackathon_id"),
                "walletAddress": row.get::<String, _>("wallet_address"),
                "identityId": row.get::<Option<String>, _>("identity_id"),
                "registeredAt": row.get::<chrono::DateTime<Utc>, _>("registered_at").to_rfc3339(),
                "status": row.get::<String, _>("status"),
                "agentCount": row.get::<i64, _>("agent_count"),
            })
        })
        .collect();

    let total_row = sqlx::query(
        "SELECT COUNT(*) AS cnt FROM hackathon_registrations WHERE hackathon_id = $1",
    )
    .bind(&hackathon_id)
    .fetch_one(pool)
    .await?;

    Ok(HttpResponse::Ok().json(json!({
        "registrations": registrations,
        "total": total_row.get::<i64, _>("cnt"),
        "limit": limit,
        "offset": offset,
    })))
}

// ---------------------------------------------------------------------------
// POST /v1/hackathons/{id}/agents
// ---------------------------------------------------------------------------

pub async fn link_agent_to_hackathon(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<LinkAgentRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_authenticated_user(&req, &state).await?;
    let hackathon_id = path.into_inner();
    let pool = state.db.pool();

    let agent_id = sanitize_string(&body.agent_id, 128);
    if agent_id.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_AGENT_ID",
            "Agent ID must be 1-128 characters",
        ));
    }

    // Verify user is registered and not disqualified
    let registration = sqlx::query(
        "SELECT wallet_address, status FROM hackathon_registrations
         WHERE hackathon_id = $1 AND wallet_address = $2",
    )
    .bind(&hackathon_id)
    .bind(&user.wallet_address)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        ApiError::bad_request(
            "NOT_REGISTERED",
            "You must register for this hackathon first",
        )
    })?;

    let reg_status: String = registration.get("status");
    if reg_status == "disqualified" {
        return Err(ApiError::forbidden(
            "You have been disqualified from this hackathon",
        ));
    }

    // Enforce max agents per wallet
    let agent_count = sqlx::query(
        "SELECT COUNT(*) AS cnt FROM hackathon_agents
         WHERE hackathon_id = $1 AND wallet_address = $2",
    )
    .bind(&hackathon_id)
    .bind(&user.wallet_address)
    .fetch_one(pool)
    .await?;

    if agent_count.get::<i64, _>("cnt") >= MAX_AGENTS_PER_WALLET {
        return Err(ApiError::bad_request(
            "MAX_AGENTS_REACHED",
            &format!("Maximum {} agents per wallet", MAX_AGENTS_PER_WALLET),
        ));
    }

    // Atomic insert — ON CONFLICT eliminates TOCTOU race
    let result = sqlx::query(
        "INSERT INTO hackathon_agents (hackathon_id, agent_id, wallet_address)
         VALUES ($1, $2, $3)
         ON CONFLICT (hackathon_id, agent_id) DO NOTHING",
    )
    .bind(&hackathon_id)
    .bind(&agent_id)
    .bind(&user.wallet_address)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(ApiError::conflict(
            "AGENT_ALREADY_LINKED",
            "This agent is already linked to this hackathon",
        ));
    }

    log::info!(
        "Agent linked: hackathon={}, agent={}, wallet={}",
        hackathon_id, agent_id, user.wallet_address
    );

    Ok(HttpResponse::Created().json(json!({
        "hackathonId": hackathon_id,
        "agentId": agent_id,
        "walletAddress": user.wallet_address,
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/hackathons/{id}/leaderboard
// ---------------------------------------------------------------------------

pub async fn get_leaderboard(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<LeaderboardQuery>,
) -> Result<impl Responder, ApiError> {
    let hackathon_id = path.into_inner();
    let (limit, offset) = validate_pagination(query.limit, query.offset)?;
    let pool = state.db.pool();

    // Get the latest snapshot timestamp
    let latest_ts = sqlx::query(
        "SELECT MAX(snapshot_time) AS latest FROM hackathon_snapshots WHERE hackathon_id = $1",
    )
    .bind(&hackathon_id)
    .fetch_one(pool)
    .await?;

    let latest: Option<chrono::DateTime<Utc>> = latest_ts.get("latest");

    if latest.is_none() {
        return Ok(HttpResponse::Ok().json(json!({
            "hackathonId": hackathon_id,
            "entries": [],
            "updatedAt": Value::Null,
            "total": 0,
        })));
    }

    let latest = latest.unwrap();

    let rows = sqlx::query(
        "SELECT wallet_address, net_pnl_usdc, total_volume_usdc, win_rate_bps,
                position_count, trade_count, rank, snapshot_time
         FROM hackathon_snapshots
         WHERE hackathon_id = $1 AND snapshot_time = $2
         ORDER BY rank ASC
         LIMIT $3 OFFSET $4",
    )
    .bind(&hackathon_id)
    .bind(latest)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let entries: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "rank": row.get::<i32, _>("rank"),
                "walletAddress": row.get::<String, _>("wallet_address"),
                "netPnlUsdc": row.get::<f64, _>("net_pnl_usdc"),
                "totalVolumeUsdc": row.get::<f64, _>("total_volume_usdc"),
                "winRateBps": row.get::<i32, _>("win_rate_bps"),
                "positionCount": row.get::<i32, _>("position_count"),
                "tradeCount": row.get::<i32, _>("trade_count"),
                "snapshotTime": row.get::<chrono::DateTime<Utc>, _>("snapshot_time").to_rfc3339(),
            })
        })
        .collect();

    let total_row = sqlx::query(
        "SELECT COUNT(*) AS cnt FROM hackathon_snapshots
         WHERE hackathon_id = $1 AND snapshot_time = $2",
    )
    .bind(&hackathon_id)
    .bind(latest)
    .fetch_one(pool)
    .await?;

    Ok(HttpResponse::Ok().json(json!({
        "hackathonId": hackathon_id,
        "entries": entries,
        "updatedAt": latest.to_rfc3339(),
        "total": total_row.get::<i64, _>("cnt"),
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/hackathons/{id}/leaderboard/snapshots
// ---------------------------------------------------------------------------

pub async fn get_leaderboard_snapshots(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<SnapshotsQuery>,
) -> Result<impl Responder, ApiError> {
    let hackathon_id = path.into_inner();
    let limit = query.limit.unwrap_or(200).min(500).max(1);
    let pool = state.db.pool();

    let rows = if let Some(ref wallet) = query.wallet_address {
        let wallet = sanitize_string(wallet, 256);
        sqlx::query(
            "SELECT wallet_address, net_pnl_usdc, total_volume_usdc, rank, snapshot_time
             FROM hackathon_snapshots
             WHERE hackathon_id = $1 AND wallet_address = $2
             ORDER BY snapshot_time ASC
             LIMIT $3",
        )
        .bind(&hackathon_id)
        .bind(&wallet)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT DISTINCT ON (wallet_address)
                wallet_address, net_pnl_usdc, total_volume_usdc, rank, snapshot_time
             FROM hackathon_snapshots
             WHERE hackathon_id = $1
             ORDER BY wallet_address, snapshot_time DESC
             LIMIT $2",
        )
        .bind(&hackathon_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    let snapshots: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "walletAddress": row.get::<String, _>("wallet_address"),
                "netPnlUsdc": row.get::<f64, _>("net_pnl_usdc"),
                "totalVolumeUsdc": row.get::<f64, _>("total_volume_usdc"),
                "rank": row.get::<i32, _>("rank"),
                "snapshotTime": row.get::<chrono::DateTime<Utc>, _>("snapshot_time").to_rfc3339(),
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "snapshots": snapshots })))
}

// ---------------------------------------------------------------------------
// POST /v1/hackathons/{id}/snapshot  (admin / cron)
// ---------------------------------------------------------------------------

pub async fn trigger_snapshot(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    check_role(user.role, UserRole::Admin)?;

    let hackathon_id = path.into_inner();
    let pool = state.db.pool();

    // Validate hackathon
    let hackathon = sqlx::query("SELECT id, status, start_time, end_time FROM hackathons WHERE id = $1")
        .bind(&hackathon_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Hackathon"))?;

    let status: String = hackathon.get("status");
    if status != "active" {
        return Err(ApiError::bad_request(
            "NOT_ACTIVE",
            "Snapshots can only be taken for active hackathons",
        ));
    }

    // Redis lock — fail explicitly if Redis is down
    let lock_key = format!("hackathon_snapshot_{}", hackathon_id);
    let locked = state
        .redis
        .check_and_record_nonce(&lock_key, SNAPSHOT_LOCK_TTL_SECS)
        .await
        .map_err(|e| {
            log::error!("Redis lock failed for hackathon snapshot {}: {}", hackathon_id, e);
            ApiError::internal("Failed to acquire snapshot lock")
        })?;

    if !locked {
        return Err(ApiError::conflict(
            "SNAPSHOT_IN_PROGRESS",
            "A snapshot is already being computed",
        ));
    }

    // Run snapshot computation, ensuring lock is released on all paths
    let result = compute_snapshot(pool, &state, &hackathon_id, &hackathon).await;

    // Always release lock
    if let Err(e) = state.redis.delete(&lock_key).await {
        log::warn!("Failed to release snapshot lock for {}: {}", hackathon_id, e);
    }

    result
}

async fn compute_snapshot(
    pool: &sqlx::PgPool,
    _state: &AppState,
    hackathon_id: &str,
    hackathon: &sqlx::postgres::PgRow,
) -> Result<HttpResponse, ApiError> {
    let start_time: chrono::DateTime<Utc> = hackathon.get("start_time");
    let end_time: chrono::DateTime<Utc> = hackathon.get("end_time");
    let effective_end = end_time.min(Utc::now());
    let snapshot_time = Utc::now();
    let started_at = std::time::Instant::now();

    // Get all registered wallets
    let wallet_rows = sqlx::query(
        "SELECT DISTINCT r.wallet_address
         FROM hackathon_registrations r
         WHERE r.hackathon_id = $1 AND r.status = 'active'",
    )
    .bind(hackathon_id)
    .fetch_all(pool)
    .await?;

    if wallet_rows.is_empty() {
        return Ok(HttpResponse::Accepted().json(json!({ "snapshotCount": 0 })));
    }

    // Batch fetch all agents for the hackathon (eliminates N+1)
    let agent_rows = sqlx::query(
        "SELECT wallet_address, agent_id FROM hackathon_agents WHERE hackathon_id = $1",
    )
    .bind(hackathon_id)
    .fetch_all(pool)
    .await?;

    let mut agents_by_wallet: HashMap<String, Vec<String>> = HashMap::new();
    for row in &agent_rows {
        let wallet: String = row.get("wallet_address");
        let agent_id: String = row.get("agent_id");
        agents_by_wallet
            .entry(wallet)
            .or_default()
            .push(agent_id);
    }

    // Compute PnL for each wallet
    let mut entries: Vec<(String, f64, f64, i32, i32, i32)> = Vec::new();

    for wallet_row in &wallet_rows {
        let wallet: String = wallet_row.get("wallet_address");

        let _agent_ids = agents_by_wallet.get(&wallet);

        // Query positions for PnL computation
        let pnl_row = sqlx::query(
            "SELECT
                COALESCE(SUM(p.realized_pnl), 0) AS realized_pnl,
                COALESCE(SUM(p.unrealized_pnl), 0) AS unrealized_pnl,
                COALESCE(SUM(p.total_cost), 0) AS total_volume,
                COUNT(*) FILTER (WHERE p.yes_balance > 0 OR p.no_balance > 0) AS position_count,
                COALESCE(SUM(p.trade_count), 0) AS trade_count,
                CASE WHEN SUM(p.trade_count) > 0
                     THEN (SUM(CASE WHEN p.realized_pnl > 0 THEN 1 ELSE 0 END) * 10000 / SUM(p.trade_count))
                     ELSE 0 END AS win_rate_bps
             FROM positions p
             WHERE p.wallet = $1
               AND p.updated_at >= $2
               AND p.updated_at <= $3",
        )
        .bind(&wallet)
        .bind(start_time)
        .bind(effective_end)
        .fetch_one(pool)
        .await;

        match pnl_row {
            Ok(row) => {
                let realized: f64 = row.try_get("realized_pnl").unwrap_or(0.0);
                let unrealized: f64 = row.try_get("unrealized_pnl").unwrap_or(0.0);
                let volume: f64 = row.try_get("total_volume").unwrap_or(0.0);
                let position_count: i64 = row.try_get("position_count").unwrap_or(0);
                let trade_count: i64 = row.try_get("trade_count").unwrap_or(0);
                let win_rate: i64 = row.try_get("win_rate_bps").unwrap_or(0);

                entries.push((
                    wallet,
                    realized + unrealized,
                    volume,
                    win_rate as i32,
                    position_count as i32,
                    trade_count as i32,
                ));
            }
            Err(e) => {
                log::warn!("PnL query failed for wallet {} in hackathon {}: {}", wallet, hackathon_id, e);
                entries.push((wallet, 0.0, 0.0, 0, 0, 0));
            }
        }
    }

    // Sort by PnL descending for ranking
    entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Insert all snapshots in a transaction (all-or-nothing)
    let mut tx = pool.begin().await.map_err(|e| {
        log::error!("Failed to begin snapshot transaction: {}", e);
        ApiError::internal("Failed to begin transaction")
    })?;

    let mut snapshot_count = 0;
    for (rank, (wallet, pnl, volume, win_rate, positions, trades)) in entries.iter().enumerate() {
        sqlx::query(
            "INSERT INTO hackathon_snapshots
                (hackathon_id, wallet_address, snapshot_time, net_pnl_usdc,
                 total_volume_usdc, win_rate_bps, position_count, trade_count, rank)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (hackathon_id, wallet_address, snapshot_time) DO UPDATE SET
                net_pnl_usdc = EXCLUDED.net_pnl_usdc,
                total_volume_usdc = EXCLUDED.total_volume_usdc,
                win_rate_bps = EXCLUDED.win_rate_bps,
                position_count = EXCLUDED.position_count,
                trade_count = EXCLUDED.trade_count,
                rank = EXCLUDED.rank",
        )
        .bind(hackathon_id)
        .bind(wallet)
        .bind(snapshot_time)
        .bind(pnl)
        .bind(volume)
        .bind(win_rate)
        .bind(positions)
        .bind(trades)
        .bind((rank + 1) as i32)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            log::error!("Failed to insert snapshot for wallet {}: {}", wallet, e);
            ApiError::internal("Failed to insert snapshot")
        })?;
        snapshot_count += 1;
    }

    tx.commit().await.map_err(|e| {
        log::error!("Failed to commit snapshot transaction: {}", e);
        ApiError::internal("Failed to commit snapshot")
    })?;

    let elapsed = started_at.elapsed();
    log::info!(
        "Snapshot complete: hackathon={}, entries={}, elapsed={:?}",
        hackathon_id, snapshot_count, elapsed
    );

    Ok(HttpResponse::Accepted().json(json!({
        "snapshotCount": snapshot_count,
        "snapshotTime": snapshot_time.to_rfc3339(),
    })))
}
