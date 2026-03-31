use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::auth::{extract_authenticated_user, extract_jwt_user};
use crate::api::jwt::{check_role, UserRole};
use crate::api::ApiError;
use crate::AppState;

const MAX_PAGE_SIZE: i64 = 100;

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

// ---------------------------------------------------------------------------
// GET /v1/hackathons
// ---------------------------------------------------------------------------

pub async fn list_hackathons(
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListHackathonsQuery>,
) -> Result<impl Responder, ApiError> {
    let limit = query.limit.unwrap_or(20).min(MAX_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0).max(0);
    let pool = state.db.pool();

    let rows = if let Some(ref status) = query.status {
        sqlx::query(
            "SELECT h.*,
                (SELECT COUNT(*) FROM hackathon_registrations r
                 WHERE r.hackathon_id = h.id AND r.status = 'active') AS participant_count,
                (SELECT COUNT(*) FROM hackathon_agents a
                 WHERE a.hackathon_id = h.id) AS agent_count
             FROM hackathons h
             WHERE h.status = $1
             ORDER BY h.start_time DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT h.*,
                (SELECT COUNT(*) FROM hackathon_registrations r
                 WHERE r.hackathon_id = h.id AND r.status = 'active') AS participant_count,
                (SELECT COUNT(*) FROM hackathon_agents a
                 WHERE a.hackathon_id = h.id) AS agent_count
             FROM hackathons h
             ORDER BY h.start_time DESC
             LIMIT $1 OFFSET $2",
        )
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

    let row = sqlx::query(
        "SELECT h.*,
            (SELECT COUNT(*) FROM hackathon_registrations r
             WHERE r.hackathon_id = h.id AND r.status = 'active') AS participant_count,
            (SELECT COUNT(*) FROM hackathon_agents a
             WHERE a.hackathon_id = h.id) AS agent_count
         FROM hackathons h
         WHERE h.id = $1",
    )
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

    let name = body.name.trim();
    if name.is_empty() || name.len() > 256 {
        return Err(ApiError::bad_request(
            "INVALID_NAME",
            "Name must be 1-256 characters",
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

    let hackathon_id = format!("hack_{}", Uuid::new_v4().simple());
    let description = body.description.as_deref().unwrap_or("");
    let prize_pool = body.prize_pool_usdc.unwrap_or(0.0);
    let scoring_method = body.scoring_method.as_deref().unwrap_or("net_pnl");
    let rules = body.rules_json.clone().unwrap_or_else(|| json!({}));
    let pool = state.db.pool();

    sqlx::query(
        "INSERT INTO hackathons (id, name, description, prize_pool_usdc, start_time, end_time,
                                 status, scoring_method, created_by, rules_json)
         VALUES ($1, $2, $3, $4, $5, $6, 'upcoming', $7, $8, $9)",
    )
    .bind(&hackathon_id)
    .bind(name)
    .bind(description)
    .bind(prize_pool)
    .bind(start_time)
    .bind(end_time)
    .bind(scoring_method)
    .bind(&user.wallet_address)
    .bind(&rules)
    .execute(pool)
    .await?;

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

    // Verify exists
    let _existing = sqlx::query("SELECT id FROM hackathons WHERE id = $1")
        .bind(&hackathon_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Hackathon"))?;

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

    push_field!(body.name, "name");
    push_field!(body.description, "description");
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

    // Re-fetch
    let row = sqlx::query(
        "SELECT h.*,
            (SELECT COUNT(*) FROM hackathon_registrations r
             WHERE r.hackathon_id = h.id AND r.status = 'active') AS participant_count,
            (SELECT COUNT(*) FROM hackathon_agents a
             WHERE a.hackathon_id = h.id) AS agent_count
         FROM hackathons h
         WHERE h.id = $1",
    )
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

    // Check not already registered
    let existing = sqlx::query(
        "SELECT wallet_address FROM hackathon_registrations
         WHERE hackathon_id = $1 AND wallet_address = $2",
    )
    .bind(&hackathon_id)
    .bind(&user.wallet_address)
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Err(ApiError::conflict(
            "ALREADY_REGISTERED",
            "You are already registered for this hackathon",
        ));
    }

    sqlx::query(
        "INSERT INTO hackathon_registrations (hackathon_id, wallet_address, identity_id, status)
         VALUES ($1, $2, $3, 'active')",
    )
    .bind(&hackathon_id)
    .bind(&user.wallet_address)
    .bind(&body.identity_id)
    .execute(pool)
    .await?;

    Ok(HttpResponse::Created().json(json!({
        "hackathonId": hackathon_id,
        "walletAddress": user.wallet_address,
        "identityId": body.identity_id,
        "status": "active",
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/hackathons/{id}/registrations
// ---------------------------------------------------------------------------

pub async fn list_registrations(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let hackathon_id = path.into_inner();
    let pool = state.db.pool();

    let rows = sqlx::query(
        "SELECT r.*,
            (SELECT COUNT(*) FROM hackathon_agents a
             WHERE a.hackathon_id = r.hackathon_id AND a.wallet_address = r.wallet_address) AS agent_count
         FROM hackathon_registrations r
         WHERE r.hackathon_id = $1
         ORDER BY r.registered_at ASC",
    )
    .bind(&hackathon_id)
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

    Ok(HttpResponse::Ok().json(json!({
        "registrations": registrations,
        "total": registrations.len(),
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

    // Verify user is registered
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

    // Check agent not already linked
    let existing = sqlx::query(
        "SELECT agent_id FROM hackathon_agents
         WHERE hackathon_id = $1 AND agent_id = $2",
    )
    .bind(&hackathon_id)
    .bind(&body.agent_id)
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Err(ApiError::conflict(
            "AGENT_ALREADY_LINKED",
            "This agent is already linked to this hackathon",
        ));
    }

    sqlx::query(
        "INSERT INTO hackathon_agents (hackathon_id, agent_id, wallet_address)
         VALUES ($1, $2, $3)",
    )
    .bind(&hackathon_id)
    .bind(&body.agent_id)
    .bind(&user.wallet_address)
    .execute(pool)
    .await?;

    Ok(HttpResponse::Created().json(json!({
        "hackathonId": hackathon_id,
        "agentId": body.agent_id,
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
    let limit = query.limit.unwrap_or(50).min(MAX_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0).max(0);
    let pool = state.db.pool();

    // Get the latest snapshot timestamp for this hackathon
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
    let limit = query.limit.unwrap_or(200).min(500);
    let pool = state.db.pool();

    let rows = if let Some(ref wallet) = query.wallet_address {
        sqlx::query(
            "SELECT wallet_address, net_pnl_usdc, total_volume_usdc, rank, snapshot_time
             FROM hackathon_snapshots
             WHERE hackathon_id = $1 AND wallet_address = $2
             ORDER BY snapshot_time ASC
             LIMIT $3",
        )
        .bind(&hackathon_id)
        .bind(wallet)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        // Return latest snapshot for each wallet (for overview chart)
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

    // Redis lock to prevent concurrent snapshots
    let lock_key = format!("hackathon_snapshot_{}", hackathon_id);
    let locked = state
        .redis
        .check_and_record_nonce(&lock_key, 300)
        .await
        .unwrap_or(false);

    if !locked {
        return Err(ApiError::conflict(
            "SNAPSHOT_IN_PROGRESS",
            "A snapshot is already being computed",
        ));
    }

    let start_time: chrono::DateTime<Utc> = hackathon.get("start_time");
    let end_time: chrono::DateTime<Utc> = hackathon.get("end_time");
    let effective_end = end_time.min(Utc::now());
    let snapshot_time = Utc::now();

    // Get all registered wallets with their agents
    let wallet_rows = sqlx::query(
        "SELECT DISTINCT r.wallet_address
         FROM hackathon_registrations r
         WHERE r.hackathon_id = $1 AND r.status = 'active'",
    )
    .bind(&hackathon_id)
    .fetch_all(pool)
    .await?;

    if wallet_rows.is_empty() {
        // Release lock
        let _ = state.redis.delete(&lock_key).await;
        return Ok(HttpResponse::Ok().json(json!({ "snapshotCount": 0 })));
    }

    // For each wallet, compute PnL from trades linked to their hackathon agents
    // This queries the indexed on-chain trade data
    let mut entries: Vec<(String, f64, f64, i32, i32, i32)> = Vec::new();

    for wallet_row in &wallet_rows {
        let wallet: String = wallet_row.get("wallet_address");

        // Get agent IDs for this wallet in this hackathon
        let agent_rows = sqlx::query(
            "SELECT agent_id FROM hackathon_agents
             WHERE hackathon_id = $1 AND wallet_address = $2",
        )
        .bind(&hackathon_id)
        .bind(&wallet)
        .fetch_all(pool)
        .await?;

        let agent_ids: Vec<String> = agent_rows
            .iter()
            .map(|r| r.get::<String, _>("agent_id"))
            .collect();

        if agent_ids.is_empty() {
            entries.push((wallet, 0.0, 0.0, 0, 0, 0));
            continue;
        }

        // Build placeholders for IN clause
        let placeholders: Vec<String> = agent_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 3))
            .collect();
        let in_clause = placeholders.join(", ");

        // Query trades for these agents within the hackathon period
        // The trades table stores on-chain indexed fills
        let trade_sql = format!(
            "SELECT
                COALESCE(SUM(CASE WHEN t.buyer = ANY(ARRAY[{}]) THEN -t.price * t.quantity / 10000.0
                                  WHEN t.seller = ANY(ARRAY[{}]) THEN t.price * t.quantity / 10000.0
                                  ELSE 0 END), 0) AS realized_pnl,
                COALESCE(SUM(t.price * t.quantity / 10000.0), 0) AS volume,
                COUNT(*) AS trade_count
             FROM trades t
             WHERE (t.buyer = ANY(ARRAY[{}]) OR t.seller = ANY(ARRAY[{}]))
               AND t.created_at >= $1
               AND t.created_at <= $2",
            in_clause, in_clause, in_clause, in_clause
        );

        // For now, use a simplified PnL approach based on positions
        // Query positions for the wallet on markets active during hackathon
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
            Err(_) => {
                entries.push((wallet, 0.0, 0.0, 0, 0, 0));
            }
        }
    }

    // Sort by PnL descending for ranking
    entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Batch insert snapshots
    let mut snapshot_count = 0;
    for (rank, (wallet, pnl, volume, win_rate, positions, trades)) in entries.iter().enumerate() {
        sqlx::query(
            "INSERT INTO hackathon_snapshots
                (hackathon_id, wallet_address, snapshot_time, net_pnl_usdc,
                 total_volume_usdc, win_rate_bps, position_count, trade_count, rank)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(&hackathon_id)
        .bind(wallet)
        .bind(snapshot_time)
        .bind(pnl)
        .bind(volume)
        .bind(win_rate)
        .bind(positions)
        .bind(trades)
        .bind((rank + 1) as i32)
        .execute(pool)
        .await?;
        snapshot_count += 1;
    }

    // Release lock
    let _ = state.redis.delete(&lock_key).await;

    Ok(HttpResponse::Ok().json(json!({
        "snapshotCount": snapshot_count,
        "snapshotTime": snapshot_time.to_rfc3339(),
    })))
}
