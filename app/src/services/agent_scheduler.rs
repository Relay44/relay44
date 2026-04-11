//! Internal agent scheduler.
//!
//! Replaces the external cron-only dependency with an in-process
//! `tokio::spawn` loop that wakes every `tick_interval` and calls
//! the runner-tick endpoint internally.

use log::{info, warn};
use std::sync::Arc;
use std::time::Duration;

use crate::AppState;

/// Default tick interval (30 seconds).
const DEFAULT_TICK_INTERVAL_SECS: u64 = 30;

/// Spawn the internal agent scheduler loop.
///
/// The scheduler queries `external_agents` for due agents and executes them
/// using the same logic as `POST /external/agents/runner/tick`. The HTTP
/// endpoint remains available as an override / manual trigger.
pub fn spawn_agent_scheduler(state: Arc<AppState>) {
    let enabled = state.config.external_agents_enabled && state.config.external_trading_enabled;
    if !enabled {
        info!("Agent scheduler disabled (external_agents_enabled=false or external_trading_enabled=false)");
        return;
    }

    let interval_secs = std::env::var("AGENT_SCHEDULER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TICK_INTERVAL_SECS)
        .max(5);

    let scan_limit = state.config.paper_runner_scan_limit.max(1) as i64;

    info!(
        "Starting internal agent scheduler (interval={}s, scan_limit={})",
        interval_secs, scan_limit
    );

    tokio::spawn(async move {
        // Initial delay to let the server finish starting.
        tokio::time::sleep(Duration::from_secs(5)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                info!("Agent scheduler shutting down");
                break;
            }

            match run_scheduler_tick(&state, scan_limit).await {
                Ok((scanned, executed)) => {
                    if executed > 0 {
                        info!(
                            "Agent scheduler tick: scanned={} executed={}",
                            scanned, executed
                        );
                    }
                }
                Err(e) => {
                    warn!("Agent scheduler tick error: {}", e);
                }
            }
        }
    });
}

/// Internal tick — loads due agents and executes them.
/// Returns (agents_scanned, agents_executed).
async fn run_scheduler_tick(state: &AppState, limit: i64) -> Result<(u64, u64), String> {
    let now = chrono::Utc::now();

    let rows = sqlx::query(
        "SELECT id, owner, name, provider, market_id, outcome, side, price, quantity,
                cadence_seconds, strategy, execution_mode, credential_id, active,
                last_executed_at, next_execution_at, consecutive_failures, last_error_code
         FROM external_agents
         WHERE active = TRUE AND next_execution_at <= $1
         ORDER BY next_execution_at ASC, id ASC
         LIMIT $2",
    )
    .bind(now)
    .bind(limit)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| e.to_string())?;

    let scanned = rows.len() as u64;
    let mut executed = 0_u64;

    for row in &rows {
        let agent_id: String = sqlx::Row::try_get(row, "id").unwrap_or_default();
        let consecutive_failures: i32 =
            sqlx::Row::try_get(row, "consecutive_failures").unwrap_or(0);

        // Auto-deactivate agents with too many failures.
        if consecutive_failures >= 20 {
            let _ = sqlx::query(
                "UPDATE external_agents SET active = FALSE, updated_at = NOW() WHERE id = $1",
            )
            .bind(agent_id.as_str())
            .execute(state.db.pool())
            .await;

            let owner: String = sqlx::Row::try_get(row, "owner").unwrap_or_default();
            state
                .event_bus
                .emit(crate::services::event_bus::PlatformEvent::AgentDeactivated(
                    crate::services::event_bus::AgentLifecycleEvent {
                        agent_id: agent_id.clone(),
                        owner,
                        reason: format!(
                            "scheduler: auto_deactivated after {} failures",
                            consecutive_failures
                        ),
                        timestamp: now,
                    },
                ));
            continue;
        }

        // Parse agent record and execute. We use the same execute path as the
        // HTTP tick, but call it via the internal function. Since execute_agent_record
        // is pub(crate), we call it directly.
        match crate::api::external::execute_agent_record_by_id(state, agent_id.as_str()).await {
            Ok(true) => executed += 1,
            Ok(false) => {} // skipped
            Err(e) => {
                warn!("Scheduler: agent {} execution error: {}", agent_id, e);
            }
        }
    }

    Ok((scanned, executed))
}
