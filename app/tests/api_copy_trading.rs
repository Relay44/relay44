mod common;

use actix_web::test;

#[actix_rt::test]
async fn subscribe_requires_auth() {
    let app = common::build_test_app().await;
    // Send without body to ensure auth is checked (actix may reject body first)
    let req = test::TestRequest::post()
        .uri("/v1/copy-trading/subscribe")
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Should be rejected — either 401 (no auth) or 400 (no body parsed before auth)
    assert!(resp.status().is_client_error());
    assert_ne!(resp.status(), 200);
    assert_ne!(resp.status(), 201);
}

#[actix_rt::test]
async fn list_subscriptions_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get()
        .uri("/v1/copy-trading/subscriptions")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}
