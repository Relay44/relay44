//! Managed agent lifecycle: durable state machine + pre-commit intent log.
//!
//! A managed agent's `lifecycle_state` is distinct from the user-facing
//! `status`. `status` expresses user intent (active / stopped / paused);
//! `lifecycle_state` expresses where the runtime actually is (initializing,
//! active, paused, liquidating, settled, failed). The runner drives
//! transitions, users drive status.
//!
//! Every side-effect (open_position, close_position, later post_order /
//! cancel_order) is preceded by an intent row in `managed_agent_intents`
//! with state='pending'. On success the row is flipped to 'confirmed'. On
//! process restart, `recover_unresolved_intents` reconciles any pending rows
//! by checking whether the side-effect actually landed (position exists,
//! order on venue, etc.) and marks them confirmed or abandoned — never
//! re-executed blindly.
//!
//! Redis mirrors the hot path (checkpoint blob, pending intents hash, active
//! set) for fast startup scans and observability, but Postgres is
//! authoritative. Redis outages log-and-continue.

use chrono::{DateTime, Utc};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::fmt;

use crate::services::RedisService;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Initializing,
    Active,
    Paused,
    Liquidating,
    Settled,
    Failed,
}

impl LifecycleState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initializing => "initializing",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Liquidating => "liquidating",
            Self::Settled => "settled",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "initializing" => Self::Initializing,
            "active" => Self::Active,
            "paused" => Self::Paused,
            "liquidating" => Self::Liquidating,
            "settled" => Self::Settled,
            "failed" => Self::Failed,
            _ => return None,
        })
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Settled | Self::Failed)
    }

    /// Whether the runner should pick this agent up for strategy evaluation.
    pub fn is_runnable(self) -> bool {
        matches!(self, Self::Active | Self::Liquidating)
    }

    fn can_transition_to(self, next: LifecycleState) -> bool {
        use LifecycleState::*;
        match (self, next) {
            (Initializing, Active | Failed | Paused) => true,
            (Active, Paused | Liquidating | Settled) => true,
            (Paused, Active | Liquidating | Settled) => true,
            (Liquidating, Settled) => true,
            (Settled | Failed, _) => false,
            (a, b) if a == b => true,
            _ => false,
        }
    }
}

impl fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentKind {
    OpenPosition,
    ClosePosition,
    PostOrder,
    CancelOrder,
}

impl IntentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenPosition => "open_position",
            Self::ClosePosition => "close_position",
            Self::PostOrder => "post_order",
            Self::CancelOrder => "cancel_order",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenPositionPayload {
    pub provider: String,
    pub market_slug: String,
    pub outcome: String,
    pub side: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClosePositionPayload {
    pub position_id: i64,
    pub market_slug: String,
    pub outcome: String,
    pub side: String,
}

/// Handle returned by `log_intent`. Must be passed to `confirm_intent` on
/// success — otherwise the intent stays pending and is reconciled on restart.
#[must_use = "intents must be confirmed or abandoned"]
#[derive(Debug, Clone, Copy)]
pub struct IntentHandle {
    pub agent_id_key: u64,
    pub seq: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CheckpointBlob {
    pub lifecycle_state: String,
    pub seq: i64,
    pub last_tick_at: DateTime<Utc>,
    pub pnl_usdc: f64,
    pub open_position_count: i64,
    pub version: u32,
}

/// Transition an agent to a new lifecycle state. DB is authoritative; Redis
/// mirror updates are best-effort. Invalid transitions return an error
/// without touching either store.
pub async fn set_lifecycle_state(
    pool: &PgPool,
    redis: &RedisService,
    agent_id: &str,
    next: LifecycleState,
    reason: Option<&str>,
) -> Result<(), String> {
    let current: Option<String> =
        sqlx::query_scalar("SELECT lifecycle_state FROM managed_agents WHERE id = $1 FOR UPDATE")
            .bind(agent_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    let current = current.ok_or_else(|| format!("agent {} not found", agent_id))?;
    let current_state = LifecycleState::parse(&current)
        .ok_or_else(|| format!("agent {} has unknown lifecycle_state={}", agent_id, current))?;

    if current_state == next {
        return Ok(());
    }

    if !current_state.can_transition_to(next) {
        return Err(format!(
            "invalid lifecycle transition {} → {} for agent {}",
            current_state, next, agent_id
        ));
    }

    sqlx::query(
        "UPDATE managed_agents \
         SET lifecycle_state = $1, lifecycle_updated_at = NOW(), \
             failure_reason = $2, updated_at = NOW() \
         WHERE id = $3",
    )
    .bind(next.as_str())
    .bind(reason)
    .bind(agent_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    if next.is_runnable() {
        if let Err(e) = redis.agent_active_mark(agent_id).await {
            debug!("redis agent_active_mark {}: {}", agent_id, e);
        }
    } else if let Err(e) = redis.agent_active_unmark(agent_id).await {
        debug!("redis agent_active_unmark {}: {}", agent_id, e);
    }

    Ok(())
}

/// Record a pending intent and return a handle. Allocates the next sequence
/// number in the same transaction as the insert so seq is strictly monotonic
/// per agent even under concurrent ticks (guarded by row-level lock).
pub async fn log_intent(
    pool: &PgPool,
    redis: &RedisService,
    agent_id: &str,
    kind: IntentKind,
    payload: &serde_json::Value,
) -> Result<IntentHandle, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let seq: i64 = sqlx::query_scalar(
        "UPDATE managed_agents \
         SET last_checkpoint_seq = last_checkpoint_seq + 1, \
             last_checkpoint_at = NOW() \
         WHERE id = $1 \
         RETURNING last_checkpoint_seq",
    )
    .bind(agent_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO managed_agent_intents (agent_id, seq, kind, payload, state) \
         VALUES ($1, $2, $3, $4::jsonb, 'pending')",
    )
    .bind(agent_id)
    .bind(seq)
    .bind(kind.as_str())
    .bind(payload)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    if let Err(e) = redis
        .agent_intent_log(agent_id, seq, &payload.to_string())
        .await
    {
        debug!("redis agent_intent_log {}: {}", agent_id, e);
    }

    Ok(IntentHandle {
        agent_id_key: fxhash(agent_id),
        seq,
    })
}

pub async fn confirm_intent(
    pool: &PgPool,
    redis: &RedisService,
    agent_id: &str,
    handle: IntentHandle,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE managed_agent_intents \
         SET state = 'confirmed', resolved_at = NOW() \
         WHERE agent_id = $1 AND seq = $2 AND state = 'pending'",
    )
    .bind(agent_id)
    .bind(handle.seq)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Err(e) = redis.agent_intent_clear(agent_id, handle.seq).await {
        debug!("redis agent_intent_clear {}: {}", agent_id, e);
    }

    Ok(())
}

pub async fn abandon_intent(
    pool: &PgPool,
    redis: &RedisService,
    agent_id: &str,
    handle: IntentHandle,
    note: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE managed_agent_intents \
         SET state = 'abandoned', resolved_at = NOW(), resolution_note = $3 \
         WHERE agent_id = $1 AND seq = $2 AND state = 'pending'",
    )
    .bind(agent_id)
    .bind(handle.seq)
    .bind(note)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Err(e) = redis.agent_intent_clear(agent_id, handle.seq).await {
        debug!("redis agent_intent_clear {}: {}", agent_id, e);
    }

    Ok(())
}

/// Write a checkpoint blob mirror to Redis. DB columns `last_checkpoint_at`
/// and `last_checkpoint_seq` are maintained by `log_intent`; this is the
/// observability mirror.
pub async fn write_checkpoint_mirror(redis: &RedisService, agent_id: &str, blob: &CheckpointBlob) {
    match serde_json::to_string(blob) {
        Ok(s) => {
            if let Err(e) = redis.agent_checkpoint_write(agent_id, &s).await {
                debug!("redis agent_checkpoint_write {}: {}", agent_id, e);
            }
        }
        Err(e) => debug!("checkpoint serialize {}: {}", agent_id, e),
    }
}

#[derive(Debug, Default)]
pub struct RecoveryStats {
    pub scanned: u64,
    pub confirmed: u64,
    pub abandoned: u64,
    pub errors: u64,
}

/// Reconcile all pending intents after process restart. For each pending
/// row, check whether the side-effect actually landed:
///   * open_position: does a matching row in managed_agent_positions exist
///     with opened_at >= intent.created_at? Confirmed: landed. Abandoned: no.
///   * close_position: does the position_id still exist? Abandoned if yes
///     (close never ran), confirmed if no (close landed).
///   * post_order / cancel_order: not yet wired. Abandon with note.
pub async fn recover_unresolved_intents(
    pool: &PgPool,
    redis: &RedisService,
) -> Result<RecoveryStats, String> {
    let mut stats = RecoveryStats::default();

    let rows = sqlx::query(
        "SELECT agent_id, seq, kind, payload::text AS payload_text, created_at \
         FROM managed_agent_intents \
         WHERE state = 'pending' \
         ORDER BY agent_id, seq",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    stats.scanned = rows.len() as u64;

    for row in rows {
        let agent_id: String = row.try_get("agent_id").unwrap_or_default();
        let seq: i64 = row.try_get("seq").unwrap_or_default();
        let kind: String = row.try_get("kind").unwrap_or_default();
        let payload_text: String = row.try_get("payload_text").unwrap_or_else(|_| "{}".into());
        let created_at: DateTime<Utc> = row.try_get("created_at").unwrap_or_else(|_| Utc::now());
        let payload: serde_json::Value =
            serde_json::from_str(&payload_text).unwrap_or(serde_json::Value::Null);

        let handle = IntentHandle {
            agent_id_key: fxhash(&agent_id),
            seq,
        };

        let outcome = match kind.as_str() {
            "open_position" => reconcile_open(pool, &agent_id, &payload, created_at).await,
            "close_position" => reconcile_close(pool, &agent_id, &payload).await,
            "post_order" | "cancel_order" => Ok(Reconciled::Abandon(
                "order execution layer not wired".into(),
            )),
            other => Ok(Reconciled::Abandon(format!(
                "unknown intent kind {}",
                other
            ))),
        };

        match outcome {
            Ok(Reconciled::Confirm) => {
                if let Err(e) = confirm_intent(pool, redis, &agent_id, handle).await {
                    warn!("recover: confirm {}/{} failed: {}", agent_id, seq, e);
                    stats.errors += 1;
                } else {
                    stats.confirmed += 1;
                }
            }
            Ok(Reconciled::Abandon(note)) => {
                if let Err(e) = abandon_intent(pool, redis, &agent_id, handle, &note).await {
                    warn!("recover: abandon {}/{} failed: {}", agent_id, seq, e);
                    stats.errors += 1;
                } else {
                    stats.abandoned += 1;
                }
            }
            Err(e) => {
                warn!("recover: reconcile {}/{} failed: {}", agent_id, seq, e);
                stats.errors += 1;
            }
        }
    }

    Ok(stats)
}

enum Reconciled {
    Confirm,
    Abandon(String),
}

async fn reconcile_open(
    pool: &PgPool,
    agent_id: &str,
    payload: &serde_json::Value,
    created_at: DateTime<Utc>,
) -> Result<Reconciled, String> {
    let market_slug = payload.get("market_slug").and_then(|v| v.as_str());
    let outcome = payload.get("outcome").and_then(|v| v.as_str());
    let side = payload.get("side").and_then(|v| v.as_str());

    let (Some(market_slug), Some(outcome), Some(side)) = (market_slug, outcome, side) else {
        return Ok(Reconciled::Abandon(
            "open_position payload malformed".into(),
        ));
    };

    let exists: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM managed_agent_positions \
         WHERE agent_id = $1 AND market_slug = $2 AND outcome = $3 AND side = $4 \
           AND opened_at >= $5 - INTERVAL '5 seconds'",
    )
    .bind(agent_id)
    .bind(market_slug)
    .bind(outcome)
    .bind(side)
    .bind(created_at)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    if exists.is_some() {
        Ok(Reconciled::Confirm)
    } else {
        Ok(Reconciled::Abandon(
            "no matching position row — side-effect did not land".into(),
        ))
    }
}

async fn reconcile_close(
    pool: &PgPool,
    agent_id: &str,
    payload: &serde_json::Value,
) -> Result<Reconciled, String> {
    let position_id = payload.get("position_id").and_then(|v| v.as_i64());
    let Some(position_id) = position_id else {
        return Ok(Reconciled::Abandon(
            "close_position payload malformed".into(),
        ));
    };

    let still_open: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM managed_agent_positions WHERE id = $1 AND agent_id = $2",
    )
    .bind(position_id)
    .bind(agent_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    if still_open.is_some() {
        Ok(Reconciled::Abandon(
            "position still open — close did not land, will be swept next tick".into(),
        ))
    } else {
        Ok(Reconciled::Confirm)
    }
}

fn fxhash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_roundtrip() {
        for s in [
            "initializing",
            "active",
            "paused",
            "liquidating",
            "settled",
            "failed",
        ] {
            let parsed = LifecycleState::parse(s).unwrap();
            assert_eq!(parsed.as_str(), s);
        }
        assert!(LifecycleState::parse("nope").is_none());
    }

    #[test]
    fn terminal_states_reject_transitions() {
        for terminal in [LifecycleState::Settled, LifecycleState::Failed] {
            for next in [
                LifecycleState::Active,
                LifecycleState::Paused,
                LifecycleState::Liquidating,
                LifecycleState::Initializing,
            ] {
                assert!(
                    !terminal.can_transition_to(next),
                    "{} → {} must be rejected",
                    terminal,
                    next
                );
            }
        }
    }

    #[test]
    fn valid_happy_path() {
        use LifecycleState::*;
        assert!(Initializing.can_transition_to(Active));
        assert!(Active.can_transition_to(Paused));
        assert!(Paused.can_transition_to(Active));
        assert!(Active.can_transition_to(Liquidating));
        assert!(Liquidating.can_transition_to(Settled));
        assert!(Initializing.can_transition_to(Failed));
    }

    #[test]
    fn invalid_skip_transitions() {
        use LifecycleState::*;
        assert!(!Initializing.can_transition_to(Liquidating));
        assert!(!Initializing.can_transition_to(Settled));
        assert!(!Liquidating.can_transition_to(Active));
        assert!(!Liquidating.can_transition_to(Paused));
    }

    #[test]
    fn runnable_set_matches_runner_scan() {
        assert!(LifecycleState::Active.is_runnable());
        assert!(LifecycleState::Liquidating.is_runnable());
        assert!(!LifecycleState::Paused.is_runnable());
        assert!(!LifecycleState::Settled.is_runnable());
        assert!(!LifecycleState::Failed.is_runnable());
        assert!(!LifecycleState::Initializing.is_runnable());
    }

    #[test]
    fn self_transition_is_noop_allowed() {
        assert!(LifecycleState::Active.can_transition_to(LifecycleState::Active));
    }
}
