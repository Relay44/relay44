mod common;

use actix_web::test;
use relay44_backend::api::jwt::UserRole;
use serde_json::json;

#[actix_rt::test]
async fn list_orders_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get().uri("/v1/orders").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn place_order_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::post()
        .uri("/v1/orders")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}

#[actix_rt::test]
async fn cancel_order_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::delete()
        .uri("/v1/orders/550e8400-e29b-41d4-a716-446655440000")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn place_order_invalid_price_rejected() {
    let app = common::build_test_app().await;
    let token = common::generate_token(common::TEST_WALLET, UserRole::User);
    let req = test::TestRequest::post()
        .uri("/v1/orders")
        .insert_header(common::auth_header(&token))
        .set_json(json!({
            "market_id": "test-market-123",
            "side": "buy",
            "outcome": "yes",
            "price": 1.5,
            "quantity": 10,
            "order_type": "limit"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn place_order_zero_quantity_rejected() {
    let app = common::build_test_app().await;
    let token = common::generate_token(common::TEST_WALLET, UserRole::User);
    let req = test::TestRequest::post()
        .uri("/v1/orders")
        .insert_header(common::auth_header(&token))
        .set_json(json!({
            "market_id": "test-market-123",
            "side": "buy",
            "outcome": "yes",
            "price": 0.5,
            "quantity": 0,
            "order_type": "limit"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_rt::test]
async fn batch_over_limit_rejected() {
    let app = common::build_test_app().await;
    let token = common::generate_token(common::TEST_WALLET, UserRole::User);
    let orders: Vec<_> = (0..21)
        .map(|i| {
            json!({
                "market_id": format!("market-{}", i),
                "side": "buy",
                "outcome": "yes",
                "price": 0.5,
                "quantity": 10,
                "order_type": "limit"
            })
        })
        .collect();
    let req = test::TestRequest::post()
        .uri("/v1/orders/batch")
        .insert_header(common::auth_header(&token))
        .set_json(json!({ "orders": orders }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}
