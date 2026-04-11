use reqwest::Client;
use serde_json::Value;

use crate::api::ApiError;
use crate::services::external::polymarket_index;
use crate::services::external::types::{
    clamp_probability, is_binary_yes_no, now_rfc3339, ExternalMarketSnapshot,
    ExternalOrderBookLevel, ExternalOrderBookSnapshot, ExternalOutcome, ExternalTradesSnapshot,
};

fn api_error(prefix: &str, err: impl ToString) -> ApiError {
    ApiError::internal(&format!("{}: {}", prefix, err.to_string()))
}

fn parse_string(value: Option<&Value>) -> String {
    value
        .and_then(|entry| entry.as_str())
        .unwrap_or_default()
        .to_string()
}

fn parse_bool(value: Option<&Value>) -> bool {
    value.and_then(|entry| entry.as_bool()).unwrap_or(false)
}

fn parse_u64(value: Option<&Value>) -> u64 {
    if let Some(raw) = value {
        if let Some(number) = raw.as_u64() {
            return number;
        }
        if let Some(raw_str) = raw.as_str() {
            if let Ok(number) = raw_str.parse::<u64>() {
                return number;
            }
        }
    }
    0
}

fn normalize_unix_timestamp(value: u64) -> u64 {
    if value > 10_000_000_000 {
        value / 1_000
    } else {
        value
    }
}

fn parse_timestamp(value: Option<&Value>) -> u64 {
    let Some(raw) = value else {
        return 0;
    };

    if let Some(number) = raw.as_u64() {
        return normalize_unix_timestamp(number);
    }

    let Some(raw_str) = raw
        .as_str()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    else {
        return 0;
    };

    if let Ok(number) = raw_str.parse::<u64>() {
        return normalize_unix_timestamp(number);
    }

    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(raw_str) {
        return parsed.timestamp().max(0) as u64;
    }

    if let Ok(parsed) = chrono::NaiveDate::parse_from_str(raw_str, "%Y-%m-%d") {
        if let Some(naive) = parsed.and_hms_opt(0, 0, 0) {
            return chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive, chrono::Utc)
                .timestamp()
                .max(0) as u64;
        }
    }

    0
}

fn parse_f64(value: Option<&Value>) -> f64 {
    if let Some(raw) = value {
        if let Some(number) = raw.as_f64() {
            return number;
        }
        if let Some(raw_str) = raw.as_str() {
            if let Ok(number) = raw_str.parse::<f64>() {
                return number;
            }
        }
    }
    0.0
}

fn parse_string_list(value: Option<&Value>) -> Vec<String> {
    let Some(raw) = value else {
        return Vec::new();
    };

    if let Some(items) = raw.as_array() {
        return items
            .iter()
            .filter_map(|item| item.as_str())
            .map(ToOwned::to_owned)
            .collect();
    }

    if let Some(text) = raw.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(text) {
            return parsed;
        }
    }

    Vec::new()
}

fn parse_outcomes(row: &Value) -> Vec<ExternalOutcome> {
    let labels = parse_string_list(row.get("outcomes"));
    let prices_raw = parse_string_list(row.get("outcomePrices"));
    let prices = prices_raw
        .into_iter()
        .map(|item| item.parse::<f64>().unwrap_or(0.5))
        .collect::<Vec<_>>();

    if labels.is_empty() {
        return vec![
            ExternalOutcome {
                label: "Yes".to_string(),
                probability: 0.5,
            },
            ExternalOutcome {
                label: "No".to_string(),
                probability: 0.5,
            },
        ];
    }

    labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| ExternalOutcome {
            label,
            probability: clamp_probability(*prices.get(index).unwrap_or(&0.5)),
        })
        .collect()
}

fn parse_polymarket_market(row: &Value) -> Option<ExternalMarketSnapshot> {
    let id = parse_string(row.get("id"));
    let slug = parse_string(row.get("slug"));
    if id.is_empty() || slug.is_empty() {
        return None;
    }

    let outcomes = parse_outcomes(row);
    let yes_price = outcomes
        .iter()
        .find(|entry| entry.label.eq_ignore_ascii_case("yes"))
        .map(|entry| entry.probability)
        .unwrap_or(0.5);
    let no_price = outcomes
        .iter()
        .find(|entry| entry.label.eq_ignore_ascii_case("no"))
        .map(|entry| entry.probability)
        .unwrap_or(1.0 - yes_price);

    let active = parse_bool(row.get("active"));
    let closed = parse_bool(row.get("closed"));
    let resolved = parse_bool(row.get("resolved")) || closed;
    let status = if resolved {
        "resolved"
    } else if active {
        "active"
    } else {
        "closed"
    };

    let executable = is_binary_yes_no(&outcomes) && parse_bool(row.get("enableOrderBook"));

    let close_time = parse_timestamp(row.get("endDate"));
    let close_time = if close_time > 0 {
        close_time
    } else {
        parse_timestamp(row.get("endDateIso"))
    };

    Some(ExternalMarketSnapshot {
        id: format!("polymarket:{}", id),
        question: parse_string(row.get("question")),
        description: parse_string(row.get("description")),
        category: parse_string(row.get("category")).to_ascii_lowercase(),
        status: status.to_string(),
        close_time,
        resolved,
        outcome: None,
        yes_price,
        no_price,
        volume: parse_f64(row.get("volume")),
        source: "external_polymarket".to_string(),
        provider: "polymarket".to_string(),
        is_external: true,
        external_url: format!("https://polymarket.com/event/{}", slug),
        chain_id: 137,
        requires_credentials: true,
        execution_users: executable,
        execution_agents: executable,
        outcomes,
        provider_market_ref: id,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_orderbook_levels, parse_polymarket_market, sort_orderbook_levels};
    use serde_json::json;

    #[test]
    fn parses_rfc3339_end_date_into_close_time() {
        let market = parse_polymarket_market(&json!({
            "id": "540816",
            "slug": "russia-ukraine-ceasefire-before-gta-vi-554",
            "question": "Russia-Ukraine Ceasefire before GTA VI?",
            "description": "test market",
            "category": "news",
            "active": true,
            "closed": false,
            "resolved": false,
            "enableOrderBook": true,
            "endDate": "2026-07-31T12:00:00Z",
            "outcomes": ["Yes", "No"],
            "outcomePrices": ["0.545", "0.455"]
        }))
        .expect("market should parse");

        assert_eq!(market.close_time, 1_785_499_200);
    }

    #[test]
    fn falls_back_to_date_only_end_date_iso() {
        let market = parse_polymarket_market(&json!({
            "id": "540843",
            "slug": "will-china-invades-taiwan-before-gta-vi-716",
            "question": "Will China invades Taiwan before GTA VI?",
            "description": "test market",
            "category": "news",
            "active": true,
            "closed": false,
            "resolved": false,
            "enableOrderBook": true,
            "endDateIso": "2026-07-31",
            "outcomes": ["Yes", "No"],
            "outcomePrices": ["0.515", "0.485"]
        }))
        .expect("market should parse");

        assert_eq!(market.close_time, 1_785_456_000);
    }

    #[test]
    fn sorts_orderbook_levels_before_truncating() {
        let mut bids = parse_orderbook_levels(Some(&json!([
            {"price": "0.01", "size": "100"},
            {"price": "0.54", "size": "30"},
            {"price": "0.53", "size": "20"}
        ])));
        let mut asks = parse_orderbook_levels(Some(&json!([
            {"price": "0.99", "size": "100"},
            {"price": "0.56", "size": "25"},
            {"price": "0.55", "size": "40"}
        ])));

        sort_orderbook_levels("bid", &mut bids);
        sort_orderbook_levels("ask", &mut asks);
        bids.truncate(2);
        asks.truncate(2);

        assert_eq!(
            bids.iter().map(|level| level.price).collect::<Vec<_>>(),
            vec![0.54, 0.53]
        );
        assert_eq!(
            asks.iter().map(|level| level.price).collect::<Vec<_>>(),
            vec![0.55, 0.56]
        );
    }
}

fn parse_orderbook_levels(value: Option<&Value>) -> Vec<ExternalOrderBookLevel> {
    let Some(rows) = value.and_then(|entry| entry.as_array()) else {
        return Vec::new();
    };

    rows.iter()
        .filter_map(|row| {
            let price = clamp_probability(parse_f64(row.get("price")));
            let quantity = parse_f64(row.get("size")).max(0.0);
            if price <= 0.0 || quantity <= 0.0 {
                return None;
            }
            Some(ExternalOrderBookLevel {
                price,
                quantity,
                orders: 1,
            })
        })
        .collect()
}

fn sort_orderbook_levels(side: &str, levels: &mut [ExternalOrderBookLevel]) {
    if side.eq_ignore_ascii_case("bid") {
        levels.sort_by(|a, b| b.price.total_cmp(&a.price));
    } else {
        levels.sort_by(|a, b| a.price.total_cmp(&b.price));
    }
}

fn token_for_outcome(
    outcome_labels: &[String],
    token_ids: &[String],
    target: &str,
) -> Option<String> {
    for (idx, label) in outcome_labels.iter().enumerate() {
        if label.eq_ignore_ascii_case(target) {
            if let Some(token) = token_ids.get(idx) {
                return Some(token.clone());
            }
        }
    }

    if target.eq_ignore_ascii_case("yes") {
        return token_ids.first().cloned();
    }

    token_ids.get(1).cloned()
}

async fn fetch_market_row(
    client: &Client,
    gamma_api_base: &str,
    market_id: &str,
) -> Result<Value, ApiError> {
    let url = format!(
        "{}/markets/{}",
        gamma_api_base.trim_end_matches('/'),
        market_id.trim()
    );

    client
        .get(url)
        .send()
        .await
        .map_err(|err| api_error("polymarket market request failed", err))?
        .error_for_status()
        .map_err(|err| api_error("polymarket market response failed", err))?
        .json::<Value>()
        .await
        .map_err(|err| api_error("polymarket market payload invalid", err))
}

pub async fn fetch_active_markets(
    client: &Client,
    gamma_api_base: &str,
    limit: u64,
    offset: u64,
) -> Result<Vec<ExternalMarketSnapshot>, ApiError> {
    let safe_limit = limit.clamp(1, 250);
    let url = format!(
        "{}/markets?limit={}&offset={}&active=true&closed=false",
        gamma_api_base.trim_end_matches('/'),
        safe_limit,
        offset
    );

    let payload = client
        .get(url)
        .send()
        .await
        .map_err(|err| api_error("polymarket markets request failed", err))?
        .error_for_status()
        .map_err(|err| api_error("polymarket markets response failed", err))?
        .json::<Value>()
        .await
        .map_err(|err| api_error("polymarket markets payload invalid", err))?;

    let mut markets = Vec::new();
    if let Some(rows) = payload.as_array() {
        for row in rows {
            if let Some(market) = parse_polymarket_market(row) {
                markets.push(market);
            }
        }
    }

    Ok(markets)
}

pub async fn fetch_market_by_id(
    client: &Client,
    gamma_api_base: &str,
    market_id: &str,
) -> Result<ExternalMarketSnapshot, ApiError> {
    let row = fetch_market_row(client, gamma_api_base, market_id).await?;
    parse_polymarket_market(&row).ok_or_else(|| {
        ApiError::bad_request(
            "POLYMARKET_MARKET_PARSE_FAILED",
            "failed to parse Polymarket market payload",
        )
    })
}

pub async fn fetch_orderbook(
    client: &Client,
    gamma_api_base: &str,
    clob_api_base: &str,
    market_id: &str,
    outcome: &str,
    depth: u64,
) -> Result<ExternalOrderBookSnapshot, ApiError> {
    let market = fetch_market_row(client, gamma_api_base, market_id).await?;
    let outcome_labels = parse_string_list(market.get("outcomes"));
    let token_ids = parse_string_list(market.get("clobTokenIds"));
    let token_id = token_for_outcome(&outcome_labels, &token_ids, outcome).ok_or_else(|| {
        ApiError::bad_request(
            "POLYMARKET_TOKEN_NOT_FOUND",
            "unable to map outcome to polymarket token id",
        )
    })?;

    let url = format!(
        "{}/book?token_id={}",
        clob_api_base.trim_end_matches('/'),
        token_id
    );
    let payload = client
        .get(url)
        .send()
        .await
        .map_err(|err| api_error("polymarket orderbook request failed", err))?
        .error_for_status()
        .map_err(|err| api_error("polymarket orderbook response failed", err))?
        .json::<Value>()
        .await
        .map_err(|err| api_error("polymarket orderbook payload invalid", err))?;

    let mut bids = parse_orderbook_levels(payload.get("bids"));
    let mut asks = parse_orderbook_levels(payload.get("asks"));
    sort_orderbook_levels("bid", &mut bids);
    sort_orderbook_levels("ask", &mut asks);
    bids.truncate(depth as usize);
    asks.truncate(depth as usize);

    Ok(ExternalOrderBookSnapshot {
        market_id: format!("polymarket:{}", market_id),
        outcome: outcome.to_string(),
        bids,
        asks,
        last_updated: now_rfc3339(),
        source: "external_polymarket".to_string(),
        provider: "polymarket".to_string(),
        chain_id: 137,
        provider_market_ref: token_id,
        is_synthetic: false,
    })
}

pub async fn fetch_trades(
    _client: &Client,
    _gamma_api_base: &str,
    _clob_api_base: &str,
    market_id: &str,
    outcome_filter: Option<&str>,
    limit: u64,
    offset: u64,
) -> Result<ExternalTradesSnapshot, ApiError> {
    polymarket_index::fetch_public_trades(market_id, outcome_filter, limit, offset).await
}
