use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;

use crate::{api::ApiError, AppState};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolMetricsResponse {
    pub markets: ProtocolMarketMetrics,
    pub volume: ProtocolVolumeMetrics,
    pub agents: ProtocolAgentMetrics,
    pub collateral: ProtocolCollateralMetrics,
    pub source: String,
    pub updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolMarketMetrics {
    pub total: i64,
    pub active: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolVolumeMetrics {
    pub settlement_usdc: f64,
    pub table_reported_usdc: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolAgentMetrics {
    pub connected: i64,
    pub active: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolCollateralMetrics {
    pub usdc: f64,
}

async fn query_i64(state: &AppState, sql: &str) -> Result<i64, ApiError> {
    sqlx::query_scalar::<_, i64>(sql)
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| {
            log::error!("protocol metrics integer query failed: {}", err);
            ApiError::internal("Failed to load protocol metrics")
        })
}

async fn query_f64(state: &AppState, sql: &str) -> Result<f64, ApiError> {
    sqlx::query_scalar::<_, f64>(sql)
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| {
            log::error!("protocol metrics decimal query failed: {}", err);
            ApiError::internal("Failed to load protocol metrics")
        })
}

pub async fn get_protocol_metrics(
    state: web::Data<Arc<AppState>>,
) -> Result<HttpResponse, ApiError> {
    let total_markets = query_i64(
        &state,
        "SELECT \
            (SELECT COUNT(*) FROM markets)::BIGINT + \
            (SELECT COUNT(*) FROM distribution_markets)::BIGINT",
    )
    .await?;

    let active_markets = query_i64(
        &state,
        "SELECT \
            (SELECT COUNT(*) FROM markets WHERE status = 0)::BIGINT + \
            (SELECT COUNT(*) FROM distribution_markets WHERE status = 0)::BIGINT",
    )
    .await?;

    let settlement_usdc = query_f64(
        &state,
        "SELECT \
            COALESCE((SELECT SUM(collateral_amount)::DOUBLE PRECISION / 1000000.0 FROM trades), 0) + \
            COALESCE((SELECT SUM(cost)::DOUBLE PRECISION FROM distribution_trades), 0)",
    )
    .await?;

    let table_reported_usdc = query_f64(
        &state,
        "SELECT \
            COALESCE((SELECT SUM(total_volume)::DOUBLE PRECISION FROM markets), 0) + \
            COALESCE((SELECT SUM(total_volume)::DOUBLE PRECISION / 1000000.0 FROM distribution_markets), 0)",
    )
    .await?;

    let collateral_usdc = query_f64(
        &state,
        "SELECT \
            COALESCE((SELECT SUM(total_collateral)::DOUBLE PRECISION / 1000000.0 FROM markets), 0) + \
            COALESCE((SELECT SUM(total_collateral)::DOUBLE PRECISION / 1000000.0 FROM distribution_markets), 0)",
    )
    .await?;

    let connected_agents = query_i64(
        &state,
        "SELECT \
            (SELECT COUNT(*) FROM external_agents)::BIGINT + \
            (SELECT COUNT(*) FROM managed_agents)::BIGINT + \
            (SELECT COUNT(DISTINCT agent_id) FROM base_market_bootstrap_agents WHERE agent_id IS NOT NULL)::BIGINT",
    )
    .await?;

    let active_agents = query_i64(
        &state,
        "SELECT \
            (SELECT COUNT(*) FROM external_agents WHERE active = true)::BIGINT + \
            (SELECT COUNT(*) FROM managed_agents WHERE status = 'active')::BIGINT + \
            (SELECT COUNT(DISTINCT agent_id) FROM base_market_bootstrap_agents WHERE active = true AND agent_id IS NOT NULL)::BIGINT",
    )
    .await?;

    Ok(HttpResponse::Ok().json(ProtocolMetricsResponse {
        markets: ProtocolMarketMetrics {
            total: total_markets,
            active: active_markets,
        },
        volume: ProtocolVolumeMetrics {
            settlement_usdc,
            table_reported_usdc,
        },
        agents: ProtocolAgentMetrics {
            connected: connected_agents,
            active: active_agents,
        },
        collateral: ProtocolCollateralMetrics {
            usdc: collateral_usdc,
        },
        source: "relay44-api".to_string(),
        updated_at: Utc::now().to_rfc3339(),
    }))
}
