use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::auth::extract_authenticated_user;
use crate::api::ApiError;
use crate::AppState;

fn ensure_agent_service_enabled(state: &AppState) -> Result<(), ApiError> {
    if !state.config.agent_service_enabled {
        return Err(ApiError::bad_request("AGENT_SERVICE_DISABLED", "agent service is disabled"));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployAgentRequest {
    pub template_id: String,
    pub name: String,
    pub seed_usdc: f64,
    pub params: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

/// GET /v1/agents/templates — list available agent templates.
pub async fn list_templates(
    _req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_agent_service_enabled(&state)?;
    let rows: Vec<(String, String, Option<String>, String, String, String, f64, String)> =
        sqlx::query_as(
            "SELECT id, name, description, strategy, category, risk_tier, \
             min_seed_usdc, default_params::text \
             FROM agent_templates WHERE active = true \
             ORDER BY category, name",
        )
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let templates: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.0,
                "name": r.1,
                "description": r.2,
                "strategy": r.3,
                "category": r.4,
                "riskTier": r.5,
                "minSeedUsdc": r.6,
                "defaultParams": serde_json::from_str::<serde_json::Value>(&r.7).unwrap_or(json!({})),
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "templates": templates })))
}

/// POST /v1/agents/deploy — deploy a managed agent from a template.
pub async fn deploy_agent(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<DeployAgentRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_agent_service_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    // Validate template exists.
    let template: Option<(String, f64, String)> = sqlx::query_as(
        "SELECT strategy, min_seed_usdc, default_params::text \
         FROM agent_templates WHERE id = $1 AND active = true",
    )
    .bind(&body.template_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let (strategy, min_seed, default_params_str) =
        template.ok_or_else(|| ApiError::not_found("Agent template"))?;

    if body.seed_usdc < min_seed {
        return Err(ApiError::bad_request(
            "INSUFFICIENT_SEED",
            &format!("minimum seed is {} USDC", min_seed),
        ));
    }

    // Enforce creator tier seed cap if creator tiers are enabled.
    if state.config.creator_tiers_enabled {
        let cap: Option<(f64,)> = sqlx::query_as(
            "SELECT t.max_seed_usdc FROM creator_tiers t \
             JOIN creator_profiles p ON p.tier_id = t.id \
             WHERE p.owner = $1",
        )
        .bind(user.wallet_address.as_str())
        .fetch_optional(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

        if let Some((max_seed,)) = cap {
            if body.seed_usdc > max_seed {
                return Err(ApiError::bad_request(
                    "SEED_EXCEEDS_TIER",
                    &format!("your tier allows max {} USDC seed", max_seed),
                ));
            }
        }
    }

    let name = body.name.trim();
    if name.is_empty() || name.len() > 128 {
        return Err(ApiError::bad_request("INVALID_NAME", "name must be 1-128 chars"));
    }

    // Merge user params over template defaults.
    let default_params: serde_json::Value =
        serde_json::from_str(&default_params_str).unwrap_or(json!({}));
    let params = if let Some(user_params) = &body.params {
        merge_json(&default_params, user_params)
    } else {
        default_params
    };

    let id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO managed_agents \
         (id, owner, template_id, name, params, seed_usdc, high_watermark_usdc) \
         VALUES ($1, $2, $3, $4, $5, $6, $6)",
    )
    .bind(&id)
    .bind(user.wallet_address.as_str())
    .bind(&body.template_id)
    .bind(name)
    .bind(&params)
    .bind(body.seed_usdc)
    .execute(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({
        "id": id,
        "strategy": strategy,
        "params": params,
        "seedUsdc": body.seed_usdc,
        "status": "active",
    })))
}

/// GET /v1/agents/managed — list user's managed agents.
pub async fn list_managed_agents(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<AgentQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_agent_service_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;
    let limit = query.limit.unwrap_or(50).min(200);

    let rows: Vec<(
        String, String, String, String, f64, String, f64, i64, f64, f64, Option<String>, String,
    )> = if let Some(status) = &query.status {
        sqlx::query_as(
            "SELECT a.id, a.name, t.name, t.strategy, a.seed_usdc, a.status, \
             a.pnl_usdc, a.total_trades, a.max_drawdown_pct, a.high_watermark_usdc, \
             a.last_executed_at::text, a.created_at::text \
             FROM managed_agents a \
             JOIN agent_templates t ON t.id = a.template_id \
             WHERE a.owner = $1 AND a.status = $2 \
             ORDER BY a.created_at DESC LIMIT $3",
        )
        .bind(user.wallet_address.as_str())
        .bind(status)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    } else {
        sqlx::query_as(
            "SELECT a.id, a.name, t.name, t.strategy, a.seed_usdc, a.status, \
             a.pnl_usdc, a.total_trades, a.max_drawdown_pct, a.high_watermark_usdc, \
             a.last_executed_at::text, a.created_at::text \
             FROM managed_agents a \
             JOIN agent_templates t ON t.id = a.template_id \
             WHERE a.owner = $1 \
             ORDER BY a.created_at DESC LIMIT $2",
        )
        .bind(user.wallet_address.as_str())
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    };

    let agents: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.0,
                "name": r.1,
                "templateName": r.2,
                "strategy": r.3,
                "seedUsdc": r.4,
                "status": r.5,
                "pnlUsdc": r.6,
                "totalTrades": r.7,
                "maxDrawdownPct": r.8,
                "highWatermarkUsdc": r.9,
                "lastExecutedAt": r.10,
                "createdAt": r.11,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "agents": agents })))
}

/// PATCH /v1/agents/managed/{agent_id} — pause/resume/stop a managed agent.
pub async fn update_managed_agent(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    agent_id: web::Path<String>,
    body: web::Json<UpdateManagedAgentRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_agent_service_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    let valid_statuses = ["active", "paused", "stopped"];
    if !valid_statuses.contains(&body.status.as_str()) {
        return Err(ApiError::bad_request(
            "INVALID_STATUS",
            "status must be active, paused, or stopped",
        ));
    }

    let result = sqlx::query(
        "UPDATE managed_agents SET status = $1, updated_at = NOW() \
         WHERE id = $2 AND owner = $3",
    )
    .bind(&body.status)
    .bind(agent_id.as_str())
    .bind(user.wallet_address.as_str())
    .execute(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("Managed agent"));
    }

    Ok(HttpResponse::Ok().json(json!({ "ok": true, "status": body.status })))
}

/// GET /v1/agents/managed/{agent_id}/trades — get trade history for a managed agent.
pub async fn get_agent_trades(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    agent_id: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    ensure_agent_service_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    // Verify ownership.
    let owns: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM managed_agents WHERE id = $1 AND owner = $2",
    )
    .bind(agent_id.as_str())
    .bind(user.wallet_address.as_str())
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    if owns.is_none() {
        return Err(ApiError::not_found("Managed agent"));
    }

    let rows: Vec<(i32, String, String, String, f64, f64, Option<f64>, Option<String>, String)> =
        sqlx::query_as(
            "SELECT id, market_slug, outcome, side, price, quantity, \
             pnl_usdc, provider, created_at::text \
             FROM managed_agent_trades \
             WHERE agent_id = $1 \
             ORDER BY created_at DESC LIMIT 100",
        )
        .bind(agent_id.as_str())
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let trades: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.0,
                "marketSlug": r.1,
                "outcome": r.2,
                "side": r.3,
                "price": r.4,
                "quantity": r.5,
                "pnlUsdc": r.6,
                "provider": r.7,
                "createdAt": r.8,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "trades": trades })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManagedAgentRequest {
    pub status: String,
}

fn merge_json(base: &serde_json::Value, overlay: &serde_json::Value) -> serde_json::Value {
    match (base, overlay) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            let mut merged = b.clone();
            for (k, v) in o {
                merged.insert(k.clone(), v.clone());
            }
            serde_json::Value::Object(merged)
        }
        _ => overlay.clone(),
    }
}
