//! Shared helpers for Telegram alert formatting.
//!
//! Every alerter service (probability_alert, cross_venue_arb, new_market_alert,
//! and future additions such as volume_spike_alert) sends HTML-formatted
//! messages through a small, consistent layout:
//!
//!   <b>{emoji} {signal} — {Venue}</b>
//!   <i>{question}</i>
//!   {signal-specific body}
//!   Liquidity: $X | 24h vol: $Y | Category: Z     (skips missing fields)
//!   <a href="...">Open on {Venue}</a>
//!
//! Formatting helpers live here; every alerter imports them to keep the surface
//! area uniform. All user-derived strings (question, category, slug) must be
//! passed through `html_escape` before embedding — Telegram's HTML parser will
//! reject the whole message if an unescaped `<`, `>` or `&` slips through.
//!
//! `TelegramClient` is the shared HTTP client. It posts with
//! `parse_mode: "HTML"` and `disable_web_page_preview: true`.

use std::time::Duration;

use serde::Serialize;

/// HTML-escapes a user-derived string for safe embedding in a Telegram HTML
/// message. Covers the characters Telegram's HTML parser requires (`<`, `>`,
/// `&`) plus the quote characters that would otherwise break attribute values
/// inside `<a href="...">` tags if the string is ever interpolated into an
/// attribute.
pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Human-readable title for a venue ("Polymarket", "Limitless", ...).
pub fn venue_title(venue: &str) -> &'static str {
    match venue {
        "polymarket" => "Polymarket",
        "limitless" => "Limitless",
        "aerodrome" => "Aerodrome",
        "internal" => "Internal",
        _ => "Unknown",
    }
}

/// Builds the Relay44 market URL for a given venue/slug pair. The slug is the
/// same one scanners write to `market_venue_links` and the Next.js screener
/// uses for `/markets/<slug>`, so alerts drive traffic back to the platform
/// where users can actually trade instead of sending them to the upstream
/// venue.
///
/// Returns `None` when the slug is empty or the venue isn't one we index. The
/// slug is NOT html-escaped here — embedding the raw URL into an href
/// attribute is the caller's responsibility (use `format_deep_link` for the
/// full HTML link).
pub fn venue_link(venue: &str, slug: &str) -> Option<String> {
    if slug.is_empty() {
        return None;
    }
    match venue {
        "polymarket" | "limitless" => Some(format!(
            "https://relay44.com/markets/by-slug/{}/{}",
            venue, slug
        )),
        _ => None,
    }
}

/// Returns the HTML anchor line for the alert footer, e.g.
/// `<a href="https://relay44.com/markets/abc">Trade on Relay44</a>`.
/// Returns `None` when there's no URL for this venue/slug.
pub fn format_deep_link(venue: &str, slug: &str) -> Option<String> {
    let url = venue_link(venue, slug)?;
    Some(format!(
        "<a href=\"{}\">Trade on Relay44</a>",
        html_escape(&url),
    ))
}

/// Renders the `<b>{emoji} {signal} — {Venue}</b>` header. `signal` is embedded
/// as-is and is expected to be a static string supplied by the caller, not a
/// user-derived value.
pub fn format_alert_header(emoji: &str, signal: &str, venue: &str) -> String {
    format!("<b>{} {} — {}</b>", emoji, signal, venue_title(venue))
}

/// Renders the compact metadata row `Liquidity: $X | 24h vol: $Y | Category: Z`.
///
/// Any of the three fields can be absent; missing fields are skipped entirely
/// so we don't print "Liquidity: -" placeholders. The category string IS
/// html-escaped inside this helper — callers pass the raw DB value.
///
/// Returns an empty string if none of the three fields are present, so callers
/// can blindly append with a leading newline and trim later.
pub fn format_metadata_row(
    liquidity_usd: Option<f64>,
    volume_24h_usd: Option<f64>,
    category: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(l) = liquidity_usd {
        parts.push(format!("Liquidity: {}", format_money(l)));
    }
    if let Some(v) = volume_24h_usd {
        parts.push(format!("24h vol: {}", format_money(v)));
    }
    if let Some(c) = category {
        let trimmed = c.trim();
        if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("unknown") {
            parts.push(format!("Category: {}", html_escape(trimmed)));
        }
    }
    parts.join(" | ")
}

/// Formats a USD value in the compact `$42`, `$4.2k`, `$4.2M` style used by
/// the alerter output. Shared so every alert row prints the same way.
pub fn format_money(v: f64) -> String {
    let abs = v.abs();
    if abs >= 1_000_000.0 {
        format!("${:.1}M", v / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("${:.1}k", v / 1_000.0)
    } else {
        format!("${:.0}", v)
    }
}

/// Shared Telegram HTTP client. Posts HTML-formatted messages with previews
/// disabled. Constructed once per alerter from env vars.
pub struct TelegramClient {
    bot_token: String,
    chat_id: String,
    http: reqwest::Client,
}

impl TelegramClient {
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            bot_token,
            chat_id,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
        }
    }

    pub async fn send(&self, text: &str) -> Result<(), String> {
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

    #[test]
    fn html_escape_covers_risky_chars() {
        assert_eq!(html_escape("A<B>&C"), "A&lt;B&gt;&amp;C");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(html_escape("it's"), "it&#39;s");
    }

    #[test]
    fn html_escape_leaves_safe_text_alone() {
        assert_eq!(html_escape("plain text 123"), "plain text 123");
        assert_eq!(html_escape(""), "");
    }

    #[test]
    fn venue_link_builds_relay44_urls() {
        assert_eq!(
            venue_link("polymarket", "will-x"),
            Some("https://relay44.com/markets/by-slug/polymarket/will-x".to_string())
        );
        assert_eq!(
            venue_link("limitless", "some-slug"),
            Some("https://relay44.com/markets/by-slug/limitless/some-slug".to_string())
        );
    }

    #[test]
    fn venue_link_returns_none_on_empty_slug_or_unknown_venue() {
        assert_eq!(venue_link("polymarket", ""), None);
        assert_eq!(venue_link("unknown-venue", "slug"), None);
    }

    #[test]
    fn deep_link_wraps_url_in_anchor() {
        let s = format_deep_link("polymarket", "abc").unwrap();
        assert!(s.contains("href=\"https://relay44.com/markets/by-slug/polymarket/abc\""));
        assert!(s.contains("Trade on Relay44"));
    }

    #[test]
    fn deep_link_none_when_no_url() {
        assert_eq!(format_deep_link("polymarket", ""), None);
        assert_eq!(format_deep_link("aerodrome", "slug"), None);
    }

    #[test]
    fn header_uses_em_dash_and_bold() {
        let h = format_alert_header("\u{1F4C8}", "Probability shift", "polymarket");
        assert!(h.starts_with("<b>"));
        assert!(h.contains("Probability shift"));
        assert!(h.contains("Polymarket"));
        assert!(h.contains("\u{2014}"));
    }

    #[test]
    fn metadata_row_includes_all_fields_when_present() {
        let s = format_metadata_row(Some(125_000.0), Some(3_400_000.0), Some("politics"));
        assert_eq!(s, "Liquidity: $125.0k | 24h vol: $3.4M | Category: politics");
    }

    #[test]
    fn metadata_row_skips_missing_fields() {
        let s = format_metadata_row(Some(500.0), None, None);
        assert_eq!(s, "Liquidity: $500");
        let s = format_metadata_row(None, Some(2_000.0), None);
        assert_eq!(s, "24h vol: $2.0k");
        let s = format_metadata_row(None, None, Some("sports"));
        assert_eq!(s, "Category: sports");
    }

    #[test]
    fn metadata_row_empty_when_all_missing() {
        let s = format_metadata_row(None, None, None);
        assert_eq!(s, "");
    }

    #[test]
    fn metadata_row_drops_unknown_and_blank_category() {
        assert_eq!(format_metadata_row(None, None, Some("unknown")), "");
        assert_eq!(format_metadata_row(None, None, Some("  ")), "");
        assert_eq!(format_metadata_row(None, None, Some("")), "");
    }

    #[test]
    fn metadata_row_html_escapes_category() {
        let s = format_metadata_row(None, None, Some("a<b>&c"));
        assert_eq!(s, "Category: a&lt;b&gt;&amp;c");
    }

    #[test]
    fn money_formatter_ranges() {
        assert_eq!(format_money(42.0), "$42");
        assert_eq!(format_money(4_200.0), "$4.2k");
        assert_eq!(format_money(4_200_000.0), "$4.2M");
    }

    #[test]
    fn venue_title_known_and_unknown() {
        assert_eq!(venue_title("polymarket"), "Polymarket");
        assert_eq!(venue_title("limitless"), "Limitless");
        assert_eq!(venue_title("nope"), "Unknown");
    }
}
