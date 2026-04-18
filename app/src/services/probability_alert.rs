//! Telegram probability-shift alerts.
//!
//! Subscribes to the market_data bus, tracks the last observed price per
//! (venue, market_key), and pushes a Telegram message when a fresh snapshot
//! moves by more than `PROB_ALERT_THRESHOLD_PCT`. Each market is on a
//! per-market cooldown so a rapidly-jittery book can't spam the channel.
//!
//! Alerts are enriched with question, deep link, category, liquidity and
//! 24h volume by joining against the scanner's own market tables.
//! `PROB_ALERT_MIN_LIQUIDITY_USD` suppresses alerts on known-thin markets
//! without dropping alerts for markets whose metadata isn't in the scanner
//! tables yet.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{info, warn};
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;

use super::market_data::{L2Event, L2Payload, Venue};
use crate::AppState;

const DEFAULT_THRESHOLD_PCT: f64 = 5.0;
const DEFAULT_COOLDOWN_SECS: u64 = 300;
const MIN_PRICE_FOR_ALERT: f64 = 0.01;

pub fn spawn(state: Arc<AppState>) {
    let enabled = std::env::var("TELEGRAM_ALERTS_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    if !enabled {
        info!("Telegram probability alerts disabled (TELEGRAM_ALERTS_ENABLED=false)");
        return;
    }

    let Ok(bot_token) = std::env::var("TELEGRAM_BOT_TOKEN") else {
        warn!("TELEGRAM_ALERTS_ENABLED=true but TELEGRAM_BOT_TOKEN missing; not starting");
        return;
    };
    let Ok(chat_id) = std::env::var("TELEGRAM_CHAT_ID") else {
        warn!("TELEGRAM_ALERTS_ENABLED=true but TELEGRAM_CHAT_ID missing; not starting");
        return;
    };

    let threshold_pct = std::env::var("PROB_ALERT_THRESHOLD_PCT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(DEFAULT_THRESHOLD_PCT);
    let cooldown = Duration::from_secs(
        std::env::var("PROB_ALERT_COOLDOWN_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_COOLDOWN_SECS),
    );
    let min_liquidity_usd = std::env::var("PROB_ALERT_MIN_LIQUIDITY_USD")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);

    info!(
        "Starting Telegram probability alerts (threshold={}%, cooldown={}s, min_liquidity=${})",
        threshold_pct,
        cooldown.as_secs(),
        min_liquidity_usd,
    );

    let mut rx = state.market_data.subscribe();
    let tg = TelegramClient::new(bot_token, chat_id);

    tokio::spawn(async move {
        let mut last_price: HashMap<String, f64> = HashMap::new();
        let mut last_alert: HashMap<String, Instant> = HashMap::new();

        loop {
            match rx.recv().await {
                Ok(ev) => {
                    handle_event(
                        &state,
                        &tg,
                        threshold_pct,
                        cooldown,
                        min_liquidity_usd,
                        &mut last_price,
                        &mut last_alert,
                        &ev,
                    )
                    .await;
                }
                Err(RecvError::Lagged(n)) => {
                    warn!("probability_alert lagged by {} events", n);
                }
                Err(RecvError::Closed) => {
                    info!("market_data bus closed; probability_alert exiting");
                    return;
                }
            }
        }
    });
}

struct MarketContext {
    question: String,
    url: Option<String>,
    category: Option<String>,
    liquidity_usd: Option<f64>,
    volume_24h_usd: Option<f64>,
}

async fn handle_event(
    state: &AppState,
    tg: &TelegramClient,
    threshold_pct: f64,
    cooldown: Duration,
    min_liquidity_usd: f64,
    last_price: &mut HashMap<String, f64>,
    last_alert: &mut HashMap<String, Instant>,
    ev: &L2Event,
) {
    let Some(price) = current_price(&ev.payload) else {
        return;
    };
    if price < MIN_PRICE_FOR_ALERT {
        return;
    }

    let key = cache_key(ev.venue, &ev.market_key);
    let prev = last_price.insert(key.clone(), price);

    let Some(prev_price) = prev else {
        return;
    };
    if prev_price < MIN_PRICE_FOR_ALERT {
        return;
    }

    let delta_pct = ((price - prev_price) / prev_price) * 100.0;
    if delta_pct.abs() < threshold_pct {
        return;
    }

    if let Some(t) = last_alert.get(&key) {
        if t.elapsed() < cooldown {
            return;
        }
    }

    let ctx = lookup_context(state, ev.venue, &ev.market_key).await;

    if min_liquidity_usd > 0.0 {
        if let Some(liq) = ctx.as_ref().and_then(|c| c.liquidity_usd) {
            if liq < min_liquidity_usd {
                return;
            }
        }
    }

    last_alert.insert(key, Instant::now());

    let text = format_alert(
        ctx.as_ref(),
        ev.venue,
        &ev.market_key,
        prev_price,
        price,
        delta_pct,
    );
    if let Err(e) = tg.send(&text).await {
        warn!("telegram send failed: {}", e);
    }
}

fn current_price(payload: &L2Payload) -> Option<f64> {
    match payload {
        L2Payload::Snapshot { bids, asks, .. } => bids
            .first()
            .map(|l| l.price)
            .or_else(|| asks.first().map(|l| l.price)),
        L2Payload::Trade { price, .. } => Some(*price),
        L2Payload::Delta { .. } => None,
    }
}

fn cache_key(venue: Venue, market_key: &str) -> String {
    format!("{}:{}", venue.as_str(), market_key)
}

async fn lookup_context(
    state: &AppState,
    venue: Venue,
    market_key: &str,
) -> Option<MarketContext> {
    match venue {
        Venue::Polymarket => lookup_polymarket(state, market_key).await,
        Venue::Limitless => lookup_limitless(state, market_key).await,
        _ => None,
    }
}

async fn lookup_polymarket(state: &AppState, token_id: &str) -> Option<MarketContext> {
    let pool = state.db.pool();
    let row: Option<(String, String, String, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT question, slug, category, \
                liquidity_usdc::double precision, volume_usdc::double precision \
         FROM polymarket_scanned_markets \
         WHERE yes_token_id = $1 OR no_token_id = $1 LIMIT 1",
    )
    .bind(token_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    row.map(|(question, slug, category, liquidity_usd, volume_24h_usd)| MarketContext {
        question,
        url: (!slug.is_empty()).then(|| format!("https://polymarket.com/event/{}", slug)),
        category: normalize_category(&category),
        liquidity_usd,
        volume_24h_usd,
    })
}

async fn lookup_limitless(state: &AppState, market_key: &str) -> Option<MarketContext> {
    // Limitless market_key is "{slug}:{outcome}" where outcome is yes|no.
    // rsplit_once tolerates colons inside the slug itself.
    let slug = market_key
        .rsplit_once(':')
        .map(|(prefix, _)| prefix)
        .unwrap_or(market_key);

    let pool = state.db.pool();
    let row: Option<(String, Option<String>, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT question, category, liquidity_usdc, volume_usdc \
         FROM limitless_scanned_markets \
         WHERE slug = $1 LIMIT 1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    row.map(|(question, category, liquidity_usd, volume_24h_usd)| MarketContext {
        question,
        url: Some(format!("https://limitless.exchange/markets/{}", slug)),
        category: category.as_deref().and_then(normalize_category),
        liquidity_usd,
        volume_24h_usd,
    })
}

fn normalize_category(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

fn format_alert(
    ctx: Option<&MarketContext>,
    venue: Venue,
    market_key: &str,
    prev: f64,
    curr: f64,
    delta_pct: f64,
) -> String {
    let arrow = if delta_pct > 0.0 { "↑" } else { "↓" };
    let question = ctx
        .map(|c| c.question.clone())
        .unwrap_or_else(|| format!("{}:{}", venue.as_str(), truncate(market_key, 12)));

    let mut lines = vec![
        format!("{arrow} {question}"),
        format!(
            "{:.1}¢ → {:.1}¢  ({:+.1}%)",
            prev * 100.0,
            curr * 100.0,
            delta_pct
        ),
    ];

    if let Some(c) = ctx {
        let mut meta: Vec<String> = Vec::new();
        if let Some(cat) = &c.category {
            meta.push(cat.clone());
        }
        if let Some(liq) = c.liquidity_usd {
            meta.push(format!("liq {}", format_money(liq)));
        }
        if let Some(vol) = c.volume_24h_usd {
            meta.push(format!("vol {}", format_money(vol)));
        }
        if !meta.is_empty() {
            lines.push(meta.join(" · "));
        }
        if let Some(url) = &c.url {
            lines.push(url.clone());
        }
    }

    lines.join("\n")
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

    fn ctx_full() -> MarketContext {
        MarketContext {
            question: "Will X happen?".to_string(),
            url: Some("https://polymarket.com/event/will-x".to_string()),
            category: Some("politics".to_string()),
            liquidity_usd: Some(125_000.0),
            volume_24h_usd: Some(3_400_000.0),
        }
    }

    #[test]
    fn format_includes_question_and_delta() {
        let c = ctx_full();
        let s = format_alert(Some(&c), Venue::Polymarket, "tok", 0.10, 0.18, 80.0);
        assert!(s.contains("Will X happen?"));
        assert!(s.contains("10.0¢"));
        assert!(s.contains("18.0¢"));
        assert!(s.contains("+80.0%"));
    }

    #[test]
    fn format_down_arrow_for_negative() {
        let c = ctx_full();
        let s = format_alert(Some(&c), Venue::Polymarket, "tok", 0.50, 0.40, -20.0);
        assert!(s.contains("↓"));
    }

    #[test]
    fn format_includes_enrichment_when_ctx_present() {
        let c = ctx_full();
        let s = format_alert(Some(&c), Venue::Polymarket, "tok", 0.10, 0.18, 80.0);
        assert!(s.contains("politics"));
        assert!(s.contains("liq $125.0k"));
        assert!(s.contains("vol $3.4M"));
        assert!(s.contains("https://polymarket.com/event/will-x"));
    }

    #[test]
    fn format_falls_back_to_venue_market_key_without_ctx() {
        let s = format_alert(None, Venue::Limitless, "some-long-market-slug", 0.10, 0.15, 50.0);
        assert!(s.contains("limitless:"));
        assert!(s.contains("50.0%") || s.contains("+50.0%"));
    }

    #[test]
    fn format_omits_missing_enrichment_fields() {
        let c = MarketContext {
            question: "Q".to_string(),
            url: None,
            category: None,
            liquidity_usd: None,
            volume_24h_usd: None,
        };
        let s = format_alert(Some(&c), Venue::Polymarket, "tok", 0.10, 0.15, 50.0);
        assert!(s.contains("Q"));
        assert!(!s.contains("liq "));
        assert!(!s.contains("vol "));
        assert!(!s.contains("http"));
    }

    #[test]
    fn money_formatter_ranges() {
        assert_eq!(format_money(42.0), "$42");
        assert_eq!(format_money(4_200.0), "$4.2k");
        assert_eq!(format_money(4_200_000.0), "$4.2M");
    }

    #[test]
    fn normalize_category_drops_unknown_and_empty() {
        assert_eq!(normalize_category("politics"), Some("politics".to_string()));
        assert_eq!(normalize_category("Unknown"), None);
        assert_eq!(normalize_category("  "), None);
        assert_eq!(normalize_category(""), None);
    }
}
