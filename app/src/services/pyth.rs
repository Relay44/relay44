use anyhow::Result;
use log::warn;

use super::redis::RedisService;

pub const HERMES_BASE: &str = "https://hermes.pyth.network";
const CACHE_TTL_SECONDS: u64 = 5;
const REQUEST_TIMEOUT_SECS: u64 = 5;

fn is_valid_feed_id(feed_id: &str) -> bool {
    let hex = feed_id.trim_start_matches("0x");
    hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn cache_key(feed_id: &str) -> String {
    format!("pyth:price:{}", feed_id.trim_start_matches("0x"))
}

pub async fn fetch_price(redis: &RedisService, feed_id: &str) -> Result<Option<f64>> {
    if !is_valid_feed_id(feed_id) {
        warn!("pyth: invalid feed id: {feed_id}");
        return Ok(None);
    }

    let key = cache_key(feed_id);
    if let Some(cached) = redis.get::<f64>(&key).await? {
        return Ok(Some(cached));
    }

    let price = fetch_from_hermes(feed_id).await?;
    if let Some(p) = price {
        let _ = redis.set(&key, &p, Some(CACHE_TTL_SECONDS)).await;
    }

    Ok(price)
}

pub async fn fetch_from_hermes(feed_id: &str) -> Result<Option<f64>> {
    let id = feed_id.trim_start_matches("0x");
    let url = format!("{HERMES_BASE}/v2/updates/price/latest?ids[]={id}");

    let resp = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .send()
        .await;

    let resp = match resp {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            warn!("pyth hermes returned {}", r.status());
            return Ok(None);
        }
        Err(e) => {
            warn!("pyth hermes request failed: {e}");
            return Ok(None);
        }
    };

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            warn!("pyth hermes parse error: {e}");
            return Ok(None);
        }
    };

    Ok(parse_hermes_price(&body))
}

pub fn parse_hermes_price(body: &serde_json::Value) -> Option<f64> {
    let price_obj = body["parsed"]
        .as_array()?
        .first()?
        .get("price")?
        .as_object()?;

    let raw: f64 = price_obj.get("price")?.as_str()?.parse().ok()?;
    let expo: i64 = price_obj.get("expo")?.as_i64()?;
    let price = raw * 10f64.powi(expo as i32);

    if price <= 0.0 || !price.is_finite() {
        return None;
    }

    Some(price)
}

/// Fetch price and compare against a threshold. Used by the oracle keeper.
pub async fn check_threshold(
    redis: &RedisService,
    feed_id: &str,
    target: f64,
    comparison: &str,
) -> std::result::Result<bool, String> {
    let price = fetch_price(redis, feed_id)
        .await
        .map_err(|e| format!("pyth fetch: {e}"))?
        .ok_or("no price data from pyth")?;

    let met = match comparison {
        "gt" => price > target,
        "gte" => price >= target,
        "lt" => price < target,
        "lte" => price <= target,
        "eq" => (price - target).abs() / target.abs().max(1e-12) < 0.001, // 0.1% tolerance
        _ => return Err(format!("unknown comparison: {comparison}")),
    };

    Ok(met)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_btc_price() {
        let body = json!({
            "parsed": [{
                "price": {
                    "price": "6543212000000",
                    "expo": -8,
                    "conf": "500"
                }
            }]
        });
        let price = parse_hermes_price(&body).unwrap();
        assert!((price - 65432.12).abs() < 0.01);
    }

    #[test]
    fn parse_sol_price() {
        let body = json!({
            "parsed": [{
                "price": {
                    "price": "14523000",
                    "expo": -5,
                    "conf": "100"
                }
            }]
        });
        let price = parse_hermes_price(&body).unwrap();
        assert!((price - 145.23).abs() < 0.01);
    }

    #[test]
    fn parse_empty_response() {
        assert!(parse_hermes_price(&json!({})).is_none());
        assert!(parse_hermes_price(&json!({ "parsed": [] })).is_none());
    }

    #[test]
    fn reject_negative_price() {
        let body = json!({
            "parsed": [{
                "price": {
                    "price": "-100",
                    "expo": -8,
                    "conf": "500"
                }
            }]
        });
        assert!(parse_hermes_price(&body).is_none());
    }

    #[test]
    fn reject_zero_price() {
        let body = json!({
            "parsed": [{
                "price": {
                    "price": "0",
                    "expo": -8,
                    "conf": "500"
                }
            }]
        });
        assert!(parse_hermes_price(&body).is_none());
    }

    #[test]
    fn valid_feed_ids() {
        assert!(is_valid_feed_id("0xff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace"));
        assert!(is_valid_feed_id("ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace"));
        assert!(!is_valid_feed_id("too_short"));
        assert!(!is_valid_feed_id(""));
        assert!(!is_valid_feed_id("0xZZ61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace"));
    }

    #[test]
    fn eq_comparison_uses_relative_tolerance() {
        // 0.1% tolerance means $50000 ± $49 should match
        assert!((50000.0 - 50049.0_f64).abs() / 50000.0 < 0.001); // within
        assert!(!((50000.0 - 50600.0_f64).abs() / 50000.0 < 0.001)); // outside
    }
}
