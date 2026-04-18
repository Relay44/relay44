use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{http::header, middleware as actix_middleware, web, App, HttpServer};
use dotenvy::dotenv;
use log::{info, warn};
use relay44_backend::{configure_routes, AppState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use relay44_backend::api::JwtService;
use relay44_backend::config::AppConfig;
use relay44_backend::services::{
    DatabaseService, EventBus, EvmIndexerService, EvmRpcService, MetricsService, OrderBookService,
    RedisService, WebSocketHub,
};

async fn graceful_shutdown(state: Arc<AppState>) {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");
    info!("Shutdown signal received, initiating graceful shutdown...");

    state.is_shutting_down.store(true, Ordering::SeqCst);

    info!("Waiting for in-flight requests to complete...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    info!("Graceful shutdown complete");
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    relay44_backend::services::logging::init();

    info!("Starting relay44 backend API...");

    let config = AppConfig::from_env();
    let bind_addr = format!("{}:{}", config.host, config.port);

    info!("Initializing services...");

    let db = DatabaseService::new(&config.database_url)
        .await
        .expect("Failed to create database pool");

    let redis = RedisService::new(&config.redis_url)
        .await
        .expect("Failed to connect to Redis");
    let evm_rpc = EvmRpcService::new(&config.base_rpc_url, &config.base_rpc_fallback_urls);
    let evm_indexer_rpc = config
        .base_indexer_rpc_url
        .as_ref()
        .map(|url| {
            let mut fallbacks = vec![config.base_rpc_url.clone()];
            fallbacks.extend(config.base_rpc_fallback_urls.clone());
            EvmRpcService::new(url, &fallbacks)
        })
        .unwrap_or_else(|| evm_rpc.clone());
    let evm_indexer = EvmIndexerService::new(evm_indexer_rpc, 20_000);

    info!(
        "Base read RPC endpoints configured: {} (dedicated indexer RPC: {})",
        config.base_rpc_fallback_urls.len() + 1,
        config.base_indexer_rpc_url.is_some()
    );

    let orderbook = OrderBookService::new();

    match db.load_orderbook_entries().await {
        Ok(entries) => {
            let count = entries.len();
            orderbook.restore_from_entries(entries);
            if count > 0 {
                info!("Restored {} order book entries from database", count);
            }
        }
        Err(e) => {
            warn!(
                "Failed to restore order book (table may not exist yet): {}",
                e
            );
        }
    }

    let jwt = JwtService::new(&config.jwt_secret);
    let metrics = MetricsService::new();
    let ws_hub = WebSocketHub::new();
    let event_bus = EventBus::new();
    let market_data = Arc::new(relay44_backend::services::market_data::MarketDataBus::new());

    let kyc = relay44_backend::services::kyc::KycService::new(
        relay44_backend::services::kyc::KycConfig::from_env(),
    );
    let limitless_partner =
        relay44_backend::services::limitless_partner::LimitlessPartnerConfig::from_config(&config);

    let app_state = Arc::new(AppState {
        config: config.clone(),
        db,
        evm_rpc,
        evm_indexer: evm_indexer.clone(),
        orderbook,
        redis,
        jwt,
        metrics,
        ws_hub,
        event_bus,
        market_data,
        kyc,
        limitless_partner,
        is_shutting_down: Arc::new(AtomicBool::new(false)),
    });

    relay44_backend::services::market_data::cache_writer::spawn(app_state.clone());
    relay44_backend::services::probability_alert::spawn(app_state.clone());
    relay44_backend::services::telegram_commands::spawn(app_state.clone());
    relay44_backend::services::cross_venue_arb::spawn(app_state.clone());
    relay44_backend::services::new_market_alert::spawn(app_state.clone());
    relay44_backend::services::volume_spike_alert::spawn(app_state.clone());

    let migration_db = app_state.db.clone();
    let background_state = app_state.clone();
    tokio::spawn(async move {
        loop {
            match migration_db.run_migrations().await {
                Ok(()) => {
                    info!("Database migrations completed; starting background services");
                    relay44_backend::services::orchestrator::spawn_background_services(
                        background_state,
                    );
                    return;
                }
                Err(e) => {
                    warn!("Migration failed: {}. Retrying in 30s...", e);
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
            }
        }
    });

    let shutdown_state = app_state.clone();
    tokio::spawn(async move {
        graceful_shutdown(shutdown_state).await;
    });

    info!("Starting HTTP server on {}", bind_addr);

    let governor_conf = GovernorConfigBuilder::default()
        .seconds_per_request(1)
        .burst_size(60)
        .finish()
        .expect("valid governor configuration");

    let config_clone = config.clone();
    let geo_blocking_enabled = std::env::var("GEO_BLOCKING_ENABLED")
        .ok()
        .map(|value| value.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(true);

    HttpServer::new(move || {
        let cors = if config_clone.is_development {
            warn!("CORS: Development mode - allowing all origins");
            Cors::default()
                .allow_any_origin()
                .allowed_methods(vec!["GET", "POST", "PATCH", "DELETE", "OPTIONS"])
                .allowed_headers(vec![
                    header::AUTHORIZATION,
                    header::ACCEPT,
                    header::CONTENT_TYPE,
                ])
                .max_age(3600)
        } else {
            let mut cors = Cors::default()
                .allowed_methods(vec!["GET", "POST", "PATCH", "DELETE", "OPTIONS"])
                .allowed_headers(vec![
                    header::AUTHORIZATION,
                    header::ACCEPT,
                    header::CONTENT_TYPE,
                ])
                .max_age(3600);

            for origin in &config_clone.cors_origins {
                if origin != "*" {
                    cors = cors.allowed_origin(origin);
                }
            }
            cors
        };

        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(Governor::new(&governor_conf))
            .wrap(relay44_backend::middleware::GeoBlock::new(
                geo_blocking_enabled,
            ))
            .wrap(cors)
            .wrap(
                actix_middleware::DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("X-XSS-Protection", "1; mode=block"))
                    .add(("Referrer-Policy", "strict-origin-when-cross-origin"))
                    .add((
                        "Permissions-Policy",
                        "geolocation=(), microphone=(), camera=()",
                    )),
            )
            .wrap(actix_middleware::Compress::default())
            .wrap(relay44_backend::middleware::AccessLog)
            .wrap(relay44_backend::middleware::RequestIdMiddleware)
            .app_data(web::JsonConfig::default().limit(4 * 1024 * 1024))
            .configure(configure_routes)
    })
    .bind(&bind_addr)?
    .run()
    .await
}
