use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::auth::extract_authenticated_user;
use crate::api::ApiError;
use crate::AppState;

fn ensure_signals_enabled(state: &AppState) -> Result<(), ApiError> {
    if !state.config.signals_enabled {
        return Err(ApiError::bad_request(
            "SIGNALS_DISABLED",
            "signal marketplace is disabled",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSignalProviderRequest {
    pub name: String,
    pub description: Option<String>,
    pub source_url: Option<String>,
    pub category: Option<String>,
    pub update_frequency_secs: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitSignalRequest {
    pub provider_id: String,
    pub market_slug: String,
    pub outcome: String,
    pub signal_value: f64,
    pub confidence: Option<f64>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalQuery {
    pub market_slug: Option<String>,
    pub category: Option<String>,
    pub min_brier: Option<f64>,
    pub limit: Option<i64>,
}

/// POST /v1/signals/providers — register a signal provider.
pub async fn create_provider(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateSignalProviderRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_signals_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    let name = body.name.trim();
    if name.is_empty() || name.len() > 128 {
        return Err(ApiError::bad_request(
            "INVALID_NAME",
            "name must be 1-128 chars",
        ));
    }

    let id = Uuid::new_v4().to_string();
    let category = body.category.as_deref().unwrap_or("general");
    let freq = body.update_frequency_secs.unwrap_or(3600).max(60);

    sqlx::query(
        "INSERT INTO signal_providers (id, owner, name, description, source_url, category, update_frequency_secs) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(&id)
    .bind(user.wallet_address.as_str())
    .bind(name)
    .bind(body.description.as_deref())
    .bind(body.source_url.as_deref())
    .bind(category)
    .bind(freq)
    .execute(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({ "id": id, "ok": true })))
}

/// GET /v1/signals/providers — list signal providers.
pub async fn list_providers(
    _req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<SignalQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_signals_enabled(&state)?;
    let limit = query.limit.unwrap_or(50).min(200);

    let rows: Vec<(
        String,
        String,
        String,
        Option<String>,
        String,
        i64,
        bool,
        Option<f64>,
        Option<i64>,
        String,
    )> = if let Some(cat) = &query.category {
        sqlx::query_as(
            "SELECT p.id, p.owner, p.name, p.description, p.category, \
                 p.update_frequency_secs, p.active, s.avg_brier_score, s.scored_signals, \
                 p.created_at::text \
                 FROM signal_providers p \
                 LEFT JOIN signal_provider_stats s ON s.provider_id = p.id \
                 WHERE p.active = true AND p.category = $1 \
                 ORDER BY s.avg_brier_score ASC NULLS LAST \
                 LIMIT $2",
        )
        .bind(cat)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    } else {
        sqlx::query_as(
            "SELECT p.id, p.owner, p.name, p.description, p.category, \
                 p.update_frequency_secs, p.active, s.avg_brier_score, s.scored_signals, \
                 p.created_at::text \
                 FROM signal_providers p \
                 LEFT JOIN signal_provider_stats s ON s.provider_id = p.id \
                 WHERE p.active = true \
                 ORDER BY s.avg_brier_score ASC NULLS LAST \
                 LIMIT $1",
        )
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
                "owner": r.1,
                "name": r.2,
                "description": r.3,
                "category": r.4,
                "updateFrequencySecs": r.5,
                "active": r.6,
                "avgBrierScore": r.7,
                "scoredSignals": r.8,
                "createdAt": r.9,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "providers": items })))
}

/// POST /v1/signals/emit — emit a signal for a market.
pub async fn emit_signal(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<EmitSignalRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_signals_enabled(&state)?;
    let user = extract_authenticated_user(&req, &state).await?;

    if body.signal_value < 0.0 || body.signal_value > 1.0 {
        return Err(ApiError::bad_request(
            "INVALID_SIGNAL",
            "signal_value must be between 0.0 and 1.0",
        ));
    }

    // Verify caller owns the specified provider.
    let provider: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM signal_providers WHERE id = $1 AND owner = $2 AND active = true",
    )
    .bind(&body.provider_id)
    .bind(user.wallet_address.as_str())
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let provider = provider.ok_or_else(|| {
        ApiError::bad_request(
            "PROVIDER_NOT_FOUND",
            "provider not found or not owned by you",
        )
    })?;

    let confidence = body.confidence.unwrap_or(0.5).clamp(0.0, 1.0);
    let metadata = body.metadata.clone().unwrap_or(json!({}));

    let mut tx = state
        .db
        .pool()
        .begin()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    sqlx::query(
        "INSERT INTO signal_emissions (provider_id, market_slug, outcome, signal_value, confidence, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&provider.0)
    .bind(&body.market_slug)
    .bind(&body.outcome)
    .bind(body.signal_value)
    .bind(confidence)
    .bind(&metadata)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    sqlx::query(
        "INSERT INTO signal_provider_stats (provider_id, total_signals, updated_at) \
         VALUES ($1, 1, NOW()) \
         ON CONFLICT (provider_id) \
         DO UPDATE SET total_signals = signal_provider_stats.total_signals + 1, \
                       updated_at = NOW()",
    )
    .bind(&provider.0)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    sqlx::query(
        "INSERT INTO signal_scores (provider_id, market_slug, outcome, predicted_prob) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (provider_id, market_slug, outcome) \
         DO UPDATE SET predicted_prob = EXCLUDED.predicted_prob, created_at = NOW()",
    )
    .bind(&provider.0)
    .bind(&body.market_slug)
    .bind(&body.outcome)
    .bind(body.signal_value)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(json!({ "ok": true })))
}

/// GET /v1/signals/market/{market_slug} — get latest signals for a market.
pub async fn get_market_signals(
    _req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    market_slug: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    ensure_signals_enabled(&state)?;
    let rows: Vec<(String, String, String, f64, f64, serde_json::Value, String)> = sqlx::query_as(
        "SELECT DISTINCT ON (e.provider_id, e.outcome) \
         p.name, p.id, e.outcome, e.signal_value, e.confidence, e.metadata, e.created_at::text \
         FROM signal_emissions e \
         JOIN signal_providers p ON p.id = e.provider_id \
         WHERE e.market_slug = $1 AND p.active = true \
         ORDER BY e.provider_id, e.outcome, e.created_at DESC",
    )
    .bind(market_slug.as_str())
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let signals: Vec<_> = rows
        .iter()
        .map(|r| {
            json!({
                "providerName": r.0,
                "providerId": r.1,
                "outcome": r.2,
                "signalValue": r.3,
                "confidence": r.4,
                "metadata": r.5,
                "createdAt": r.6,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({ "signals": signals })))
}

/// POST /v1/signals/score — score resolved markets (operator only).
pub async fn score_market(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<ScoreMarketRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_signals_enabled(&state)?;
    crate::api::compliance::ensure_admin_public(&req, &state)?;

    if body.actual_outcome != "yes" && body.actual_outcome != "no" {
        return Err(ApiError::bad_request(
            "INVALID_OUTCOME",
            "actual_outcome must be 'yes' or 'no'",
        ));
    }
    let actual = if body.actual_outcome == "yes" {
        1.0_f64
    } else {
        0.0
    };

    // Score all pending predictions for this market.
    let updated = sqlx::query(
        "UPDATE signal_scores \
         SET actual_outcome = $1, \
             brier_score = (predicted_prob - $2) * (predicted_prob - $2), \
             scored_at = NOW() \
         WHERE market_slug = $3 AND scored_at IS NULL",
    )
    .bind(actual > 0.5)
    .bind(actual)
    .bind(&body.market_slug)
    .execute(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Refresh provider stats.
    sqlx::query(
        "UPDATE signal_provider_stats SET \
         scored_signals = sub.cnt, \
         avg_brier_score = sub.avg_bs, \
         updated_at = NOW() \
         FROM ( \
             SELECT provider_id, COUNT(*) as cnt, AVG(brier_score) as avg_bs \
             FROM signal_scores \
             WHERE brier_score IS NOT NULL \
             GROUP BY provider_id \
         ) sub \
         WHERE signal_provider_stats.provider_id = sub.provider_id",
    )
    .execute(state.db.pool())
    .await
    .ok();

    Ok(HttpResponse::Ok().json(json!({
        "ok": true,
        "scored": updated.rows_affected(),
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreMarketRequest {
    pub market_slug: String,
    pub actual_outcome: String,
}
