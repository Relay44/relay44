mod common;

use actix_web::test;

#[actix_rt::test]
async fn generate_code_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::post()
        .uri("/v1/referrals/generate")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_rt::test]
async fn apply_code_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::post()
        .uri("/v1/referrals/apply")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}

#[actix_rt::test]
async fn get_stats_requires_auth() {
    let app = common::build_test_app().await;
    let req = test::TestRequest::get()
        .uri("/v1/referrals/stats")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}
