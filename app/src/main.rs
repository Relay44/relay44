use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{http::header, middleware as actix_middleware, web, App, HttpServer};
use dotenvy::dotenv;
use log::{info, warn};
use relay44_backend::{configure_routes, AppState};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

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

fn is_base_rpc_rate_limited(err: &anyhow::Error) -> bool {
    err.to_string().contains("429 Too Many Requests")
}

fn base_indexer_backoff_seconds(rate_limit_streak: u32) -> u64 {
    match rate_limit_streak {
        0 => 20,
        1 => 60,
        2 => 120,
        3 => 240,
        _ => 300,
    }
}

fn spawn_background_services(app_state: Arc<AppState>) {
    let config = app_state.config.clone();

    if config.evm_enabled && config.evm_reads_enabled {
        let market_core = config.market_core_address.clone();
        let order_book = config.order_book_address.clone();
        let indexer = app_state.evm_indexer.clone();
        let db = app_state.db.clone();
        let rpc = app_state.evm_rpc.clone();
        let lookback_blocks = config.indexer_lookback_blocks;
        let confirmations = config.indexer_confirmations;

        if market_core.is_empty() || order_book.is_empty() {
            warn!(
                "Skipping EVM indexer start: MARKET_CORE_ADDRESS or ORDER_BOOK_ADDRESS is missing"
            );
        } else {
            tokio::spawn(async move {
                match db.get_chain_sync_cursor("evm_indexer_main").await {
                    Ok(Some(cursor)) => {
                        indexer.set_last_synced_block(cursor.last_block).await;
                        info!(
                            "Restored EVM indexer cursor from DB at block {}",
                            cursor.last_block
                        );
                    }
                    Ok(None) => {}
                    Err(err) => warn!("Failed to restore EVM indexer cursor: {}", err),
                }

                info!("Starting EVM log indexer background loop");
                const TOPICS: [&str; 6] = [
                    "0x550857481380e1875f94e5eac6470eff69ecd368405067d9d5dfdf645d3d1f8e",
                    "0xbc7c1013df472d2b00db2b9da4c476dbf8f0bc22116913d78750cf21d2c80fc2",
                    "0xac1c16fb14f9a45ec49f65d268ff0d0f1945c504b82df54a9c6ad9f01b059be5",
                    "0x9384174c8517f5537b08e79211fc039e8a098571a3a2b4cb21dfa6f3237e8de1",
                    "0x5aac01386940f75e601757cfe5dc1d4ab2bac84f98d30664486114a8abb38a45",
                    "0x93c1c30a0fa404e7a08a9f6a9d68323786a7e120f3adc0c16eb8855922e35dfa",
                ];
                let mut rate_limit_streak = 0_u32;

                loop {
                    let latest_block = match rpc.eth_block_number().await {
                        Ok(block) => block,
                        Err(err) => {
                            let delay_seconds = if is_base_rpc_rate_limited(&err) {
                                rate_limit_streak = rate_limit_streak.saturating_add(1);
                                let delay = base_indexer_backoff_seconds(rate_limit_streak);
                                warn!(
                                    "Base RPC rate limited for indexer latest block fetch; backing off {}s (streak={})",
                                    delay, rate_limit_streak
                                );
                                delay
                            } else {
                                rate_limit_streak = 0;
                                warn!("Failed to fetch latest Base block for indexer: {}", err);
                                base_indexer_backoff_seconds(0)
                            };
                            tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds))
                                .await;
                            continue;
                        }
                    };
                    let target_block = latest_block.saturating_sub(confirmations);

                    match indexer
                        .sync(
                            &market_core,
                            &order_book,
                            lookback_blocks,
                            &TOPICS,
                            Some(target_block),
                        )
                        .await
                    {
                        Ok(log_count) => {
                            rate_limit_streak = 0;
                            let last_synced = indexer.last_synced_block().await;
                            let meta = json!({
                                "latestBlock": latest_block,
                                "targetBlock": target_block,
                                "logsIndexed": log_count,
                                "updatedAt": chrono::Utc::now().to_rfc3339(),
                            });
                            if let Err(err) = db
                                .upsert_chain_sync_cursor("evm_indexer_main", last_synced, meta)
                                .await
                            {
                                warn!("Failed to persist EVM indexer cursor: {}", err);
                            }
                        }
                        Err(err) => {
                            if is_base_rpc_rate_limited(&err) {
                                rate_limit_streak = rate_limit_streak.saturating_add(1);
                                let delay = base_indexer_backoff_seconds(rate_limit_streak);
                                warn!(
                                    "Base RPC rate limited during indexer sync; backing off {}s (streak={})",
                                    delay, rate_limit_streak
                                );
                                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                                continue;
                            }
                            rate_limit_streak = 0;
                            warn!("EVM indexer sync failed: {}", err);
                        }
                    }

                    tokio::time::sleep(tokio::time::Duration::from_secs(
                        base_indexer_backoff_seconds(0),
                    ))
                    .await;
                }
            });
        }
    } else {
        info!("EVM indexer disabled by config toggles");
    }

    {
        let ws_state = app_state.clone();
        let mut event_rx = app_state.event_bus.subscribe();
        tokio::spawn(async move {
            use relay44_backend::services::websocket::{MarketUpdate, TradeUpdate};
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        let now = chrono::Utc::now().timestamp();
                        match &event {
                            relay44_backend::services::event_bus::PlatformEvent::AgentExecuted(e) => {
                                ws_state
                                    .ws_hub
                                    .broadcast_trade(TradeUpdate {
                                        market_id: e.market_id.clone(),
                                        outcome: e.outcome.clone(),
                                        price: e.price,
                                        quantity: 0,
                                        buyer: e.owner.clone(),
                                        seller: String::new(),
                                        timestamp: now,
                                    })
                                    .await;
                            }
                            relay44_backend::services::event_bus::PlatformEvent::PositionOpened(e)
                            | relay44_backend::services::event_bus::PlatformEvent::PositionClosed(e) => {
                                ws_state
                                    .ws_hub
                                    .broadcast_market(MarketUpdate {
                                        market_id: e.market_id.clone(),
                                        yes_price: e.entry_price,
                                        no_price: 1.0 - e.entry_price,
                                        status: "active".to_string(),
                                        timestamp: now,
                                    })
                                    .await;
                            }
                            _ => {}
                        }

                        if let Ok(json_str) = serde_json::to_string(&event) {
                            let _ = ws_state.ws_hub.broadcast_raw_global(json_str).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("EventBus→WS bridge lagged by {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    relay44_backend::services::agent_scheduler::spawn_agent_scheduler(app_state.clone());
    relay44_backend::services::distribution_scheduler::spawn_distribution_scheduler(app_state.clone());
    relay44_backend::services::liquidity_mirror::spawn_liquidity_mirror(app_state.clone());
    relay44_backend::services::hedge_engine::spawn_hedge_engine(app_state.clone());
    relay44_backend::services::smart_router::spawn_arb_scanner(app_state.clone());
    relay44_backend::services::portfolio_snapshot::spawn_portfolio_snapshotter(app_state.clone());
    relay44_backend::services::polymarket_scanner::spawn_scanner(app_state.clone());
    relay44_backend::services::limitless_scanner::spawn_limitless_scanner(app_state.clone());
    relay44_backend::services::aerodrome_scanner::spawn_aerodrome_scanner(app_state.clone());
    relay44_backend::services::market_creator::spawn_market_creator(app_state);
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
        kyc,
        limitless_partner,
        is_shutting_down: Arc::new(AtomicBool::new(false)),
    });

    let migration_db = app_state.db.clone();
    let background_state = app_state.clone();
    tokio::spawn(async move {
        loop {
            match migration_db.run_migrations().await {
                Ok(()) => {
                    info!("Database migrations completed; starting background services");
                    spawn_background_services(background_state);
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
            .wrap(relay44_backend::middleware::GeoBlock::new(geo_blocking_enabled))
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
