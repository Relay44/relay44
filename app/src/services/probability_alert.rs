//! Telegram probability-shift alerts.
//!
//! Subscribes to the market_data bus, tracks the last observed price per
//! (venue, market_key), and pushes a Telegram message when a fresh snapshot
//! moves by more than `PROB_ALERT_THRESHOLD_PCT`. Each market is on a
//! per-market cooldown so a rapidly-jittery book can't spam the channel.
//!
//! Alerts are enriched with question, deep link, category, liquidity and
//! 24h volume by joining against the scanner's own market tables
//! (`polymarket_scanned_markets`, `limitless_scanned_markets`).
//! `PROB_ALERT_MIN_LIQUIDITY_USD` suppresses alerts on known-thin markets
//! without dropping alerts for markets whose metadata isn't in the scanner
//! tables yet.
//!
//! Message layout is HTML and goes through the shared helpers in
//! `telegram_format` so this alerter stays visually consistent with
//! `cross_venue_arb` and `new_market_alert`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{info, warn};
use tokio::sync::broadcast::error::RecvError;

use super::market_data::{L2Event, L2Payload, Venue};
use super::telegram_format::{
    format_alert_header, format_deep_link, format_metadata_row, html_escape, TelegramClient,
};
use crate::AppState;

const DEFAULT_THRESHOLD_PCT: f64 = 5.0;
const DEFAULT_COOLDOWN_SECS: u64 = 300;
const DEFAULT_MIN_PRICE: f64 = 0.05;
const HARD_MIN_PRICE: f64 = 0.01;

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
    let min_price = std::env::var("PROB_ALERT_MIN_PRICE")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(DEFAULT_MIN_PRICE)
        .max(HARD_MIN_PRICE);

    info!(
        "Starting Telegram probability alerts (threshold={}%, cooldown={}s, min_liquidity=${}, min_price={}¢)",
        threshold_pct,
        cooldown.as_secs(),
        min_liquidity_usd,
        (min_price * 100.0) as i32,
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
                        min_price,
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
    slug: Option<String>,
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
    min_price: f64,
    last_price: &mut HashMap<String, f64>,
    last_alert: &mut HashMap<String, Instant>,
    ev: &L2Event,
) {
    let Some(price) = current_price(&ev.payload) else {
        return;
    };
    if price < HARD_MIN_PRICE {
        return;
    }

    let key = cache_key(ev.venue, &ev.market_key);
    let prev = last_price.insert(key.clone(), price);

    let Some(prev_price) = prev else {
        return;
    };
    if prev_price < HARD_MIN_PRICE {
        return;
    }

    // Dust-market gate: if either end of the move is below the configured
    // min price, this is almost certainly a penny bouncing around rather
    // than a meaningful probability shift.
    if price < min_price || prev_price < min_price {
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

    if should_skip_for_liquidity(min_liquidity_usd, ctx.as_ref()) {
        return;
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

fn should_skip_for_liquidity(min_liquidity_usd: f64, ctx: Option<&MarketContext>) -> bool {
    if min_liquidity_usd <= 0.0 {
        return false;
    }
    match ctx.and_then(|c| c.liquidity_usd) {
        // Gate suppresses only markets whose liquidity is known and below the
        // threshold. Unknown liquidity (metadata not yet ingested) is allowed
        // through — we'd rather surface a possibly-thin market than miss a
        // genuine move on a brand-new listing.
        Some(liq) => liq < min_liquidity_usd,
        None => false,
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
        slug: (!slug.is_empty()).then_some(slug),
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
        slug: Some(slug.to_string()),
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
    let emoji = if delta_pct > 0.0 {
        "\u{1F4C8}"
    } else {
        "\u{1F4C9}"
    };
    let question_raw = ctx
        .map(|c| c.question.clone())
        .unwrap_or_else(|| format!("{}:{}", venue.as_str(), truncate(market_key, 12)));

    let mut lines: Vec<String> = Vec::new();
    lines.push(format_alert_header(emoji, "Probability shift", venue.as_str()));
    lines.push(format!("<i>{}</i>", html_escape(&question_raw)));
    lines.push(format!(
        "{arrow} {:.1}¢ → {:.1}¢  ({:+.1}%)",
        prev * 100.0,
        curr * 100.0,
        delta_pct
    ));

    if let Some(c) = ctx {
        let meta = format_metadata_row(
            c.liquidity_usd,
            c.volume_24h_usd,
            c.category.as_deref(),
        );
        if !meta.is_empty() {
            lines.push(meta);
        }
        if let Some(slug) = &c.slug {
            if let Some(link) = format_deep_link(venue.as_str(), slug) {
                lines.push(link);
            }
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_full() -> MarketContext {
        MarketContext {
            question: "Will X happen?".to_string(),
            slug: Some("will-x".to_string()),
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
    fn format_header_is_bold_and_venue_titled() {
        let c = ctx_full();
        let s = format_alert(Some(&c), Venue::Polymarket, "tok", 0.10, 0.18, 80.0);
        assert!(s.contains("<b>"));
        assert!(s.contains("Probability shift"));
        assert!(s.contains("Polymarket"));
    }

    #[test]
    fn format_down_arrow_for_negative() {
        let c = ctx_full();
        let s = format_alert(Some(&c), Venue::Polymarket, "tok", 0.50, 0.40, -20.0);
        assert!(s.contains("↓"));
    }

    #[test]
    fn format_includes_metadata_row_when_ctx_present() {
        let c = ctx_full();
        let s = format_alert(Some(&c), Venue::Polymarket, "tok", 0.10, 0.18, 80.0);
        assert!(s.contains("Liquidity: $125.0k"));
        assert!(s.contains("24h vol: $3.4M"));
        assert!(s.contains("Category: politics"));
        assert!(s.contains("https://relay44.com/markets/by-slug/polymarket/will-x"));
        assert!(s.contains("Trade on Relay44"));
    }

    #[test]
    fn format_html_escapes_question() {
        let c = MarketContext {
            question: "Will A<B>&C?".to_string(),
            slug: None,
            category: None,
            liquidity_usd: None,
            volume_24h_usd: None,
        };
        let s = format_alert(Some(&c), Venue::Polymarket, "tok", 0.10, 0.15, 50.0);
        assert!(s.contains("A&lt;B&gt;&amp;C"));
        assert!(!s.contains("A<B>"));
    }

    #[test]
    fn format_limitless_link_uses_relay44_domain() {
        let c = MarketContext {
            question: "Q".to_string(),
            slug: Some("lim-slug".to_string()),
            category: None,
            liquidity_usd: None,
            volume_24h_usd: None,
        };
        let s = format_alert(Some(&c), Venue::Limitless, "lim-slug:yes", 0.10, 0.15, 50.0);
        assert!(s.contains("relay44.com/markets/by-slug/limitless/lim-slug"));
        assert!(s.contains("Trade on Relay44"));
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
            slug: None,
            category: None,
            liquidity_usd: None,
            volume_24h_usd: None,
        };
        let s = format_alert(Some(&c), Venue::Polymarket, "tok", 0.10, 0.15, 50.0);
        assert!(s.contains("Q"));
        assert!(!s.contains("Liquidity:"));
        assert!(!s.contains("24h vol:"));
        assert!(!s.contains("Category:"));
        assert!(!s.contains("href="));
    }

    #[test]
    fn normalize_category_drops_unknown_and_empty() {
        assert_eq!(normalize_category("politics"), Some("politics".to_string()));
        assert_eq!(normalize_category("Unknown"), None);
        assert_eq!(normalize_category("  "), None);
        assert_eq!(normalize_category(""), None);
    }

    #[test]
    fn liquidity_gate_skips_below_threshold() {
        let c = MarketContext {
            question: "Q".to_string(),
            slug: None,
            category: None,
            liquidity_usd: Some(500.0),
            volume_24h_usd: None,
        };
        assert!(should_skip_for_liquidity(1_000.0, Some(&c)));
    }

    #[test]
    fn liquidity_gate_passes_above_threshold() {
        let c = MarketContext {
            question: "Q".to_string(),
            slug: None,
            category: None,
            liquidity_usd: Some(5_000.0),
            volume_24h_usd: None,
        };
        assert!(!should_skip_for_liquidity(1_000.0, Some(&c)));
    }

    #[test]
    fn liquidity_gate_disabled_when_threshold_zero() {
        let c = MarketContext {
            question: "Q".to_string(),
            slug: None,
            category: None,
            liquidity_usd: Some(10.0),
            volume_24h_usd: None,
        };
        assert!(!should_skip_for_liquidity(0.0, Some(&c)));
    }

    #[test]
    fn liquidity_gate_allows_unknown_liquidity_through() {
        // Metadata not yet ingested → don't suppress; we'd rather risk a
        // possibly-thin alert than miss a genuine move on a brand-new listing.
        let c = MarketContext {
            question: "Q".to_string(),
            slug: None,
            category: None,
            liquidity_usd: None,
            volume_24h_usd: None,
        };
        assert!(!should_skip_for_liquidity(1_000.0, Some(&c)));
    }

    #[test]
    fn liquidity_gate_allows_missing_ctx_through() {
        assert!(!should_skip_for_liquidity(1_000.0, None));
    }
}
