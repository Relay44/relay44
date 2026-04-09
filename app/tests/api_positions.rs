mod common;

use actix_web::test;

#[actix_rt::test]
async fn list_positions_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get().uri("/v1/positions").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn claim_winnings_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::post()
        .uri("/v1/positions/42/claim")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}
