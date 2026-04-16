//! Telegram probability-shift alerts.
//!
//! Subscribes to the market_data bus, tracks the last observed price per
//! (venue, market_key), and pushes a Telegram message when a fresh snapshot
//! moves by more than `PROB_ALERT_THRESHOLD_PCT`. Each market is on a
//! per-market cooldown so a rapidly-jittery book can't spam the channel.

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

    info!(
        "Starting Telegram probability alerts (threshold={}%, cooldown={}s)",
        threshold_pct,
        cooldown.as_secs()
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

async fn handle_event(
    state: &AppState,
    tg: &TelegramClient,
    threshold_pct: f64,
    cooldown: Duration,
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
    last_alert.insert(key, Instant::now());

    let question = lookup_question(state, ev.venue, &ev.market_key)
        .await
        .unwrap_or_else(|| format!("{}:{}", ev.venue.as_str(), truncate(&ev.market_key, 12)));

    let text = format_alert(&question, prev_price, price, delta_pct);
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

async fn lookup_question(state: &AppState, venue: Venue, market_key: &str) -> Option<String> {
    if venue != Venue::Polymarket {
        return None;
    }
    let pool = state.db.pool();
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT question FROM polymarket_scanned_markets \
         WHERE yes_token_id = $1 OR no_token_id = $1 LIMIT 1",
    )
    .bind(market_key)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    row.map(|r| r.0)
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

fn format_alert(question: &str, prev: f64, curr: f64, delta_pct: f64) -> String {
    let arrow = if delta_pct > 0.0 { "↑" } else { "↓" };
    format!(
        "{arrow} {question}\n{:.1}¢ → {:.1}¢  ({:+.1}%)",
        prev * 100.0,
        curr * 100.0,
        delta_pct
    )
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

    #[test]
    fn format_includes_question_and_delta() {
        let s = format_alert("Will X happen?", 0.10, 0.18, 80.0);
        assert!(s.contains("Will X happen?"));
        assert!(s.contains("10.0¢"));
        assert!(s.contains("18.0¢"));
        assert!(s.contains("+80.0%"));
    }

    #[test]
    fn format_down_arrow_for_negative() {
        let s = format_alert("Q", 0.50, 0.40, -20.0);
        assert!(s.contains("↓"));
    }
}
