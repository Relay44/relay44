use actix_web::{web, HttpResponse, Responder};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

use crate::services::{ComponentHealth, HealthChecks, HealthStatus, SystemHealth};
use crate::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    timestamp: String,
}

pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

pub async fn health_detailed(state: web::Data<Arc<AppState>>) -> impl Responder {
    let uptime = state.metrics.get_metrics().uptime_seconds;

    let db_health = check_database_health(&state).await;
    let redis_health = check_redis_health(&state).await;
    let base_health = check_base_health(&state).await;
    let solana_health = check_solana_health(&state).await;

    let overall_status = determine_overall_status(
        &db_health,
        &redis_health,
        &base_health,
        &solana_health,
        state.config.evm_enabled,
        state.config.solana_enabled,
    );

    let health = SystemHealth {
        status: overall_status,
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
        checks: HealthChecks {
            database: db_health,
            redis: redis_health,
            base: base_health,
            solana: solana_health,
        },
    };

    match overall_status {
        HealthStatus::Healthy => HttpResponse::Ok().json(health),
        HealthStatus::Degraded => HttpResponse::Ok().json(health),
        HealthStatus::Unhealthy => HttpResponse::ServiceUnavailable().json(health),
    }
}

pub async fn get_metrics(state: web::Data<Arc<AppState>>) -> impl Responder {
    let metrics = state.metrics.get_metrics();
    HttpResponse::Ok().json(metrics)
}

pub async fn get_metrics_prometheus(state: web::Data<Arc<AppState>>) -> impl Responder {
    let prometheus_output = state.metrics.export_prometheus();
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(prometheus_output)
}

async fn check_database_health(state: &web::Data<Arc<AppState>>) -> ComponentHealth {
    let start = Instant::now();

    match sqlx::query("SELECT 1").execute(state.db.pool()).await {
        Ok(_) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            let stats = state.db.pool_stats();

            if stats.size == 0 {
                ComponentHealth::unhealthy("No database connections available")
            } else if latency_ms > 500 {
                ComponentHealth::degraded(latency_ms, "High query latency")
            } else {
                ComponentHealth::healthy(latency_ms)
            }
        }
        Err(e) => ComponentHealth::unhealthy(&format!("Database query failed: {}", e)),
    }
}

async fn check_redis_health(state: &web::Data<Arc<AppState>>) -> ComponentHealth {
    let start = Instant::now();

    match state.redis.get::<String>("health_check").await {
        Ok(_) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            if latency_ms > 100 {
                ComponentHealth::degraded(latency_ms, "High latency")
            } else {
                ComponentHealth::healthy(latency_ms)
            }
        }
        Err(e) => ComponentHealth::unhealthy(&format!("Redis error: {}", e)),
    }
}

#[derive(serde::Deserialize)]
struct SolanaSlotResponse {
    result: Option<u64>,
}

async fn check_base_health(state: &web::Data<Arc<AppState>>) -> ComponentHealth {
    if !state.config.evm_enabled {
        return ComponentHealth::disabled("Base integration disabled");
    }

    let start = Instant::now();
    if state.evm_rpc.eth_block_number().await.is_err() {
        return ComponentHealth::unhealthy("Base RPC request failed");
    }
    let latency_ms = start.elapsed().as_millis() as u64;

    if latency_ms > 2000 {
        ComponentHealth::degraded(latency_ms, "High RPC latency")
    } else {
        ComponentHealth::healthy(latency_ms)
    }
}

async fn check_solana_health(state: &web::Data<Arc<AppState>>) -> ComponentHealth {
    if !state.config.solana_enabled {
        return ComponentHealth::disabled("Solana integration disabled");
    }
    if !state.config.solana_reads_enabled {
        return ComponentHealth::disabled("Solana reads disabled");
    }

    let start = Instant::now();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getSlot",
        "params": [{ "commitment": "confirmed" }]
    });

    let response = reqwest::Client::new()
        .post(&state.config.solana_rpc_url)
        .json(&body)
        .send()
        .await;

    let latency_ms = start.elapsed().as_millis() as u64;
    let Ok(response) = response else {
        return ComponentHealth::unhealthy("Solana RPC request failed");
    };
    if !response.status().is_success() {
        return ComponentHealth::unhealthy("Solana RPC returned non-success status");
    }

    let payload = response.json::<SolanaSlotResponse>().await;
    let Ok(payload) = payload else {
        return ComponentHealth::unhealthy("Failed to decode Solana RPC response");
    };
    if payload.result.is_none() {
        return ComponentHealth::unhealthy("Solana RPC response missing slot");
    }

    if latency_ms > 2000 {
        ComponentHealth::degraded(latency_ms, "High RPC latency")
    } else {
        ComponentHealth::healthy(latency_ms)
    }
}

fn determine_overall_status(
    db: &ComponentHealth,
    redis: &ComponentHealth,
    base: &ComponentHealth,
    solana: &ComponentHealth,
    evm_enabled: bool,
    solana_enabled: bool,
) -> HealthStatus {
    if db.status == HealthStatus::Unhealthy {
        return HealthStatus::Unhealthy;
    }

    if db.status == HealthStatus::Degraded || redis.status == HealthStatus::Degraded {
        return HealthStatus::Degraded;
    }

    if evm_enabled
        && (base.status == HealthStatus::Degraded || base.status == HealthStatus::Unhealthy)
    {
        return HealthStatus::Degraded;
    }

    if solana_enabled
        && (solana.status == HealthStatus::Degraded || solana.status == HealthStatus::Unhealthy)
    {
        return HealthStatus::Degraded;
    }

    if redis.status == HealthStatus::Unhealthy {
        return HealthStatus::Degraded;
    }

    HealthStatus::Healthy
}
