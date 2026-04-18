//! New-market launch alerter.
//!
//! Polls the scanner tables for rows created since the last tick. If a new
//! market's question matches the `NEW_MARKET_WATCHLIST` (case-insensitive
//! substring, comma-separated keywords) — or if the watchlist is empty and a
//! `NEW_MARKET_CATEGORIES` filter matches — we push a Telegram alert. One
//! cooldown per keyword-match prevents the same keyword from re-alerting on a
//! backlog of near-simultaneous launches.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use log::{info, warn};
use serde::Serialize;

use crate::AppState;

const DEFAULT_POLL_SECS: u64 = 60;
const DEFAULT_COOLDOWN_SECS: u64 = 900;
const MAX_ALERTS_PER_TICK: usize = 5;

pub fn spawn(state: Arc<AppState>) {
    let enabled = env_bool("NEW_MARKET_ALERTS_ENABLED", false);
    if !enabled {
        info!("New-market alerts disabled (NEW_MARKET_ALERTS_ENABLED=false)");
        return;
    }

    let Ok(bot_token) = std::env::var("TELEGRAM_BOT_TOKEN") else {
        warn!("NEW_MARKET_ALERTS_ENABLED=true but TELEGRAM_BOT_TOKEN missing; not starting");
        return;
    };
    let Ok(chat_id) = std::env::var("TELEGRAM_CHAT_ID") else {
        warn!("NEW_MARKET_ALERTS_ENABLED=true but TELEGRAM_CHAT_ID missing; not starting");
        return;
    };

    let watchlist = parse_csv_lower(std::env::var("NEW_MARKET_WATCHLIST").unwrap_or_default());
    let categories = parse_csv_lower(std::env::var("NEW_MARKET_CATEGORIES").unwrap_or_default());
    let poll = Duration::from_secs(env_u64("NEW_MARKET_POLL_SECS", DEFAULT_POLL_SECS));
    let cooldown = Duration::from_secs(env_u64(
        "NEW_MARKET_COOLDOWN_SECS",
        DEFAULT_COOLDOWN_SECS,
    ));

    if watchlist.is_empty() && categories.is_empty() {
        warn!(
            "NEW_MARKET_ALERTS_ENABLED=true but both NEW_MARKET_WATCHLIST and \
             NEW_MARKET_CATEGORIES are empty; that would spam the channel with \
             every new market — refusing to start"
        );
        return;
    }

    info!(
        "Starting new-market alerts (watchlist={:?}, categories={:?}, poll={}s)",
        watchlist,
        categories,
        poll.as_secs()
    );

    let tg = TelegramClient::new(bot_token, chat_id);

    tokio::spawn(async move {
        let mut cursor_poly: Option<DateTime<Utc>> = None;
        let mut cursor_lim: Option<DateTime<Utc>> = None;
        let mut last_alert: HashMap<String, Instant> = HashMap::new();

        let mut interval = tokio::time::interval(poll);
        interval.tick().await;
        init_cursors(&state, &mut cursor_poly, &mut cursor_lim).await;

        loop {
            interval.tick().await;

            match fetch_new_polymarket(&state, cursor_poly).await {
                Ok(rows) => {
                    for r in rows.iter().take(MAX_ALERTS_PER_TICK) {
                        maybe_alert(&tg, &watchlist, &categories, cooldown, &mut last_alert, r).await;
                    }
                    if let Some(latest) = rows.iter().map(|r| r.created_at).max() {
                        cursor_poly = Some(match cursor_poly {
                            Some(c) => c.max(latest),
                            None => latest,
                        });
                    }
                }
                Err(e) => warn!("new_market_alert poly fetch: {}", e),
            }

            match fetch_new_limitless(&state, cursor_lim).await {
                Ok(rows) => {
                    for r in rows.iter().take(MAX_ALERTS_PER_TICK) {
                        maybe_alert(&tg, &watchlist, &categories, cooldown, &mut last_alert, r).await;
                    }
                    if let Some(latest) = rows.iter().map(|r| r.created_at).max() {
                        cursor_lim = Some(match cursor_lim {
                            Some(c) => c.max(latest),
                            None => latest,
                        });
                    }
                }
                Err(e) => warn!("new_market_alert limitless fetch: {}", e),
            }
        }
    });
}

#[derive(Debug, Clone)]
struct NewMarket {
    venue: &'static str,
    question: String,
    category: String,
    slug: String,
    created_at: DateTime<Utc>,
}

async fn init_cursors(
    state: &AppState,
    cursor_poly: &mut Option<DateTime<Utc>>,
    cursor_lim: &mut Option<DateTime<Utc>>,
) {
    let pool = state.db.pool();
    if let Ok((ts,)) = sqlx::query_as::<_, (Option<DateTime<Utc>>,)>(
        "SELECT MAX(created_at) FROM polymarket_scanned_markets",
    )
    .fetch_one(pool)
    .await
    {
        *cursor_poly = ts;
    }
    if let Ok((ts,)) = sqlx::query_as::<_, (Option<DateTime<Utc>>,)>(
        "SELECT MAX(created_at) FROM limitless_scanned_markets",
    )
    .fetch_one(pool)
    .await
    {
        *cursor_lim = ts;
    }
}

async fn fetch_new_polymarket(
    state: &AppState,
    cursor: Option<DateTime<Utc>>,
) -> Result<Vec<NewMarket>, String> {
    let pool = state.db.pool();
    let cutoff = cursor.unwrap_or_else(|| Utc::now() - chrono::Duration::minutes(5));

    let rows: Vec<(String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT question, COALESCE(category, 'unknown'), COALESCE(slug, ''), created_at \
         FROM polymarket_scanned_markets \
         WHERE created_at > $1 AND active = TRUE \
         ORDER BY created_at ASC \
         LIMIT 200",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("poly query: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|(question, category, slug, created_at)| NewMarket {
            venue: "polymarket",
            question,
            category,
            slug,
            created_at,
        })
        .collect())
}

async fn fetch_new_limitless(
    state: &AppState,
    cursor: Option<DateTime<Utc>>,
) -> Result<Vec<NewMarket>, String> {
    let pool = state.db.pool();
    let cutoff = cursor.unwrap_or_else(|| Utc::now() - chrono::Duration::minutes(5));

    let rows: Vec<(String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT question, COALESCE(category, 'unknown'), slug, created_at \
         FROM limitless_scanned_markets \
         WHERE created_at > $1 AND active = TRUE \
         ORDER BY created_at ASC \
         LIMIT 200",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("limitless query: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|(question, category, slug, created_at)| NewMarket {
            venue: "limitless",
            question,
            category,
            slug,
            created_at,
        })
        .collect())
}

async fn maybe_alert(
    tg: &TelegramClient,
    watchlist: &[String],
    categories: &[String],
    cooldown: Duration,
    last_alert: &mut HashMap<String, Instant>,
    m: &NewMarket,
) {
    let hit = match_hit(watchlist, categories, m);
    let Some(reason) = hit else { return };

    let cooldown_key = format!("{}::{}", reason, m.venue);
    if let Some(t) = last_alert.get(&cooldown_key) {
        if t.elapsed() < cooldown {
            return;
        }
    }
    last_alert.insert(cooldown_key, Instant::now());

    let text = format_alert(m, &reason);
    if let Err(e) = tg.send(&text).await {
        warn!("telegram send failed (new-market): {}", e);
    }
}

fn match_hit(watchlist: &[String], categories: &[String], m: &NewMarket) -> Option<String> {
    let q_lower = m.question.to_lowercase();
    for kw in watchlist {
        if q_lower.contains(kw) {
            return Some(format!("kw:{}", kw));
        }
    }
    if watchlist.is_empty() {
        let cat_lower = m.category.to_lowercase();
        for cat in categories {
            if cat_lower == *cat {
                return Some(format!("cat:{}", cat));
            }
        }
    }
    None
}

fn format_alert(m: &NewMarket, reason: &str) -> String {
    let link = market_link(m);
    format!(
        "🆕 New market ({venue}, {category})\n{question}\nmatch: {reason}{link}",
        venue = m.venue,
        category = m.category,
        question = m.question,
        reason = reason,
        link = link
            .map(|l| format!("\n{}", l))
            .unwrap_or_default(),
    )
}

fn market_link(m: &NewMarket) -> Option<String> {
    if m.slug.is_empty() {
        return None;
    }
    match m.venue {
        "polymarket" => Some(format!("https://polymarket.com/event/{}", m.slug)),
        "limitless" => Some(format!("https://limitless.exchange/markets/{}", m.slug)),
        _ => None,
    }
}

fn parse_csv_lower(raw: String) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

struct TelegramClient {
    bot_token: String,
    chat_id: String,
    http: reqwest::Client,
}

impl TelegramClient {
    fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            bot_token,
            chat_id,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
        }
    }

    async fn send(&self, text: &str) -> Result<(), String> {
        #[derive(Serialize)]
        struct Payload<'a> {
            chat_id: &'a str,
            text: &'a str,
            disable_web_page_preview: bool,
        }
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let resp = self
            .http
            .post(&url)
            .json(&Payload {
                chat_id: &self.chat_id,
                text,
                disable_web_page_preview: true,
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

    fn sample(question: &str, category: &str) -> NewMarket {
        NewMarket {
            venue: "polymarket",
            question: question.to_string(),
            category: category.to_string(),
            slug: "example-slug".to_string(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn watchlist_matches_substring_case_insensitive() {
        let wl = parse_csv_lower("BTC, Fed, Trump".to_string());
        let m = sample("Will Bitcoin (BTC) hit 100k?", "crypto");
        assert_eq!(match_hit(&wl, &[], &m), Some("kw:btc".to_string()));
    }

    #[test]
    fn watchlist_empty_falls_back_to_category() {
        let cats = parse_csv_lower("Sports".to_string());
        let m = sample("Will team X win?", "sports");
        assert_eq!(match_hit(&[], &cats, &m), Some("cat:sports".to_string()));
    }

    #[test]
    fn no_match_when_watchlist_present_and_category_only() {
        let wl = parse_csv_lower("btc".to_string());
        let cats = parse_csv_lower("sports".to_string());
        let m = sample("Will team X win?", "sports");
        // watchlist takes precedence — even though category matches, question does not contain "btc"
        assert_eq!(match_hit(&wl, &cats, &m), None);
    }

    #[test]
    fn alert_body_contains_question_and_link() {
        let m = sample("Will BTC hit 100k?", "crypto");
        let s = format_alert(&m, "kw:btc");
        assert!(s.contains("🆕 New market"));
        assert!(s.contains("polymarket"));
        assert!(s.contains("Will BTC hit 100k?"));
        assert!(s.contains("polymarket.com/event/example-slug"));
        assert!(s.contains("kw:btc"));
    }

    #[test]
    fn alert_skips_link_when_slug_empty() {
        let mut m = sample("Will BTC hit 100k?", "crypto");
        m.slug.clear();
        let s = format_alert(&m, "kw:btc");
        assert!(!s.contains("polymarket.com"));
    }

    #[test]
    fn csv_lowercase_trims_and_drops_empty() {
        let parsed = parse_csv_lower("  Foo ,, bar,BAZ ".to_string());
        assert_eq!(parsed, vec!["foo", "bar", "baz"]);
    }
}
