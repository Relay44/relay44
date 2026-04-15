use actix_web::{test, web, App};
use relay44_backend::api::jwt::UserRole;
use relay44_backend::api::JwtService;
use relay44_backend::config::AppConfig;
use relay44_backend::services::{
    DatabaseService, EventBus, EvmIndexerService, EvmRpcService, MetricsService, OrderBookService,
    RedisService, WebSocketHub,
};
use relay44_backend::{configure_routes, AppState};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub const TEST_JWT_SECRET: &str = "dev-secret-key-do-not-use-in-production";
pub const TEST_WALLET: &str = "0x71c7656ec7ab88b098defb751b7401b5f6d8976f";

pub fn test_jwt() -> JwtService {
    JwtService::new(TEST_JWT_SECRET)
}

pub fn generate_token(wallet: &str, role: UserRole) -> String {
    test_jwt().generate_access_token(wallet, role).unwrap()
}

pub fn auth_header(token: &str) -> (&'static str, String) {
    ("Authorization", format!("Bearer {}", token))
}

pub async fn build_test_state() -> Arc<AppState> {
    let config = AppConfig::from_env();
    let db = DatabaseService::test_stub();
    let redis = RedisService::new("redis://127.0.0.1:1")
        .await
        .expect("RedisService::new should not fail on construction");
    let evm_rpc = EvmRpcService::new("http://127.0.0.1:1", &[]);
    let evm_indexer = EvmIndexerService::new(EvmRpcService::new("http://127.0.0.1:1", &[]), 100);
    let orderbook = OrderBookService::new();
    let jwt = test_jwt();
    let metrics = MetricsService::new();
    let ws_hub = WebSocketHub::new();
    let event_bus = EventBus::new();
    let market_data = Arc::new(relay44_backend::services::market_data::MarketDataBus::new());
    let kyc = relay44_backend::services::kyc::KycService::new(
        relay44_backend::services::kyc::KycConfig::from_env(),
    );

    Arc::new(AppState {
        config,
        db,
        evm_rpc,
        evm_indexer,
        orderbook,
        redis,
        jwt,
        metrics,
        ws_hub,
        event_bus,
        market_data,
        kyc,
        limitless_partner: None,
        is_shutting_down: Arc::new(AtomicBool::new(false)),
    })
}

pub async fn build_test_app() -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    let state = build_test_state().await;
    test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::JsonConfig::default().limit(4 * 1024 * 1024))
            .configure(configure_routes),
    )
    .await
}
