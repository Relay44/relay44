//! Portfolio snapshot background service.
//!
//! Periodically computes portfolio-level risk metrics for all active users
//! and inserts them into `portfolio_snapshots`.

use log::{info, warn};
use std::sync::Arc;
use std::time::Duration;

use crate::AppState;

const DEFAULT_INTERVAL_SECS: u64 = 300; // 5 minutes

pub fn spawn_portfolio_snapshotter(state: Arc<AppState>) {
    if !state.config.risk_console_enabled {
        info!("Portfolio snapshotter disabled (RISK_CONSOLE_ENABLED=false)");
        return;
    }

    let interval_secs = std::env::var("PORTFOLIO_SNAPSHOT_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_INTERVAL_SECS)
        .max(60);

    info!("Starting portfolio snapshotter (interval={}s)", interval_secs);

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if state.is_shutting_down.load(std::sync::atomic::Ordering::Relaxed) {
                info!("Portfolio snapshotter shutting down");
                break;
            }

            match snapshot_all_portfolios(&state).await {
                Ok(count) => {
                    if count > 0 {
                        info!("Portfolio snapshot: computed {} user snapshots", count);
                    }
                }
                Err(e) => {
                    warn!("Portfolio snapshot error: {}", e);
                }
            }
        }
    });
}

async fn snapshot_all_portfolios(state: &AppState) -> Result<usize, String> {
    // Get all owners with open positions.
    let owners: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT owner FROM positions \
         WHERE yes_balance > 0 OR no_balance > 0 OR locked_collateral > 0",
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| format!("Query active owners: {}", e))?;

    let mut count = 0;

    for (owner,) in &owners {
        if let Err(e) = snapshot_user_portfolio(state, owner).await {
            warn!("Snapshot for {}: {}", owner, e);
            continue;
        }
        count += 1;
    }

    Ok(count)
}

async fn snapshot_user_portfolio(state: &AppState, owner: &str) -> Result<(), String> {
    let stats: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT \
         COUNT(*) FILTER (WHERE yes_balance > 0 OR no_balance > 0) as open_positions, \
         COALESCE(SUM(locked_collateral), 0) as total_locked, \
         COALESCE(SUM(total_deposited), 0) as total_deposited, \
         COALESCE(SUM(total_withdrawn), 0) as total_withdrawn, \
         COALESCE(SUM(realized_pnl), 0) as realized_pnl \
         FROM positions WHERE owner = $1",
    )
    .bind(owner)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| format!("Query positions: {}", e))?;

    let (open_positions, total_locked, total_deposited, total_withdrawn, realized_pnl) =
        stats.unwrap_or((0, 0, 0, 0, 0));

    // Values are in integer cents (USDC 6 decimals stored as bigint).
    let scale = 1_000_000.0_f64;
    let gross_exposure = total_locked as f64 / scale;
    let total_value = (total_deposited - total_withdrawn + total_locked) as f64 / scale;
    let realized_pnl_usdc = realized_pnl as f64 / scale;

    // Max single position as % of total.
    let max_single_pct = if total_value > 0.0 {
        let max_pos: Option<(i64,)> = sqlx::query_as(
            "SELECT MAX(locked_collateral) FROM positions WHERE owner = $1",
        )
        .bind(owner)
        .fetch_optional(state.db.pool())
        .await
        .map_err(|e| format!("Query max position: {}", e))?;

        max_pos
            .and_then(|r| if r.0 > 0 { Some(r.0 as f64 / scale / total_value * 100.0) } else { None })
            .unwrap_or(0.0)
    } else {
        0.0
    };

    // Get peak from previous snapshots.
    let prev_peak: Option<(f64,)> = sqlx::query_as(
        "SELECT MAX(peak_value_usdc) FROM portfolio_snapshots WHERE owner = $1",
    )
    .bind(owner)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| format!("Query peak: {}", e))?;

    let peak = prev_peak
        .and_then(|r| if r.0 > 0.0 { Some(r.0) } else { None })
        .unwrap_or(total_value)
        .max(total_value);

    let drawdown_pct = if peak > 0.0 {
        (peak - total_value) / peak * 100.0
    } else {
        0.0
    };

    sqlx::query(
        "INSERT INTO portfolio_snapshots \
         (owner, total_value_usdc, unrealized_pnl_usdc, realized_pnl_usdc, \
          open_positions, gross_exposure_usdc, max_single_position_pct, \
          drawdown_from_peak_pct, peak_value_usdc) \
         VALUES ($1, $2, 0, $3, $4, $5, $6, $7, $8)",
    )
    .bind(owner)
    .bind(total_value)
    .bind(realized_pnl_usdc)
    .bind(open_positions as i32)
    .bind(gross_exposure)
    .bind(max_single_pct)
    .bind(drawdown_pct)
    .bind(peak)
    .execute(state.db.pool())
    .await
    .map_err(|e| format!("Insert snapshot: {}", e))?;

    Ok(())
}
