//! Distribution market scheduler.
//!
//! Periodic tasks for distribution markets:
//! 1. Oracle auto-resolution — checks Pyth feed for markets past trading_end
//! 2. 24h volume reset — resets volume_24h counters daily

use chrono::Utc;
use log::{info, warn};
use std::sync::Arc;
use std::time::Duration;

use crate::api::notifications::{create_notification, NewNotification, NotificationType};
use crate::services::distribution::DistributionMarketState;
use crate::services::websocket::DistResolveUpdate;
use crate::AppState;

/// Default tick interval (60 seconds).
const DEFAULT_TICK_INTERVAL_SECS: u64 = 60;

/// Spawn the distribution scheduler loop.
pub fn spawn_distribution_scheduler(state: Arc<AppState>) {
    let interval_secs = std::env::var("DIST_SCHEDULER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TICK_INTERVAL_SECS)
        .max(10);

    info!(
        "Starting distribution scheduler (interval={}s)",
        interval_secs
    );

    tokio::spawn(async move {
        // Initial delay to let the server finish starting.
        tokio::time::sleep(Duration::from_secs(10)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut last_volume_reset = Utc::now().date_naive();

        loop {
            interval.tick().await;

            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                info!("Distribution scheduler shutting down");
                break;
            }

            // --- Oracle auto-resolution ---
            if let Err(e) = check_oracle_resolutions(&state).await {
                warn!("Distribution scheduler oracle check error: {}", e);
            }

            // --- Curve snapshots (every tick for active markets) ---
            if let Err(e) = capture_curve_snapshots(&state).await {
                warn!("Distribution scheduler snapshot error: {}", e);
            }

            // --- 24h volume reset (once per calendar day) ---
            let today = Utc::now().date_naive();
            if today != last_volume_reset {
                if let Err(e) = reset_daily_volume(&state).await {
                    warn!("Distribution scheduler volume reset error: {}", e);
                } else {
                    last_volume_reset = today;
                    info!("Distribution scheduler: reset 24h volumes");
                }
            }
        }
    });
}

/// Check for distribution markets that are past trading_end, use_oracle=true,
/// and not yet resolved. Fetch oracle price and resolve if available.
async fn check_oracle_resolutions(state: &AppState) -> Result<(), String> {
    let now = Utc::now();

    // Find oracle-enabled markets past trading_end that aren't resolved
    let rows: Vec<(String, String, f64, f64, f64, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT id, oracle_feed_id, outcome_min, outcome_max, liquidity_param, market_mu, market_sigma \
         FROM distribution_markets \
         WHERE use_oracle = TRUE \
           AND oracle_feed_id IS NOT NULL \
           AND status = 0 \
           AND trading_end IS NOT NULL \
           AND trading_end <= $1",
    )
    .bind(now)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    for (market_id, feed_id, outcome_min, outcome_max, liquidity_param, market_mu, market_sigma) in
        rows
    {
        // Fetch price from Pyth
        let price = match crate::services::pyth::fetch_price(&state.redis, &feed_id).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                warn!(
                    "Distribution oracle: no price for market {} (feed={})",
                    market_id, feed_id
                );
                continue;
            }
            Err(e) => {
                warn!(
                    "Distribution oracle: error fetching price for market {} (feed={}): {}",
                    market_id, feed_id, e
                );
                continue;
            }
        };

        // Clamp to outcome range
        let resolved_value = price.clamp(outcome_min, outcome_max);

        info!(
            "Distribution oracle: auto-resolving market {} with value {} (raw price: {})",
            market_id, resolved_value, price
        );

        // Resolve the market
        let current_mu = market_mu.unwrap_or((outcome_min + outcome_max) / 2.0);
        let current_sigma = market_sigma.unwrap_or((outcome_max - outcome_min) / 4.0);

        let ms = DistributionMarketState {
            mu: current_mu,
            sigma: current_sigma,
            liquidity_b: liquidity_param,
            outcome_min,
            outcome_max,
        };

        let mut tx = state.db.pool().begin().await.map_err(|e| e.to_string())?;

        // Atomically resolve
        let result = sqlx::query(
            "UPDATE distribution_markets \
             SET status = 3, resolved_value = $1, resolved_at = $2 \
             WHERE id = $3 AND status = 0",
        )
        .bind(resolved_value)
        .bind(now)
        .bind(&market_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        if result.rows_affected() == 0 {
            continue; // Already resolved by another process
        }

        // Calculate payouts for open positions
        let positions: Vec<(i32, f64, f64, i64, i16)> = sqlx::query_as(
            "SELECT id, mu, sigma, collateral, fee_bps \
             FROM (SELECT dp.id, dp.mu, dp.sigma, dp.collateral, dm.fee_bps \
                   FROM distribution_positions dp \
                   JOIN distribution_markets dm ON dm.id = dp.market_id \
                   WHERE dp.market_id = $1 AND dp.status = 0) sub",
        )
        .bind(&market_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        // Get fee_bps from market
        let fee_bps: (i16,) =
            sqlx::query_as("SELECT fee_bps FROM distribution_markets WHERE id = $1")
                .bind(&market_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;

        let total_pool_row: (i64,) =
            sqlx::query_as("SELECT total_collateral FROM distribution_markets WHERE id = $1")
                .bind(&market_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        let total_pool = total_pool_row.0 as f64;
        let mut total_gross_paid = 0.0;

        for (pos_id, pos_mu, pos_sigma, pos_collateral, _) in &positions {
            let payout_result = ms.calculate_payout(
                *pos_mu,
                *pos_sigma,
                *pos_collateral as f64,
                resolved_value,
                fee_bps.0 as u32,
                0,
            );

            let mut gross = payout_result.gross_payout;
            if total_gross_paid + gross > total_pool {
                gross = (total_pool - total_gross_paid).max(0.0);
            }
            total_gross_paid += gross;

            let fee = gross * (fee_bps.0 as f64) / 10_000.0;
            let net = (gross - fee).max(0.0);
            let payout = net.ceil() as i64;
            let pnl = payout as f64 - *pos_collateral as f64;

            sqlx::query(
                "UPDATE distribution_positions \
                 SET status = 2, payout = $1, pnl = $2, closed_at = $3 \
                 WHERE id = $4",
            )
            .bind(payout)
            .bind(pnl)
            .bind(now)
            .bind(pos_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }

        tx.commit().await.map_err(|e| e.to_string())?;

        // Broadcast resolution
        state
            .ws_hub
            .broadcast_dist_resolve(DistResolveUpdate {
                market_id: market_id.clone(),
                resolved_value,
                timestamp: now.timestamp(),
            })
            .await;

        // Notify all position holders that the market resolved
        let owners: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT owner FROM distribution_positions WHERE market_id = $1 AND status = 2",
        )
        .bind(&market_id)
        .fetch_all(state.db.pool())
        .await
        .unwrap_or_default();

        for (owner,) in owners {
            let _ = create_notification(
                state,
                NewNotification {
                    owner,
                    kind: NotificationType::DistributionMarketResolved,
                    title: "Distribution market resolved".to_string(),
                    message: format!("Resolved at {:.4}. Claim your payout.", resolved_value),
                    market_id: Some(market_id.clone()),
                    order_id: None,
                    decision_cell_id: None,
                    metadata: serde_json::json!({ "resolvedValue": resolved_value }),
                },
            )
            .await;
        }
    }

    Ok(())
}

/// Capture periodic curve snapshots for active distribution markets.
async fn capture_curve_snapshots(state: &AppState) -> Result<(), String> {
    let rows: Vec<(String, f64, f64, i64, i64)> = sqlx::query_as(
        "SELECT dm.id, COALESCE(dm.market_mu, (dm.outcome_min + dm.outcome_max) / 2), \
                COALESCE(dm.market_sigma, (dm.outcome_max - dm.outcome_min) / 4), \
                dm.total_collateral, \
                (SELECT COUNT(*) FROM distribution_positions dp WHERE dp.market_id = dm.id AND dp.status = 0) \
         FROM distribution_markets dm WHERE dm.status = 0",
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    for (market_id, mu, sigma, total_collateral, position_count) in rows {
        sqlx::query(
            "INSERT INTO distribution_curve_snapshots \
             (market_id, market_mu, market_sigma, total_collateral, position_count) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&market_id)
        .bind(mu)
        .bind(sigma)
        .bind(total_collateral)
        .bind(position_count as i32)
        .execute(state.db.pool())
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Reset volume_24h for all distribution markets.
async fn reset_daily_volume(state: &AppState) -> Result<(), String> {
    sqlx::query("UPDATE distribution_markets SET volume_24h = 0")
        .execute(state.db.pool())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
