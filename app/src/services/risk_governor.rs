use chrono::Utc;
use log::{info, warn};
use sqlx::PgPool;

const MAX_ENTRY_NOTIONAL: f64 = 25.0;
const MAX_BANKROLL_FRACTION_PER_ENTRY: f64 = 0.01;
const MAX_GROSS_EXPOSURE_FRACTION: f64 = 0.10;
const DAILY_DRAWDOWN_KILL_PCT: f64 = 0.03;
const WEEKLY_DRAWDOWN_KILL_PCT: f64 = 0.08;

#[derive(Debug, Clone)]
pub struct RiskState {
    pub bankroll_usdc: f64,
    pub daily_pnl_usdc: f64,
    pub daily_reset_at: chrono::DateTime<Utc>,
    pub weekly_high_usdc: f64,
    pub gross_open_usdc: f64,
    pub kill_switch_active: bool,
    pub kill_switch_reason: Option<String>,
}

#[derive(Debug)]
pub struct RiskCheckResult {
    pub allowed: bool,
    pub reason: Option<String>,
}

pub async fn get_or_init(pool: &PgPool, owner: &str) -> anyhow::Result<RiskState> {
    let row = sqlx::query_as::<
        _,
        (
            f64,
            f64,
            chrono::DateTime<Utc>,
            f64,
            f64,
            bool,
            Option<String>,
        ),
    >(
        "INSERT INTO risk_governor_state (owner) VALUES ($1)
         ON CONFLICT (owner) DO NOTHING;
         SELECT bankroll_usdc, daily_pnl_usdc, daily_reset_at, weekly_high_usdc,
                gross_open_usdc, kill_switch_active, kill_switch_reason
         FROM risk_governor_state WHERE owner = $1",
    )
    .bind(owner)
    .fetch_one(pool)
    .await?;

    Ok(RiskState {
        bankroll_usdc: row.0,
        daily_pnl_usdc: row.1,
        daily_reset_at: row.2,
        weekly_high_usdc: row.3,
        gross_open_usdc: row.4,
        kill_switch_active: row.5,
        kill_switch_reason: row.6,
    })
}

/// Check if a new order is allowed under portfolio risk limits.
pub async fn check_order(
    pool: &PgPool,
    owner: &str,
    order_notional: f64,
) -> anyhow::Result<RiskCheckResult> {
    let mut state = get_or_init(pool, owner).await?;

    // Reset daily PnL if the day rolled over.
    let now = Utc::now();
    if now.date_naive() != state.daily_reset_at.date_naive() {
        sqlx::query(
            "UPDATE risk_governor_state
             SET daily_pnl_usdc = 0, daily_reset_at = now(), updated_at = now()
             WHERE owner = $1",
        )
        .bind(owner)
        .execute(pool)
        .await?;
        state.daily_pnl_usdc = 0.0;
    }

    if state.kill_switch_active {
        return Ok(RiskCheckResult {
            allowed: false,
            reason: Some(format!(
                "kill switch active: {}",
                state.kill_switch_reason.as_deref().unwrap_or("unknown")
            )),
        });
    }

    // Hard cap per entry.
    if order_notional > MAX_ENTRY_NOTIONAL {
        return Ok(RiskCheckResult {
            allowed: false,
            reason: Some(format!(
                "order notional ${:.2} exceeds hard cap ${:.2}",
                order_notional, MAX_ENTRY_NOTIONAL
            )),
        });
    }

    // Bankroll fraction per entry.
    let max_from_bankroll = state.bankroll_usdc * MAX_BANKROLL_FRACTION_PER_ENTRY;
    if order_notional > max_from_bankroll {
        return Ok(RiskCheckResult {
            allowed: false,
            reason: Some(format!(
                "order notional ${:.2} exceeds {}% of bankroll (${:.2})",
                order_notional,
                MAX_BANKROLL_FRACTION_PER_ENTRY * 100.0,
                max_from_bankroll
            )),
        });
    }

    // Gross exposure cap.
    let max_exposure = state.bankroll_usdc * MAX_GROSS_EXPOSURE_FRACTION;
    if state.gross_open_usdc + order_notional > max_exposure {
        return Ok(RiskCheckResult {
            allowed: false,
            reason: Some(format!(
                "gross exposure ${:.2} + ${:.2} exceeds {}% cap (${:.2})",
                state.gross_open_usdc,
                order_notional,
                MAX_GROSS_EXPOSURE_FRACTION * 100.0,
                max_exposure
            )),
        });
    }

    Ok(RiskCheckResult {
        allowed: true,
        reason: None,
    })
}

/// Record a fill and update PnL / exposure. Call after every confirmed fill.
pub async fn record_fill(
    pool: &PgPool,
    owner: &str,
    notional: f64,
    realized_pnl: f64,
) -> anyhow::Result<()> {
    let state = get_or_init(pool, owner).await?;
    let new_daily_pnl = state.daily_pnl_usdc + realized_pnl;
    let new_gross = (state.gross_open_usdc + notional).max(0.0);

    // Update weekly high watermark.
    let effective_equity = state.bankroll_usdc + new_daily_pnl;
    let new_weekly_high = state.weekly_high_usdc.max(effective_equity);

    sqlx::query(
        "UPDATE risk_governor_state
         SET daily_pnl_usdc = $2, gross_open_usdc = $3,
             weekly_high_usdc = $4, updated_at = now()
         WHERE owner = $1",
    )
    .bind(owner)
    .bind(new_daily_pnl)
    .bind(new_gross)
    .bind(new_weekly_high)
    .execute(pool)
    .await?;

    // Check drawdown kill switches.
    let daily_drawdown = -new_daily_pnl / state.bankroll_usdc;
    if daily_drawdown >= DAILY_DRAWDOWN_KILL_PCT {
        trigger_kill_switch(
            pool,
            owner,
            &format!(
                "daily drawdown {:.1}% >= {:.1}% limit",
                daily_drawdown * 100.0,
                DAILY_DRAWDOWN_KILL_PCT * 100.0
            ),
        )
        .await?;
        return Ok(());
    }

    let weekly_drawdown = (new_weekly_high - effective_equity) / new_weekly_high;
    if weekly_drawdown >= WEEKLY_DRAWDOWN_KILL_PCT {
        trigger_kill_switch(
            pool,
            owner,
            &format!(
                "weekly drawdown {:.1}% >= {:.1}% limit (high ${:.2}, equity ${:.2})",
                weekly_drawdown * 100.0,
                WEEKLY_DRAWDOWN_KILL_PCT * 100.0,
                new_weekly_high,
                effective_equity
            ),
        )
        .await?;
    }

    Ok(())
}

/// Reduce gross exposure when a position is closed.
pub async fn record_position_close(
    pool: &PgPool,
    owner: &str,
    notional_closed: f64,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE risk_governor_state
         SET gross_open_usdc = GREATEST(gross_open_usdc - $2, 0),
             updated_at = now()
         WHERE owner = $1",
    )
    .bind(owner)
    .bind(notional_closed)
    .execute(pool)
    .await?;
    Ok(())
}

async fn trigger_kill_switch(pool: &PgPool, owner: &str, reason: &str) -> anyhow::Result<()> {
    warn!("RISK KILL SWITCH triggered for {owner}: {reason}");

    sqlx::query(
        "UPDATE risk_governor_state
         SET kill_switch_active = true, kill_switch_reason = $2,
             kill_switch_at = now(), updated_at = now()
         WHERE owner = $1",
    )
    .bind(owner)
    .bind(reason)
    .execute(pool)
    .await?;

    // Pause all live agents for this owner.
    let paused = sqlx::query(
        "UPDATE external_agents
         SET active = false, updated_at = now()
         WHERE owner = $1 AND execution_mode = 'live' AND active = true",
    )
    .bind(owner)
    .execute(pool)
    .await?;

    info!(
        "Kill switch paused {} live agents for {owner}",
        paused.rows_affected()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_sane() {
        assert!(MAX_ENTRY_NOTIONAL > 0.0);
        assert!(MAX_BANKROLL_FRACTION_PER_ENTRY > 0.0 && MAX_BANKROLL_FRACTION_PER_ENTRY < 1.0);
        assert!(MAX_GROSS_EXPOSURE_FRACTION > 0.0 && MAX_GROSS_EXPOSURE_FRACTION < 1.0);
        assert!(DAILY_DRAWDOWN_KILL_PCT > 0.0 && DAILY_DRAWDOWN_KILL_PCT < 1.0);
        assert!(WEEKLY_DRAWDOWN_KILL_PCT > DAILY_DRAWDOWN_KILL_PCT);
    }

    #[test]
    fn hard_cap_is_25() {
        assert_eq!(MAX_ENTRY_NOTIONAL, 25.0);
    }

    #[test]
    fn bankroll_fraction_is_one_percent() {
        assert!((MAX_BANKROLL_FRACTION_PER_ENTRY - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn daily_kill_at_three_percent() {
        assert!((DAILY_DRAWDOWN_KILL_PCT - 0.03).abs() < f64::EPSILON);
    }

    #[test]
    fn weekly_kill_at_eight_percent() {
        assert!((WEEKLY_DRAWDOWN_KILL_PCT - 0.08).abs() < f64::EPSILON);
    }
}
