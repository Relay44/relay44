//! Integration tests for the managed-agent lifecycle + intent log.
//!
//! These tests require a migrated Postgres instance. They are gated behind
//! the `TEST_DATABASE_URL` env var so `cargo test` in a plain dev setup
//! skips them rather than erroring.
//!
//! Run with:
//!   TEST_DATABASE_URL=postgres://user@localhost:5432/relay44_test \
//!     cargo test --test managed_agent_lifecycle -- --test-threads=1

use relay44_backend::services::managed_agent_lifecycle::{
    self as lifecycle, IntentKind, LifecycleState,
};
use relay44_backend::services::RedisService;
use sqlx::PgPool;

async fn setup() -> Option<(PgPool, RedisService)> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let pool = PgPool::connect(&url).await.ok()?;

    sqlx::query("DELETE FROM managed_agent_intents")
        .execute(&pool)
        .await
        .ok()?;
    sqlx::query("DELETE FROM managed_agent_positions")
        .execute(&pool)
        .await
        .ok()?;
    sqlx::query("DELETE FROM managed_agents WHERE id LIKE 'test-lc-%'")
        .execute(&pool)
        .await
        .ok()?;

    let redis = RedisService::new("redis://127.0.0.1:6379")
        .await
        .expect("redis");

    Some((pool, redis))
}

async fn make_agent(pool: &PgPool, id: &str) {
    sqlx::query(
        "INSERT INTO managed_agents \
         (id, owner, template_id, name, params, seed_usdc, status, lifecycle_state) \
         VALUES ($1, 'test-owner', (SELECT id FROM agent_templates LIMIT 1), \
                 $1, '{}'::jsonb, 100.0, 'active', 'active') \
         ON CONFLICT (id) DO UPDATE SET lifecycle_state = 'active'",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn intent_log_confirm_roundtrip() {
    let Some((pool, redis)) = setup().await else {
        return;
    };
    make_agent(&pool, "test-lc-1").await;

    let payload = serde_json::json!({
        "provider": "polymarket",
        "market_slug": "test-slug",
        "outcome": "yes",
        "side": "buy",
    });

    let handle = lifecycle::log_intent(
        &pool,
        &redis,
        "test-lc-1",
        IntentKind::OpenPosition,
        &payload,
    )
    .await
    .expect("log_intent");

    assert_eq!(handle.seq, 1);

    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM managed_agent_intents WHERE agent_id = 'test-lc-1' AND state = 'pending'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending, 1);

    lifecycle::confirm_intent(&pool, &redis, "test-lc-1", handle)
        .await
        .expect("confirm");

    let confirmed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM managed_agent_intents WHERE agent_id = 'test-lc-1' AND state = 'confirmed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(confirmed, 1);
}

#[tokio::test]
async fn recover_abandons_pending_open_with_no_position() {
    let Some((pool, redis)) = setup().await else {
        return;
    };
    make_agent(&pool, "test-lc-2").await;

    let payload = serde_json::json!({
        "provider": "polymarket",
        "market_slug": "ghost-market",
        "outcome": "yes",
        "side": "buy",
    });
    let _ = lifecycle::log_intent(
        &pool,
        &redis,
        "test-lc-2",
        IntentKind::OpenPosition,
        &payload,
    )
    .await
    .unwrap();

    let stats = lifecycle::recover_unresolved_intents(&pool, &redis)
        .await
        .unwrap();
    assert_eq!(stats.abandoned, 1);
    assert_eq!(stats.confirmed, 0);
}

#[tokio::test]
async fn recover_confirms_pending_open_with_matching_position() {
    let Some((pool, redis)) = setup().await else {
        return;
    };
    make_agent(&pool, "test-lc-3").await;

    let payload = serde_json::json!({
        "provider": "polymarket",
        "market_slug": "real-market",
        "outcome": "yes",
        "side": "buy",
    });
    let _ = lifecycle::log_intent(
        &pool,
        &redis,
        "test-lc-3",
        IntentKind::OpenPosition,
        &payload,
    )
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO managed_agent_positions \
         (agent_id, market_slug, provider, outcome, side, entry_price, quantity, \
          notional_usdc, mark_price, hold_until) \
         VALUES ('test-lc-3', 'real-market', 'polymarket', 'yes', 'buy', \
                 0.5, 10.0, 5.0, 0.5, NOW() + INTERVAL '10 minutes')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let stats = lifecycle::recover_unresolved_intents(&pool, &redis)
        .await
        .unwrap();
    assert_eq!(stats.confirmed, 1);
    assert_eq!(stats.abandoned, 0);
}

#[tokio::test]
async fn invalid_transition_is_rejected() {
    let Some((pool, redis)) = setup().await else {
        return;
    };
    make_agent(&pool, "test-lc-4").await;

    lifecycle::set_lifecycle_state(&pool, &redis, "test-lc-4", LifecycleState::Settled, None)
        .await
        .expect("settle ok");

    let res = lifecycle::set_lifecycle_state(
        &pool,
        &redis,
        "test-lc-4",
        LifecycleState::Active,
        Some("should fail"),
    )
    .await;

    assert!(res.is_err(), "settled → active must be rejected");
}

#[tokio::test]
async fn seq_is_monotonic_across_intents() {
    let Some((pool, redis)) = setup().await else {
        return;
    };
    make_agent(&pool, "test-lc-5").await;

    let p = serde_json::json!({"market_slug": "x", "outcome": "yes", "side": "buy"});
    let h1 = lifecycle::log_intent(&pool, &redis, "test-lc-5", IntentKind::OpenPosition, &p)
        .await
        .unwrap();
    let h2 = lifecycle::log_intent(&pool, &redis, "test-lc-5", IntentKind::OpenPosition, &p)
        .await
        .unwrap();
    let h3 = lifecycle::log_intent(&pool, &redis, "test-lc-5", IntentKind::OpenPosition, &p)
        .await
        .unwrap();

    assert_eq!(h1.seq, 1);
    assert_eq!(h2.seq, 2);
    assert_eq!(h3.seq, 3);
}
