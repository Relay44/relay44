//! Cross-venue arbitrage alerter.
//!
//! Subscribes to the market_data bus. For every Polymarket ↔ Limitless pair
//! that `limitless_scanner` has linked (rows in `market_venue_links`), we keep
//! the most recent observed price per (pair, outcome, venue). When the same
//! outcome diverges across the two venues by more than
//! `CROSS_VENUE_ARB_THRESHOLD` (absolute probability, default 0.05 = 5¢) and
//! both sides are fresh, we push a Telegram alert. Each (pair, outcome) has
//! an independent cooldown so a flapping book can't spam the channel.
//!
//! Message layout is HTML and uses the shared helpers in `telegram_format` so
//! this alerter stays consistent with `probability_alert` and
//! `new_market_alert`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{info, warn};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::RwLock;

use super::market_data::{L2Event, L2Payload, Venue};
use super::telegram_format::{
    format_deep_link, format_metadata_row, html_escape, TelegramClient,
};
use crate::AppState;

const DEFAULT_THRESHOLD: f64 = 0.05;
const DEFAULT_COOLDOWN_SECS: u64 = 600;
const DEFAULT_REFRESH_SECS: u64 = 600;
const FRESHNESS_SECS: u64 = 300;
const MIN_PRICE: f64 = 0.03;
const MAX_PRICE: f64 = 0.97;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Outcome {
    Yes,
    No,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Yes => "YES",
            Outcome::No => "NO",
        }
    }
}

#[derive(Clone, Debug, Default)]
struct PairMeta {
    question: String,
    pm_slug: Option<String>,
    lim_slug: Option<String>,
    category: Option<String>,
    liquidity_usd: Option<f64>,
    volume_24h_usd: Option<f64>,
}

struct PairLookup {
    by_venue_key: HashMap<(Venue, String), (String, Outcome)>,
    meta: HashMap<String, PairMeta>,
}

pub fn spawn(state: Arc<AppState>) {
    let enabled = env_bool("CROSS_VENUE_ARB_ENABLED", false);
    if !enabled {
        info!("Cross-venue arb alerts disabled (CROSS_VENUE_ARB_ENABLED=false)");
        return;
    }

    let Ok(bot_token) = std::env::var("TELEGRAM_BOT_TOKEN") else {
        warn!("CROSS_VENUE_ARB_ENABLED=true but TELEGRAM_BOT_TOKEN missing; not starting");
        return;
    };
    let Ok(chat_id) = std::env::var("TELEGRAM_CHAT_ID") else {
        warn!("CROSS_VENUE_ARB_ENABLED=true but TELEGRAM_CHAT_ID missing; not starting");
        return;
    };

    let threshold = env_f64("CROSS_VENUE_ARB_THRESHOLD", DEFAULT_THRESHOLD);
    let cooldown = Duration::from_secs(env_u64(
        "CROSS_VENUE_ARB_COOLDOWN_SECS",
        DEFAULT_COOLDOWN_SECS,
    ));
    let refresh_secs = env_u64("CROSS_VENUE_ARB_REFRESH_SECS", DEFAULT_REFRESH_SECS);

    info!(
        "Starting cross-venue arb alerts (threshold={:.3}, cooldown={}s, refresh={}s)",
        threshold,
        cooldown.as_secs(),
        refresh_secs
    );

    let lookup: Arc<RwLock<PairLookup>> = Arc::new(RwLock::new(PairLookup {
        by_venue_key: HashMap::new(),
        meta: HashMap::new(),
    }));

    {
        let lookup = lookup.clone();
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                match load_pairs(&state).await {
                    Ok(new_lookup) => {
                        let n = new_lookup.by_venue_key.len();
                        *lookup.write().await = new_lookup;
                        info!("cross-venue arb: loaded {} venue-keys", n);
                    }
                    Err(e) => warn!("cross-venue arb: pair refresh failed: {}", e),
                }
                tokio::time::sleep(Duration::from_secs(refresh_secs)).await;
            }
        });
    }

    let mut rx = state.market_data.subscribe();
    let tg = TelegramClient::new(bot_token, chat_id);

    tokio::spawn(async move {
        let mut last_price: HashMap<(String, Outcome, Venue), (f64, Instant)> = HashMap::new();
        let mut last_alert: HashMap<(String, Outcome), Instant> = HashMap::new();

        loop {
            match rx.recv().await {
                Ok(ev) => {
                    handle_event(
                        &tg,
                        &lookup,
                        threshold,
                        cooldown,
                        &mut last_price,
                        &mut last_alert,
                        &ev,
                    )
                    .await;
                }
                Err(RecvError::Lagged(n)) => {
                    warn!("cross_venue_arb lagged by {} events", n);
                }
                Err(RecvError::Closed) => {
                    info!("market_data bus closed; cross_venue_arb exiting");
                    return;
                }
            }
        }
    });
}

async fn handle_event(
    tg: &TelegramClient,
    lookup: &RwLock<PairLookup>,
    threshold: f64,
    cooldown: Duration,
    last_price: &mut HashMap<(String, Outcome, Venue), (f64, Instant)>,
    last_alert: &mut HashMap<(String, Outcome), Instant>,
    ev: &L2Event,
) {
    let Some(price) = current_price(&ev.payload) else {
        return;
    };
    if price < MIN_PRICE || price > MAX_PRICE {
        return;
    }

    let (pair_id, outcome) = {
        let l = lookup.read().await;
        let Some((p, o)) = l.by_venue_key.get(&(ev.venue, ev.market_key.clone())) else {
            return;
        };
        (p.clone(), *o)
    };

    let now = Instant::now();
    last_price.insert((pair_id.clone(), outcome, ev.venue), (price, now));

    let other = match ev.venue {
        Venue::Polymarket => Venue::Limitless,
        Venue::Limitless => Venue::Polymarket,
        _ => return,
    };

    let Some(&(other_price, other_ts)) = last_price.get(&(pair_id.clone(), outcome, other)) else {
        return;
    };
    if now.duration_since(other_ts) > Duration::from_secs(FRESHNESS_SECS) {
        return;
    }

    let delta = (price - other_price).abs();
    if delta < threshold {
        return;
    }

    let alert_key = (pair_id.clone(), outcome);
    if let Some(t) = last_alert.get(&alert_key) {
        if t.elapsed() < cooldown {
            return;
        }
    }
    last_alert.insert(alert_key, now);

    let (pm_price, lim_price) = match ev.venue {
        Venue::Polymarket => (price, other_price),
        Venue::Limitless => (other_price, price),
        _ => return,
    };

    let meta = {
        let l = lookup.read().await;
        l.meta.get(&pair_id).cloned().unwrap_or_else(|| PairMeta {
            question: pair_id.clone(),
            ..Default::default()
        })
    };

    let text = format_alert(&meta, outcome, pm_price, lim_price);
    if let Err(e) = tg.send(&text).await {
        warn!("telegram send failed (cross-venue arb): {}", e);
    }
}

fn current_price(payload: &L2Payload) -> Option<f64> {
    match payload {
        L2Payload::Snapshot { bids, asks, .. } => match (bids.first(), asks.first()) {
            (Some(b), Some(a)) => Some((b.price + a.price) / 2.0),
            (Some(b), None) => Some(b.price),
            (None, Some(a)) => Some(a.price),
            (None, None) => None,
        },
        L2Payload::Trade { price, .. } => Some(*price),
        L2Payload::Delta { .. } => None,
    }
}

async fn load_pairs(state: &AppState) -> Result<PairLookup, String> {
    let pool = state.db.pool();

    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT
            pm.market_slug,
            lm.provider_market_id AS limitless_slug,
            psm.yes_token_id,
            psm.no_token_id
        FROM market_venue_links pm
        JOIN market_venue_links lm
            ON pm.market_slug = lm.market_slug
           AND lm.provider = 'limitless'
           AND lm.active = TRUE
        JOIN polymarket_scanned_markets psm
            ON psm.condition_id = pm.provider_market_id
        WHERE pm.provider = 'polymarket'
          AND pm.active = TRUE
          AND psm.active = TRUE
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("load_pairs venue-keys: {}", e))?;

    // Per-pair metadata from the Polymarket side (question, slug, category,
    // liquidity, volume). We deliberately prefer PM's metadata here because
    // it's historically more complete — Limitless categories are often
    // unset or "unknown".
    let meta_rows: Vec<(
        String,
        String,
        Option<String>,
        Option<String>,
        Option<f64>,
        Option<f64>,
    )> = sqlx::query_as(
        r#"
        SELECT
            pm.market_slug,
            psm.question,
            psm.slug,
            psm.category,
            psm.liquidity_usdc::double precision,
            psm.volume_usdc::double precision
        FROM market_venue_links pm
        JOIN polymarket_scanned_markets psm
            ON psm.condition_id = pm.provider_market_id
        WHERE pm.provider = 'polymarket' AND pm.active = TRUE
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("load_pairs meta: {}", e))?;

    let limitless_slugs: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT market_slug, provider_market_id
        FROM market_venue_links
        WHERE provider = 'limitless' AND active = TRUE
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("load_pairs lim slugs: {}", e))?;

    let mut by_venue_key = HashMap::new();
    for (market_slug, lim_slug, yes_tok, no_tok) in rows {
        by_venue_key.insert(
            (Venue::Polymarket, yes_tok),
            (market_slug.clone(), Outcome::Yes),
        );
        by_venue_key.insert(
            (Venue::Polymarket, no_tok),
            (market_slug.clone(), Outcome::No),
        );
        by_venue_key.insert(
            (Venue::Limitless, format!("{}:yes", lim_slug)),
            (market_slug.clone(), Outcome::Yes),
        );
        by_venue_key.insert(
            (Venue::Limitless, format!("{}:no", lim_slug)),
            (market_slug, Outcome::No),
        );
    }

    let mut meta: HashMap<String, PairMeta> = HashMap::new();
    for (market_slug, question, pm_slug, category, liquidity, volume) in meta_rows {
        meta.insert(
            market_slug,
            PairMeta {
                question,
                pm_slug: pm_slug.filter(|s| !s.is_empty()),
                lim_slug: None,
                category: category.and_then(|c| {
                    let t = c.trim();
                    if t.is_empty() || t.eq_ignore_ascii_case("unknown") {
                        None
                    } else {
                        Some(t.to_string())
                    }
                }),
                liquidity_usd: liquidity,
                volume_24h_usd: volume,
            },
        );
    }
    for (market_slug, lim_slug) in limitless_slugs {
        meta.entry(market_slug)
            .or_insert_with(PairMeta::default)
            .lim_slug = Some(lim_slug);
    }

    Ok(PairLookup { by_venue_key, meta })
}

fn format_alert(meta: &PairMeta, outcome: Outcome, pm: f64, lim: f64) -> String {
    let edge_cents = (pm - lim).abs() * 100.0;
    let direction = if pm > lim { "PM↑ LIM↓" } else { "PM↓ LIM↑" };

    let mut lines: Vec<String> = Vec::new();
    // Arb header uses a neutral venue label since the signal spans both.
    lines.push("<b>\u{2696}\u{FE0F} Cross-venue arb — PM vs LIM</b>".to_string());
    lines.push(format!("<i>{}</i>", html_escape(&meta.question)));
    lines.push(format!(
        "{} {}: Polymarket {:.1}¢ | Limitless {:.1}¢ (Δ {:.1}¢)",
        outcome.as_str(),
        direction,
        pm * 100.0,
        lim * 100.0,
        edge_cents
    ));

    let meta_row = format_metadata_row(
        meta.liquidity_usd,
        meta.volume_24h_usd,
        meta.category.as_deref(),
    );
    if !meta_row.is_empty() {
        lines.push(meta_row);
    }

    // Dual footer: one link per venue so readers can jump to whichever side
    // they're more comfortable trading on.
    let mut link_parts: Vec<String> = Vec::new();
    if let Some(slug) = &meta.pm_slug {
        if let Some(link) = format_deep_link("polymarket", slug) {
            link_parts.push(link);
        }
    }
    if let Some(slug) = &meta.lim_slug {
        if let Some(link) = format_deep_link("limitless", slug) {
            link_parts.push(link);
        }
    }
    if !link_parts.is_empty() {
        lines.push(link_parts.join(" | "));
    }

    lines.join("\n")
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

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::market_data::L2Level;

    fn meta(question: &str) -> PairMeta {
        PairMeta {
            question: question.to_string(),
            pm_slug: Some("pm-slug".to_string()),
            lim_slug: Some("lim-slug".to_string()),
            category: Some("politics".to_string()),
            liquidity_usd: Some(50_000.0),
            volume_24h_usd: Some(1_200_000.0),
        }
    }

    #[test]
    fn format_shows_question_outcome_and_edge() {
        let s = format_alert(&meta("Will BTC hit 100k by EOY?"), Outcome::Yes, 0.42, 0.37);
        assert!(s.contains("Will BTC hit 100k by EOY?"));
        assert!(s.contains("YES"));
        assert!(s.contains("42.0¢"));
        assert!(s.contains("37.0¢"));
        assert!(s.contains("Δ 5.0¢"));
    }

    #[test]
    fn format_header_identifies_pm_vs_lim() {
        let s = format_alert(&meta("Q"), Outcome::Yes, 0.50, 0.40);
        assert!(s.contains("<b>"));
        assert!(s.contains("Cross-venue arb"));
        assert!(s.contains("PM vs LIM"));
    }

    #[test]
    fn format_marks_direction() {
        let up = format_alert(&meta("Q"), Outcome::No, 0.60, 0.45);
        assert!(up.contains("PM↑ LIM↓"));
        let dn = format_alert(&meta("Q"), Outcome::No, 0.30, 0.45);
        assert!(dn.contains("PM↓ LIM↑"));
    }

    #[test]
    fn format_includes_metadata_and_dual_links() {
        let s = format_alert(&meta("Q"), Outcome::Yes, 0.50, 0.40);
        assert!(s.contains("Liquidity: $50.0k"));
        assert!(s.contains("24h vol: $1.2M"));
        assert!(s.contains("Category: politics"));
        assert!(s.contains("polymarket.com/market/pm-slug"));
        assert!(s.contains("limitless.exchange/markets/lim-slug"));
        assert!(s.contains("Open on Polymarket"));
        assert!(s.contains("Open on Limitless"));
    }

    #[test]
    fn format_html_escapes_question() {
        let s = format_alert(&meta("A<B>&C"), Outcome::Yes, 0.50, 0.40);
        assert!(s.contains("A&lt;B&gt;&amp;C"));
    }

    #[test]
    fn current_price_uses_midpoint_when_both_sides_present() {
        let p = current_price(&L2Payload::Snapshot {
            bids: vec![L2Level {
                price: 0.40,
                size: 100.0,
            }],
            asks: vec![L2Level {
                price: 0.50,
                size: 100.0,
            }],
            last_trade: None,
        });
        assert_eq!(p, Some(0.45));
    }

    #[test]
    fn current_price_falls_back_to_one_side() {
        let only_bid = current_price(&L2Payload::Snapshot {
            bids: vec![L2Level {
                price: 0.33,
                size: 10.0,
            }],
            asks: vec![],
            last_trade: None,
        });
        assert_eq!(only_bid, Some(0.33));
    }

    #[test]
    fn delta_is_ignored_on_book_update() {
        let p = current_price(&L2Payload::Delta {
            bid_updates: vec![],
            ask_updates: vec![],
            removed_bids: vec![],
            removed_asks: vec![],
        });
        assert!(p.is_none());
    }
}
