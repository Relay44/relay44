//! Per-chat one-shot price-cross alerts driven by `/notify`.
//!
//! Storage layer for `tg_notify_rules`. Pure CRUD over the table — the
//! scheduling/firing path lives in `notify_scheduler`. Threshold values are
//! kept in YES-price space (0.0..1.0); the command parser converts user-facing
//! pcts/cents to that shape before insert.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NotifyRule {
    pub id: i64,
    pub chat_id: i64,
    pub venue: String,
    pub slug: String,
    pub threshold: f64,
    pub baseline_price: f64,
    pub created_at: DateTime<Utc>,
    pub fired_at: Option<DateTime<Utc>>,
    pub fired_price: Option<f64>,
}

/// Create a new active rule. Returns the rule id so callers can reference it
/// in confirmation messages.
pub async fn create(
    pool: &PgPool,
    chat_id: i64,
    venue: &str,
    slug: &str,
    threshold: f64,
    baseline_price: f64,
) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO tg_notify_rules \
            (chat_id, venue, slug, threshold, baseline_price) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id",
    )
    .bind(chat_id)
    .bind(venue)
    .bind(slug)
    .bind(threshold)
    .bind(baseline_price)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// All active (unfired) rules for a chat, newest first.
pub async fn list_active_for_chat(
    pool: &PgPool,
    chat_id: i64,
) -> Result<Vec<NotifyRule>, sqlx::Error> {
    sqlx::query_as::<_, NotifyRule>(
        "SELECT id, chat_id, venue, slug, threshold, baseline_price, \
                created_at, fired_at, fired_price \
         FROM tg_notify_rules \
         WHERE chat_id = $1 AND fired_at IS NULL \
         ORDER BY created_at DESC",
    )
    .bind(chat_id)
    .fetch_all(pool)
    .await
}

/// All active rules across every chat, used by the scheduler tick.
pub async fn list_all_active(pool: &PgPool) -> Result<Vec<NotifyRule>, sqlx::Error> {
    sqlx::query_as::<_, NotifyRule>(
        "SELECT id, chat_id, venue, slug, threshold, baseline_price, \
                created_at, fired_at, fired_price \
         FROM tg_notify_rules \
         WHERE fired_at IS NULL",
    )
    .fetch_all(pool)
    .await
}

/// Mark a rule as fired so the scheduler skips it next tick.
pub async fn mark_fired(
    pool: &PgPool,
    rule_id: i64,
    fired_price: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tg_notify_rules \
         SET fired_at = NOW(), fired_price = $2 \
         WHERE id = $1 AND fired_at IS NULL",
    )
    .bind(rule_id)
    .bind(fired_price)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Delete every active rule for a chat (`/notify clear`). Returns the number
/// of rows removed so the caller can confirm with the user.
pub async fn clear_for_chat(pool: &PgPool, chat_id: i64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM tg_notify_rules WHERE chat_id = $1 AND fired_at IS NULL",
    )
    .bind(chat_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Returns true if `current` has crossed `threshold` starting from the side
/// `baseline` was on. Used by the scheduler to decide whether a rule fired
/// since it was created.
pub fn crossed(baseline: f64, current: f64, threshold: f64) -> bool {
    (baseline < threshold && current >= threshold)
        || (baseline > threshold && current <= threshold)
}

/// Parse a user-facing threshold ("60%", "60c", "0.60") into YES-price space.
/// Returns an error for inputs outside (0, 1) or non-numeric.
pub fn parse_threshold_arg(raw: &str) -> Result<f64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("threshold required".to_string());
    }
    let stripped = trimmed
        .strip_suffix('%')
        .or_else(|| trimmed.strip_suffix('c'))
        .or_else(|| trimmed.strip_suffix('¢'))
        .unwrap_or(trimmed);
    let n: f64 = stripped
        .parse()
        .map_err(|_| format!("'{}' is not a number", trimmed))?;
    if !n.is_finite() {
        return Err("threshold must be finite".to_string());
    }
    // Treat any value > 1 as a percentage (the caller dropped the % sign).
    let normalized = if n > 1.0 { n / 100.0 } else { n };
    if normalized <= 0.0 || normalized >= 1.0 {
        return Err("threshold must be between 0 and 100%".to_string());
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossed_detects_upward_movement() {
        assert!(crossed(0.45, 0.61, 0.60));
        assert!(crossed(0.45, 0.60, 0.60));
        assert!(!crossed(0.45, 0.59, 0.60));
    }

    #[test]
    fn crossed_detects_downward_movement() {
        assert!(crossed(0.70, 0.59, 0.60));
        assert!(crossed(0.70, 0.60, 0.60));
        assert!(!crossed(0.70, 0.61, 0.60));
    }

    #[test]
    fn crossed_returns_false_when_baseline_equals_threshold() {
        // The rule was created exactly at the threshold; we fire on the
        // first move that strictly leaves it, not on the create itself.
        assert!(!crossed(0.60, 0.60, 0.60));
    }

    #[test]
    fn parse_threshold_accepts_pct_and_cents_and_decimal() {
        assert_eq!(parse_threshold_arg("60%").unwrap(), 0.60);
        assert_eq!(parse_threshold_arg("60c").unwrap(), 0.60);
        assert_eq!(parse_threshold_arg("60¢").unwrap(), 0.60);
        assert_eq!(parse_threshold_arg("60").unwrap(), 0.60);
        assert!((parse_threshold_arg("0.60").unwrap() - 0.60).abs() < 1e-9);
        assert!((parse_threshold_arg("0.5").unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn parse_threshold_rejects_out_of_range() {
        assert!(parse_threshold_arg("0").is_err());
        assert!(parse_threshold_arg("100").is_err());
        assert!(parse_threshold_arg("-5").is_err());
        assert!(parse_threshold_arg("150%").is_err());
        assert!(parse_threshold_arg("").is_err());
        assert!(parse_threshold_arg("abc").is_err());
    }
}
