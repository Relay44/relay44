use anyhow::Result;
use log::warn;

use super::redis::RedisService;

const HERMES_BASE: &str = "https://hermes.pyth.network";
const CACHE_TTL_SECONDS: u64 = 5;

fn cache_key(feed_id: &str) -> String {
    format!("pyth:price:{}", feed_id.trim_start_matches("0x"))
}

pub async fn fetch_price(redis: &RedisService, feed_id: &str) -> Result<Option<f64>> {
    let key = cache_key(feed_id);
    if let Some(cached) = redis.get::<f64>(&key).await? {
        return Ok(Some(cached));
    }

    let id = feed_id.trim_start_matches("0x");
    let url = format!("{HERMES_BASE}/v2/updates/price/latest?ids[]={id}");

    let resp = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
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

    let price = parse_hermes_price(&body);
    if let Some(p) = price {
        let _ = redis.set(&key, &p, Some(CACHE_TTL_SECONDS)).await;
    }

    Ok(price)
}

fn parse_hermes_price(body: &serde_json::Value) -> Option<f64> {
    let price_obj = body["parsed"]
        .as_array()?
        .first()?
        .get("price")?
        .as_object()?;

    let raw: f64 = price_obj.get("price")?.as_str()?.parse().ok()?;
    let expo: i64 = price_obj.get("expo")?.as_i64()?;

    Some(raw * 10f64.powi(expo as i32))
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
    fn parse_empty_response() {
        assert!(parse_hermes_price(&json!({})).is_none());
        assert!(parse_hermes_price(&json!({ "parsed": [] })).is_none());
    }
}
