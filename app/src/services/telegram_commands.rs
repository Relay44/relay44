//! Telegram command handler.
//!
//! Long-polls the Telegram Bot API (`getUpdates`) and dispatches slash
//! commands sent into the configured chat. Read-only commands:
//! `/status`, `/help`, `/top [category]`, `/market <slug>`. State-changing
//! commands persist into `tg_chat_config`: `/mute`, `/unmute`, `/threshold`,
//! `/cooldown`, `/config`, `/link`, `/verify`, `/unlink`.
//!
//! `/link` + `/verify` establish a read-only wallet identity binding via
//! EIP-191 `personal_sign`. No trade-execution commands are wired up and
//! none are planned in this patch — the binding exists so future
//! portfolio-aware routing can target the right wallet without re-prompting.
//!
//! Messages from chats other than `TELEGRAM_CHAT_ID` are ignored — the
//! bot isn't open to the public yet.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::services::tg_chat_config;
use crate::AppState;

const POLL_TIMEOUT_SECS: u64 = 30;
const HTTP_TIMEOUT_SECS: u64 = POLL_TIMEOUT_SECS + 10;
const ERROR_BACKOFF_SECS: u64 = 5;
const NONCE_TTL_SECS: u64 = 600;

pub fn spawn(state: Arc<AppState>) {
    let enabled = std::env::var("TELEGRAM_COMMANDS_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    if !enabled {
        info!("Telegram commands disabled (TELEGRAM_COMMANDS_ENABLED=false)");
        return;
    }

    let Ok(bot_token) = std::env::var("TELEGRAM_BOT_TOKEN") else {
        warn!("TELEGRAM_COMMANDS_ENABLED=true but TELEGRAM_BOT_TOKEN missing; not starting");
        return;
    };
    let allowed_chat_id: Option<i64> = std::env::var("TELEGRAM_CHAT_ID")
        .ok()
        .and_then(|v| v.parse().ok());
    if allowed_chat_id.is_none() {
        warn!("TELEGRAM_CHAT_ID missing or non-numeric; commands will be ignored from every chat");
    }
    let dm_enabled = std::env::var("TELEGRAM_DM_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    if dm_enabled {
        info!("Telegram DM commands enabled — strangers can DM the bot");
    }

    let http = match reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to build reqwest client: {}", e);
            return;
        }
    };
    let client = TelegramClient { bot_token, http };
    let nonces = Arc::new(NonceStore::new());
    info!("Starting Telegram command handler");

    tokio::spawn(async move {
        let mut offset: i64 = 0;
        loop {
            match client.get_updates(offset).await {
                Ok(updates) => {
                    for upd in updates {
                        offset = upd.update_id + 1;
                        if let Some(msg) = upd.message {
                            handle_message(
                                &state,
                                &client,
                                allowed_chat_id,
                                dm_enabled,
                                &nonces,
                                &msg,
                            )
                            .await;
                        }
                    }
                }
                Err(e) => {
                    warn!("telegram long-poll error: {}", e);
                    tokio::time::sleep(Duration::from_secs(ERROR_BACKOFF_SECS)).await;
                }
            }
        }
    });
}

/// Stores the outstanding `/link` nonce for each chat. Redis is the primary
/// backing store with TTL, so a single API restart no longer invalidates
/// every in-flight `/link`. An in-memory map shadows Redis as a fallback for
/// the (rare) case of Redis being unavailable mid-flow — `/link` still works,
/// just without persistence. Latest `/link` per chat wins.
///
/// Single-use semantics live at the consume step: `peek` is non-destructive
/// so the caller can verify a signature first, then `consume` to invalidate.
struct NonceStore {
    fallback: Mutex<HashMap<i64, PendingNonce>>,
}

#[derive(Clone)]
struct PendingNonce {
    wallet: String,
    nonce: String,
    created_at: Instant,
}

const NONCE_REDIS_PREFIX: &str = "tg:link_pending";

fn nonce_key(chat_id: i64) -> String {
    format!("{}:{}", NONCE_REDIS_PREFIX, chat_id)
}

impl NonceStore {
    fn new() -> Self {
        Self {
            fallback: Mutex::new(HashMap::new()),
        }
    }

    async fn put(
        &self,
        redis: &super::redis::RedisService,
        chat_id: i64,
        wallet_lower: &str,
        nonce: &str,
    ) {
        let value = serde_json::json!({
            "wallet": wallet_lower,
            "nonce": nonce,
        });
        match redis.set(&nonce_key(chat_id), &value, Some(NONCE_TTL_SECS)).await {
            Ok(()) => {}
            Err(e) => {
                warn!(
                    "nonce store: redis put failed, using in-memory fallback: {}",
                    e
                );
                self.put_in_memory(chat_id, wallet_lower, nonce);
            }
        }
    }

    async fn peek(
        &self,
        redis: &super::redis::RedisService,
        chat_id: i64,
    ) -> Option<(String, String)> {
        match redis.get::<serde_json::Value>(&nonce_key(chat_id)).await {
            Ok(Some(v)) => {
                let wallet = v.get("wallet").and_then(|w| w.as_str())?;
                let nonce = v.get("nonce").and_then(|n| n.as_str())?;
                Some((wallet.to_string(), nonce.to_string()))
            }
            Ok(None) => self.peek_in_memory(chat_id),
            Err(e) => {
                warn!(
                    "nonce store: redis peek failed, falling back to memory: {}",
                    e
                );
                self.peek_in_memory(chat_id)
            }
        }
    }

    async fn consume(&self, redis: &super::redis::RedisService, chat_id: i64) {
        if let Err(e) = redis.delete(&nonce_key(chat_id)).await {
            warn!("nonce store: redis delete failed: {}", e);
        }
        // Always purge the in-memory shadow regardless of which path served
        // the value — single-use must hold across both stores.
        self.consume_in_memory(chat_id);
    }

    fn put_in_memory(&self, chat_id: i64, wallet_lower: &str, nonce: &str) {
        let mut guard = self.fallback.lock().expect("nonce store poisoned");
        purge_expired_in_memory(&mut guard);
        guard.insert(
            chat_id,
            PendingNonce {
                wallet: wallet_lower.to_string(),
                nonce: nonce.to_string(),
                created_at: Instant::now(),
            },
        );
    }

    fn peek_in_memory(&self, chat_id: i64) -> Option<(String, String)> {
        let mut guard = self.fallback.lock().expect("nonce store poisoned");
        purge_expired_in_memory(&mut guard);
        guard
            .get(&chat_id)
            .map(|p| (p.wallet.clone(), p.nonce.clone()))
    }

    fn consume_in_memory(&self, chat_id: i64) {
        let mut guard = self.fallback.lock().expect("nonce store poisoned");
        purge_expired_in_memory(&mut guard);
        guard.remove(&chat_id);
    }
}

fn purge_expired_in_memory(map: &mut HashMap<i64, PendingNonce>) {
    let ttl = Duration::from_secs(NONCE_TTL_SECS);
    map.retain(|_, v| v.created_at.elapsed() < ttl);
}

fn fresh_nonce() -> String {
    let hi: u64 = rand::random();
    let lo: u64 = rand::random();
    format!("{:016x}{:016x}", hi, lo)
}

async fn handle_message(
    state: &AppState,
    client: &TelegramClient,
    allowed_chat_id: Option<i64>,
    dm_enabled: bool,
    nonces: &Arc<NonceStore>,
    msg: &Message,
) {
    let is_group_match = Some(msg.chat.id) == allowed_chat_id;
    let is_dm = msg.chat.is_dm();
    if !(is_group_match || (dm_enabled && is_dm)) {
        return;
    }
    // Ignore messages from other bots — they post `/setup_raid` etc. and we
    // don't want to reply "unknown command" to every one.
    if msg.from.as_ref().map(|u| u.is_bot).unwrap_or(false) {
        return;
    }
    let Some(text) = &msg.text else { return };
    let parsed = match parse_command(text) {
        Some(c) => c,
        None => return,
    };

    let reply = match parsed.name.as_str() {
        "start" | "help" => help_text(),
        "status" => status_text(),
        "version" => version_text(state),
        "top" => top_text(state, parsed.args.as_deref()).await,
        "market" => market_text(state, parsed.args.as_deref()).await,
        "mute" => mute_text(state, msg.chat.id, parsed.args.as_deref()).await,
        "unmute" => unmute_text(state, msg.chat.id, parsed.args.as_deref()).await,
        "threshold" => threshold_text(state, msg.chat.id, parsed.args.as_deref()).await,
        "cooldown" => cooldown_text(state, msg.chat.id, parsed.args.as_deref()).await,
        "quiet" => quiet_text(state, msg.chat.id, parsed.args.as_deref()).await,
        "unquiet" => unquiet_text(state, msg.chat.id).await,
        "subscribe" => subscribe_text(state, msg.chat.id, parsed.args.as_deref()).await,
        "unsubscribe" => unsubscribe_text(state, msg.chat.id, parsed.args.as_deref()).await,
        "digest" => digest_text(state).await,
        "quote" => quote_text(state, is_dm, parsed.args.as_deref()).await,
        "config" => config_text(state, msg.chat.id).await,
        "link" => link_text(state, msg.chat.id, nonces, parsed.args.as_deref()).await,
        "verify" => verify_text(state, msg.chat.id, nonces, parsed.args.as_deref()).await,
        "unlink" => unlink_text(state, msg.chat.id).await,
        "notify" => notify_text(state, msg.chat.id, parsed.args.as_deref()).await,
        _ => {
            // Groups: stay silent — third-party bots own commands like
            // /setup_raid and we don't want to spam "unknown command".
            if !is_dm {
                return;
            }
            // DMs: only reply when the command plausibly belongs to us.
            // Skip the well-known third-party command vocabulary (raid bots,
            // volume bots, sniper bots) and anything that doesn't look like
            // a typo of one of our short alphabetic commands.
            if is_third_party_command(&parsed.name) || !looks_like_our_command(&parsed.name) {
                return;
            }
            format!("unknown command /{}. try /help", html_escape(&parsed.name))
        }
    };

    if let Err(e) = client.send(msg.chat.id, &reply, Some(msg.message_id)).await {
        warn!("telegram send failed: {}", e);
    }
}

struct ParsedCommand {
    name: String,
    args: Option<String>,
}

fn parse_command(text: &str) -> Option<ParsedCommand> {
    let trimmed = text.trim();
    let rest = trimmed.strip_prefix('/')?;
    let (head, args) = match rest.split_once(char::is_whitespace) {
        Some((h, a)) => (h, Some(a.trim().to_string())),
        None => (rest, None),
    };
    // /command@BotName style — drop the suffix.
    let name = head.split('@').next().unwrap_or(head).to_ascii_lowercase();
    if name.is_empty() {
        return None;
    }
    Some(ParsedCommand { name, args })
}

/// Vocabulary owned by other Telegram bots that frequently show up in the same
/// chats as the relay44 bot — raid coordinators, sniper bots, volume bots,
/// generic group-engagement bots. We never reply with "unknown command" to
/// any of these, regardless of chat type. Lowercase, no leading slash.
const THIRD_PARTY_COMMANDS: &[&str] = &[
    // Raid / engagement bots.
    "raid",
    "raids",
    "raidstart",
    "raidend",
    "setup_raid",
    "setupraid",
    "newraid",
    "checkraid",
    "endraid",
    "leaderboard",
    "leaderboards",
    "tweet",
    "tweets",
    "retweet",
    "like",
    "comment",
    "shill",
    "engage",
    "engagement",
    "task",
    "tasks",
    "claim",
    // Sniper / trading bots that share command surface area.
    "snipe",
    "swap",
    "send",
    "transfer",
    "approve",
    "withdraw",
    "deposit",
    "limit",
    "long",
    "short",
    "pnl",
    "balance",
    "wallet",
    "wallets",
    "portfolio",
    // Generic group-mod / chat bots.
    "ban",
    "kick",
    "mute_user",
    "warn",
    "rules",
    "captcha",
    "welcome",
    "pin",
    "unpin",
    "report",
    // Common chat slang sometimes typed with a leading slash.
    "gm",
    "gn",
    "lfg",
    "wagmi",
    "ngmi",
    "wen",
    "ca",
];

fn is_third_party_command(name: &str) -> bool {
    THIRD_PARTY_COMMANDS.contains(&name)
}

/// Heuristic for "this could be a typo of one of our commands and a hint is
/// useful." Our entire command surface is purely-alphabetic ASCII, between 3
/// and 11 chars. Anything outside that shape (digits, underscores, very long
/// names, single letters) belongs to another bot or is junk and stays silent.
fn looks_like_our_command(name: &str) -> bool {
    let len = name.len();
    if !(2..=12).contains(&len) {
        return false;
    }
    name.chars().all(|c| c.is_ascii_alphabetic())
}

fn help_text() -> String {
    [
        "<b>Relay44Bot commands</b>",
        "/status — alerter config + health",
        "/version — build sha and uptime",
        "/top [category] — top 5 live markets by opportunity score",
        "/market &lt;slug&gt; — price, depth, and link for one market",
        "/config — show this chat's overrides + linked wallet",
        "/mute &lt;slug_or_id&gt; — suppress alerts for a market",
        "/unmute &lt;slug_or_id&gt; — undo a mute",
        "/threshold &lt;pct&gt; — per-chat alert threshold (e.g. 3 or 3.5%)",
        "/cooldown &lt;secs&gt; — per-chat alert cooldown (60-3600)",
        "/quiet &lt;hours&gt; — snooze digest for this chat (0.25-168)",
        "/unquiet — cancel an active quiet window",
        "/subscribe &lt;kind&gt; — subscribe to a signal kind (probability_shift, volume_spike, new_market)",
        "/unsubscribe &lt;kind&gt; — remove a signal kind",
        "/digest — on-demand: drain bus and send current top signals",
        "/quote &lt;slug&gt; &lt;outcome&gt; &lt;buy|sell&gt; [size] — DM-only: best-execution quote",
        "/link &lt;0x…&gt; — start read-only wallet binding",
        "/verify &lt;signature&gt; — finish wallet binding",
        "/unlink — drop linked wallet",
        "/notify &lt;slug&gt; &lt;pct&gt; — one-shot DM when a market crosses pct",
        "/help — this list",
        "",
        "<i>Binding a wallet grants no spending authority. Trade execution is not available.</i>",
    ]
    .join("\n")
}

fn status_text() -> String {
    let enabled = env_bool("TELEGRAM_ALERTS_ENABLED");
    let threshold = env_or("PROB_ALERT_THRESHOLD_PCT", "5");
    let cooldown = env_or("PROB_ALERT_COOLDOWN_SECS", "300");
    let min_liquidity = env_or("PROB_ALERT_MIN_LIQUIDITY_USD", "0");
    let min_price = env_or("PROB_ALERT_MIN_PRICE", "0.05");

    let state_label = if enabled { "ON" } else { "OFF" };
    format!(
        "<b>Relay44Bot status</b>\n\
         Alerts: <b>{state}</b>\n\
         threshold: {threshold}%\n\
         cooldown: {cooldown}s\n\
         min liquidity: ${min_liq}\n\
         min price: {min_px}",
        state = state_label,
        threshold = threshold,
        cooldown = cooldown,
        min_liq = min_liquidity,
        min_px = min_price,
    )
}

/// `/version` — what commit is live and how long the process has been up.
/// Used to verify deploys without leaving Telegram. The short SHA matches
/// the one shown in the Render dashboard and on GitHub.
fn version_text(state: &AppState) -> String {
    let pkg_version = env!("CARGO_PKG_VERSION");
    let sha = build_short_sha();
    let uptime = format_uptime(state.metrics.get_metrics().uptime_seconds);
    format!(
        "<b>Relay44Bot version</b>\n\
         build: <code>{}</code> ({})\n\
         uptime: {}",
        sha, pkg_version, uptime,
    )
}

/// Resolve the short commit SHA from any of the build-time env vars Render
/// (or a manual deploy) might set. Returns "unknown" when none are present
/// so the command still answers cleanly in local development.
fn build_short_sha() -> String {
    resolve_short_sha(|key| std::env::var(key).ok())
}

/// Pure form for testability — env-var reads are global and would race
/// across parallel tests. Caller supplies the lookup fn.
fn resolve_short_sha<F: Fn(&str) -> Option<String>>(lookup: F) -> String {
    for key in ["RENDER_GIT_COMMIT", "GIT_COMMIT", "BUILD_SHA", "GIT_SHA"] {
        if let Some(v) = lookup(key) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                let len = trimmed.len().min(7);
                return trimmed[..len].to_string();
            }
        }
    }
    "unknown".to_string()
}

/// Render an uptime in seconds as `1d 2h 3m`, dropping leading zero units.
/// Used by `/version` so the chat output stays compact.
fn format_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    let seconds = secs % 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

async fn top_text(state: &AppState, args: Option<&str>) -> String {
    let category_filter = args.map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty());
    let pool = state.db.pool();
    let rows: Result<Vec<(String, String, String, f64, Option<f64>)>, sqlx::Error> = sqlx::query_as(
        "SELECT question, slug, category, opportunity_score, liquidity_usdc::double precision \
         FROM polymarket_scanned_markets \
         WHERE active = true \
           AND opportunity_type IS NOT NULL \
           AND ($1::text IS NULL OR category = $1) \
         ORDER BY opportunity_score DESC NULLS LAST \
         LIMIT 5",
    )
    .bind(category_filter.as_deref())
    .fetch_all(pool)
    .await;

    match rows {
        Ok(r) if r.is_empty() => "no markets match".to_string(),
        Ok(r) => {
            let header = match category_filter {
                Some(c) => format!("<b>Top 5 — {}</b>", html_escape(&c)),
                None => "<b>Top 5 markets</b>".to_string(),
            };
            let lines: Vec<String> = r
                .into_iter()
                .enumerate()
                .map(|(i, (q, slug, cat, score, liq))| {
                    let q = html_escape(&q);
                    let url = format!("https://relay44.com/markets/by-slug/polymarket/{}", slug);
                    let liq_s = liq.map(format_money).unwrap_or_else(|| "—".to_string());
                    format!(
                        "{}. <a href=\"{}\">{}</a>\n   score {:.2} · {} · liq {}",
                        i + 1,
                        url,
                        q,
                        score,
                        html_escape(&cat),
                        liq_s,
                    )
                })
                .collect();
            format!("{}\n{}", header, lines.join("\n"))
        }
        Err(e) => {
            warn!("/top query failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn market_text(state: &AppState, args: Option<&str>) -> String {
    let slug = match args.and_then(|s| s.split_whitespace().next()) {
        Some(s) => s.to_string(),
        None => return "usage: /market &lt;slug&gt;".to_string(),
    };

    let pool = state.db.pool();
    let row: Result<Option<(String, String, String, f64, f64, i32, Option<f64>, Option<f64>)>, sqlx::Error> =
        sqlx::query_as(
            "SELECT question, slug, category, yes_price, no_price, spread_bps, \
                    liquidity_usdc::double precision, volume_usdc::double precision \
             FROM polymarket_scanned_markets \
             WHERE slug = $1 \
             LIMIT 1",
        )
        .bind(&slug)
        .fetch_optional(pool)
        .await;

    match row {
        Ok(Some((question, slug, category, yes, no, spread_bps, liq, vol))) => {
            let url = format!("https://relay44.com/markets/by-slug/polymarket/{}", slug);
            let liq_s = liq.map(format_money).unwrap_or_else(|| "—".to_string());
            let vol_s = vol.map(format_money).unwrap_or_else(|| "—".to_string());
            format!(
                "<a href=\"{}\">{}</a>\n\
                 YES {:.1}¢ · NO {:.1}¢ · spread {}bps\n\
                 {} · liq {} · vol {}",
                url,
                html_escape(&question),
                yes * 100.0,
                no * 100.0,
                spread_bps,
                html_escape(&category),
                liq_s,
                vol_s,
            )
        }
        Ok(None) => format!("no market with slug <code>{}</code>", html_escape(&slug)),
        Err(e) => {
            warn!("/market query failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn mute_text(state: &AppState, chat_id: i64, args: Option<&str>) -> String {
    let market = match args.and_then(|s| s.split_whitespace().next()) {
        Some(s) => s.to_string(),
        None => return "usage: /mute &lt;slug_or_market_id&gt;".to_string(),
    };
    match tg_chat_config::add_muted_market(state.db.pool(), chat_id, &market).await {
        Ok(()) => format!("muted <code>{}</code>", html_escape(&market)),
        Err(e) => {
            warn!("/mute persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn unmute_text(state: &AppState, chat_id: i64, args: Option<&str>) -> String {
    let market = match args.and_then(|s| s.split_whitespace().next()) {
        Some(s) => s.to_string(),
        None => return "usage: /unmute &lt;slug_or_market_id&gt;".to_string(),
    };
    match tg_chat_config::remove_muted_market(state.db.pool(), chat_id, &market).await {
        Ok(()) => format!("unmuted <code>{}</code>", html_escape(&market)),
        Err(e) => {
            warn!("/unmute persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn threshold_text(state: &AppState, chat_id: i64, args: Option<&str>) -> String {
    let raw = match args.and_then(|s| s.split_whitespace().next()) {
        Some(s) => s.to_string(),
        None => return "usage: /threshold &lt;pct&gt; (e.g. 3 or 3.5%)".to_string(),
    };
    let parsed = match tg_chat_config::parse_threshold_arg(&raw) {
        Ok(v) => v,
        Err(e) => return html_escape(&e),
    };
    match tg_chat_config::set_threshold_override(state.db.pool(), chat_id, parsed).await {
        Ok(()) => format!("threshold set to {:.2}%", parsed),
        Err(e) => {
            warn!("/threshold persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn cooldown_text(state: &AppState, chat_id: i64, args: Option<&str>) -> String {
    let raw = match args.and_then(|s| s.split_whitespace().next()) {
        Some(s) => s.to_string(),
        None => return "usage: /cooldown &lt;secs&gt; (60-3600)".to_string(),
    };
    let parsed = match tg_chat_config::parse_cooldown_arg(&raw) {
        Ok(v) => v,
        Err(e) => return html_escape(&e),
    };
    match tg_chat_config::set_cooldown_override(state.db.pool(), chat_id, parsed).await {
        Ok(()) => format!("cooldown set to {}s", parsed),
        Err(e) => {
            warn!("/cooldown persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn quiet_text(state: &AppState, chat_id: i64, args: Option<&str>) -> String {
    let raw = match args.and_then(|s| s.split_whitespace().next()) {
        Some(s) => s.to_string(),
        None => return "usage: /quiet &lt;hours&gt; (0.25-168)".to_string(),
    };
    let hours = match tg_chat_config::parse_quiet_hours_arg(&raw) {
        Ok(v) => v,
        Err(e) => return html_escape(&e),
    };
    match tg_chat_config::set_quiet_until(state.db.pool(), chat_id, hours).await {
        Ok(until) => format!(
            "quiet until {}",
            html_escape(&until.format("%Y-%m-%d %H:%M UTC").to_string())
        ),
        Err(e) => {
            warn!("/quiet persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn unquiet_text(state: &AppState, chat_id: i64) -> String {
    match tg_chat_config::clear_quiet_until(state.db.pool(), chat_id).await {
        Ok(()) => "quiet window cleared".to_string(),
        Err(e) => {
            warn!("/unquiet persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn subscribe_text(state: &AppState, chat_id: i64, args: Option<&str>) -> String {
    let raw = match args.and_then(|s| s.split_whitespace().next()) {
        Some(s) => s.to_string(),
        None => {
            return format!(
                "usage: /subscribe &lt;kind&gt; ({})",
                tg_chat_config::VALID_KINDS.join(", ")
            );
        }
    };
    let kind = match tg_chat_config::parse_kind_arg(&raw) {
        Ok(k) => k,
        Err(e) => return html_escape(&e),
    };
    match tg_chat_config::add_subscribed_kind(state.db.pool(), chat_id, kind).await {
        Ok(()) => format!("subscribed to <code>{}</code>", kind),
        Err(e) => {
            warn!("/subscribe persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn unsubscribe_text(state: &AppState, chat_id: i64, args: Option<&str>) -> String {
    let raw = match args.and_then(|s| s.split_whitespace().next()) {
        Some(s) => s.to_string(),
        None => {
            return format!(
                "usage: /unsubscribe &lt;kind&gt; ({})",
                tg_chat_config::VALID_KINDS.join(", ")
            );
        }
    };
    let kind = match tg_chat_config::parse_kind_arg(&raw) {
        Ok(k) => k,
        Err(e) => return html_escape(&e),
    };
    match tg_chat_config::remove_subscribed_kind(state.db.pool(), chat_id, kind).await {
        Ok(()) => format!("unsubscribed from <code>{}</code>", kind),
        Err(e) => {
            warn!("/unsubscribe persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

/// Drains the alert bus immediately and returns a formatted digest. Bypasses
/// the scheduler's cooldown and send path — the reply lands in the /digest
/// caller's chat as a normal command response. Useful to peek at what would
/// be sent on the next scheduled tick without waiting.
async fn digest_text(state: &AppState) -> String {
    use crate::services::digest_scheduler;
    let drained = state.alert_bus.drain().await;
    if drained.is_empty() {
        return "bus empty — nothing queued".to_string();
    }
    let now = crate::services::alert_bus::now_secs();
    let top_n = std::env::var("DIGEST_TOP_N")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3)
        .max(1);
    let selected = digest_scheduler::select_top_signals(
        drained,
        top_n,
        0,
        now,
        &std::collections::HashMap::new(),
    );
    if selected.is_empty() {
        return "no signals after dedup".to_string();
    }
    digest_scheduler::format_digest(&selected)
}

/// Build a best-execution quote across linked venues for a market. Read-only
/// — does NOT place or sign any order. Gated to DM chats because the reply
/// may reveal which markets the user is watching.
async fn quote_text(state: &AppState, is_dm: bool, args: Option<&str>) -> String {
    if !is_dm {
        return "/quote is available in DM only — open a private chat with the bot.".to_string();
    }
    let (slug, outcome, side, size) = match parse_quote_args(args) {
        Ok(v) => v,
        Err(e) => return html_escape(&e),
    };
    let decision = match crate::services::smart_router::route_order(
        state, &slug, &outcome, &side, size,
    )
    .await
    {
        Ok(d) => d,
        Err(e) => return format!("no quote: {}", html_escape(&e)),
    };

    let is_buy = side == "buy";
    let chosen_px = if is_buy {
        decision.chosen.effective_buy
    } else {
        decision.chosen.effective_sell
    };
    let chosen_px_s = chosen_px
        .map(|p| format!("{:.2}¢", p * 100.0))
        .unwrap_or_else(|| "—".to_string());
    let mut out = String::new();
    out.push_str(&format!(
        "<b>Best {} — {}</b>\n",
        html_escape(&side),
        html_escape(&slug)
    ));
    out.push_str(&format!(
        "<b>{}</b> · {} (fee {}bps)\n",
        html_escape(&decision.chosen.provider),
        chosen_px_s,
        decision.chosen.fee_bps
    ));
    if !decision.alternatives.is_empty() {
        out.push_str("vs:\n");
        for alt in &decision.alternatives {
            let px = if is_buy {
                alt.effective_buy
            } else {
                alt.effective_sell
            };
            let px_s = px
                .map(|p| format!("{:.2}¢", p * 100.0))
                .unwrap_or_else(|| "—".to_string());
            out.push_str(&format!(
                "  {} · {}\n",
                html_escape(&alt.provider),
                px_s
            ));
        }
    }
    if decision.savings_bps > 0 {
        out.push_str(&format!("savings: {}bps", decision.savings_bps));
    }
    out.push_str(
        "\n\n<i>Quote only — no order was placed. Trade execution from Telegram is not yet available.</i>",
    );
    out
}

fn parse_quote_args(args: Option<&str>) -> Result<(String, String, String, f64), String> {
    let raw = args.ok_or_else(|| "usage: /quote <slug> <outcome> <buy|sell> [size]".to_string())?;
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() < 3 {
        return Err("usage: /quote <slug> <outcome> <buy|sell> [size]".to_string());
    }
    let slug = parts[0].to_string();
    let outcome = parts[1].to_string();
    let side = parts[2].to_ascii_lowercase();
    if side != "buy" && side != "sell" {
        return Err(format!("side must be 'buy' or 'sell', got '{}'", parts[2]));
    }
    let size = match parts.get(3) {
        Some(s) => s
            .parse::<f64>()
            .map_err(|_| format!("'{}' is not a number", s))?,
        None => 1.0,
    };
    if !(size > 0.0 && size.is_finite()) {
        return Err("size must be a positive finite number".to_string());
    }
    Ok((slug, outcome, side, size))
}

async fn config_text(state: &AppState, chat_id: i64) -> String {
    let cfg = match tg_chat_config::fetch(state.db.pool(), chat_id).await {
        Ok(v) => v,
        Err(e) => {
            warn!("/config fetch failed: {}", e);
            return "query failed".to_string();
        }
    };
    let mut out = vec!["<b>chat config</b>".to_string()];
    for line in tg_chat_config::dump_config_lines(cfg.as_ref()) {
        out.push(html_escape(&line));
    }
    out.join("\n")
}

async fn link_text(
    state: &AppState,
    chat_id: i64,
    nonces: &Arc<NonceStore>,
    args: Option<&str>,
) -> String {
    let raw = match args.and_then(|s| s.split_whitespace().next()) {
        Some(s) => s.to_string(),
        None => return "usage: /link &lt;0x…&gt;".to_string(),
    };
    let wallet_lower = match tg_chat_config::validate_and_normalize_evm_address(&raw) {
        Ok(w) => w,
        Err(e) => return html_escape(&e),
    };
    let nonce = fresh_nonce();
    let message = tg_chat_config::link_message(chat_id, &wallet_lower, &nonce);
    nonces.put(&state.redis, chat_id, &wallet_lower, &nonce).await;
    format!(
        "Sign this message with your wallet and send <code>/verify &lt;signature&gt;</code> within 10 min:\n\n<code>{}</code>\n\n<i>Read-only binding. No spending authority is granted.</i>",
        html_escape(&message)
    )
}

async fn verify_text(
    state: &AppState,
    chat_id: i64,
    nonces: &Arc<NonceStore>,
    args: Option<&str>,
) -> String {
    let signature = match args.and_then(|s| s.split_whitespace().next()) {
        Some(s) => s.to_string(),
        None => return "usage: /verify &lt;signature&gt;".to_string(),
    };
    let (expected_wallet, nonce) = match nonces.peek(&state.redis, chat_id).await {
        Some(pair) => pair,
        None => {
            return "no pending /link for this chat. run /link &lt;0x…&gt; first.".to_string();
        }
    };
    let message = tg_chat_config::link_message(chat_id, &expected_wallet, &nonce);
    let recovered = match tg_chat_config::recover_personal_sign(&message, &signature) {
        Ok(a) => a,
        Err(e) => return format!("signature invalid: {}", html_escape(&e)),
    };
    if recovered != expected_wallet {
        return "signature does not match the wallet you linked".to_string();
    }

    // Nonce is single-use; consume on success. Keeps replay closed.
    nonces.consume(&state.redis, chat_id).await;

    // If the existing users table has a row for this wallet, we'd resolve an
    // id here. Today the table is keyed on a Solana-length `wallet` primary
    // key, so there is no UUID to resolve. Leaving linked_user_id NULL is
    // correct until an EVM-native user identifier is introduced.
    let linked_user_id = None;

    match tg_chat_config::set_linked_wallet(
        state.db.pool(),
        chat_id,
        &expected_wallet,
        linked_user_id,
    )
    .await
    {
        Ok(()) => format!(
            "wallet <code>{}</code> linked (read-only).",
            html_escape(&expected_wallet)
        ),
        Err(e) => {
            warn!("/verify persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn unlink_text(state: &AppState, chat_id: i64) -> String {
    match tg_chat_config::clear_linked_wallet(state.db.pool(), chat_id).await {
        Ok(()) => "wallet unlinked".to_string(),
        Err(e) => {
            warn!("/unlink persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

/// `/notify <slug> <pct>` — create a one-shot DM alert when a market crosses
/// a threshold. `/notify list` shows active rules; `/notify clear` deletes
/// every active rule for the chat.
async fn notify_text(state: &AppState, chat_id: i64, args: Option<&str>) -> String {
    let raw = args.map(|s| s.trim()).unwrap_or_default();
    if raw.is_empty() {
        return notify_usage();
    }

    let mut iter = raw.split_whitespace();
    let head = iter.next().unwrap_or_default();
    match head {
        "list" => return notify_list_text(state, chat_id).await,
        "clear" => return notify_clear_text(state, chat_id).await,
        _ => {}
    }

    let slug = head;
    let threshold_arg = iter.next();
    if iter.next().is_some() {
        return notify_usage();
    }
    let threshold_arg = match threshold_arg {
        Some(t) => t,
        None => return notify_usage(),
    };

    let threshold = match crate::services::notify_rules::parse_threshold_arg(threshold_arg) {
        Ok(t) => t,
        Err(e) => return html_escape(&e),
    };

    let pool = state.db.pool();
    let resolved = match resolve_notify_market(pool, slug).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    let (venue, canonical_slug, baseline) = resolved;

    if (baseline - threshold).abs() < 1e-9 {
        return format!(
            "current price for <code>{}</code> is already at the threshold ({:.0}%); pick a different value",
            html_escape(&canonical_slug),
            threshold * 100.0
        );
    }

    match crate::services::notify_rules::create(
        pool,
        chat_id,
        &venue,
        &canonical_slug,
        threshold,
        baseline,
    )
    .await
    {
        Ok(_id) => {
            let direction = if baseline < threshold {
                "rises to"
            } else {
                "falls to"
            };
            format!(
                "watching <b>{}</b> on {}. baseline {:.0}% \u{2192} ping you when it {} <b>{:.0}%</b>.",
                html_escape(&canonical_slug),
                super::telegram_format::venue_title(&venue),
                baseline * 100.0,
                direction,
                threshold * 100.0,
            )
        }
        Err(e) => {
            warn!("/notify persist failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn notify_list_text(state: &AppState, chat_id: i64) -> String {
    match crate::services::notify_rules::list_active_for_chat(state.db.pool(), chat_id).await {
        Ok(rules) if rules.is_empty() => "no active /notify rules. set one with /notify &lt;slug&gt; &lt;pct&gt;".to_string(),
        Ok(rules) => {
            let mut lines = Vec::with_capacity(rules.len() + 1);
            lines.push(format!("<b>active /notify rules ({})</b>", rules.len()));
            for r in rules {
                lines.push(format!(
                    "• {} <code>{}</code> — baseline {:.0}% \u{2192} {:.0}%",
                    super::telegram_format::venue_title(&r.venue),
                    html_escape(&r.slug),
                    r.baseline_price * 100.0,
                    r.threshold * 100.0,
                ));
            }
            lines.join("\n")
        }
        Err(e) => {
            warn!("/notify list failed: {}", e);
            "query failed".to_string()
        }
    }
}

async fn notify_clear_text(state: &AppState, chat_id: i64) -> String {
    match crate::services::notify_rules::clear_for_chat(state.db.pool(), chat_id).await {
        Ok(0) => "no active /notify rules to clear".to_string(),
        Ok(n) => format!("cleared {} /notify rule(s)", n),
        Err(e) => {
            warn!("/notify clear failed: {}", e);
            "query failed".to_string()
        }
    }
}

fn notify_usage() -> String {
    "usage:\n\
     /notify &lt;slug&gt; &lt;pct&gt; — DM you when the market crosses pct (e.g. 60%)\n\
     /notify list — show active rules\n\
     /notify clear — clear every active rule for this chat"
        .to_string()
}

/// Look up a market by slug across both scanner tables and return
/// (venue, canonical_slug, current_yes_price). Errors as a user-facing
/// HTML-escaped string the command handler can return verbatim.
async fn resolve_notify_market(
    pool: &sqlx::PgPool,
    slug: &str,
) -> Result<(String, String, f64), String> {
    let slug_trimmed = slug.trim();
    if slug_trimmed.is_empty() {
        return Err("slug required".to_string());
    }

    let poly: Option<(String, f64)> = sqlx::query_as(
        "SELECT slug, yes_price::double precision \
         FROM polymarket_scanned_markets \
         WHERE active = TRUE AND slug = $1 LIMIT 1",
    )
    .bind(slug_trimmed)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        warn!("/notify resolve poly query failed: {}", e);
        "query failed".to_string()
    })?;
    if let Some((s, price)) = poly {
        return Ok(("polymarket".to_string(), s, price));
    }

    let lim: Option<(String, f64)> = sqlx::query_as(
        "SELECT slug, yes_price::double precision \
         FROM limitless_scanned_markets \
         WHERE active = TRUE AND slug = $1 LIMIT 1",
    )
    .bind(slug_trimmed)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        warn!("/notify resolve limitless query failed: {}", e);
        "query failed".to_string()
    })?;
    if let Some((s, price)) = lim {
        return Ok(("limitless".to_string(), s, price));
    }

    Err(format!(
        "market <code>{}</code> not found in active scanner tables",
        html_escape(slug_trimmed)
    ))
}

fn env_bool(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false)
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn format_money(v: f64) -> String {
    let abs = v.abs();
    if abs >= 1_000_000.0 {
        format!("${:.1}M", v / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("${:.1}k", v / 1_000.0)
    } else {
        format!("${:.0}", v)
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ── Telegram API types ──

#[derive(Debug, Deserialize)]
struct Update {
    update_id: i64,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Debug, Deserialize)]
struct Message {
    message_id: i64,
    chat: Chat,
    #[serde(default)]
    from: Option<User>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct User {
    #[serde(default)]
    is_bot: bool,
}

#[derive(Debug, Deserialize)]
struct Chat {
    id: i64,
    /// Telegram chat type: "private" for DMs, otherwise "group", "supergroup",
    /// "channel". Optional because some legacy updates don't include it.
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

impl Chat {
    fn is_dm(&self) -> bool {
        self.kind.as_deref() == Some("private")
    }
}

#[derive(Debug, Deserialize)]
struct ApiResult<T> {
    ok: bool,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    result: Option<T>,
}

struct TelegramClient {
    bot_token: String,
    http: reqwest::Client,
}

impl TelegramClient {
    async fn get_updates(&self, offset: i64) -> Result<Vec<Update>, String> {
        #[derive(Serialize)]
        struct Req<'a> {
            offset: i64,
            timeout: u64,
            allowed_updates: &'a [&'a str],
        }
        let url = format!("https://api.telegram.org/bot{}/getUpdates", self.bot_token);
        let resp = self
            .http
            .post(&url)
            .json(&Req {
                offset,
                timeout: POLL_TIMEOUT_SECS,
                allowed_updates: &["message"],
            })
            .send()
            .await
            .map_err(|e| format!("request: {}", e))?;
        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("telegram {}: {}", code, body));
        }
        let body: ApiResult<Vec<Update>> = resp.json().await.map_err(|e| format!("json: {}", e))?;
        if !body.ok {
            return Err(format!(
                "telegram error: {}",
                body.description.unwrap_or_default()
            ));
        }
        Ok(body.result.unwrap_or_default())
    }

    async fn send(&self, chat_id: i64, text: &str, reply_to: Option<i64>) -> Result<(), String> {
        #[derive(Serialize)]
        struct Req<'a> {
            chat_id: i64,
            text: &'a str,
            parse_mode: &'a str,
            disable_web_page_preview: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            reply_to_message_id: Option<i64>,
        }
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let payload = Req {
            chat_id,
            text,
            parse_mode: "HTML",
            disable_web_page_preview: true,
            reply_to_message_id: reply_to,
        };
        super::telegram_format::send_with_retry("sendMessage", || {
            self.http.post(&url).json(&payload).send()
        })
        .await
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_command() {
        let c = parse_command("/status").unwrap();
        assert_eq!(c.name, "status");
        assert!(c.args.is_none());
    }

    #[test]
    fn parse_command_with_args() {
        let c = parse_command("/top politics").unwrap();
        assert_eq!(c.name, "top");
        assert_eq!(c.args.as_deref(), Some("politics"));
    }

    #[test]
    fn parse_command_with_bot_suffix() {
        let c = parse_command("/market@Relay44Bot some-slug").unwrap();
        assert_eq!(c.name, "market");
        assert_eq!(c.args.as_deref(), Some("some-slug"));
    }

    #[test]
    fn parse_command_uppercase_normalized() {
        let c = parse_command("/Status").unwrap();
        assert_eq!(c.name, "status");
    }

    #[test]
    fn parse_non_command_returns_none() {
        assert!(parse_command("hello").is_none());
        assert!(parse_command("").is_none());
        assert!(parse_command("/").is_none());
    }

    #[test]
    fn third_party_raid_commands_are_silenced() {
        for cmd in [
            "raid",
            "setup_raid",
            "setupraid",
            "raidstart",
            "leaderboard",
            "tweet",
            "shill",
        ] {
            assert!(
                is_third_party_command(cmd),
                "raid-vocabulary command {cmd} should be silenced"
            );
        }
    }

    #[test]
    fn third_party_sniper_and_chat_commands_are_silenced() {
        for cmd in ["snipe", "swap", "ban", "captcha", "gm", "lfg"] {
            assert!(
                is_third_party_command(cmd),
                "third-party command {cmd} should be silenced"
            );
        }
    }

    #[test]
    fn our_commands_are_not_in_third_party_list() {
        for cmd in [
            "start",
            "help",
            "status",
            "top",
            "market",
            "config",
            "mute",
            "unmute",
            "threshold",
            "cooldown",
            "quiet",
            "unquiet",
            "subscribe",
            "unsubscribe",
            "digest",
            "quote",
            "link",
            "verify",
            "unlink",
        ] {
            assert!(
                !is_third_party_command(cmd),
                "{cmd} is one of our commands, must not be in the third-party list"
            );
        }
    }

    #[test]
    fn typo_heuristic_matches_short_alphabetic_commands() {
        // Typos of our actual commands should still get a hint.
        assert!(looks_like_our_command("statu"));
        assert!(looks_like_our_command("hlep"));
        assert!(looks_like_our_command("config"));
        assert!(looks_like_our_command("toop"));
    }

    #[test]
    fn typo_heuristic_rejects_third_party_shapes() {
        // Underscores, digits, very long, very short → not a typo of ours.
        assert!(!looks_like_our_command("setup_raid"));
        assert!(!looks_like_our_command("raid123"));
        assert!(!looks_like_our_command("a"));
        assert!(!looks_like_our_command("supercalifragilistic"));
        assert!(!looks_like_our_command(""));
    }

    #[test]
    fn html_escape_covers_risky_chars() {
        assert_eq!(html_escape("A<B>&C"), "A&lt;B&gt;&amp;C");
    }

    #[test]
    fn money_formatter_spans_ranges() {
        assert_eq!(format_money(42.0), "$42");
        assert_eq!(format_money(4_200.0), "$4.2k");
        assert_eq!(format_money(4_200_000.0), "$4.2M");
    }

    #[test]
    fn help_text_lists_core_commands() {
        let h = help_text();
        assert!(h.contains("/status"));
        assert!(h.contains("/top"));
        assert!(h.contains("/market"));
        assert!(h.contains("/help"));
        assert!(h.contains("/mute"));
        assert!(h.contains("/threshold"));
        assert!(h.contains("/cooldown"));
        assert!(h.contains("/link"));
        assert!(h.contains("/verify"));
        assert!(h.contains("/unlink"));
        assert!(h.contains("/config"));
        assert!(h.contains("/version"));
    }

    #[test]
    fn fresh_nonce_is_hex_and_unique_enough() {
        let a = fresh_nonce();
        let b = fresh_nonce();
        assert_eq!(a.len(), 32);
        assert_eq!(b.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn nonce_store_in_memory_roundtrip() {
        let store = NonceStore::new();
        store.put_in_memory(7, "0xabc", "n1");
        let peeked = store.peek_in_memory(7);
        assert_eq!(
            peeked.as_ref().map(|(w, n)| (w.as_str(), n.as_str())),
            Some(("0xabc", "n1"))
        );
        // peek is non-destructive.
        assert!(store.peek_in_memory(7).is_some());
        // consume invalidates.
        store.consume_in_memory(7);
        assert!(store.peek_in_memory(7).is_none());
    }

    #[test]
    fn nonce_store_in_memory_returns_none_for_unknown_chat() {
        let store = NonceStore::new();
        store.put_in_memory(7, "0xabc", "n1");
        assert!(store.peek_in_memory(99).is_none());
    }

    #[test]
    fn nonce_store_latest_link_overwrites_previous() {
        // Simulates a user re-running /link with a different wallet in the same
        // chat. We keep latest-wins semantics so the new /link is what
        // /verify will check against.
        let store = NonceStore::new();
        store.put_in_memory(7, "0xaaa", "first-nonce");
        store.put_in_memory(7, "0xbbb", "second-nonce");
        let peeked = store.peek_in_memory(7).unwrap();
        assert_eq!(peeked.0, "0xbbb");
        assert_eq!(peeked.1, "second-nonce");
    }

    #[test]
    fn format_uptime_drops_leading_zero_units() {
        assert_eq!(format_uptime(0), "0s");
        assert_eq!(format_uptime(45), "45s");
        assert_eq!(format_uptime(60), "1m 0s");
        assert_eq!(format_uptime(125), "2m 5s");
        assert_eq!(format_uptime(3_600), "1h 0m");
        assert_eq!(format_uptime(3_725), "1h 2m");
        assert_eq!(format_uptime(86_400), "1d 0h 0m");
        assert_eq!(format_uptime(90_061), "1d 1h 1m");
    }

    #[test]
    fn resolve_short_sha_truncates_to_seven_chars() {
        let lookup = |k: &str| {
            (k == "RENDER_GIT_COMMIT").then(|| "abcdef0123456789".to_string())
        };
        assert_eq!(resolve_short_sha(lookup), "abcdef0");
    }

    #[test]
    fn resolve_short_sha_falls_through_to_next_var() {
        // Render not set; manual GIT_COMMIT is. Caller falls through.
        let lookup = |k: &str| match k {
            "GIT_COMMIT" => Some("deadbeef".to_string()),
            _ => None,
        };
        assert_eq!(resolve_short_sha(lookup), "deadbee");
    }

    #[test]
    fn resolve_short_sha_returns_unknown_when_no_var_set() {
        let lookup = |_: &str| None;
        assert_eq!(resolve_short_sha(lookup), "unknown");
    }

    #[test]
    fn resolve_short_sha_skips_blank_values() {
        // Render sometimes injects "" for unset values; treat as absent.
        let lookup = |k: &str| match k {
            "RENDER_GIT_COMMIT" => Some("   ".to_string()),
            "GIT_COMMIT" => Some("real-sha-here".to_string()),
            _ => None,
        };
        assert_eq!(resolve_short_sha(lookup), "real-sh");
    }

    #[test]
    fn resolve_short_sha_handles_short_sha_intact() {
        let lookup = |k: &str| (k == "RENDER_GIT_COMMIT").then(|| "abc".to_string());
        assert_eq!(resolve_short_sha(lookup), "abc");
    }

    #[test]
    fn nonce_redis_key_includes_chat_id_under_link_pending_prefix() {
        // Keys live under tg:link_pending:* so an ops engineer can easily list
        // outstanding /link flows in production via Redis SCAN.
        assert_eq!(nonce_key(123), "tg:link_pending:123");
        assert_eq!(nonce_key(-1003504563338), "tg:link_pending:-1003504563338");
    }

    #[test]
    fn quote_parses_slug_outcome_side() {
        let (slug, outcome, side, size) =
            parse_quote_args(Some("btc-100k-by-eoy YES buy")).unwrap();
        assert_eq!(slug, "btc-100k-by-eoy");
        assert_eq!(outcome, "YES");
        assert_eq!(side, "buy");
        assert!((size - 1.0).abs() < 1e-9);
    }

    #[test]
    fn quote_parses_optional_size() {
        let (_, _, _, size) = parse_quote_args(Some("slug YES sell 250")).unwrap();
        assert!((size - 250.0).abs() < 1e-9);
    }

    #[test]
    fn quote_normalizes_side_case() {
        let (_, _, side, _) = parse_quote_args(Some("slug YES BUY")).unwrap();
        assert_eq!(side, "buy");
    }

    #[test]
    fn quote_rejects_bad_side() {
        assert!(parse_quote_args(Some("slug YES maybe")).is_err());
    }

    #[test]
    fn quote_rejects_missing_fields() {
        assert!(parse_quote_args(None).is_err());
        assert!(parse_quote_args(Some("slug YES")).is_err());
    }

    #[test]
    fn quote_rejects_bad_size() {
        assert!(parse_quote_args(Some("slug YES buy -5")).is_err());
        assert!(parse_quote_args(Some("slug YES buy abc")).is_err());
        assert!(parse_quote_args(Some("slug YES buy 0")).is_err());
    }
}
