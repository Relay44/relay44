//! New-market launch alerter.
//!
//! Polls the scanner tables for rows created since the last tick. If a new
//! market's question matches the `NEW_MARKET_WATCHLIST` (case-insensitive
//! substring, comma-separated keywords) — or if the watchlist is empty and a
//! `NEW_MARKET_CATEGORIES` filter matches — we push a Telegram alert. One
//! cooldown per keyword-match prevents the same keyword from re-alerting on a
//! backlog of near-simultaneous launches.
//!
//! Message layout is HTML and uses the shared helpers in `telegram_format` so
//! this alerter stays consistent with `probability_alert` and
//! `cross_venue_arb`.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use log::{info, warn};

use super::telegram_format::{
    format_alert_header, format_deep_link, format_metadata_row, html_escape, TelegramClient,
};
use crate::AppState;

const DEFAULT_POLL_SECS: u64 = 60;
const DEFAULT_COOLDOWN_SECS: u64 = 900;
const DEFAULT_RATE_LIMIT_PER_HOUR: usize = 3;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(3600);
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
    let rate_limit_per_hour =
        env_usize("NEW_MARKET_RATE_LIMIT_PER_HOUR", DEFAULT_RATE_LIMIT_PER_HOUR);

    if watchlist.is_empty() && categories.is_empty() {
        warn!(
            "NEW_MARKET_ALERTS_ENABLED=true but both NEW_MARKET_WATCHLIST and \
             NEW_MARKET_CATEGORIES are empty; that would spam the channel with \
             every new market — refusing to start"
        );
        return;
    }

    info!(
        "Starting new-market alerts (watchlist={:?}, categories={:?}, poll={}s, rate_limit={}/hr)",
        watchlist,
        categories,
        poll.as_secs(),
        rate_limit_per_hour,
    );

    let tg = TelegramClient::new(bot_token, chat_id);

    tokio::spawn(async move {
        let mut cursor_poly: Option<DateTime<Utc>> = None;
        let mut cursor_lim: Option<DateTime<Utc>> = None;
        let mut last_alert: HashMap<String, Instant> = HashMap::new();
        let mut sent_times: VecDeque<Instant> = VecDeque::new();

        let mut interval = tokio::time::interval(poll);
        interval.tick().await;
        init_cursors(&state, &mut cursor_poly, &mut cursor_lim).await;

        loop {
            interval.tick().await;

            match fetch_new_polymarket(&state, cursor_poly).await {
                Ok(rows) => {
                    for r in rows.iter().take(MAX_ALERTS_PER_TICK) {
                        maybe_alert(
                            &tg,
                            &watchlist,
                            &categories,
                            cooldown,
                            rate_limit_per_hour,
                            &mut last_alert,
                            &mut sent_times,
                            r,
                        )
                        .await;
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
                        maybe_alert(
                            &tg,
                            &watchlist,
                            &categories,
                            cooldown,
                            rate_limit_per_hour,
                            &mut last_alert,
                            &mut sent_times,
                            r,
                        )
                        .await;
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
    liquidity_usd: Option<f64>,
    volume_24h_usd: Option<f64>,
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

    let rows: Vec<(
        String,
        String,
        String,
        Option<f64>,
        Option<f64>,
        DateTime<Utc>,
    )> = sqlx::query_as(
        "SELECT question, COALESCE(category, 'unknown'), COALESCE(slug, ''), \
                liquidity_usdc::double precision, volume_usdc::double precision, \
                created_at \
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
        .map(
            |(question, category, slug, liquidity_usd, volume_24h_usd, created_at)| NewMarket {
                venue: "polymarket",
                question,
                category,
                slug,
                liquidity_usd,
                volume_24h_usd,
                created_at,
            },
        )
        .collect())
}

async fn fetch_new_limitless(
    state: &AppState,
    cursor: Option<DateTime<Utc>>,
) -> Result<Vec<NewMarket>, String> {
    let pool = state.db.pool();
    let cutoff = cursor.unwrap_or_else(|| Utc::now() - chrono::Duration::minutes(5));

    let rows: Vec<(
        String,
        String,
        String,
        Option<f64>,
        Option<f64>,
        DateTime<Utc>,
    )> = sqlx::query_as(
        "SELECT question, COALESCE(category, 'unknown'), slug, \
                liquidity_usdc, volume_usdc, created_at \
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
        .map(
            |(question, category, slug, liquidity_usd, volume_24h_usd, created_at)| NewMarket {
                venue: "limitless",
                question,
                category,
                slug,
                liquidity_usd,
                volume_24h_usd,
                created_at,
            },
        )
        .collect())
}

async fn maybe_alert(
    tg: &TelegramClient,
    watchlist: &[String],
    categories: &[String],
    cooldown: Duration,
    rate_limit_per_hour: usize,
    last_alert: &mut HashMap<String, Instant>,
    sent_times: &mut VecDeque<Instant>,
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

    prune_rate_window(sent_times);
    if rate_limit_per_hour > 0 && sent_times.len() >= rate_limit_per_hour {
        info!(
            "new_market_alert suppressed ({}): rate limit {}/hr reached",
            reason, rate_limit_per_hour
        );
        return;
    }

    last_alert.insert(cooldown_key, Instant::now());

    let text = format_alert(m, &reason);
    match tg.send(&text).await {
        Ok(()) => sent_times.push_back(Instant::now()),
        Err(e) => warn!("telegram send failed (new-market): {}", e),
    }
}

fn prune_rate_window(sent_times: &mut VecDeque<Instant>) {
    while let Some(front) = sent_times.front() {
        if front.elapsed() >= RATE_LIMIT_WINDOW {
            sent_times.pop_front();
        } else {
            break;
        }
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
    let mut lines: Vec<String> = Vec::new();
    lines.push(format_alert_header("\u{1F195}", "New market", m.venue));
    lines.push(format!("<i>{}</i>", html_escape(&m.question)));
    lines.push(format!("match: <code>{}</code>", html_escape(reason)));

    let meta = format_metadata_row(m.liquidity_usd, m.volume_24h_usd, Some(&m.category));
    if !meta.is_empty() {
        lines.push(meta);
    }

    if let Some(link) = format_deep_link(m.venue, &m.slug) {
        lines.push(link);
    }

    lines.join("\n")
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

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
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
            liquidity_usd: Some(10_000.0),
            volume_24h_usd: Some(250_000.0),
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
    fn alert_body_contains_question_header_and_link() {
        let m = sample("Will BTC hit 100k?", "crypto");
        let s = format_alert(&m, "kw:btc");
        assert!(s.contains("<b>"));
        assert!(s.contains("New market"));
        assert!(s.contains("Polymarket"));
        assert!(s.contains("<i>Will BTC hit 100k?</i>"));
        assert!(s.contains("relay44.com/markets/by-slug/polymarket/example-slug"));
        assert!(s.contains("Trade on Relay44"));
        assert!(s.contains("kw:btc"));
    }

    #[test]
    fn alert_includes_metadata_row() {
        let m = sample("Will BTC hit 100k?", "crypto");
        let s = format_alert(&m, "kw:btc");
        assert!(s.contains("Liquidity: $10.0k"));
        assert!(s.contains("24h vol: $250.0k"));
        assert!(s.contains("Category: crypto"));
    }

    #[test]
    fn alert_html_escapes_question_and_reason() {
        let mut m = sample("Will <X> happen?", "crypto");
        m.slug = "slug".to_string();
        let s = format_alert(&m, "kw:<evil>");
        assert!(s.contains("Will &lt;X&gt; happen?"));
        assert!(s.contains("kw:&lt;evil&gt;"));
        assert!(!s.contains("<evil>"));
    }

    #[test]
    fn alert_skips_link_when_slug_empty() {
        let mut m = sample("Will BTC hit 100k?", "crypto");
        m.slug.clear();
        let s = format_alert(&m, "kw:btc");
        assert!(!s.contains("relay44.com"));
    }

    #[test]
    fn alert_limitless_uses_relay44_link() {
        let mut m = sample("Will BTC hit 100k?", "crypto");
        m.venue = "limitless";
        m.slug = "lim-slug".to_string();
        let s = format_alert(&m, "kw:btc");
        assert!(s.contains("relay44.com/markets/by-slug/limitless/lim-slug"));
        assert!(s.contains("Trade on Relay44"));
        assert!(s.contains("Limitless"));
    }

    #[test]
    fn csv_lowercase_trims_and_drops_empty() {
        let parsed = parse_csv_lower("  Foo ,, bar,BAZ ".to_string());
        assert_eq!(parsed, vec!["foo", "bar", "baz"]);
    }
}
