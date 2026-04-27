//! Per-Telegram-chat config persistence + helpers.
//!
//! Pure data layer on top of the `tg_chat_config` table. Called by the
//! command handler in `telegram_commands.rs` and (eventually) by the
//! alerter send-loops so per-chat threshold / cooldown / mute overrides
//! are respected when the bot broadcasts beyond the single supergroup.
//!
//! Follow-up (out of scope for this patch): the probability_alert,
//! new_market_alert, and cross_venue_arb send-paths currently address a
//! single `TELEGRAM_CHAT_ID` from env. When the bot is opened to more
//! chats, those paths need to loop over `tg_chat_config` rows and call
//! [`effective_threshold_for_chat`] + [`is_market_muted`] before sending.
//!
//! Wallet-link is a read-only identity binding. No trade-execution surface
//! touches this module and none is planned in this PR.

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use serde_json::Value as JsonValue;
use sha3::{Digest, Keccak256};
use sqlx::PgPool;
use uuid::Uuid;

const MIN_THRESHOLD_PCT: f32 = 0.1;
const MAX_THRESHOLD_PCT: f32 = 50.0;
const MIN_COOLDOWN_SECS: i32 = 60;
const MAX_COOLDOWN_SECS: i32 = 3600;
const MIN_QUIET_HOURS: f32 = 0.25;
const MAX_QUIET_HOURS: f32 = 168.0;

/// Signal kinds a chat may subscribe to. Strings match
/// `SignalKind::as_str()` in `alert_bus.rs`.
pub const VALID_KINDS: &[&str] = &["probability_shift", "volume_spike", "new_market"];

/// In-memory snapshot of a `tg_chat_config` row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TgChatConfig {
    pub chat_id: i64,
    pub threshold_override: Option<f32>,
    pub cooldown_override: Option<i32>,
    pub muted_markets: JsonValue,
    pub allow_categories: JsonValue,
    pub linked_user_id: Option<Uuid>,
    pub linked_wallet: Option<String>,
    pub linked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub quiet_until: Option<chrono::DateTime<chrono::Utc>>,
    pub subscribed_kinds: JsonValue,
}

/// Fetch the config row for `chat_id`, if any.
pub async fn fetch(pool: &PgPool, chat_id: i64) -> Result<Option<TgChatConfig>, sqlx::Error> {
    sqlx::query_as::<_, TgChatConfig>(
        "SELECT chat_id, threshold_override, cooldown_override, muted_markets, \
                allow_categories, linked_user_id, linked_wallet, linked_at, \
                quiet_until, subscribed_kinds \
         FROM tg_chat_config WHERE chat_id = $1",
    )
    .bind(chat_id)
    .fetch_optional(pool)
    .await
}

/// Upsert `threshold_override` (percent, 0.1-50). Clamped before persisting.
pub async fn set_threshold_override(
    pool: &PgPool,
    chat_id: i64,
    threshold_pct: f32,
) -> Result<(), sqlx::Error> {
    let clamped = clamp_threshold(threshold_pct);
    sqlx::query(
        "INSERT INTO tg_chat_config (chat_id, threshold_override, updated_at) \
         VALUES ($1, $2, NOW()) \
         ON CONFLICT (chat_id) DO UPDATE \
           SET threshold_override = EXCLUDED.threshold_override, updated_at = NOW()",
    )
    .bind(chat_id)
    .bind(clamped)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Upsert `cooldown_override` (seconds, 60-3600). Clamped before persisting.
pub async fn set_cooldown_override(
    pool: &PgPool,
    chat_id: i64,
    cooldown_secs: i32,
) -> Result<(), sqlx::Error> {
    let clamped = clamp_cooldown(cooldown_secs);
    sqlx::query(
        "INSERT INTO tg_chat_config (chat_id, cooldown_override, updated_at) \
         VALUES ($1, $2, NOW()) \
         ON CONFLICT (chat_id) DO UPDATE \
           SET cooldown_override = EXCLUDED.cooldown_override, updated_at = NOW()",
    )
    .bind(chat_id)
    .bind(clamped)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Append `market` to the muted_markets JSONB array. No-op if already present.
pub async fn add_muted_market(
    pool: &PgPool,
    chat_id: i64,
    market: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let current: Option<JsonValue> = sqlx::query_scalar(
        "SELECT muted_markets FROM tg_chat_config WHERE chat_id = $1 FOR UPDATE",
    )
    .bind(chat_id)
    .fetch_optional(&mut *tx)
    .await?;

    let next = add_to_json_array(current.as_ref(), market);
    sqlx::query(
        "INSERT INTO tg_chat_config (chat_id, muted_markets, updated_at) \
         VALUES ($1, $2, NOW()) \
         ON CONFLICT (chat_id) DO UPDATE \
           SET muted_markets = EXCLUDED.muted_markets, updated_at = NOW()",
    )
    .bind(chat_id)
    .bind(&next)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Remove `market` from the muted_markets JSONB array. No-op if absent.
pub async fn remove_muted_market(
    pool: &PgPool,
    chat_id: i64,
    market: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let current: Option<JsonValue> = sqlx::query_scalar(
        "SELECT muted_markets FROM tg_chat_config WHERE chat_id = $1 FOR UPDATE",
    )
    .bind(chat_id)
    .fetch_optional(&mut *tx)
    .await?;

    let next = remove_from_json_array(current.as_ref(), market);
    sqlx::query(
        "INSERT INTO tg_chat_config (chat_id, muted_markets, updated_at) \
         VALUES ($1, $2, NOW()) \
         ON CONFLICT (chat_id) DO UPDATE \
           SET muted_markets = EXCLUDED.muted_markets, updated_at = NOW()",
    )
    .bind(chat_id)
    .bind(&next)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Persist a verified wallet binding.
pub async fn set_linked_wallet(
    pool: &PgPool,
    chat_id: i64,
    wallet_lower: &str,
    linked_user_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tg_chat_config (chat_id, linked_wallet, linked_user_id, linked_at, updated_at) \
         VALUES ($1, $2, $3, NOW(), NOW()) \
         ON CONFLICT (chat_id) DO UPDATE \
           SET linked_wallet = EXCLUDED.linked_wallet, \
               linked_user_id = EXCLUDED.linked_user_id, \
               linked_at = EXCLUDED.linked_at, \
               updated_at = NOW()",
    )
    .bind(chat_id)
    .bind(wallet_lower)
    .bind(linked_user_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Set `quiet_until = NOW() + hours`. Clamped to [0.25, 168] hours.
pub async fn set_quiet_until(
    pool: &PgPool,
    chat_id: i64,
    hours: f32,
) -> Result<chrono::DateTime<chrono::Utc>, sqlx::Error> {
    let clamped = hours.clamp(MIN_QUIET_HOURS, MAX_QUIET_HOURS);
    let secs = (clamped as f64 * 3600.0) as i64;
    let until = chrono::Utc::now() + chrono::Duration::seconds(secs);
    sqlx::query(
        "INSERT INTO tg_chat_config (chat_id, quiet_until, updated_at) \
         VALUES ($1, $2, NOW()) \
         ON CONFLICT (chat_id) DO UPDATE \
           SET quiet_until = EXCLUDED.quiet_until, updated_at = NOW()",
    )
    .bind(chat_id)
    .bind(until)
    .execute(pool)
    .await?;
    Ok(until)
}

/// Clear `quiet_until` for `chat_id`. No-op if none was set.
pub async fn clear_quiet_until(pool: &PgPool, chat_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tg_chat_config SET quiet_until = NULL, updated_at = NOW() \
         WHERE chat_id = $1",
    )
    .bind(chat_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Report whether `chat_id` is currently in a quiet window.
pub async fn is_quiet_now(pool: &PgPool, chat_id: i64) -> bool {
    let row: Result<Option<(Option<chrono::DateTime<chrono::Utc>>,)>, sqlx::Error> =
        sqlx::query_as("SELECT quiet_until FROM tg_chat_config WHERE chat_id = $1")
            .bind(chat_id)
            .fetch_optional(pool)
            .await;
    matches!(row, Ok(Some((Some(until),))) if until > chrono::Utc::now())
}

/// Append `kind` to `subscribed_kinds`. No-op if already present.
pub async fn add_subscribed_kind(
    pool: &PgPool,
    chat_id: i64,
    kind: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let current: Option<JsonValue> = sqlx::query_scalar(
        "SELECT subscribed_kinds FROM tg_chat_config WHERE chat_id = $1 FOR UPDATE",
    )
    .bind(chat_id)
    .fetch_optional(&mut *tx)
    .await?;

    let next = add_to_json_array(current.as_ref(), kind);
    sqlx::query(
        "INSERT INTO tg_chat_config (chat_id, subscribed_kinds, updated_at) \
         VALUES ($1, $2, NOW()) \
         ON CONFLICT (chat_id) DO UPDATE \
           SET subscribed_kinds = EXCLUDED.subscribed_kinds, updated_at = NOW()",
    )
    .bind(chat_id)
    .bind(&next)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Remove `kind` from `subscribed_kinds`. No-op if absent.
pub async fn remove_subscribed_kind(
    pool: &PgPool,
    chat_id: i64,
    kind: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let current: Option<JsonValue> = sqlx::query_scalar(
        "SELECT subscribed_kinds FROM tg_chat_config WHERE chat_id = $1 FOR UPDATE",
    )
    .bind(chat_id)
    .fetch_optional(&mut *tx)
    .await?;

    let next = remove_from_json_array(current.as_ref(), kind);
    sqlx::query(
        "INSERT INTO tg_chat_config (chat_id, subscribed_kinds, updated_at) \
         VALUES ($1, $2, NOW()) \
         ON CONFLICT (chat_id) DO UPDATE \
           SET subscribed_kinds = EXCLUDED.subscribed_kinds, updated_at = NOW()",
    )
    .bind(chat_id)
    .bind(&next)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Report whether the chat is subscribed to `kind`. An empty or NULL
/// `subscribed_kinds` list is treated as "all kinds" — so a fresh chat
/// keeps receiving everything without explicit /subscribe calls.
pub async fn is_kind_subscribed(pool: &PgPool, chat_id: i64, kind: &str) -> bool {
    let row: Result<Option<(JsonValue,)>, sqlx::Error> = sqlx::query_as(
        "SELECT subscribed_kinds FROM tg_chat_config WHERE chat_id = $1",
    )
    .bind(chat_id)
    .fetch_optional(pool)
    .await;
    match row {
        Ok(Some((val,))) => kind_is_subscribed(&val, kind),
        _ => true,
    }
}

/// Pure form of [`is_kind_subscribed`] for unit-testability.
pub fn kind_is_subscribed(subscribed: &JsonValue, kind: &str) -> bool {
    match subscribed {
        JsonValue::Array(a) if a.is_empty() => true,
        JsonValue::Array(_) => json_array_contains(subscribed, kind),
        _ => true,
    }
}

/// Normalize a /subscribe or /unsubscribe arg to a valid SignalKind
/// string. Accepts the kind as-is or common aliases.
pub fn parse_kind_arg(raw: &str) -> Result<&'static str, String> {
    let lower = raw.trim().to_ascii_lowercase();
    let normalized = match lower.as_str() {
        "probability_shift" | "probability" | "prob" | "shift" => "probability_shift",
        "volume_spike" | "volume" | "spike" => "volume_spike",
        "new_market" | "new" | "newmarket" | "markets" => "new_market",
        _ => {
            return Err(format!(
                "unknown kind '{}'. valid: {}",
                raw,
                VALID_KINDS.join(", ")
            ));
        }
    };
    Ok(normalized)
}

/// Parse the `<hours>` arg for `/quiet`. Accepts "2", "2h", "0.5".
pub fn parse_quiet_hours_arg(raw: &str) -> Result<f32, String> {
    let trimmed = raw.trim();
    let numeric = trimmed.strip_suffix('h').unwrap_or(trimmed).trim();
    let v: f32 = numeric
        .parse()
        .map_err(|_| format!("'{}' is not a number", trimmed))?;
    if !v.is_finite() {
        return Err("hours must be finite".to_string());
    }
    if v < MIN_QUIET_HOURS || v > MAX_QUIET_HOURS {
        return Err(format!(
            "quiet hours must be between {} and {}",
            MIN_QUIET_HOURS, MAX_QUIET_HOURS
        ));
    }
    Ok(v)
}

/// Clear any linked wallet for `chat_id`. No-op if none was set.
pub async fn clear_linked_wallet(pool: &PgPool, chat_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tg_chat_config \
         SET linked_wallet = NULL, linked_user_id = NULL, linked_at = NULL, updated_at = NOW() \
         WHERE chat_id = $1",
    )
    .bind(chat_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Return every chat id that has a `tg_chat_config` row. Used by the digest
/// scheduler to fan out per-chat filtered messages on each tick. Returns an
/// empty vec on DB error so the digest still posts to the env-default chat.
pub async fn list_chat_ids(pool: &PgPool) -> Vec<i64> {
    let rows: Result<Vec<(i64,)>, sqlx::Error> =
        sqlx::query_as("SELECT chat_id FROM tg_chat_config").fetch_all(pool).await;
    match rows {
        Ok(rows) => rows.into_iter().map(|(id,)| id).collect(),
        Err(err) => {
            log::warn!("tg_chat_config: list_chat_ids failed: {}", err);
            Vec::new()
        }
    }
}

/// Resolve the effective threshold for `chat_id`, falling back to `env_default`
/// when no override is set. Always returns a clamped value.
pub async fn effective_threshold_for_chat(
    pool: &PgPool,
    chat_id: i64,
    env_default: f64,
) -> f64 {
    let row: Result<Option<(Option<f32>,)>, sqlx::Error> = sqlx::query_as(
        "SELECT threshold_override FROM tg_chat_config WHERE chat_id = $1",
    )
    .bind(chat_id)
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some((Some(v),))) => clamp_threshold(v) as f64,
        _ => env_default,
    }
}

/// Report whether `market` (a slug or market id) is muted for `chat_id`.
pub async fn is_market_muted(pool: &PgPool, chat_id: i64, market: &str) -> bool {
    let row: Result<Option<(JsonValue,)>, sqlx::Error> = sqlx::query_as(
        "SELECT muted_markets FROM tg_chat_config WHERE chat_id = $1",
    )
    .bind(chat_id)
    .fetch_optional(pool)
    .await;

    matches!(row, Ok(Some((val,))) if json_array_contains(&val, market))
}

/// Human-readable dump for `/config`. Caller HTML-escapes as needed.
pub fn dump_config_lines(cfg: Option<&TgChatConfig>) -> Vec<String> {
    let (threshold, cooldown, muted, cats, wallet, quiet, subs) = match cfg {
        Some(c) => (
            c.threshold_override
                .map(|v| format!("{:.2}%", v))
                .unwrap_or_else(|| "env default".to_string()),
            c.cooldown_override
                .map(|v| format!("{}s", v))
                .unwrap_or_else(|| "env default".to_string()),
            json_array_len(&c.muted_markets),
            json_array_len(&c.allow_categories),
            c.linked_wallet
                .as_deref()
                .map(shorten_wallet)
                .unwrap_or_else(|| "—".to_string()),
            match c.quiet_until {
                Some(until) if until > chrono::Utc::now() => {
                    until.format("until %Y-%m-%d %H:%M UTC").to_string()
                }
                _ => "off".to_string(),
            },
            match &c.subscribed_kinds {
                JsonValue::Array(a) if a.is_empty() => "all".to_string(),
                JsonValue::Array(a) => a
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => "all".to_string(),
            },
        ),
        None => (
            "env default".to_string(),
            "env default".to_string(),
            0,
            0,
            "—".to_string(),
            "off".to_string(),
            "all".to_string(),
        ),
    };
    vec![
        format!("threshold: {}", threshold),
        format!("cooldown: {}", cooldown),
        format!("muted markets: {}", muted),
        format!("allow categories: {}", cats),
        format!("linked wallet: {}", wallet),
        format!("quiet: {}", quiet),
        format!("subscribed: {}", subs),
    ]
}

// ── Parsing helpers (pure, unit-tested) ──

/// Parse the `<pct>` arg for `/threshold`. Accepts "3", "3%", "3.5".
pub fn parse_threshold_arg(raw: &str) -> Result<f32, String> {
    let trimmed = raw.trim();
    let numeric = trimmed.strip_suffix('%').unwrap_or(trimmed).trim();
    let v: f32 = numeric
        .parse()
        .map_err(|_| format!("'{}' is not a number", trimmed))?;
    if !v.is_finite() {
        return Err("threshold must be finite".to_string());
    }
    if v < MIN_THRESHOLD_PCT || v > MAX_THRESHOLD_PCT {
        return Err(format!(
            "threshold must be between {} and {} percent",
            MIN_THRESHOLD_PCT, MAX_THRESHOLD_PCT
        ));
    }
    Ok(v)
}

/// Parse the `<secs>` arg for `/cooldown`.
pub fn parse_cooldown_arg(raw: &str) -> Result<i32, String> {
    let trimmed = raw.trim();
    let v: i32 = trimmed
        .parse()
        .map_err(|_| format!("'{}' is not an integer", trimmed))?;
    if v < MIN_COOLDOWN_SECS || v > MAX_COOLDOWN_SECS {
        return Err(format!(
            "cooldown must be between {} and {} seconds",
            MIN_COOLDOWN_SECS, MAX_COOLDOWN_SECS
        ));
    }
    Ok(v)
}

/// Validate an EVM address is 0x-prefixed with 40 hex chars. Returns the
/// lowercase form. Casing is not EIP-55 checked here: wallet UIs often send
/// lowercase or mixed case, and the recovery step is authoritative.
pub fn validate_and_normalize_evm_address(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.len() != 42 || !trimmed.starts_with("0x") {
        return Err("address must be 0x-prefixed and 40 hex chars".to_string());
    }
    if !trimmed[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("address must contain only hex characters".to_string());
    }
    Ok(trimmed.to_ascii_lowercase())
}

/// Build the exact message a user must personal_sign to verify `/link`.
pub fn link_message(chat_id: i64, wallet_lower: &str, nonce: &str) -> String {
    format!(
        "Relay44 binds chat {} to wallet {} nonce {}",
        chat_id, wallet_lower, nonce
    )
}

/// Recover the signer address (lowercase, 0x-prefixed) of an EIP-191
/// `personal_sign` signature over `message`.
///
/// Accepts hex (with or without `0x`) of length 130 (64 bytes sig + 1 byte
/// recovery id). The recovery byte may be 0/1 or 27/28.
pub fn recover_personal_sign(message: &str, signature_hex: &str) -> Result<String, String> {
    let sig = signature_hex
        .strip_prefix("0x")
        .unwrap_or(signature_hex)
        .trim();
    let bytes = hex::decode(sig).map_err(|_| "signature is not hex".to_string())?;
    if bytes.len() != 65 {
        return Err(format!(
            "signature must be 65 bytes (got {})",
            bytes.len()
        ));
    }
    let v_raw = bytes[64];
    let v = match v_raw {
        0 | 1 => v_raw,
        27 | 28 => v_raw - 27,
        _ => return Err(format!("invalid recovery byte {}", v_raw)),
    };
    let recovery_id =
        RecoveryId::from_byte(v).ok_or_else(|| format!("invalid recovery id {}", v))?;
    let sig = Signature::from_slice(&bytes[..64])
        .map_err(|e| format!("invalid signature: {}", e))?;

    // EIP-191 prehash: keccak256("\x19Ethereum Signed Message:\n" + len + message)
    let mut hasher = Keccak256::new();
    let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
    hasher.update(prefix.as_bytes());
    hasher.update(message.as_bytes());
    let digest = hasher.finalize();

    let verifying_key = VerifyingKey::recover_from_prehash(&digest, &sig, recovery_id)
        .map_err(|e| format!("recover failed: {}", e))?;
    Ok(eth_address_from_verifying_key(&verifying_key))
}

fn eth_address_from_verifying_key(key: &VerifyingKey) -> String {
    let public_key = key.to_encoded_point(false);
    let pubkey_bytes = &public_key.as_bytes()[1..];
    let hash = Keccak256::digest(pubkey_bytes);
    format!("0x{}", hex::encode(&hash[12..]))
}

// ── JSONB helpers ──

fn add_to_json_array(current: Option<&JsonValue>, item: &str) -> JsonValue {
    let mut arr: Vec<JsonValue> = match current {
        Some(JsonValue::Array(a)) => a.clone(),
        _ => Vec::new(),
    };
    let already = arr
        .iter()
        .any(|v| v.as_str().map(|s| s == item).unwrap_or(false));
    if !already {
        arr.push(JsonValue::String(item.to_string()));
    }
    JsonValue::Array(arr)
}

fn remove_from_json_array(current: Option<&JsonValue>, item: &str) -> JsonValue {
    let arr: Vec<JsonValue> = match current {
        Some(JsonValue::Array(a)) => a
            .iter()
            .filter(|v| v.as_str().map(|s| s != item).unwrap_or(true))
            .cloned()
            .collect(),
        _ => Vec::new(),
    };
    JsonValue::Array(arr)
}

fn json_array_contains(value: &JsonValue, needle: &str) -> bool {
    match value {
        JsonValue::Array(arr) => arr
            .iter()
            .any(|v| v.as_str().map(|s| s == needle).unwrap_or(false)),
        _ => false,
    }
}

fn json_array_len(value: &JsonValue) -> usize {
    match value {
        JsonValue::Array(arr) => arr.len(),
        _ => 0,
    }
}

fn shorten_wallet(w: &str) -> String {
    if w.len() >= 10 {
        format!("{}…{}", &w[..6], &w[w.len() - 4..])
    } else {
        w.to_string()
    }
}

fn clamp_threshold(v: f32) -> f32 {
    v.clamp(MIN_THRESHOLD_PCT, MAX_THRESHOLD_PCT)
}

fn clamp_cooldown(v: i32) -> i32 {
    v.clamp(MIN_COOLDOWN_SECS, MAX_COOLDOWN_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn threshold_parses_plain_number() {
        assert!((parse_threshold_arg("3.5").unwrap() - 3.5).abs() < 1e-6);
    }

    #[test]
    fn threshold_parses_with_percent_suffix() {
        assert!((parse_threshold_arg("3.5%").unwrap() - 3.5).abs() < 1e-6);
    }

    #[test]
    fn threshold_parses_integer_string() {
        assert!((parse_threshold_arg("3").unwrap() - 3.0).abs() < 1e-6);
    }

    #[test]
    fn threshold_rejects_non_number() {
        assert!(parse_threshold_arg("not a number").is_err());
    }

    #[test]
    fn threshold_rejects_out_of_range() {
        assert!(parse_threshold_arg("0.01").is_err());
        assert!(parse_threshold_arg("100").is_err());
    }

    #[test]
    fn cooldown_parses_and_clamps_range() {
        assert_eq!(parse_cooldown_arg("120").unwrap(), 120);
        assert!(parse_cooldown_arg("10").is_err());
        assert!(parse_cooldown_arg("999999").is_err());
        assert!(parse_cooldown_arg("abc").is_err());
    }

    #[test]
    fn validates_evm_address() {
        let good = "0x1234567890abcdef1234567890ABCDEF12345678";
        assert_eq!(
            validate_and_normalize_evm_address(good).unwrap(),
            good.to_ascii_lowercase()
        );
        assert!(validate_and_normalize_evm_address("0xdeadbeef").is_err());
        assert!(
            validate_and_normalize_evm_address("1234567890abcdef1234567890abcdef12345678").is_err()
        );
        assert!(
            validate_and_normalize_evm_address("0xZZZZ567890abcdef1234567890abcdef12345678")
                .is_err()
        );
    }

    #[test]
    fn add_muted_market_is_idempotent() {
        let start = json!(["a"]);
        let after_first = add_to_json_array(Some(&start), "b");
        let after_second = add_to_json_array(Some(&after_first), "b");
        assert_eq!(after_first, json!(["a", "b"]));
        assert_eq!(after_second, json!(["a", "b"]));
    }

    #[test]
    fn remove_muted_market_is_idempotent() {
        let start = json!(["a", "b"]);
        let after_first = remove_from_json_array(Some(&start), "b");
        let after_second = remove_from_json_array(Some(&after_first), "b");
        assert_eq!(after_first, json!(["a"]));
        assert_eq!(after_second, json!(["a"]));
    }

    #[test]
    fn add_from_null_creates_new_array() {
        let result = add_to_json_array(None, "x");
        assert_eq!(result, json!(["x"]));
    }

    #[test]
    fn json_array_contains_works() {
        assert!(json_array_contains(&json!(["a", "b"]), "b"));
        assert!(!json_array_contains(&json!(["a", "b"]), "c"));
        assert!(!json_array_contains(&json!({}), "a"));
    }

    #[test]
    fn clamps_thresholds_and_cooldowns() {
        assert!((clamp_threshold(0.0) - MIN_THRESHOLD_PCT).abs() < 1e-6);
        assert!((clamp_threshold(9999.0) - MAX_THRESHOLD_PCT).abs() < 1e-6);
        assert_eq!(clamp_cooldown(0), MIN_COOLDOWN_SECS);
        assert_eq!(clamp_cooldown(10_000), MAX_COOLDOWN_SECS);
    }

    #[test]
    fn shorten_wallet_abbreviates_long_addresses() {
        let s = shorten_wallet("0x1234567890abcdef1234567890abcdef12345678");
        assert!(s.starts_with("0x1234"));
        assert!(s.ends_with("5678"));
    }

    /// Known test vector: a personal_sign signature generated with a known
    /// private key. Exercises the EIP-191 prehash + k256 recovery path and
    /// both the 0/1 and 27/28 recovery byte encodings.
    #[test]
    fn personal_sign_recovery_matches_known_signer() {
        // `PrehashSigner` trait import mirrors `evm_signer.rs` usage so the
        // call resolves on toolchains where the inherent method is gated.
        #[allow(unused_imports)]
        use k256::ecdsa::signature::hazmat::PrehashSigner;
        use k256::ecdsa::SigningKey;

        // Widely-used Ethereum test private key. NOT a live wallet.
        let priv_hex = "4c0883a69102937d6231471b5dbb6204fe512961708279e2e3b4e0f4b9f3d7b1";
        let priv_bytes = hex::decode(priv_hex).unwrap();
        let signing_key = SigningKey::from_bytes(priv_bytes.as_slice().into()).unwrap();
        let expected_address =
            eth_address_from_verifying_key(signing_key.verifying_key());

        let message = "Relay44 binds chat 12345 to wallet 0xabc nonce deadbeef";

        let mut hasher = Keccak256::new();
        let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
        hasher.update(prefix.as_bytes());
        hasher.update(message.as_bytes());
        let digest = hasher.finalize();

        let (sig, rec_id): (Signature, RecoveryId) = signing_key
            .sign_prehash_recoverable(&digest)
            .unwrap();
        let mut sig_bytes = sig.to_bytes().to_vec();
        sig_bytes.push(rec_id.to_byte() + 27);
        let sig_hex = format!("0x{}", hex::encode(&sig_bytes));

        let recovered = recover_personal_sign(message, &sig_hex).unwrap();
        assert_eq!(recovered, expected_address);

        let mut alt_bytes = sig.to_bytes().to_vec();
        alt_bytes.push(rec_id.to_byte());
        let alt_hex = hex::encode(&alt_bytes);
        let recovered_alt = recover_personal_sign(message, &alt_hex).unwrap();
        assert_eq!(recovered_alt, expected_address);
    }

    #[test]
    fn personal_sign_rejects_bad_signature() {
        assert!(recover_personal_sign("hello", "0xdeadbeef").is_err());
        assert!(recover_personal_sign("hello", "not-hex").is_err());
        let mut bad = vec![0u8; 65];
        bad[64] = 42;
        let bad_hex = hex::encode(&bad);
        assert!(recover_personal_sign("hello", &bad_hex).is_err());
    }

    #[test]
    fn link_message_is_stable() {
        let m = link_message(99, "0xabc", "nonce1");
        assert_eq!(m, "Relay44 binds chat 99 to wallet 0xabc nonce nonce1");
    }

    #[test]
    fn dump_config_handles_missing_row() {
        let lines = dump_config_lines(None);
        assert!(lines.iter().any(|l| l.contains("env default")));
        assert!(lines.iter().any(|l| l.contains("linked wallet: —")));
        assert!(lines.iter().any(|l| l.contains("quiet: off")));
        assert!(lines.iter().any(|l| l.contains("subscribed: all")));
    }

    #[test]
    fn parse_quiet_hours_accepts_numbers_and_suffix() {
        assert!((parse_quiet_hours_arg("2").unwrap() - 2.0).abs() < 1e-6);
        assert!((parse_quiet_hours_arg("2h").unwrap() - 2.0).abs() < 1e-6);
        assert!((parse_quiet_hours_arg("0.5").unwrap() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn parse_quiet_hours_rejects_out_of_range() {
        assert!(parse_quiet_hours_arg("0").is_err());
        assert!(parse_quiet_hours_arg("999").is_err());
        assert!(parse_quiet_hours_arg("abc").is_err());
    }

    #[test]
    fn parse_kind_accepts_canonical_and_aliases() {
        assert_eq!(parse_kind_arg("probability_shift").unwrap(), "probability_shift");
        assert_eq!(parse_kind_arg("prob").unwrap(), "probability_shift");
        assert_eq!(parse_kind_arg("VOLUME").unwrap(), "volume_spike");
        assert_eq!(parse_kind_arg("volume_spike").unwrap(), "volume_spike");
        assert_eq!(parse_kind_arg("new_market").unwrap(), "new_market");
        assert_eq!(parse_kind_arg("new").unwrap(), "new_market");
        assert_eq!(parse_kind_arg("newmarket").unwrap(), "new_market");
        assert_eq!(parse_kind_arg("Markets").unwrap(), "new_market");
        assert!(parse_kind_arg("garbage").is_err());
    }

    #[test]
    fn valid_kinds_includes_new_market() {
        // Subscribers see the canonical list when /subscribe rejects an arg.
        assert!(VALID_KINDS.contains(&"new_market"));
    }

    #[test]
    fn empty_subscribed_kinds_means_all() {
        assert!(kind_is_subscribed(&json!([]), "probability_shift"));
        assert!(kind_is_subscribed(&json!([]), "volume_spike"));
    }

    #[test]
    fn non_empty_subscribed_kinds_is_exact_match() {
        let subs = json!(["volume_spike"]);
        assert!(kind_is_subscribed(&subs, "volume_spike"));
        assert!(!kind_is_subscribed(&subs, "probability_shift"));
    }
}
