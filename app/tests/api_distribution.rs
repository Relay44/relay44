mod common;

use actix_web::test;

#[actix_rt::test]
async fn list_dist_markets_is_public() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get()
        .uri("/v1/distribution/markets")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_ne!(resp.status(), 401);
}

#[actix_rt::test]
async fn open_position_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::post()
        .uri("/v1/distribution/markets/test-market/trade")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}

#[actix_rt::test]
async fn claim_payout_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::post()
        .uri("/v1/distribution/positions/1/claim")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn create_dist_market_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::post()
        .uri("/v1/distribution/markets")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}
