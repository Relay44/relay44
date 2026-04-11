mod common;

use actix_web::test;

#[actix_rt::test]
async fn list_markets_is_public() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get().uri("/v1/markets").to_request();
    let resp = test::call_service(&app, req).await;
    // Public endpoint — should not return 401
    assert_ne!(resp.status(), 401);
}

#[actix_rt::test]
async fn get_market_is_public() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get()
        .uri("/v1/markets/test-market-123")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_ne!(resp.status(), 401);
}

#[actix_rt::test]
async fn create_market_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::post().uri("/v1/markets").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}
