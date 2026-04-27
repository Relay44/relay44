//! Tick that fires `/notify` rules.
//!
//! Pulls every active rule per tick, looks up the current YES price for each
//! rule's market via the scanner table the rule's venue points at, and DMs
//! the chat when the price has crossed the rule's threshold. Marks the rule
//! fired so it is not evaluated again — users get a single shot.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};

use super::notify_rules::{self, NotifyRule};
use super::telegram_format::{html_escape, venue_title, TelegramClient};
use crate::AppState;

const DEFAULT_INTERVAL_SECS: u64 = 60;

pub fn spawn(state: Arc<AppState>) {
    let enabled = env_bool("NOTIFY_SCHEDULER_ENABLED", false);
    if !enabled {
        info!("Notify scheduler disabled (NOTIFY_SCHEDULER_ENABLED=false)");
        return;
    }

    let Ok(bot_token) = std::env::var("TELEGRAM_BOT_TOKEN") else {
        warn!("NOTIFY_SCHEDULER_ENABLED=true but TELEGRAM_BOT_TOKEN missing; not starting");
        return;
    };

    let interval_secs = env_u64("NOTIFY_INTERVAL_SECS", DEFAULT_INTERVAL_SECS).max(15);
    info!("Starting notify scheduler (interval={}s)", interval_secs);

    // The chat id is per-rule, so the client's default chat id is unused.
    let tg = TelegramClient::new(bot_token, String::new());

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.tick().await;
        loop {
            interval.tick().await;
            run_tick(&state, &tg).await;
        }
    });
}

async fn run_tick(state: &AppState, tg: &TelegramClient) {
    let rules = match notify_rules::list_all_active(state.db.pool()).await {
        Ok(r) => r,
        Err(e) => {
            warn!("notify scheduler: list_all_active failed: {}", e);
            return;
        }
    };
    if rules.is_empty() {
        return;
    }

    // Group by (venue, slug) so we read each market's current price once even
    // when multiple chats have rules on it.
    let mut by_market: HashMap<(String, String), Vec<NotifyRule>> = HashMap::new();
    for rule in rules {
        by_market
            .entry((rule.venue.clone(), rule.slug.clone()))
            .or_default()
            .push(rule);
    }

    let prices = fetch_current_prices(state, by_market.keys()).await;

    for ((venue, slug), market_rules) in by_market {
        let Some(price) = prices.get(&(venue.clone(), slug.clone())).copied() else {
            // Market not in the active scanner table (likely settled or
            // delisted). Leave the rule alone — once the market reappears or
            // the user clears it, it'll resolve.
            continue;
        };
        for rule in market_rules {
            if !notify_rules::crossed(rule.baseline_price, price, rule.threshold) {
                continue;
            }
            let text = format_alert(&venue, &slug, &rule, price);
            match tg.send_to(rule.chat_id, &text).await {
                Ok(()) => {
                    if let Err(e) = notify_rules::mark_fired(state.db.pool(), rule.id, price)
                        .await
                    {
                        warn!("notify: mark_fired failed for rule {}: {}", rule.id, e);
                    } else {
                        info!(
                            "notify: rule {} fired (chat={}, {}:{} now {:.2}%)",
                            rule.id,
                            rule.chat_id,
                            venue,
                            slug,
                            price * 100.0
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "notify: send failed for rule {} (chat={}): {}",
                        rule.id, rule.chat_id, e
                    );
                    // Leave fired_at NULL so a retry happens on the next tick.
                }
            }
        }
    }
}

async fn fetch_current_prices<'a>(
    state: &AppState,
    keys: impl Iterator<Item = &'a (String, String)>,
) -> HashMap<(String, String), f64> {
    let mut out = HashMap::new();
    let mut poly_slugs: Vec<String> = Vec::new();
    let mut lim_slugs: Vec<String> = Vec::new();
    for (venue, slug) in keys {
        match venue.as_str() {
            "polymarket" => poly_slugs.push(slug.clone()),
            "limitless" => lim_slugs.push(slug.clone()),
            _ => {}
        }
    }

    if !poly_slugs.is_empty() {
        let rows: Result<Vec<(String, f64)>, sqlx::Error> = sqlx::query_as(
            "SELECT slug, yes_price::double precision \
             FROM polymarket_scanned_markets \
             WHERE active = TRUE AND slug = ANY($1)",
        )
        .bind(&poly_slugs)
        .fetch_all(state.db.pool())
        .await;
        match rows {
            Ok(rows) => {
                for (slug, price) in rows {
                    out.insert(("polymarket".to_string(), slug), price);
                }
            }
            Err(e) => warn!("notify scheduler: poly price query failed: {}", e),
        }
    }

    if !lim_slugs.is_empty() {
        let rows: Result<Vec<(String, f64)>, sqlx::Error> = sqlx::query_as(
            "SELECT slug, yes_price::double precision \
             FROM limitless_scanned_markets \
             WHERE active = TRUE AND slug = ANY($1)",
        )
        .bind(&lim_slugs)
        .fetch_all(state.db.pool())
        .await;
        match rows {
            Ok(rows) => {
                for (slug, price) in rows {
                    out.insert(("limitless".to_string(), slug), price);
                }
            }
            Err(e) => warn!("notify scheduler: limitless price query failed: {}", e),
        }
    }

    out
}

fn format_alert(venue: &str, slug: &str, rule: &NotifyRule, price: f64) -> String {
    let direction = if price > rule.baseline_price { "↑" } else { "↓" };
    let link = super::telegram_format::format_deep_link(venue, slug)
        .unwrap_or_else(|| html_escape(slug));
    format!(
        "<b>\u{1F514} /notify fired</b>\n\
         {} on {} — <code>{}</code>\n\
         baseline {:.0}% \u{2192} now <b>{:.0}%</b> (threshold {:.0}%)\n\
         {}",
        venue_title(venue),
        direction,
        html_escape(slug),
        rule.baseline_price * 100.0,
        price * 100.0,
        rule.threshold * 100.0,
        link,
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn rule(baseline: f64, threshold: f64) -> NotifyRule {
        NotifyRule {
            id: 1,
            chat_id: -100,
            venue: "polymarket".to_string(),
            slug: "test-slug".to_string(),
            threshold,
            baseline_price: baseline,
            created_at: Utc::now(),
            fired_at: None,
            fired_price: None,
        }
    }

    #[test]
    fn alert_includes_baseline_threshold_and_current() {
        let r = rule(0.45, 0.60);
        let text = format_alert("polymarket", "btc-100k", &r, 0.62);
        assert!(text.contains("45%"));
        assert!(text.contains("62%"));
        assert!(text.contains("60%"));
        assert!(text.contains("Polymarket"));
        assert!(text.contains("btc-100k"));
    }

    #[test]
    fn alert_renders_upward_arrow_for_rising_price() {
        let r = rule(0.45, 0.60);
        let text = format_alert("polymarket", "x", &r, 0.62);
        assert!(text.contains("\u{2191}"), "expected up arrow in {text}");
    }

    #[test]
    fn alert_renders_downward_arrow_for_falling_price() {
        let r = rule(0.70, 0.60);
        let text = format_alert("polymarket", "x", &r, 0.58);
        assert!(text.contains("\u{2193}"), "expected down arrow in {text}");
    }

    #[test]
    fn alert_html_escapes_slug() {
        let r = rule(0.45, 0.60);
        let text = format_alert("polymarket", "<evil>", &r, 0.62);
        assert!(text.contains("&lt;evil&gt;"));
        assert!(!text.contains("<evil>"));
    }
}
