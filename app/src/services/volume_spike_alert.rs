//! Volume-spike alerter.
//!
//! Polls `polymarket_scanned_markets` and `limitless_scanned_markets` every
//! `VOLUME_SPIKE_POLL_SECS` seconds. The scanner tables store `volume_usdc` as
//! a cumulative counter that is overwritten on every scan, so we keep a small
//! in-memory ring buffer of recent (timestamp, volume) snapshots per market
//! and derive per-minute rates from the diffs.
//!
//! A market triggers an alert when its most-recent 5-minute volume rate is at
//! least `VOLUME_SPIKE_MULTIPLIER` times larger than its 1-hour baseline rate,
//! and the absolute 5-minute volume is at least `VOLUME_SPIKE_MIN_USD`. Each
//! market has its own cooldown so a sustained spike does not spam the channel.
//! During the first hour after startup, markets that lack a sufficiently old
//! sample are in "warmup" and simply skipped.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use log::{info, warn};
use serde::Serialize;

use crate::AppState;

const DEFAULT_MULTIPLIER: f64 = 5.0;
const DEFAULT_MIN_USD: f64 = 1000.0;
const DEFAULT_COOLDOWN_SECS: u64 = 1800;
const DEFAULT_POLL_SECS: u64 = 60;
const RETENTION_SECS: i64 = 65 * 60;
const SHORT_WINDOW_MIN: i64 = 5;
const LONG_WINDOW_MIN: i64 = 60;
const SHORT_MIN_AGE_MIN: i64 = 4;
const LONG_MIN_AGE_MIN: i64 = 55;
const MAX_ALERTS_PER_TICK: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Venue {
    Polymarket,
    Limitless,
}

impl Venue {
    fn as_str(self) -> &'static str {
        match self {
            Venue::Polymarket => "polymarket",
            Venue::Limitless => "limitless",
        }
    }

    fn header(self) -> &'static str {
        match self {
            Venue::Polymarket => "Polymarket",
            Venue::Limitless => "Limitless",
        }
    }
}

#[derive(Debug)]
struct MarketSnapshot {
    venue: Venue,
    key: String,
    question: String,
    slug: String,
    volume_usdc: f64,
    captured_at: DateTime<Utc>,
}

fn market_key(venue: Venue, id: &str) -> String {
    format!("{}:{}", venue.as_str(), id)
}

pub fn spawn(state: Arc<AppState>) {
    let enabled = env_bool("VOLUME_SPIKE_ALERTS_ENABLED", false);
    if !enabled {
        info!("Volume-spike alerts disabled (VOLUME_SPIKE_ALERTS_ENABLED=false)");
        return;
    }

    let Ok(bot_token) = std::env::var("TELEGRAM_BOT_TOKEN") else {
        warn!("VOLUME_SPIKE_ALERTS_ENABLED=true but TELEGRAM_BOT_TOKEN missing; not starting");
        return;
    };
    let Ok(chat_id) = std::env::var("TELEGRAM_CHAT_ID") else {
        warn!("VOLUME_SPIKE_ALERTS_ENABLED=true but TELEGRAM_CHAT_ID missing; not starting");
        return;
    };

    let multiplier = env_f64("VOLUME_SPIKE_MULTIPLIER", DEFAULT_MULTIPLIER);
    let min_usd = env_f64("VOLUME_SPIKE_MIN_USD", DEFAULT_MIN_USD);
    let cooldown = Duration::from_secs(env_u64(
        "VOLUME_SPIKE_COOLDOWN_SECS",
        DEFAULT_COOLDOWN_SECS,
    ));
    let poll = Duration::from_secs(env_u64("VOLUME_SPIKE_POLL_SECS", DEFAULT_POLL_SECS));

    info!(
        "Starting volume-spike alerts (multiplier={:.2}x, min=${:.0}, cooldown={}s, poll={}s)",
        multiplier,
        min_usd,
        cooldown.as_secs(),
        poll.as_secs()
    );

    let tg = TelegramClient::new(bot_token, chat_id);

    tokio::spawn(async move {
        let mut buffers: HashMap<String, VecDeque<(DateTime<Utc>, f64)>> = HashMap::new();
        let mut last_alert: HashMap<String, Instant> = HashMap::new();

        let mut interval = tokio::time::interval(poll);
        interval.tick().await;

        loop {
            interval.tick().await;

            let snapshots = match fetch_snapshots(&state).await {
                Ok(s) => s,
                Err(e) => {
                    warn!("volume_spike_alert fetch: {}", e);
                    continue;
                }
            };

            let now = Utc::now();
            let mut candidates: Vec<(String, f64, f64, f64)> = Vec::new();
            // Meta rebuilt every tick from the current scan — drops rows for
            // markets that have gone inactive.
            let mut meta: HashMap<String, (Venue, String, String)> = HashMap::new();

            for snap in snapshots {
                let key = snap.key.clone();
                meta.insert(
                    key.clone(),
                    (snap.venue, snap.question.clone(), snap.slug.clone()),
                );

                let buf = buffers.entry(key.clone()).or_default();
                push_sample(buf, snap.captured_at, snap.volume_usdc);
                prune_buffer(buf, now);

                if let Some(ev) = evaluate_spike(buf, multiplier, min_usd) {
                    candidates.push((key, ev.rate_5m, ev.rate_1h, ev.volume_5m));
                }
            }

            // Drop buffers for markets that disappeared from the scan so the
            // map does not grow unbounded.
            retain_known(&mut buffers, &meta);

            // Apply cooldown + cap alerts per tick.
            let mut sent = 0usize;
            // Sort by short-window rate descending so hottest markets win the cap.
            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (key, rate_5m, rate_1h, _volume_5m) in candidates {
                if sent >= MAX_ALERTS_PER_TICK {
                    break;
                }
                if let Some(t) = last_alert.get(&key) {
                    if t.elapsed() < cooldown {
                        continue;
                    }
                }
                let Some((venue, question, slug)) = meta.get(&key).cloned() else {
                    continue;
                };
                let vol_24h = buffers
                    .get(&key)
                    .and_then(|b| b.back())
                    .map(|(_, v)| *v)
                    .unwrap_or(0.0);
                let text = format_alert(venue, &question, &slug, rate_5m, rate_1h, vol_24h);
                if let Err(e) = tg.send(&text).await {
                    warn!("telegram send failed (volume-spike): {}", e);
                    continue;
                }
                last_alert.insert(key, Instant::now());
                sent += 1;
            }
        }
    });
}

fn retain_known(
    buffers: &mut HashMap<String, VecDeque<(DateTime<Utc>, f64)>>,
    meta: &HashMap<String, (Venue, String, String)>,
) {
    buffers.retain(|k, _| meta.contains_key(k));
}

async fn fetch_snapshots(state: &AppState) -> Result<Vec<MarketSnapshot>, String> {
    let pool = state.db.pool();
    let now = Utc::now();

    let mut out = Vec::new();

    let pm_rows: Vec<(String, String, Option<String>, Option<f64>)> = sqlx::query_as(
        "SELECT condition_id, question, slug, \
                volume_usdc::double precision \
         FROM polymarket_scanned_markets \
         WHERE active = TRUE",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("poly query: {}", e))?;

    for (condition_id, question, slug, volume) in pm_rows {
        let Some(v) = volume else { continue };
        out.push(MarketSnapshot {
            venue: Venue::Polymarket,
            key: market_key(Venue::Polymarket, &condition_id),
            question,
            slug: slug.unwrap_or_default(),
            volume_usdc: v,
            captured_at: now,
        });
    }

    let lim_rows: Vec<(String, String, Option<f64>)> = sqlx::query_as(
        "SELECT slug, question, volume_usdc \
         FROM limitless_scanned_markets \
         WHERE active = TRUE",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("limitless query: {}", e))?;

    for (slug, question, volume) in lim_rows {
        let Some(v) = volume else { continue };
        out.push(MarketSnapshot {
            venue: Venue::Limitless,
            key: market_key(Venue::Limitless, &slug),
            question,
            slug,
            volume_usdc: v,
            captured_at: now,
        });
    }

    Ok(out)
}

fn push_sample(buf: &mut VecDeque<(DateTime<Utc>, f64)>, ts: DateTime<Utc>, volume: f64) {
    // Drop out-of-order samples so rate math stays sane.
    if let Some((last_ts, _)) = buf.back() {
        if ts <= *last_ts {
            return;
        }
    }
    buf.push_back((ts, volume));
}

fn prune_buffer(buf: &mut VecDeque<(DateTime<Utc>, f64)>, now: DateTime<Utc>) {
    let cutoff = now - chrono::Duration::seconds(RETENTION_SECS);
    while let Some((ts, _)) = buf.front() {
        if *ts < cutoff {
            buf.pop_front();
        } else {
            break;
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SpikeEval {
    rate_5m: f64,
    rate_1h: f64,
    volume_5m: f64,
}

/// Evaluate the buffer for a spike. Returns `None` if the buffer is not yet
/// warm enough (no sample older than `LONG_MIN_AGE_MIN`) or if no spike.
fn evaluate_spike(
    buf: &VecDeque<(DateTime<Utc>, f64)>,
    multiplier: f64,
    min_usd: f64,
) -> Option<SpikeEval> {
    let (now_ts, now_vol) = *buf.back()?;

    let short_target = now_ts - chrono::Duration::minutes(SHORT_WINDOW_MIN);
    let long_target = now_ts - chrono::Duration::minutes(LONG_WINDOW_MIN);

    // Short window sample must be at least SHORT_MIN_AGE_MIN old.
    let short_cutoff = now_ts - chrono::Duration::minutes(SHORT_MIN_AGE_MIN);
    let long_cutoff = now_ts - chrono::Duration::minutes(LONG_MIN_AGE_MIN);

    let short_sample = closest_at_or_before(buf, short_target, short_cutoff)?;
    let long_sample = closest_at_or_before(buf, long_target, long_cutoff)?;

    let short_span_min = (now_ts - short_sample.0).num_seconds() as f64 / 60.0;
    let long_span_min = (now_ts - long_sample.0).num_seconds() as f64 / 60.0;
    if short_span_min <= 0.0 || long_span_min <= 0.0 {
        return None;
    }

    let volume_5m = (now_vol - short_sample.1).max(0.0);
    let volume_1h = (now_vol - long_sample.1).max(0.0);
    let rate_5m = volume_5m / short_span_min;
    let rate_1h = volume_1h / long_span_min;

    if rate_1h <= 0.0 {
        return None;
    }
    if volume_5m < min_usd {
        return None;
    }
    if rate_5m / rate_1h < multiplier {
        return None;
    }

    Some(SpikeEval {
        rate_5m,
        rate_1h,
        volume_5m,
    })
}

/// Find the sample whose timestamp is closest to `target` while being no newer
/// than `newest_allowed` (i.e. at least as old as the required min-age). Returns
/// `None` if the buffer has no sample at or before `newest_allowed`.
fn closest_at_or_before(
    buf: &VecDeque<(DateTime<Utc>, f64)>,
    target: DateTime<Utc>,
    newest_allowed: DateTime<Utc>,
) -> Option<(DateTime<Utc>, f64)> {
    let mut best: Option<(DateTime<Utc>, f64, i64)> = None;
    for (ts, v) in buf.iter() {
        if *ts > newest_allowed {
            continue;
        }
        let dist = (ts.timestamp() - target.timestamp()).abs();
        match best {
            Some((_, _, d)) if dist >= d => {}
            _ => best = Some((*ts, *v, dist)),
        }
    }
    best.map(|(ts, v, _)| (ts, v))
}

fn format_alert(
    venue: Venue,
    question: &str,
    slug: &str,
    rate_5m: f64,
    rate_1h: f64,
    vol_24h_total: f64,
) -> String {
    let ratio = if rate_1h > 0.0 { rate_5m / rate_1h } else { 0.0 };
    let venue_slug = match venue {
        Venue::Polymarket => "polymarket",
        Venue::Limitless => "limitless",
    };
    let link = format!("https://relay44.com/markets/by-slug/{}/{}", venue_slug, slug);
    format!(
        "🌊 <b>Volume spike — {header}</b>\n\
         <i>{question}</i>\n\n\
         Rate: <b>${rate_5m_fmt}/min</b> last 5m vs ${rate_1h_fmt}/min 1h baseline (<b>{ratio:.1}x</b>)\n\
         24h vol: ${vol_24h_fmt}\n\
         <a href=\"{link}\">Trade on Relay44</a>",
        header = venue.header(),
        question = html_escape(question),
        rate_5m_fmt = format_money(rate_5m),
        rate_1h_fmt = format_money(rate_1h),
        ratio = ratio,
        vol_24h_fmt = format_money(vol_24h_total),
        link = link,
    )
}

fn format_money(v: f64) -> String {
    let v = v.round();
    if v.abs() < 1000.0 {
        return format!("{}", v as i64);
    }
    // Thousands-separated with commas.
    let int = v as i64;
    let s = int.to_string();
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(chars.len() + chars.len() / 3);
    let negative = chars.first() == Some(&'-');
    let start = if negative { 1 } else { 0 };
    let digits = &chars[start..];
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c);
    }
    if negative {
        format!("-{}", out)
    } else {
        out
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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

    fn ts(offset_minutes: i64) -> DateTime<Utc> {
        // Use a fixed anchor so tests are deterministic.
        let anchor = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
        anchor + chrono::Duration::minutes(offset_minutes)
    }

    #[test]
    fn prune_drops_entries_older_than_retention() {
        let mut buf: VecDeque<(DateTime<Utc>, f64)> = VecDeque::new();
        // 0, 30, 60, 90 minutes old relative to a "now" of 120min.
        buf.push_back((ts(0), 10.0));
        buf.push_back((ts(30), 20.0));
        buf.push_back((ts(60), 30.0));
        buf.push_back((ts(90), 40.0));
        buf.push_back((ts(120), 50.0));

        prune_buffer(&mut buf, ts(120));
        // RETENTION_SECS = 65*60 → cutoff = 120 - 65 = 55. Keep ts(60,90,120).
        let kept: Vec<i64> = buf.iter().map(|(t, _)| (t.timestamp() - ts(0).timestamp()) / 60).collect();
        assert_eq!(kept, vec![60, 90, 120]);
    }

    #[test]
    fn push_ignores_out_of_order_sample() {
        let mut buf: VecDeque<(DateTime<Utc>, f64)> = VecDeque::new();
        push_sample(&mut buf, ts(10), 100.0);
        push_sample(&mut buf, ts(5), 50.0); // older, ignored
        push_sample(&mut buf, ts(15), 150.0);
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.front().unwrap().1, 100.0);
        assert_eq!(buf.back().unwrap().1, 150.0);
    }

    #[test]
    fn rate_computation_matches_expected() {
        // Build a 61-min buffer with linear 100/min baseline, then a surge in
        // the last 5 min at 2000/min.
        let mut buf: VecDeque<(DateTime<Utc>, f64)> = VecDeque::new();
        for minute in 0..=55 {
            buf.push_back((ts(minute), 100.0 * minute as f64));
        }
        // Push the 5-min surge: +2000/min for 5 minutes.
        let base = 100.0 * 55.0;
        for step in 1..=5 {
            let m = 55 + step;
            let v = base + 2000.0 * step as f64;
            buf.push_back((ts(m), v));
        }

        let ev = evaluate_spike(&buf, 5.0, 1000.0).expect("should detect spike");
        assert!(
            (ev.rate_5m - 2000.0).abs() < 1e-6,
            "rate_5m={}",
            ev.rate_5m
        );
        // 1h baseline spans 60 min, total delta = 5500 + 10000 = 15500, /60 ≈ 258.33.
        // (5500 from baseline 55min*100, 10000 from surge.)
        let expected_1h = (100.0 * 55.0 + 2000.0 * 5.0) / 60.0;
        assert!((ev.rate_1h - expected_1h).abs() < 1e-6, "rate_1h={}", ev.rate_1h);
        assert!(ev.volume_5m >= 1000.0);
    }

    #[test]
    fn spike_detected_when_ratio_and_abs_met() {
        // Buffer: t=0 → 0, t=60 → 600, t=65 → 1100.
        // At now_ts = t=65, closest short-window sample (target t=60, newest
        // allowed t=61) is t=60 value 600. Closest long-window sample (target
        // t=5, newest allowed t=10) is t=0 value 0. Short: 500/5 = 100/min.
        // Long: 1100/65 ≈ 16.92/min. Ratio ≈ 5.9x.
        let mut buf: VecDeque<(DateTime<Utc>, f64)> = VecDeque::new();
        buf.push_back((ts(0), 0.0));
        buf.push_back((ts(60), 600.0));
        buf.push_back((ts(65), 1100.0));
        let ev = evaluate_spike(&buf, 5.0, 100.0).expect("spike");
        assert!((ev.rate_5m - 100.0).abs() < 1e-6);
        let expected_1h = 1100.0 / 65.0;
        assert!(
            (ev.rate_1h - expected_1h).abs() < 1e-6,
            "rate_1h={}",
            ev.rate_1h
        );
    }

    #[test]
    fn spike_rejected_when_abs_below_min() {
        let mut buf: VecDeque<(DateTime<Utc>, f64)> = VecDeque::new();
        buf.push_back((ts(0), 0.0));
        buf.push_back((ts(60), 60.0));
        buf.push_back((ts(65), 70.0)); // 5m delta = 10, way below min
        assert!(evaluate_spike(&buf, 2.0, 1000.0).is_none());
    }

    #[test]
    fn spike_rejected_when_ratio_below_multiplier() {
        let mut buf: VecDeque<(DateTime<Utc>, f64)> = VecDeque::new();
        buf.push_back((ts(0), 0.0));
        buf.push_back((ts(60), 6_000.0)); // 100/min baseline
        buf.push_back((ts(65), 6_750.0)); // 150/min — 1.5x, not a spike
        assert!(evaluate_spike(&buf, 5.0, 500.0).is_none());
    }

    #[test]
    fn warmup_returns_none_when_no_old_sample() {
        // Buffer only covers the last 10 minutes → no sample older than 55min.
        let mut buf: VecDeque<(DateTime<Utc>, f64)> = VecDeque::new();
        for m in 55..=65 {
            buf.push_back((ts(m), 1000.0 * m as f64));
        }
        assert!(evaluate_spike(&buf, 2.0, 100.0).is_none());
    }

    #[test]
    fn warmup_returns_none_when_empty() {
        let buf: VecDeque<(DateTime<Utc>, f64)> = VecDeque::new();
        assert!(evaluate_spike(&buf, 2.0, 100.0).is_none());
    }

    #[test]
    fn format_alert_polymarket_html() {
        let s = format_alert(
            Venue::Polymarket,
            "Will BTC hit 100k by EOY?",
            "btc-100k-eoy",
            2340.0,
            180.0,
            485_200.0,
        );
        assert!(s.contains("🌊"));
        assert!(s.contains("<b>Volume spike — Polymarket</b>"));
        assert!(s.contains("<i>Will BTC hit 100k by EOY?</i>"));
        assert!(s.contains("$2,340/min"));
        assert!(s.contains("$180/min"));
        assert!(s.contains("<b>13.0x</b>"));
        assert!(s.contains("$485,200"));
        assert!(s.contains("https://relay44.com/markets/by-slug/polymarket/btc-100k-eoy"));
        assert!(s.contains("Trade on Relay44"));
    }

    #[test]
    fn format_alert_limitless_link_and_header() {
        let s = format_alert(
            Venue::Limitless,
            "Will X happen?",
            "x-happen",
            500.0,
            50.0,
            12_345.0,
        );
        assert!(s.contains("<b>Volume spike — Limitless</b>"));
        assert!(s.contains("https://relay44.com/markets/by-slug/limitless/x-happen"));
        assert!(s.contains("Trade on Relay44"));
        assert!(s.contains("<b>10.0x</b>"));
    }

    #[test]
    fn format_alert_html_escapes_question() {
        let s = format_alert(
            Venue::Polymarket,
            "Will <foo> & <bar> win?",
            "slug",
            100.0,
            10.0,
            1000.0,
        );
        assert!(s.contains("&lt;foo&gt;"));
        assert!(s.contains("&amp;"));
        assert!(!s.contains("<foo>"));
    }

    #[test]
    fn format_money_adds_commas() {
        assert_eq!(format_money(0.0), "0");
        assert_eq!(format_money(999.0), "999");
        assert_eq!(format_money(1_000.0), "1,000");
        assert_eq!(format_money(12_345.0), "12,345");
        assert_eq!(format_money(1_234_567.0), "1,234,567");
    }
}
