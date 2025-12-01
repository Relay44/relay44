use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{http::header, middleware as actix_middleware, web, App, HttpServer};
use dotenvy::dotenv;
use log::{info, warn};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod api;
mod config;
mod middleware;
mod models;
mod services;

use api::JwtService;
use config::AppConfig;
use services::{
    DatabaseService, EvmIndexerService, EvmRpcService, MetricsService, OrderBookService,
    RedisService, WebSocketHub,
};

pub struct AppState {
    pub config: AppConfig,
    pub db: DatabaseService,
    pub evm_rpc: EvmRpcService,
    pub evm_indexer: EvmIndexerService,
    pub orderbook: OrderBookService,
    pub redis: RedisService,
    pub jwt: JwtService,
    pub metrics: MetricsService,
    pub ws_hub: WebSocketHub,
    pub is_shutting_down: Arc<AtomicBool>,
}

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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    services::logging::init();

    info!("Starting relay44 backend API...");

    let config = AppConfig::from_env();
    let bind_addr = format!("{}:{}", config.host, config.port);

    info!("Initializing services...");

    let db = DatabaseService::new(&config.database_url)
        .await
        .expect("Failed to connect to database");

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
        is_shutting_down: Arc::new(AtomicBool::new(false)),
    });

    if config.evm_enabled && config.evm_reads_enabled {
        let market_core = config.market_core_address.clone();
        let order_book = config.order_book_address.clone();
        let indexer = evm_indexer.clone();
        let db = app_state.db.clone();
        let rpc = app_state.evm_rpc.clone();
        let lookback_blocks = config.indexer_lookback_blocks;
        let confirmations = config.indexer_confirmations;

        if market_core.is_empty() || order_book.is_empty() {
            warn!(
                "Skipping EVM indexer start: MARKET_CORE_ADDRESS or ORDER_BOOK_ADDRESS is missing"
            );
        } else {
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
            tokio::spawn(async move {
                const TOPICS: [&str; 6] = [
                    "0x550857481380e1875f94e5eac6470eff69ecd368405067d9d5dfdf645d3d1f8e", // MarketCreated
                    "0xbc7c1013df472d2b00db2b9da4c476dbf8f0bc22116913d78750cf21d2c80fc2", // MarketResolved
                    "0xac1c16fb14f9a45ec49f65d268ff0d0f1945c504b82df54a9c6ad9f01b059be5", // OrderPlaced
                    "0x9384174c8517f5537b08e79211fc039e8a098571a3a2b4cb21dfa6f3237e8de1", // OrderCanceled
                    "0x5aac01386940f75e601757cfe5dc1d4ab2bac84f98d30664486114a8abb38a45", // OrderFilled
                    "0x93c1c30a0fa404e7a08a9f6a9d68323786a7e120f3adc0c16eb8855922e35dfa", // Claimed
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
            .wrap(crate::middleware::GeoBlock::new(geo_blocking_enabled))
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
            .wrap(crate::middleware::AccessLog)
            .wrap(crate::middleware::RequestIdMiddleware)
            .app_data(web::JsonConfig::default().limit(4096))
            .route("/health", web::get().to(api::health::health_check))
            .route(
                "/health/detailed",
                web::get().to(api::health::health_detailed),
            )
            .route("/metrics", web::get().to(api::health::get_metrics))
            .route(
                "/metrics/prometheus",
                web::get().to(api::health::get_metrics_prometheus),
            )
            .route("/ws", web::get().to(api::ws_handler))
            .service(
                web::scope("/v1")
                    .service(
                        web::scope("/markets")
                            .route("", web::get().to(api::markets::list_markets))
                            .route("", web::post().to(api::markets::create_market))
                            .route("/{market_id}", web::get().to(api::markets::get_market))
                            .route(
                                "/{market_id}/orderbook",
                                web::get().to(api::markets::get_orderbook),
                            )
                            .route(
                                "/{market_id}/trades",
                                web::get().to(api::markets::get_trades),
                            ),
                    )
                    .service(
                        web::scope("/orders")
                            .route("", web::get().to(api::orders::list_orders))
                            .route("", web::post().to(api::orders::place_order))
                            .route("/{order_id}", web::get().to(api::orders::get_order))
                            .route("/{order_id}", web::delete().to(api::orders::cancel_order)),
                    )
                    .service(
                        web::scope("/positions")
                            .route("", web::get().to(api::positions::list_positions))
                            .route("/{market_id}", web::get().to(api::positions::get_position))
                            .route(
                                "/{market_id}/claim",
                                web::post().to(api::positions::claim_winnings),
                            ),
                    )
                    .service(
                        web::scope("/user")
                            .route("/profile", web::get().to(api::user::get_profile))
                            .route("/transactions", web::get().to(api::user::get_transactions)),
                    )
                    .service(
                        web::scope("/notifications")
                            .route("", web::get().to(api::notifications::list_notifications))
                            .route(
                                "/unread-count",
                                web::get().to(api::notifications::get_unread_count),
                            )
                            .route(
                                "/read-all",
                                web::put().to(api::notifications::mark_all_notifications_read),
                            )
                            .route(
                                "/preferences",
                                web::get().to(api::notifications::get_notification_preferences),
                            )
                            .route(
                                "/preferences",
                                web::put()
                                    .to(api::notifications::update_notification_preferences),
                            )
                            .route(
                                "/{notification_id}/read",
                                web::put().to(api::notifications::mark_notification_read),
                            ),
                    )
                    .service(
                        web::scope("/wallet")
                            .route("/balance", web::get().to(api::wallet::get_balance))
                            .route(
                                "/deposit/address",
                                web::get().to(api::wallet::get_deposit_address),
                            )
                            .route("/deposit", web::post().to(api::wallet::deposit))
                            .route("/withdraw", web::post().to(api::wallet::withdraw)),
                    )
                    .route(
                        "/webhooks/blindfold",
                        web::post().to(api::wallet::blindfold_webhook),
                    )
                    .service(
                        web::scope("/auth")
                            .route("/nonce", web::get().to(api::auth::get_nonce))
                            .route("/login", web::post().to(api::auth::login))
                            .route("/siwe/nonce", web::get().to(api::auth::get_siwe_nonce))
                            .route("/siwe/login", web::post().to(api::auth::siwe_login))
                            .route("/solana/nonce", web::get().to(api::auth::get_solana_nonce))
                            .route("/solana/login", web::post().to(api::auth::solana_login))
                            .route("/farcaster/nonce", web::get().to(api::auth::get_farcaster_nonce))
                            .route("/farcaster/login", web::post().to(api::auth::farcaster_login))
                            .route("/refresh", web::post().to(api::auth::refresh_token))
                            .route("/logout", web::post().to(api::auth::logout)),
                    )
                    .service(
                        web::scope("/payments").service(
                            web::scope("/x402")
                                .route("/quote", web::get().to(api::payments::get_x402_quote))
                                .route(
                                    "/verify",
                                    web::post().to(api::payments::verify_x402_payment),
                                ),
                        ),
                    )
                    .service(
                        web::scope("/evm")
                            .route("/markets", web::get().to(api::evm::get_base_markets))
                            .route(
                                "/markets/{market_id}",
                                web::get().to(api::evm::get_base_market),
                            )
                            .route("/agents", web::get().to(api::evm::get_base_agents))
                            .route(
                                "/payouts/candidates",
                                web::get().to(api::evm::get_base_payout_candidates),
                            )
                            .route(
                                "/matcher/health",
                                web::get().to(api::evm::get_matcher_health),
                            )
                            .route("/matcher/stats", web::get().to(api::evm::get_matcher_stats))
                            .route("/matcher/pause", web::post().to(api::evm::pause_matcher))
                            .route("/matcher/resume", web::post().to(api::evm::resume_matcher))
                            .route(
                                "/matcher/report",
                                web::post().to(api::evm::report_matcher_cycle),
                            )
                            .route(
                                "/payouts/health",
                                web::get().to(api::evm::get_payout_health),
                            )
                            .route(
                                "/payouts/backlog",
                                web::get().to(api::evm::get_payout_backlog),
                            )
                            .route("/payouts/jobs", web::get().to(api::evm::get_payout_jobs))
                            .route(
                                "/payouts/report",
                                web::post().to(api::evm::report_payout_job),
                            )
                            .route(
                                "/indexer/health",
                                web::get().to(api::evm::get_indexer_health),
                            )
                            .route("/indexer/lag", web::get().to(api::evm::get_indexer_lag))
                            .route(
                                "/indexer/backfill",
                                web::post().to(api::evm::trigger_indexer_backfill),
                            )
                            .route(
                                "/agents/{agent_id}",
                                web::get().to(api::evm::get_base_agent),
                            )
                            .route(
                                "/identity/{wallet}",
                                web::get().to(api::evm::get_base_identity),
                            )
                            .route(
                                "/reputation/{wallet}",
                                web::get().to(api::evm::get_base_reputation),
                            )
                            .route(
                                "/validation/{request_hash}",
                                web::get().to(api::evm::get_base_validation),
                            )
                            .route(
                                "/markets/{market_id}/orderbook",
                                web::get().to(api::evm::get_base_orderbook),
                            )
                            .route(
                                "/markets/{market_id}/trades",
                                web::get().to(api::evm::get_base_trades),
                            )
                            .route(
                                "/token/state",
                                web::get().to(api::evm::get_r44_token_state),
                            )
                            .service(
                                web::scope("/write")
                                    .route(
                                        "/markets/create",
                                        web::post().to(api::evm::prepare_create_market_write),
                                    )
                                    .route(
                                        "/orders/place",
                                        web::post().to(api::evm::prepare_place_order_write),
                                    )
                                    .route(
                                        "/orders/cancel",
                                        web::post().to(api::evm::prepare_cancel_order_write),
                                    )
                                    .route(
                                        "/orders/match",
                                        web::post().to(api::evm::prepare_match_orders_write),
                                    )
                                    .route(
                                        "/positions/claim",
                                        web::post().to(api::evm::prepare_claim_write),
                                    )
                                    .route(
                                        "/positions/claim-for",
                                        web::post().to(api::evm::prepare_claim_for_write),
                                    )
                                    .route(
                                        "/agents/create",
                                        web::post().to(api::evm::prepare_create_agent_write),
                                    )
                                    .route(
                                        "/agents/execute",
                                        web::post().to(api::evm::prepare_execute_agent_write),
                                    )
                                    .route(
                                        "/identity/register",
                                        web::post()
                                            .to(api::evm::prepare_erc8004_register_identity_write),
                                    )
                                    .route(
                                        "/identity/tier",
                                        web::post().to(api::evm::prepare_erc8004_set_tier_write),
                                    )
                                    .route(
                                        "/identity/active",
                                        web::post().to(api::evm::prepare_erc8004_set_active_write),
                                    )
                                    .route(
                                        "/reputation/outcome",
                                        web::post()
                                            .to(api::evm::prepare_erc8004_submit_outcome_write),
                                    )
                                    .route(
                                        "/validation/request",
                                        web::post()
                                            .to(api::evm::prepare_erc8004_validation_request_write),
                                    )
                                    .route(
                                        "/validation/response",
                                        web::post().to(
                                            api::evm::prepare_erc8004_validation_response_write,
                                        ),
                                    )
                                    .route(
                                        "/relay",
                                        web::post().to(api::evm::relay_raw_transaction),
                                    ),
                            ),
                    )
                    .service(
                        web::scope("/external")
                            .route(
                                "/credentials",
                                web::get().to(api::external::list_external_credentials),
                            )
                            .route(
                                "/credentials",
                                web::post().to(api::external::upsert_external_credentials),
                            )
                            .route(
                                "/credentials/status",
                                web::get().to(api::external::get_external_credential_status),
                            )
                            .route(
                                "/credentials/limitless/wallet-bind",
                                web::post().to(api::external::bind_limitless_wallet),
                            )
                            .route(
                                "/credentials/{credential_id}",
                                web::delete().to(api::external::delete_external_credentials),
                            )
                            .route(
                                "/orders/intent",
                                web::post().to(api::external::create_external_order_intent),
                            )
                            .route(
                                "/orders/submit",
                                web::post().to(api::external::submit_external_order),
                            )
                            .route(
                                "/orders/cancel",
                                web::post().to(api::external::cancel_external_order),
                            )
                            .route(
                                "/orders",
                                web::get().to(api::external::list_external_orders),
                            )
                            .route(
                                "/agents",
                                web::get().to(api::external::list_external_agents),
                            )
                            .route(
                                "/agents",
                                web::post().to(api::external::create_external_agent),
                            )
                            .route(
                                "/agents/performance",
                                web::get().to(api::external::get_external_agents_performance),
                            )
                            .route(
                                "/agents/runner/tick",
                                web::post().to(api::external::run_external_agents_tick),
                            )
                            .route(
                                "/agents/{agent_id}",
                                web::patch().to(api::external::update_external_agent),
                            )
                            .route(
                                "/agents/{agent_id}/execute",
                                web::post().to(api::external::execute_external_agent),
                            ),
                    )
                    .service(
                        web::scope("/decisions")
                            .route("", web::get().to(api::decisions::list_decision_cells))
                            .route("", web::post().to(api::decisions::create_decision_cell))
                            .route(
                                "/runner/tick",
                                web::post().to(api::decisions::run_decision_cells_tick),
                            )
                            .route("/{cell_id}", web::get().to(api::decisions::get_decision_cell))
                            .route(
                                "/{cell_id}",
                                web::patch().to(api::decisions::update_decision_cell),
                            )
