#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::manual_clamp,
    clippy::manual_div_ceil,
    clippy::manual_is_multiple_of,
    clippy::manual_ignore_case_cmp,
    clippy::manual_abs_diff,
    clippy::manual_range_contains,
    clippy::manual_async_fn,
    clippy::manual_flatten,
    clippy::needless_as_bytes,
    clippy::needless_borrow,
    clippy::needless_lifetimes,
    clippy::needless_bool,
    clippy::needless_question_mark,
    clippy::cloned_ref_to_slice_refs,
    clippy::format_in_format_args,
    clippy::unnecessary_map_or,
    clippy::unnecessary_cast,
    clippy::collapsible_if,
    clippy::clone_on_copy,
    clippy::bind_instead_of_map,
    clippy::redundant_closure,
    clippy::should_implement_trait,
    clippy::match_like_matches_macro,
    clippy::if_same_then_else,
    dead_code,
    unused_imports,
    unused_variables,
    private_interfaces
)]

pub mod api;
pub mod config;
pub mod middleware;
pub mod models;
pub mod services;

use actix_web::web;
use api::JwtService;
use config::AppConfig;
use services::{
    DatabaseService, EventBus, EvmIndexerService, EvmRpcService, MetricsService, OrderBookService,
    RedisService, WebSocketHub,
};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

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
    pub event_bus: EventBus,
    pub kyc: services::kyc::KycService,
    pub limitless_partner: Option<services::limitless_partner::LimitlessPartnerConfig>,
    pub is_shutting_down: Arc<AtomicBool>,
}

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
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
                        .route("/batch", web::post().to(api::orders::batch_place_orders))
                        .route("/cancel-batch", web::post().to(api::orders::batch_cancel_orders))
                        .route("/replace", web::post().to(api::orders::replace_orders))
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
                    web::scope("/distribution")
                        .service(
                            web::scope("/markets")
                                .route("", web::get().to(api::distribution::list_dist_markets))
                                .route("", web::post().to(api::distribution::create_dist_market))
                                .route("/{id}", web::get().to(api::distribution::get_dist_market))
                                .route("/{id}/quote", web::get().to(api::distribution::get_quote))
                                .route("/{id}/trade", web::post().to(api::distribution::open_position))
                                .route("/{id}/resolve", web::post().to(api::distribution::resolve_market))
                                .route("/{id}/curve", web::get().to(api::distribution::get_curve))
                                .route("/{id}/history", web::get().to(api::distribution::get_curve_history))
                                .route("/{id}/activity", web::get().to(api::distribution::get_market_activity)),
                        )
                        .service(
                            web::scope("/positions")
                                .route("", web::get().to(api::distribution::list_positions))
                                .route("/{id}", web::delete().to(api::distribution::close_position))
                                .route("/{id}/claim", web::post().to(api::distribution::claim_payout)),
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
                            web::put().to(api::notifications::update_notification_preferences),
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
                .service(
                    web::scope("/auth")
                        .route("/nonce", web::get().to(api::auth::get_nonce))
                        .route("/login", web::post().to(api::auth::login))
                        .route("/siwe/nonce", web::get().to(api::auth::get_siwe_nonce))
                        .route("/siwe/login", web::post().to(api::auth::siwe_login))
                        .route("/solana/nonce", web::get().to(api::auth::get_solana_nonce))
                        .route("/solana/login", web::post().to(api::auth::solana_login))
                        .route(
                            "/farcaster/nonce",
                            web::get().to(api::auth::get_farcaster_nonce),
                        )
                        .route(
                            "/farcaster/login",
                            web::post().to(api::auth::farcaster_login),
                        )
                        .route("/refresh", web::post().to(api::auth::refresh_token))
                        .route("/logout", web::post().to(api::auth::logout))
                        .route("/api-keys", web::post().to(api::api_key::create_api_key_handler))
                        .route("/api-keys", web::get().to(api::api_key::list_api_keys_handler))
                        .route("/api-keys/{key_id}", web::delete().to(api::api_key::revoke_api_key_handler)),
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
                        .configure(api::creator::configure)
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
                            "/internal/markets/{market_id}/bootstrap",
                            web::post().to(api::evm::register_base_market_bootstrap),
                        )
                        .route(
                            "/internal/markets/{market_id}/bootstrap/runtime",
                            web::patch().to(api::evm::update_base_market_bootstrap_runtime),
                        )
                        .route(
                            "/internal/markets/{market_id}/bootstrap/pause",
                            web::post().to(api::evm::pause_base_market_bootstrap),
                        )
                        .route(
                            "/internal/markets/{market_id}/bootstrap/resume",
                            web::post().to(api::evm::resume_base_market_bootstrap),
                        )
                        .route(
                            "/internal/markets/{market_id}/bootstrap/refresh",
                            web::post().to(api::evm::refresh_base_market_bootstrap),
                        )
                        .route(
                            "/internal/markets/{market_id}/bootstrap/graduate",
                            web::post().to(api::evm::graduate_base_market_bootstrap_now),
                        )
                        .route(
                            "/bootstrap/operator",
                            web::get().to(api::evm::get_bootstrap_operator_status),
                        )
                        .route(
                            "/bootstrap/admin/backfill",
                            web::post().to(api::evm::backfill_base_market_bootstrap),
                        )
                        .route(
                            "/bootstrap/runner/tick",
                            web::post().to(api::evm::bootstrap_runner_tick),
                        )
                        .route(
                            "/bootstrap/runner/report",
                            web::post().to(api::evm::bootstrap_runner_report),
                        )
                        .route(
                            "/ops/runner-state/{runner_name}",
                            web::get().to(api::evm::get_ops_runner_state),
                        )
                        .route(
                            "/ops/runner-state/report",
                            web::post().to(api::evm::report_ops_runner_state),
                        )
                        // ---- Liquidity Mirror routes ----
                        .route(
                            "/mirror/links",
                            web::post().to(api::evm::create_mirror_link),
                        )
                        .route("/mirror/links", web::get().to(api::evm::list_mirror_links))
                        .route(
                            "/mirror/links/{link_id}",
                            web::patch().to(api::evm::update_mirror_link),
                        )
                        .route("/mirror/status", web::get().to(api::evm::get_mirror_status))
                        .route(
                            "/oracle/markets/{market_id}/config",
                            web::get().to(api::evm::get_oracle_market_config),
                        )
                        .route(
                            "/oracle/markets/{market_id}/config",
                            web::post().to(api::evm::register_oracle_market_config),
                        )
                        .route(
                            "/oracle/keeper/tick",
                            web::post().to(api::evm::oracle_keeper_tick),
                        )
                        .route(
                            "/oracle/keeper/report",
                            web::post().to(api::evm::oracle_keeper_report),
                        )
                        .route("/token/state", web::get().to(api::evm::get_relay_token_state))
                        // ---- Scanner endpoints ----
                        .route("/scanner/limitless", web::get().to(api::evm::get_scanned_limitless))
                        .route("/scanner/aerodrome", web::get().to(api::evm::get_scanned_aerodrome))
                        .service(
                            web::scope("/write")
                                .route(
                                    "/markets/create",
                                    web::post().to(api::evm::prepare_create_market_write),
                                )
                                .route(
                                    "/markets/resolve",
                                    web::post().to(api::evm::prepare_resolve_market_write),
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
                                    "/agents/manager-approval",
                                    web::post()
                                        .to(api::evm::prepare_set_manager_approval_write),
                                )
                                .route(
                                    "/agents/bootstrap-create",
                                    web::post()
                                        .to(api::evm::prepare_bootstrap_create_agents_write),
                                )
                                .route(
                                    "/agents/update",
                                    web::post().to(api::evm::prepare_update_agents_write),
                                )
                                .route(
                                    "/agents/deactivate",
                                    web::post().to(api::evm::prepare_deactivate_agents_write),
                                )
                                .route(
                                    "/agents/manager",
                                    web::post().to(api::evm::prepare_set_agent_manager_write),
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
                                    "/oracle/configure",
                                    web::post().to(api::evm::prepare_configure_oracle_write),
                                )
                                .route(
                                    "/oracle/resolve",
                                    web::post().to(api::evm::prepare_oracle_resolve_write),
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
                    web::scope("/kyc")
                        .route("/verify", web::post().to(api::kyc::verify_kyc))
                        .route("/status", web::get().to(api::kyc::get_kyc_status)),
                )
                .service(
                    web::scope("/social")
                        .route(
                            "/follow/{wallet}",
                            web::post().to(api::social::follow_trader),
                        )
                        .route(
                            "/follow/{wallet}",
                            web::delete().to(api::social::unfollow_trader),
                        )
                        .route(
                            "/follow/{wallet}/status",
                            web::get().to(api::social::get_follow_status),
                        )
                        .route("/following", web::get().to(api::social::get_following))
                        .route("/followers", web::get().to(api::social::get_followers))
                        .route(
                            "/markets/{market_id}/comments",
                            web::get().to(api::social::get_market_comments),
                        )
                        .route(
                            "/markets/{market_id}/comments",
                            web::post().to(api::social::post_market_comment),
                        ),
                )
                .service(
                    web::scope("/profiles")
                        .route("/me", web::patch().to(api::social::update_profile))
                        .route(
                            "/{wallet}/followers-count",
                            web::get().to(api::social::get_follower_counts),
                        )
                        .route(
                            "/{wallet}",
                            web::get().to(api::profiles::get_public_profile),
                        )
                        .route(
                            "/{wallet}/activity",
                            web::get().to(api::profiles::get_profile_activity),
                        )
                        .route(
                            "/{wallet}/positions",
                            web::get().to(api::profiles::get_profile_positions),
                        ),
                )
                .service(
                    web::scope("/copy-trading")
                        .route(
                            "/subscribe",
                            web::post().to(api::copy_trading::subscribe),
                        )
                        .route(
                            "/subscribe/{id}",
                            web::delete().to(api::copy_trading::unsubscribe),
                        )
                        .route(
                            "/subscribe/{id}",
                            web::put().to(api::copy_trading::update_subscription),
                        )
                        .route(
                            "/subscribe/{id}/history",
                            web::get().to(api::copy_trading::get_subscription_history),
                        )
                        .route(
                            "/subscriptions",
                            web::get().to(api::copy_trading::list_subscriptions),
                        )
                        .route(
                            "/subscribers",
                            web::get().to(api::copy_trading::get_subscriber_count),
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
                            "/credentials/{credential_id}/rotate",
                            web::post().to(api::external::rotate_external_credential),
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
                            "/orders/prepare-submit",
                            web::post().to(api::external::prepare_external_order_submit),
                        )
                        .route(
                            "/orders/cancel",
                            web::post().to(api::external::cancel_external_order),
                        )
                        .route(
                            "/orders/prepare-cancel",
                            web::post().to(api::external::prepare_external_order_cancel),
                        )
                        .route(
                            "/orders",
                            web::get().to(api::external::list_external_orders),
                        )
                        .route(
                            "/markets/{market_id}",
                            web::get().to(api::external::get_external_market_snapshot),
                        )
                        .route(
                            "/markets/{market_id}/orderbook",
                            web::get().to(api::external::get_external_market_orderbook),
                        )
                        .route(
                            "/markets/{market_id}/trades",
                            web::get().to(api::external::get_external_market_trades),
                        )
                        .route(
                            "/polymarket/public-trades",
                            web::get().to(api::external::get_polymarket_public_trades),
                        )
                        .route(
                            "/polymarket/orderbook-history",
                            web::get().to(api::external::get_polymarket_orderbook_history),
                        )
                        .route(
                            "/indexers/polymarket/health",
                            web::get().to(api::external::get_polymarket_indexer_health),
                        )
                        .route(
                            "/indexers/polymarket/backfill",
                            web::post().to(api::external::trigger_polymarket_indexer_backfill),
                        )
                        .route(
                            "/research/wallets",
                            web::get().to(api::external::list_research_wallets),
                        )
                        .route(
                            "/research/replay",
                            web::post().to(api::external::create_strategy_replay),
                        )
                        .route(
                            "/research/replay/{replay_id}",
                            web::get().to(api::external::get_strategy_replay),
                        )
                        .route(
                            "/agents/{agent_id}/promotion-readiness",
                            web::get().to(api::external::get_agent_promotion_readiness),
                        )
                        .route(
                            "/signals",
                            web::get().to(api::external::list_external_signals),
                        )
                        .route(
                            "/signals",
                            web::post().to(api::external::create_external_signal),
                        )
                        .route(
                            "/edge-scanner/signals",
                            web::post().to(api::external::ingest_edge_scanner_signals),
                        )
                        .route(
                            "/edge-scanner/signals",
                            web::get().to(api::external::list_edge_scanner_signals),
                        )
                        .route(
                            "/edge-scanner/calibration",
                            web::post().to(api::external::ingest_calibration_curve),
                        )
                        .route(
                            "/edge-scanner/calibration",
                            web::get().to(api::external::get_calibration_curve),
                        )
                        .route(
                            "/agents",
                            web::get().to(api::external::list_external_agents),
                        )
                        .route(
                            "/agents/public",
                            web::get().to(api::external::list_public_external_agents),
                        )
                        .route(
                            "/agents",
                            web::post().to(api::external::create_external_agent),
                        )
                        .route(
                            "/agents/public/performance",
                            web::get()
                                .to(api::external::get_public_external_agents_performance),
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
                            "/admin/state/reset",
                            web::post().to(api::external::reset_imported_external_state),
                        )
                        .route(
                            "/admin/state/{table}",
                            web::post().to(api::external::import_external_state_batch),
                        )
                        .route(
                            "/agents/{agent_id}",
                            web::patch().to(api::external::update_external_agent),
                        )
                        .route(
                            "/agents/{agent_id}/execute",
                            web::post().to(api::external::execute_external_agent),
                        )
                        .route(
                            "/partner/status",
                            web::get().to(api::external::get_limitless_partner_status),
                        )
                        .route(
                            "/partner/sub-account",
                            web::post().to(api::external::create_limitless_sub_account),
                        )
                        .route(
                            "/partner/order",
                            web::post().to(api::external::place_limitless_delegated_order),
                        )
                        .route(
                            "/partner/order/{order_id}",
                            web::delete().to(api::external::cancel_limitless_order),
                        ),
                )
                .service(
                    web::scope("/risk")
                        .route(
                            "/portfolio",
                            web::get().to(api::risk_console::get_portfolio),
                        )
                        .route(
                            "/history",
                            web::get().to(api::risk_console::get_portfolio_history),
                        )
                        .route(
                            "/drawdown",
                            web::get().to(api::risk_console::get_drawdown),
                        )
                        .route(
                            "/compliance/export",
                            web::get().to(api::risk_console::export_compliance),
                        ),
                )
                .service(
                    web::scope("/parlays")
                        .route(
                            "",
                            web::post().to(api::parlays::create_parlay),
                        )
                        .route(
                            "",
                            web::get().to(api::parlays::list_parlays),
                        )
                        .route(
                            "/{parlay_id}",
                            web::get().to(api::parlays::get_parlay),
                        )
                        .route(
                            "/{parlay_id}/resolve",
                            web::post().to(api::parlays::resolve_leg),
                        ),
                )
                .service(
                    web::scope("/creator")
                        .route(
                            "/tiers",
                            web::get().to(api::creator_tiers::list_tiers),
                        )
                        .route(
                            "/profile",
                            web::get().to(api::creator_tiers::get_profile),
                        )
                        .route(
                            "/upgrade",
                            web::post().to(api::creator_tiers::upgrade_tier),
                        )
                        .route(
                            "/fees",
                            web::get().to(api::creator_tiers::list_fees),
                        ),
                )
                .service(
                    web::scope("/agents")
                        .route(
                            "/templates",
                            web::get().to(api::agent_service::list_templates),
                        )
                        .route(
                            "/deploy",
                            web::post().to(api::agent_service::deploy_agent),
                        )
                        .route(
                            "/managed",
                            web::get().to(api::agent_service::list_managed_agents),
                        )
                        .route(
                            "/managed/{agent_id}",
                            web::patch().to(api::agent_service::update_managed_agent),
                        )
                        .route(
                            "/managed/{agent_id}/trades",
                            web::get().to(api::agent_service::get_agent_trades),
                        ),
                )
                .service(
                    web::scope("/signals")
                        .route(
                            "/providers",
                            web::post().to(api::signals::create_provider),
                        )
                        .route(
                            "/providers",
                            web::get().to(api::signals::list_providers),
                        )
                        .route(
                            "/emit",
                            web::post().to(api::signals::emit_signal),
                        )
                        .route(
                            "/market/{market_slug}",
                            web::get().to(api::signals::get_market_signals),
                        )
                        .route(
                            "/score",
                            web::post().to(api::signals::score_market),
                        ),
                )
                .service(
                    web::scope("/routing")
                        .route(
                            "/quote",
                            web::post().to(api::routing::route_order),
                        )
                        .route(
                            "/arbitrage",
                            web::get().to(api::routing::list_arbitrage),
                        )
                        .route(
                            "/venues",
                            web::post().to(api::routing::upsert_venue_link),
                        ),
                )
                .service(
                    web::scope("/pm-scanner")
                        .route(
                            "/opportunities",
                            web::get().to(api::pm_scanner::list_opportunities),
                        )
                        .route(
                            "/scan",
                            web::post().to(api::pm_scanner::trigger_scan),
                        )
                        .route(
                            "/calibration",
                            web::get().to(api::pm_scanner::get_calibration),
                        )
                        .route(
                            "/runs",
                            web::get().to(api::pm_scanner::list_scan_runs),
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
                        .route(
                            "/{cell_id}",
                            web::get().to(api::decisions::get_decision_cell),
                        )
                        .route(
                            "/{cell_id}",
                            web::patch().to(api::decisions::update_decision_cell),
                        )
                        .route(
                            "/{cell_id}/actions",
                            web::post().to(api::decisions::add_decision_action),
                        )
                        .route(
                            "/{cell_id}/nodes",
                            web::post().to(api::decisions::add_decision_node),
                        )
                        .route(
                            "/{cell_id}/nodes/{node_id}",
                            web::patch().to(api::decisions::update_decision_node),
                        )
                        .route(
                            "/{cell_id}/nodes/{node_id}/attach-market",
                            web::post().to(api::decisions::attach_market_to_decision_node),
                        )
                        .route(
                            "/{cell_id}/nodes/{node_id}/attach-agent",
                            web::post().to(api::decisions::attach_agent_to_decision_node),
                        )
                        .route(
                            "/{cell_id}/recalculate",
                            web::post().to(api::decisions::recalculate_decision_cell),
                        )
                        .route(
                            "/{cell_id}/automation",
                            web::post().to(api::decisions::update_decision_automation),
                        )
                        .route(
                            "/{cell_id}/alerts",
                            web::post().to(api::decisions::upsert_decision_alert),
                        )
                        .route(
                            "/{cell_id}/events",
                            web::get().to(api::decisions::list_decision_events),
                        ),
                )
                .service(
                    web::scope("/hackathons")
                        .route("", web::get().to(api::hackathon::list_hackathons))
                        .route("", web::post().to(api::hackathon::create_hackathon))
                        .route("/{id}", web::get().to(api::hackathon::get_hackathon))
                        .route("/{id}", web::patch().to(api::hackathon::update_hackathon))
                        .route(
                            "/{id}/register",
                            web::post().to(api::hackathon::register_for_hackathon),
                        )
                        .route(
                            "/{id}/registrations",
                            web::get().to(api::hackathon::list_registrations),
                        )
                        .route(
                            "/{id}/agents",
                            web::post().to(api::hackathon::link_agent_to_hackathon),
                        )
                        .route(
                            "/{id}/leaderboard",
                            web::get().to(api::hackathon::get_leaderboard),
                        )
                        .route(
                            "/{id}/leaderboard/snapshots",
                            web::get().to(api::hackathon::get_leaderboard_snapshots),
                        )
                        .route(
                            "/{id}/snapshot",
                            web::post().to(api::hackathon::trigger_snapshot),
                        ),
                )
                .service(
                    web::scope("/leaderboard")
                        .route("", web::get().to(api::leaderboard::get_leaderboard))
                        .route(
                            "/rank/{wallet}",
                            web::get().to(api::leaderboard::get_user_rank),
                        )
                        .route(
                            "/compute",
                            web::post().to(api::leaderboard::compute_leaderboard),
                        ),
                )
                .service(
                    web::scope("/referrals")
                        .route("/generate", web::post().to(api::referrals::generate_code))
                        .route("/apply", web::post().to(api::referrals::apply_code))
                        .route("/stats", web::get().to(api::referrals::get_stats))
                        .route("/code", web::get().to(api::referrals::get_code)),
                )
                .service(
                    web::scope("/compliance")
                        .route(
                            "/policy",
                            web::get().to(api::compliance::get_compliance_policy),
                        )
                        .route(
                            "/decision",
                            web::post().to(api::compliance::create_compliance_decision),
                        ),
                )
                .service(
                    web::scope("/solana")
                        .route("/programs", web::get().to(api::solana::get_solana_programs))
                        .service(web::scope("/write").route(
                            "/relay",
                            web::post().to(api::solana::relay_raw_transaction),
                        )),
                )
                .service(
                    web::scope("/web4")
                        .route(
                            "/capabilities",
                            web::get().to(api::web4::get_web4_capabilities),
                        )
                        .route(
                            "/runtime/health",
                            web::get().to(api::web4::get_web4_runtime_health),
                        )
                        .route("/mcp", web::get().to(api::web4::get_mcp_manifest))
                        .route("/mcp", web::post().to(api::web4::handle_mcp_jsonrpc))
                        .route("/agent-card", web::get().to(api::web4::get_agent_card))
                        .service(
                            web::scope("/xmtp")
                                .route(
                                    "/health",
                                    web::get().to(api::web4::get_xmtp_swarm_health),
                                )
                                .route(
                                    "/swarm/send",
                                    web::post().to(api::web4::send_xmtp_swarm_message),
                                )
                                .route(
                                    "/swarm/{swarm_id}/messages",
                                    web::get().to(api::web4::list_xmtp_swarm_messages),
                                ),
                        ),
                ),
        );
}
