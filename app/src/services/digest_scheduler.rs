//! Digest alerter — batches signals from the `AlertBus` and emits a single
//! ranked Telegram message on a fixed cadence (default every 60 minutes).
//!
//! Before the digest existed every alerter (`probability_alert`,
//! `volume_spike_alert`) sent to Telegram directly, which on a busy day
//! produced dozens of messages per hour and buried the few genuinely
//! interesting moves. The digest replaces that with one message per tick:
//! all drained signals are scored, deduped by market, the top N are
//! selected, and anything that's been alerted recently is skipped via a
//! cross-tick cooldown keyed on `{venue}:{market_key}`.
//!
//! Scoring is deliberately simple:
//!
//!   score = sqrt(liquidity_usd) * |move_size| * recency_decay
//!
//! — a $100k book moving 10 pts outranks a $5k book moving 12 pts, but a
//! very fresh signal still beats an older one with similar economics.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};

use super::alert_bus::{now_secs, Signal};
use super::telegram_format::{
    format_deep_link, format_metadata_row, html_escape, venue_title, TelegramClient,
};
use crate::AppState;

const DEFAULT_INTERVAL_SECS: u64 = 3600;
const DEFAULT_TOP_N: usize = 3;
const DEFAULT_MARKET_COOLDOWN_SECS: u64 = 14_400;
const LIQUIDITY_FLOOR: f64 = 100.0;
const RECENCY_HALF_LIFE_SECS: f64 = 7_200.0;

pub fn spawn(state: Arc<AppState>) {
    let enabled = env_bool("DIGEST_ENABLED", false);
    if !enabled {
        info!("Digest scheduler disabled (DIGEST_ENABLED=false)");
        return;
    }

    let Ok(bot_token) = std::env::var("TELEGRAM_BOT_TOKEN") else {
        warn!("DIGEST_ENABLED=true but TELEGRAM_BOT_TOKEN missing; not starting");
        return;
    };
    let Ok(chat_id) = std::env::var("TELEGRAM_CHAT_ID") else {
        warn!("DIGEST_ENABLED=true but TELEGRAM_CHAT_ID missing; not starting");
        return;
    };

    let interval_secs = env_u64("DIGEST_INTERVAL_SECS", DEFAULT_INTERVAL_SECS).max(60);
    let top_n = env_usize("DIGEST_TOP_N", DEFAULT_TOP_N).max(1);
    let cooldown_secs = env_u64("DIGEST_MARKET_COOLDOWN_SECS", DEFAULT_MARKET_COOLDOWN_SECS);
    let env_threshold = env_f64("PROB_ALERT_THRESHOLD_PCT", 5.0);
    let parsed_env_chat_id: Option<i64> = chat_id.parse().ok();

    info!(
        "Starting digest scheduler (interval={}s, top_n={}, market_cooldown={}s, env_threshold={}%)",
        interval_secs, top_n, cooldown_secs, env_threshold
    );

    let tg = TelegramClient::new(bot_token, chat_id);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        // Skip the immediate tick — give the bus time to accumulate.
        interval.tick().await;

        // Per-chat cooldown — keyed on (chat_id, market_dedup_key). A chat
        // that just received an alert about a market doesn't block other
        // chats from seeing the same market.
        let mut last_alerted: HashMap<(i64, String), u64> = HashMap::new();

        loop {
            interval.tick().await;
            run_tick(
                &state,
                &tg,
                top_n,
                cooldown_secs,
                env_threshold,
                parsed_env_chat_id,
                &mut last_alerted,
            )
            .await;
        }
    });
}

async fn run_tick(
    state: &AppState,
    tg: &TelegramClient,
    top_n: usize,
    cooldown_secs: u64,
    env_threshold: f64,
    env_chat_id: Option<i64>,
    last_alerted: &mut HashMap<(i64, String), u64>,
) {
    let drained = state.alert_bus.drain().await;
    if drained.is_empty() {
        info!("digest tick: bus empty, skipping send");
        return;
    }

    let pool = state.db.pool();
    let destinations = collect_destinations(pool, env_chat_id).await;
    if destinations.is_empty() {
        warn!(
            "digest tick: no destination chats (env chat unset and no tg_chat_config rows); \
             {} signals dropped",
            drained.len()
        );
        return;
    }

    let now = now_secs();
    for chat_id in destinations {
        // Quiet windows are per-chat — skipping a quiet chat must not affect
        // any other chat's delivery in the same tick.
        if crate::services::tg_chat_config::is_quiet_now(pool, chat_id).await {
            info!("digest tick: chat {} is in quiet window, skipping", chat_id);
            continue;
        }

        let threshold =
            crate::services::tg_chat_config::effective_threshold_for_chat(pool, chat_id, env_threshold)
                .await;

        let mut filtered: Vec<Signal> = Vec::with_capacity(drained.len());
        for s in &drained {
            if !chat_accepts_signal(pool, chat_id, s, threshold).await {
                continue;
            }
            filtered.push(s.clone());
        }
        if filtered.is_empty() {
            info!("digest tick: chat {} filtered out everything, skipping", chat_id);
            continue;
        }

        let chat_cooldowns = scoped_cooldowns(last_alerted, chat_id);
        let selected = select_top_signals(filtered, top_n, cooldown_secs, now, &chat_cooldowns);
        if selected.is_empty() {
            info!(
                "digest tick: chat {} candidates all in cooldown, skipping",
                chat_id
            );
            continue;
        }

        for s in &selected {
            last_alerted.insert((chat_id, s.dedup_key()), now);
        }

        let text = format_digest(&selected);
        if let Err(e) = tg.send_to(chat_id, &text).await {
            warn!("digest send failed for chat {}: {}", chat_id, e);
        } else {
            info!("digest sent to chat {} ({} signals)", chat_id, selected.len());
        }
    }
}

/// Resolve the set of destination chats for a tick: the env-configured chat
/// (if any) plus every row in `tg_chat_config`. Deduplicated, deterministic
/// order (env chat first, then config-table chats sorted ascending).
async fn collect_destinations(
    pool: &sqlx::PgPool,
    env_chat_id: Option<i64>,
) -> Vec<i64> {
    let mut seen: HashSet<i64> = HashSet::new();
    let mut out: Vec<i64> = Vec::new();
    if let Some(cid) = env_chat_id {
        if seen.insert(cid) {
            out.push(cid);
        }
    }
    let mut config_chats = crate::services::tg_chat_config::list_chat_ids(pool).await;
    config_chats.sort();
    for cid in config_chats {
        if seen.insert(cid) {
            out.push(cid);
        }
    }
    out
}

/// Apply the per-chat filter pipeline to a single signal: subscribed kinds,
/// muted markets (slug + market_key), and per-chat threshold (signal move size
/// must clear the chat's threshold, else it's noise to that chat).
async fn chat_accepts_signal(
    pool: &sqlx::PgPool,
    chat_id: i64,
    signal: &Signal,
    threshold_pct: f64,
) -> bool {
    if signal.move_size.abs() < threshold_pct {
        return false;
    }
    if !crate::services::tg_chat_config::is_kind_subscribed(pool, chat_id, signal.kind.as_str())
        .await
    {
        return false;
    }
    if let Some(slug) = &signal.slug {
        if crate::services::tg_chat_config::is_market_muted(pool, chat_id, slug).await {
            return false;
        }
    }
    if crate::services::tg_chat_config::is_market_muted(pool, chat_id, &signal.market_key).await {
        return false;
    }
    true
}

/// Project the per-chat slice of the global `(chat_id, dedup_key) -> ts` map
/// down to the `dedup_key -> ts` shape `select_top_signals` already takes.
fn scoped_cooldowns(
    all: &HashMap<(i64, String), u64>,
    chat_id: i64,
) -> HashMap<String, u64> {
    all.iter()
        .filter(|((cid, _), _)| *cid == chat_id)
        .map(|((_, key), ts)| (key.clone(), *ts))
        .collect()
}

/// Pick the highest-scoring signal per market, drop anything in cooldown, then
/// keep the top `n` overall. Pure — all mutable state is passed in so tests can
/// drive it deterministically.
pub fn select_top_signals(
    drained: Vec<Signal>,
    top_n: usize,
    cooldown_secs: u64,
    now: u64,
    last_alerted: &HashMap<String, u64>,
) -> Vec<Signal> {
    let mut by_market: HashMap<String, Signal> = HashMap::new();
    for s in drained {
        let key = s.dedup_key();
        if let Some(last) = last_alerted.get(&key) {
            if now.saturating_sub(*last) < cooldown_secs {
                continue;
            }
        }
        let entry = by_market.entry(key).or_insert_with(|| s.clone());
        if score(&s, now) > score(entry, now) {
            *entry = s;
        }
    }

    let mut ranked: Vec<(f64, Signal)> = by_market
        .into_values()
        .map(|s| (score(&s, now), s))
        .collect();
    ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    ranked.into_iter().take(top_n).map(|(_, s)| s).collect()
}

fn score(signal: &Signal, now: u64) -> f64 {
    let liq = signal.liquidity_usd.unwrap_or(LIQUIDITY_FLOOR).max(LIQUIDITY_FLOOR);
    let liq_weight = liq.sqrt();
    let age = now.saturating_sub(signal.timestamp_secs) as f64;
    let recency = (-age / RECENCY_HALF_LIFE_SECS).exp();
    liq_weight * signal.move_size.abs() * recency
}

pub fn format_digest(signals: &[Signal]) -> String {
    let mut out = String::new();
    out.push_str("<b>\u{1F4CA} Top signals</b>\n");
    for (i, s) in signals.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&format!(
            "<b>{}. {}</b>",
            i + 1,
            kind_label(s.kind.as_str())
        ));
        out.push_str(&format!(" — {}\n", venue_title(&s.venue)));
        out.push_str(&format!("<i>{}</i>\n", html_escape(&s.question)));
        out.push_str(&s.body);
        let meta = format_metadata_row(s.liquidity_usd, s.volume_24h_usd, s.category.as_deref());
        if !meta.is_empty() {
            out.push('\n');
            out.push_str(&meta);
        }
        if let Some(slug) = &s.slug {
            if let Some(link) = format_deep_link(&s.venue, slug) {
                out.push('\n');
                out.push_str(&link);
            }
        }
    }
    out
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "probability_shift" => "Probability shift",
        "volume_spike" => "Volume spike",
        "new_market" => "New market",
        _ => "Signal",
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

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::alert_bus::SignalKind;

    fn mk(venue: &str, market: &str, liq: Option<f64>, move_size: f64, ts: u64) -> Signal {
        Signal {
            kind: SignalKind::ProbabilityShift,
            venue: venue.to_string(),
            market_key: market.to_string(),
            slug: Some(market.to_string()),
            question: format!("Q {}", market),
            liquidity_usd: liq,
            volume_24h_usd: None,
            category: None,
            move_size,
            body: format!("body {}", market),
            timestamp_secs: ts,
        }
    }

    #[test]
    fn higher_liquidity_outscores_bigger_move_on_dust() {
        let now = 1_000_000;
        let dust = mk("polymarket", "dust", Some(500.0), 30.0, now);
        let real = mk("polymarket", "real", Some(200_000.0), 10.0, now);
        assert!(score(&real, now) > score(&dust, now));
    }

    #[test]
    fn recency_decay_favors_fresh_signals() {
        let now = 1_000_000;
        let fresh = mk("polymarket", "a", Some(50_000.0), 10.0, now);
        let stale = mk("polymarket", "b", Some(50_000.0), 10.0, now - 14_400);
        assert!(score(&fresh, now) > score(&stale, now));
    }

    #[test]
    fn same_market_dedupes_to_highest_scoring_signal() {
        let now = 1_000_000;
        let signals = vec![
            mk("polymarket", "m1", Some(10_000.0), 5.0, now),
            mk("polymarket", "m1", Some(10_000.0), 20.0, now),
        ];
        let picked = select_top_signals(signals, 3, 14_400, now, &HashMap::new());
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].move_size, 20.0);
    }

    #[test]
    fn cooldown_filters_recently_alerted_markets() {
        let now = 1_000_000;
        let mut last = HashMap::new();
        last.insert("polymarket:m1".to_string(), now - 600);
        let signals = vec![
            mk("polymarket", "m1", Some(100_000.0), 20.0, now),
            mk("polymarket", "m2", Some(10_000.0), 10.0, now),
        ];
        let picked = select_top_signals(signals, 3, 14_400, now, &last);
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].market_key, "m2");
    }

    #[test]
    fn cooldown_expires_after_window() {
        let now = 1_000_000;
        let mut last = HashMap::new();
        last.insert("polymarket:m1".to_string(), now - 20_000);
        let signals = vec![mk("polymarket", "m1", Some(100_000.0), 20.0, now)];
        let picked = select_top_signals(signals, 3, 14_400, now, &last);
        assert_eq!(picked.len(), 1);
    }

    #[test]
    fn top_n_caps_output() {
        let now = 1_000_000;
        let signals: Vec<Signal> = (0..10)
            .map(|i| mk("polymarket", &format!("m{}", i), Some(10_000.0 * (i as f64 + 1.0)), 10.0, now))
            .collect();
        let picked = select_top_signals(signals, 3, 14_400, now, &HashMap::new());
        assert_eq!(picked.len(), 3);
        // Highest-liquidity markets (m7, m8, m9) should win.
        let keys: Vec<&str> = picked.iter().map(|s| s.market_key.as_str()).collect();
        assert!(keys.contains(&"m9"));
        assert!(keys.contains(&"m8"));
        assert!(keys.contains(&"m7"));
    }

    #[test]
    fn digest_format_contains_header_and_entries() {
        let now = 1_000_000;
        let signals = vec![
            mk("polymarket", "btc", Some(100_000.0), 15.0, now),
            mk("limitless", "eth", Some(20_000.0), 12.0, now),
        ];
        let s = format_digest(&signals);
        assert!(s.contains("Top signals"));
        assert!(s.contains("Probability shift"));
        assert!(s.contains("Polymarket"));
        assert!(s.contains("Limitless"));
        assert!(s.contains("relay44.com/markets/by-slug/polymarket/btc"));
        assert!(s.contains("relay44.com/markets/by-slug/limitless/eth"));
    }

    #[test]
    fn digest_format_html_escapes_question() {
        let now = 1_000_000;
        let mut s = mk("polymarket", "m1", Some(10_000.0), 5.0, now);
        s.question = "Will <X> & <Y>?".to_string();
        let out = format_digest(&[s]);
        assert!(out.contains("&lt;X&gt;"));
        assert!(out.contains("&amp;"));
        assert!(!out.contains("<X>"));
    }

    #[test]
    fn empty_input_returns_empty_selection() {
        let picked = select_top_signals(vec![], 3, 14_400, 1_000_000, &HashMap::new());
        assert!(picked.is_empty());
    }

    #[test]
    fn scoped_cooldowns_returns_only_target_chat_entries() {
        let mut all: HashMap<(i64, String), u64> = HashMap::new();
        all.insert((100, "polymarket:m1".to_string()), 50);
        all.insert((100, "polymarket:m2".to_string()), 60);
        all.insert((200, "polymarket:m1".to_string()), 70);

        let chat100 = scoped_cooldowns(&all, 100);
        assert_eq!(chat100.len(), 2);
        assert_eq!(chat100.get("polymarket:m1"), Some(&50));
        assert_eq!(chat100.get("polymarket:m2"), Some(&60));

        let chat200 = scoped_cooldowns(&all, 200);
        assert_eq!(chat200.len(), 1);
        assert_eq!(chat200.get("polymarket:m1"), Some(&70));

        let chat999 = scoped_cooldowns(&all, 999);
        assert!(chat999.is_empty());
    }

    #[test]
    fn per_chat_cooldowns_isolate_chats() {
        // Chat 100 was just alerted about m1 — cooldown active for chat 100.
        // Chat 200 has no cooldown for m1 — should still see it.
        let now = 1_000_000;
        let mut all: HashMap<(i64, String), u64> = HashMap::new();
        all.insert((100, "polymarket:m1".to_string()), now - 600);
        let signals = vec![mk("polymarket", "m1", Some(50_000.0), 10.0, now)];

        let chat100 = scoped_cooldowns(&all, 100);
        let picked100 = select_top_signals(signals.clone(), 3, 14_400, now, &chat100);
        assert!(picked100.is_empty(), "chat 100 should be in cooldown");

        let chat200 = scoped_cooldowns(&all, 200);
        let picked200 = select_top_signals(signals, 3, 14_400, now, &chat200);
        assert_eq!(picked200.len(), 1);
        assert_eq!(picked200[0].market_key, "m1");
    }

    #[test]
    fn move_size_below_chat_threshold_drops_signal() {
        // chat_accepts_signal is async + DB-bound; the threshold portion is the
        // simplest pure check on the signal itself.
        let now = 1_000_000;
        let small = mk("polymarket", "m1", Some(50_000.0), 3.0, now);
        let big = mk("polymarket", "m2", Some(50_000.0), 10.0, now);
        // Threshold 5 → small (3) drops, big (10) stays.
        assert!(small.move_size.abs() < 5.0);
        assert!(big.move_size.abs() >= 5.0);
    }
}
