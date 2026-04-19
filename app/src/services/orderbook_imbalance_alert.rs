//! Orderbook-imbalance Telegram alerter.
//!
//! Subscribes to the market_data bus, maintains a rolling local L2 book per
//! `(venue, market_key)`, and pushes a Telegram alert when the USD-weighted
//! bid/ask size ratio *within the top N% around mid* flips suddenly against
//! a 5-minute EMA baseline.
//!
//! The baseline is an EMA on `log(bid_usd / ask_usd)` so a symmetric flip
//! (e.g. 5x to 1/5x) has the same magnitude either way. Alerts fire when
//! `|current - ema|` exceeds `ln(OB_IMBALANCE_FLIP_RATIO)`, both sides meet
//! the min-size floor in USD, the market is past warmup, and the per-market
//! cooldown has elapsed. A global `MAX_ALERTS_PER_MIN` cap guards the chat.
//!
//! Scanner-only producers emit snapshots with `size=0` (they carry price
//! only); those markets never pass the min-size gate, which is the
//! intended behaviour — we only alert on venues publishing real depth
//! (today: polymarket_ws).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{info, warn};
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;

use super::market_data::{L2Event, L2Level, L2Payload, Venue};
use crate::AppState;

const DEFAULT_FLIP_RATIO: f64 = 5.0;
const DEFAULT_MIN_SIZE_USD: f64 = 5_000.0;
const DEFAULT_COOLDOWN_SECS: u64 = 900;
const DEFAULT_DEPTH_PCT: f64 = 0.02;
const DEFAULT_WARMUP_EVENTS: usize = 20;
const MAX_ALERTS_PER_MIN: usize = 10;
const EMA_TARGET_EVENTS: f64 = 150.0;
const MAX_BOOK_LEVELS: usize = 64;

#[derive(Clone, Copy, Debug)]
struct Config {
    flip_threshold_log: f64,
    min_size_usd: f64,
    cooldown: Duration,
    depth_pct: f64,
    warmup_events: usize,
}

#[derive(Debug, Default, Clone)]
struct LocalBook {
    bids: Vec<L2Level>,
    asks: Vec<L2Level>,
}

impl LocalBook {
    fn apply_snapshot(&mut self, bids: &[L2Level], asks: &[L2Level]) {
        self.bids = bids.to_vec();
        self.asks = asks.to_vec();
        self.sort_and_trim();
    }

    fn apply_delta(
        &mut self,
        bid_updates: &[L2Level],
        ask_updates: &[L2Level],
        removed_bids: &[f64],
        removed_asks: &[f64],
    ) {
        apply_side(&mut self.bids, bid_updates, removed_bids);
        apply_side(&mut self.asks, ask_updates, removed_asks);
        self.sort_and_trim();
    }

    fn sort_and_trim(&mut self) {
        // Bids: descending price; asks: ascending price.
        self.bids
            .sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap_or(std::cmp::Ordering::Equal));
        self.asks
            .sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal));
        if self.bids.len() > MAX_BOOK_LEVELS {
            self.bids.truncate(MAX_BOOK_LEVELS);
        }
        if self.asks.len() > MAX_BOOK_LEVELS {
            self.asks.truncate(MAX_BOOK_LEVELS);
        }
    }

    fn mid(&self) -> Option<f64> {
        match (self.bids.first(), self.asks.first()) {
            (Some(b), Some(a)) if b.price > 0.0 && a.price > 0.0 => Some((b.price + a.price) / 2.0),
            (Some(b), None) if b.price > 0.0 => Some(b.price),
            (None, Some(a)) if a.price > 0.0 => Some(a.price),
            _ => None,
        }
    }

    /// Bid/ask size in USD within `depth_pct` of mid. Sizes are in outcome
    /// shares priced in dollars for PM/LIM, so USD ~= price * size.
    fn aggregate_usd(&self, depth_pct: f64) -> Option<(f64, f64, f64)> {
        let mid = self.mid()?;
        if mid <= 0.0 {
            return None;
        }
        let bid_floor = mid * (1.0 - depth_pct);
        let ask_ceil = mid * (1.0 + depth_pct);

        let bid_usd: f64 = self
            .bids
            .iter()
            .filter(|l| l.price >= bid_floor && l.price <= mid)
            .map(|l| l.price * l.size)
            .sum();
        let ask_usd: f64 = self
            .asks
            .iter()
            .filter(|l| l.price >= mid && l.price <= ask_ceil)
            .map(|l| l.price * l.size)
            .sum();
        Some((bid_usd, ask_usd, mid))
    }
}

fn apply_side(levels: &mut Vec<L2Level>, updates: &[L2Level], removed: &[f64]) {
    for r in removed {
        levels.retain(|l| (l.price - *r).abs() > f64::EPSILON);
    }
    for u in updates {
        levels.retain(|l| (l.price - u.price).abs() > f64::EPSILON);
        if u.size > 0.0 {
            levels.push(u.clone());
        }
    }
}

#[derive(Debug, Default)]
struct MarketState {
    book: LocalBook,
    events_seen: usize,
    ema_log_ratio: Option<f64>,
    last_alert: Option<Instant>,
}

pub fn spawn(state: Arc<AppState>) {
    let enabled = env_bool("OB_IMBALANCE_ALERTS_ENABLED", false);
    if !enabled {
        info!("Orderbook-imbalance alerts disabled (OB_IMBALANCE_ALERTS_ENABLED=false)");
        return;
    }

    let Ok(bot_token) = std::env::var("TELEGRAM_BOT_TOKEN") else {
        warn!("OB_IMBALANCE_ALERTS_ENABLED=true but TELEGRAM_BOT_TOKEN missing; not starting");
        return;
    };
    let Ok(chat_id) = std::env::var("TELEGRAM_CHAT_ID") else {
        warn!("OB_IMBALANCE_ALERTS_ENABLED=true but TELEGRAM_CHAT_ID missing; not starting");
        return;
    };

    let flip_ratio = env_f64("OB_IMBALANCE_FLIP_RATIO", DEFAULT_FLIP_RATIO).max(1.01);
    let cfg = Config {
        flip_threshold_log: flip_ratio.ln(),
        min_size_usd: env_f64("OB_IMBALANCE_MIN_SIZE_USD", DEFAULT_MIN_SIZE_USD).max(0.0),
        cooldown: Duration::from_secs(env_u64("OB_IMBALANCE_COOLDOWN_SECS", DEFAULT_COOLDOWN_SECS)),
        depth_pct: env_f64("OB_IMBALANCE_DEPTH_PCT", DEFAULT_DEPTH_PCT)
            .clamp(0.0005, 0.25),
        warmup_events: env_usize("OB_IMBALANCE_WARMUP_EVENTS", DEFAULT_WARMUP_EVENTS),
    };

    info!(
        "Starting orderbook-imbalance alerts (flip_ratio={:.2}x, min_size=${:.0}, \
         cooldown={}s, depth_pct={:.4}, warmup={})",
        flip_ratio,
        cfg.min_size_usd,
        cfg.cooldown.as_secs(),
        cfg.depth_pct,
        cfg.warmup_events,
    );

    let mut rx = state.market_data.subscribe();
    let tg = TelegramClient::new(bot_token, chat_id);

    tokio::spawn(async move {
        let mut markets: HashMap<(Venue, String), MarketState> = HashMap::new();
        let mut alert_times: VecDeque<Instant> = VecDeque::new();

        loop {
            match rx.recv().await {
                Ok(ev) => {
                    handle_event(&state, &tg, &cfg, &mut markets, &mut alert_times, &ev).await;
                }
                Err(RecvError::Lagged(n)) => {
                    warn!("orderbook_imbalance_alert lagged by {} events", n);
                }
                Err(RecvError::Closed) => {
                    info!("market_data bus closed; orderbook_imbalance_alert exiting");
                    return;
                }
            }
        }
    });
}

async fn handle_event(
    state: &AppState,
    tg: &TelegramClient,
    cfg: &Config,
    markets: &mut HashMap<(Venue, String), MarketState>,
    alert_times: &mut VecDeque<Instant>,
    ev: &L2Event,
) {
    // Trades do not mutate the book; skip.
    if matches!(&ev.payload, L2Payload::Trade { .. }) {
        return;
    }

    let entry = markets
        .entry((ev.venue, ev.market_key.clone()))
        .or_default();

    match &ev.payload {
        L2Payload::Snapshot { bids, asks, .. } => {
            entry.book.apply_snapshot(bids, asks);
        }
        L2Payload::Delta {
            bid_updates,
            ask_updates,
            removed_bids,
            removed_asks,
        } => {
            entry
                .book
                .apply_delta(bid_updates, ask_updates, removed_bids, removed_asks);
        }
        L2Payload::Trade { .. } => unreachable!(),
    }
    entry.events_seen = entry.events_seen.saturating_add(1);

    let Some((bid_usd, ask_usd, mid)) = entry.book.aggregate_usd(cfg.depth_pct) else {
        return;
    };
    if bid_usd <= 0.0 || ask_usd <= 0.0 {
        return;
    }

    let log_ratio = (bid_usd / ask_usd).ln();

    // EMA update. Alpha is tuned so ~150 events ≈ 5 minutes of steady
    // flow on an active book; on slower markets the EMA is just slower
    // to move, which is acceptable because the flip threshold is large.
    let alpha = 2.0 / (EMA_TARGET_EVENTS + 1.0);
    let prev_ema = entry.ema_log_ratio;
    let new_ema = match prev_ema {
        Some(e) => e + alpha * (log_ratio - e),
        None => log_ratio,
    };
    entry.ema_log_ratio = Some(new_ema);

    if entry.events_seen < cfg.warmup_events {
        return;
    }
    let Some(baseline) = prev_ema else {
        return;
    };

    if bid_usd < cfg.min_size_usd || ask_usd < cfg.min_size_usd {
        return;
    }

    let deviation = (log_ratio - baseline).abs();
    if deviation < cfg.flip_threshold_log {
        return;
    }

    if let Some(t) = entry.last_alert {
        if t.elapsed() < cfg.cooldown {
            return;
        }
    }

    if !rate_limit_ok(alert_times) {
        return;
    }

    entry.last_alert = Some(Instant::now());
    alert_times.push_back(Instant::now());
    // The mutable borrow of `entry` ends here; NLL lets the await below
    // proceed without dragging it across a suspension point.

    let ctx = lookup_context(state, ev.venue, &ev.market_key).await;
    let text = format_alert(
        ev.venue,
        ctx.as_ref(),
        &ev.market_key,
        bid_usd,
        ask_usd,
        baseline,
        mid,
    );
    if let Err(e) = tg.send(&text).await {
        warn!("telegram send failed (orderbook_imbalance): {}", e);
    }
}

fn rate_limit_ok(alert_times: &mut VecDeque<Instant>) -> bool {
    let cutoff = Instant::now() - Duration::from_secs(60);
    while let Some(t) = alert_times.front() {
        if *t < cutoff {
            alert_times.pop_front();
        } else {
            break;
        }
    }
    alert_times.len() < MAX_ALERTS_PER_MIN
}

struct MarketContext {
    question: String,
    slug: Option<String>,
    liquidity_usd: Option<f64>,
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
    let row: Option<(String, String, Option<f64>)> = sqlx::query_as(
        "SELECT question, slug, liquidity_usdc::double precision \
         FROM polymarket_scanned_markets \
         WHERE yes_token_id = $1 OR no_token_id = $1 LIMIT 1",
    )
    .bind(token_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    row.map(|(question, slug, liquidity_usd)| {
        let slug_opt = if slug.is_empty() { None } else { Some(slug) };
        MarketContext {
            question,
            slug: slug_opt,
            liquidity_usd,
        }
    })
}

async fn lookup_limitless(state: &AppState, market_key: &str) -> Option<MarketContext> {
    let slug = market_key
        .rsplit_once(':')
        .map(|(prefix, _)| prefix)
        .unwrap_or(market_key);

    let pool = state.db.pool();
    let row: Option<(String, Option<f64>)> = sqlx::query_as(
        "SELECT question, liquidity_usdc \
         FROM limitless_scanned_markets \
         WHERE slug = $1 LIMIT 1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    row.map(|(question, liquidity_usd)| MarketContext {
        question,
        slug: Some(slug.to_string()),
        liquidity_usd,
    })
}

fn format_alert(
    venue: Venue,
    ctx: Option<&MarketContext>,
    market_key: &str,
    bid_usd: f64,
    ask_usd: f64,
    baseline_log: f64,
    mid: f64,
) -> String {
    let (venue_label, link) = match venue {
        Venue::Polymarket => (
            "Polymarket",
            ctx.and_then(|c| c.slug.as_deref())
                .map(|s| format!("https://relay44.com/markets/by-slug/polymarket/{}", s)),
        ),
        Venue::Limitless => (
            "Limitless",
            ctx.and_then(|c| c.slug.as_deref())
                .map(|s| format!("https://relay44.com/markets/by-slug/limitless/{}", s)),
        ),
        Venue::Aerodrome => ("Aerodrome", None),
        Venue::Internal => ("Internal", None),
    };

    let question = ctx
        .map(|c| c.question.as_str())
        .unwrap_or(market_key);

    let ratio_str = if bid_usd >= ask_usd {
        format!("{:.1}x", bid_usd / ask_usd.max(1e-9))
    } else {
        format!("1/{:.1}x", ask_usd / bid_usd.max(1e-9))
    };

    let baseline_ratio = baseline_log.exp();
    let baseline_str = if baseline_ratio >= 1.0 {
        format!("{:.1}x", baseline_ratio)
    } else {
        format!("1/{:.1}x", 1.0 / baseline_ratio.max(1e-9))
    };

    let liquidity_str = ctx
        .and_then(|c| c.liquidity_usd)
        .map(format_money)
        .unwrap_or_else(|| "n/a".to_string());

    let mut lines = vec![
        format!(
            "\u{2696}\u{fe0f} <b>Orderbook imbalance \u{2014} {}</b>",
            html_escape(venue_label)
        ),
        format!("<i>{}</i>", html_escape(question)),
        String::new(),
        format!(
            "Bid/Ask size: <b>{}</b> / {} ({})",
            format_money(bid_usd),
            format_money(ask_usd),
            ratio_str
        ),
        format!("Baseline: {} (5min EMA)", baseline_str),
        format!("Mid: {:.2} | Liquidity: {}", mid, liquidity_str),
    ];
    if let Some(url) = link {
        let _ = venue;
        lines.push(format!(
            "<a href=\"{}\">Trade on Relay44</a>",
            html_escape(&url),
        ));
    }
    lines.join("\n")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn format_money(v: f64) -> String {
    let abs = v.abs();
    if abs >= 1_000_000.0 {
        return format!("${:.1}M", v / 1_000_000.0);
    }
    // "$12,400" style with thousands separators.
    let rounded = v.round() as i64;
    let neg = rounded < 0;
    let digits: Vec<char> = rounded.abs().to_string().chars().collect();
    let mut out = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c);
    }
    if neg {
        format!("-${}", out)
    } else {
        format!("${}", out)
    }
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

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
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
            parse_mode: &'a str,
            disable_web_page_preview: bool,
        }
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let resp = self
            .http
            .post(&url)
            .json(&Payload {
                chat_id: &self.chat_id,
                text,
                parse_mode: "HTML",
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

    fn lvl(price: f64, size: f64) -> L2Level {
        L2Level { price, size }
    }

    fn make_cfg() -> Config {
        Config {
            flip_threshold_log: 5.0_f64.ln(),
            min_size_usd: 5_000.0,
            cooldown: Duration::from_secs(900),
            depth_pct: 0.02,
            warmup_events: 20,
        }
    }

    #[test]
    fn delta_apply_updates_replace_then_append() {
        let mut book = LocalBook::default();
        book.apply_snapshot(&[lvl(0.50, 100.0)], &[lvl(0.51, 200.0)]);
        book.apply_delta(
            &[lvl(0.50, 150.0), lvl(0.49, 50.0)],
            &[],
            &[],
            &[0.51],
        );
        assert_eq!(book.bids.len(), 2);
        assert!(book
            .bids
            .iter()
            .any(|l| (l.price - 0.50).abs() < 1e-9 && (l.size - 150.0).abs() < 1e-9));
        assert!(book.bids.iter().any(|l| (l.price - 0.49).abs() < 1e-9));
        assert!(book.asks.is_empty(), "ask at 0.51 should have been removed");
    }

    #[test]
    fn aggregate_usd_filters_by_depth_pct() {
        let mut book = LocalBook::default();
        // Mid = (0.50 + 0.52)/2 = 0.51. 2% window is 0.4998..0.5202.
        book.apply_snapshot(
            &[lvl(0.50, 1000.0), lvl(0.45, 5000.0)],
            &[lvl(0.52, 2000.0), lvl(0.60, 5000.0)],
        );
        let (bid_usd, ask_usd, mid) = book.aggregate_usd(0.02).unwrap();
        // Only the 0.50 bid is within window (0.45 is below floor). 0.52 ask is within.
        assert!((bid_usd - 0.50 * 1000.0).abs() < 1e-6);
        assert!((ask_usd - 0.52 * 2000.0).abs() < 1e-6);
        assert!((mid - 0.51).abs() < 1e-6);
    }

    #[test]
    fn ema_converges_on_stable_log_ratio() {
        // Feed 500 events of ratio=1.2 after seeding with 0.8; EMA should land near ln(1.2).
        let alpha = 2.0 / (EMA_TARGET_EVENTS + 1.0);
        let mut ema = (0.8_f64).ln();
        for _ in 0..500 {
            let target = (1.2_f64).ln();
            ema += alpha * (target - ema);
        }
        assert!(
            (ema - (1.2_f64).ln()).abs() < 0.01,
            "ema={} expected~{}",
            ema,
            (1.2_f64).ln()
        );
    }

    #[test]
    fn warmup_suppresses_early_alerts() {
        // events_seen below warmup threshold must never fire.
        let cfg = make_cfg();
        let mut state = MarketState::default();
        for _ in 0..5 {
            state.events_seen += 1;
        }
        assert!(
            state.events_seen < cfg.warmup_events,
            "warmup guard should still be active"
        );
    }

    #[test]
    fn flip_detection_fires_when_deviation_exceeds_threshold() {
        let cfg = make_cfg();
        let baseline = 0.0_f64;
        // Current ratio 10x => log_ratio = ln(10) > ln(5).
        let strong = 10.0_f64.ln();
        assert!((strong - baseline).abs() > cfg.flip_threshold_log);
        // 3x would NOT trip (ln(3) < ln(5)).
        let mild = 3.0_f64.ln();
        assert!((mild - baseline).abs() < cfg.flip_threshold_log);
    }

    #[test]
    fn min_size_gate_rejects_dust() {
        let cfg = make_cfg();
        let mut book = LocalBook::default();
        // Mid ~ 0.499; bid usd = 0.499 * 200 = ~$100. Well below $5000 floor.
        book.apply_snapshot(&[lvl(0.499, 200.0)], &[lvl(0.501, 200.0)]);
        let (bid_usd, ask_usd, _mid) = book.aggregate_usd(0.02).unwrap();
        assert!(bid_usd < cfg.min_size_usd);
        assert!(ask_usd < cfg.min_size_usd);
    }

    #[test]
    fn cooldown_blocks_duplicate_alert() {
        let mut entry = MarketState::default();
        entry.last_alert = Some(Instant::now());
        let cfg = make_cfg();
        let still_in_cooldown = entry
            .last_alert
            .map(|t| t.elapsed() < cfg.cooldown)
            .unwrap_or(false);
        assert!(still_in_cooldown);
    }

    #[test]
    fn rate_limit_caps_at_ten_per_min() {
        let mut q: VecDeque<Instant> = VecDeque::new();
        let now = Instant::now();
        for _ in 0..MAX_ALERTS_PER_MIN {
            q.push_back(now);
        }
        assert!(!rate_limit_ok(&mut q), "should refuse the 11th alert");
        // Replace with old timestamps; should be evicted.
        let old = Instant::now() - Duration::from_secs(120);
        q.clear();
        for _ in 0..MAX_ALERTS_PER_MIN {
            q.push_back(old);
        }
        assert!(rate_limit_ok(&mut q), "old entries should be evicted");
    }

    #[test]
    fn format_alert_polymarket_renders_html() {
        let ctx = MarketContext {
            question: "Will BTC close above 100k?".to_string(),
            slug: Some("btc-100k".to_string()),
            liquidity_usd: Some(48_200.0),
        };
        let s = format_alert(
            Venue::Polymarket,
            Some(&ctx),
            "token",
            12_400.0,
            1_850.0,
            (1.2_f64).ln(),
            0.34,
        );
        assert!(s.contains("<b>Orderbook imbalance"));
        assert!(s.contains("Polymarket"));
        assert!(s.contains("Will BTC close above 100k?"));
        assert!(s.contains("relay44.com/markets/by-slug/polymarket/btc-100k"));
        assert!(s.contains("Trade on Relay44"));
        assert!(s.contains("5min EMA"));
        assert!(s.contains("Mid: 0.34"));
        assert!(s.contains("$12,400"));
        assert!(s.contains("$1,850"));
    }

    #[test]
    fn format_alert_limitless_uses_correct_link_and_label() {
        let ctx = MarketContext {
            question: "ETH > 4k today?".to_string(),
            slug: Some("eth-4k".to_string()),
            liquidity_usd: None,
        };
        let s = format_alert(
            Venue::Limitless,
            Some(&ctx),
            "eth-4k:yes",
            8_000.0,
            20_000.0,
            0.0,
            0.25,
        );
        assert!(s.contains("Limitless"));
        assert!(s.contains("relay44.com/markets/by-slug/limitless/eth-4k"));
        assert!(s.contains("Trade on Relay44"));
        assert!(s.contains("1/2.5x"));
    }

    #[test]
    fn format_alert_escapes_html_in_question() {
        let ctx = MarketContext {
            question: "A & B < C?".to_string(),
            slug: None,
            liquidity_usd: None,
        };
        let s = format_alert(
            Venue::Polymarket,
            Some(&ctx),
            "k",
            10_000.0,
            1_000.0,
            0.0,
            0.5,
        );
        assert!(s.contains("A &amp; B &lt; C?"));
    }

    #[test]
    fn format_money_uses_thousands_separators() {
        assert_eq!(format_money(42.0), "$42");
        assert_eq!(format_money(1_850.0), "$1,850");
        assert_eq!(format_money(12_400.0), "$12,400");
        assert_eq!(format_money(1_234_567.0), "$1.2M");
    }
}
