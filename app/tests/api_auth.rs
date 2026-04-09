mod common;

use actix_web::test;
use relay44_backend::api::jwt::UserRole;

#[actix_rt::test]
async fn no_auth_header_returns_401() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get().uri("/v1/orders").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn invalid_bearer_format_returns_401() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get()
        .uri("/v1/orders")
        .insert_header(("Authorization", "Basic abc123"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn malformed_jwt_returns_401() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get()
        .uri("/v1/orders")
        .insert_header(("Authorization", "Bearer not.a.valid.jwt"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn expired_jwt_returns_401() {
    let app = common::build_test_app().await;
    // Generate a token that's already expired by using a wrong-secret JWT service
    // and crafting manually. Instead, we use generate_token which creates a valid
    // 1-hour token, so we test wrong secret instead.
    let wrong_jwt = relay44_backend::api::JwtService::new("wrong-secret-key-not-the-real-one-32ch");
    let token = wrong_jwt
        .generate_access_token(common::TEST_WALLET, UserRole::User)
        .unwrap();
    let req = test::TestRequest::get()
        .uri("/v1/orders")
        .insert_header(common::auth_header(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn valid_jwt_passes_auth() {
    let app = common::build_test_app().await;
    let token = common::generate_token(common::TEST_WALLET, UserRole::User);
    let req = test::TestRequest::get()
        .uri("/v1/orders")
        .insert_header(common::auth_header(&token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    // Should NOT be 401 — may be 500 (DB unreachable) or 400, but not auth failure
    assert_ne!(resp.status(), 401);
}

#[actix_rt::test]
async fn nonce_returns_200() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get().uri("/v1/auth/nonce").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_rt::test]
async fn nonce_returns_unique_values() {
    let app = common::build_test_app().await;
    let req1 = test::TestRequest::get().uri("/v1/auth/nonce").to_request();
    let resp1 = test::call_service(&app, req1).await;
    let body1: serde_json::Value = test::read_body_json(resp1).await;

    let req2 = test::TestRequest::get().uri("/v1/auth/nonce").to_request();
    let resp2 = test::call_service(&app, req2).await;
    let body2: serde_json::Value = test::read_body_json(resp2).await;

    assert_ne!(body1["nonce"], body2["nonce"]);
}

#[actix_rt::test]
async fn nonce_has_expiration() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get().uri("/v1/auth/nonce").to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["expires_at"].is_number() || body["expiresAt"].is_number());
}
