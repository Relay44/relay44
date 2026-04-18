//! Telegram command handler.
//!
//! Long-polls the Telegram Bot API (`getUpdates`) and dispatches slash
//! commands sent into the configured chat. Read-only commands in this
//! first cut: `/status`, `/help`, `/top [category]`, `/market <slug>`.
//! State-changing commands (mute / threshold override) land in Phase 2b
//! once `tg_chat_config` migration is in place.
//!
//! Messages from chats other than `TELEGRAM_CHAT_ID` are ignored — the
//! bot isn't open to the public yet.

use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::AppState;

const POLL_TIMEOUT_SECS: u64 = 30;
const HTTP_TIMEOUT_SECS: u64 = POLL_TIMEOUT_SECS + 10;
const ERROR_BACKOFF_SECS: u64 = 5;

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
    info!("Starting Telegram command handler");

    tokio::spawn(async move {
        let mut offset: i64 = 0;
        loop {
            match client.get_updates(offset).await {
                Ok(updates) => {
                    for upd in updates {
                        offset = upd.update_id + 1;
                        if let Some(msg) = upd.message {
                            handle_message(&state, &client, allowed_chat_id, &msg).await;
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

async fn handle_message(
    state: &AppState,
    client: &TelegramClient,
    allowed_chat_id: Option<i64>,
    msg: &Message,
) {
    if allowed_chat_id.is_some() && Some(msg.chat.id) != allowed_chat_id {
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
        "top" => top_text(state, parsed.args.as_deref()).await,
        "market" => market_text(state, parsed.args.as_deref()).await,
        _ => format!("unknown command /{}. try /help", parsed.name),
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

fn help_text() -> String {
    [
        "<b>Relay44Bot commands</b>",
        "/status — alerter config + health",
        "/top [category] — top 5 live markets by opportunity score",
        "/market &lt;slug&gt; — price, depth, and link for one market",
        "/help — this list",
        "",
        "<i>Write-commands (/mute, /threshold) coming in the next drop.</i>",
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
                    let url = format!("https://polymarket.com/event/{}", slug);
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
            let url = format!("https://polymarket.com/event/{}", slug);
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
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Chat {
    id: i64,
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
        let resp = self
            .http
            .post(&url)
            .json(&Req {
                chat_id,
                text,
                parse_mode: "HTML",
                disable_web_page_preview: true,
                reply_to_message_id: reply_to,
            })
            .send()
            .await
            .map_err(|e| format!("request: {}", e))?;
        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("telegram {}: {}", code, body));
        }
        Ok(())
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
    }
}
