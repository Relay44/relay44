use reqwest::Client;
use serde_json::Value;

use crate::api::ApiError;
use crate::services::external::types::{
    clamp_probability, is_binary_yes_no, now_rfc3339, price_to_bps, ExternalMarketSnapshot,
    ExternalOrderBookLevel, ExternalOrderBookSnapshot, ExternalOutcome, ExternalTradeSnapshot,
    ExternalTradesSnapshot,
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

    Some(ExternalMarketSnapshot {
        id: format!("polymarket:{}", id),
        question: parse_string(row.get("question")),
        description: parse_string(row.get("description")),
        category: parse_string(row.get("category")).to_ascii_lowercase(),
        status: status.to_string(),
        close_time: parse_u64(row.get("endDate")),
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

async fn fetch_price_history(
    client: &Client,
    clob_api_base: &str,
    token_id: &str,
) -> Result<Vec<(u64, f64)>, ApiError> {
    let url = format!(
        "{}/prices-history?market={}&interval=1h&fidelity=60",
        clob_api_base.trim_end_matches('/'),
        token_id
    );

    let payload = client
        .get(url)
        .send()
        .await
        .map_err(|err| api_error("polymarket prices-history request failed", err))?
        .error_for_status()
        .map_err(|err| api_error("polymarket prices-history response failed", err))?
        .json::<Value>()
        .await
        .map_err(|err| api_error("polymarket prices-history payload invalid", err))?;

    let mut rows = Vec::new();
    if let Some(history) = payload.get("history").and_then(|value| value.as_array()) {
        for item in history {
            let timestamp = parse_u64(item.get("t"));
            let price = clamp_probability(parse_f64(item.get("p")));
            if timestamp == 0 {
                continue;
            }
            rows.push((timestamp, price));
        }
    }
    Ok(rows)
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
    client: &Client,
    gamma_api_base: &str,
    clob_api_base: &str,
    market_id: &str,
    outcome_filter: Option<&str>,
    limit: u64,
    offset: u64,
) -> Result<ExternalTradesSnapshot, ApiError> {
    let market = fetch_market_row(client, gamma_api_base, market_id).await?;
    let outcome_labels = parse_string_list(market.get("outcomes"));
    let token_ids = parse_string_list(market.get("clobTokenIds"));

    let mut targets: Vec<(&str, String)> = Vec::new();
    match outcome_filter {
        Some("yes") => {
            if let Some(token) = token_for_outcome(&outcome_labels, &token_ids, "yes") {
                targets.push(("yes", token));
            }
        }
        Some("no") => {
            if let Some(token) = token_for_outcome(&outcome_labels, &token_ids, "no") {
                targets.push(("no", token));
            }
        }
        _ => {
            if let Some(token) = token_for_outcome(&outcome_labels, &token_ids, "yes") {
                targets.push(("yes", token));
            }
            if let Some(token) = token_for_outcome(&outcome_labels, &token_ids, "no") {
                targets.push(("no", token));
            }
        }
    }

    let mut all_trades = Vec::new();
    for (side, token_id) in targets {
        let history = fetch_price_history(client, clob_api_base, token_id.as_str()).await?;
        for (timestamp, price) in history {
            all_trades.push(ExternalTradeSnapshot {
                id: format!("polymarket:{}:{}:{}", market_id, side, timestamp),
                market_id: format!("polymarket:{}", market_id),
                outcome: side.to_string(),
                price,
                price_bps: price_to_bps(price),
                quantity: 0,
                tx_hash: String::new(),
                block_number: 0,
                created_at: chrono::DateTime::from_timestamp(timestamp as i64, 0)
                    .map(|entry| entry.to_rfc3339())
                    .unwrap_or_else(now_rfc3339),
            });
        }
    }

    all_trades.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let total = all_trades.len() as u64;
    let start = (offset as usize).min(all_trades.len());
    let end = (start + limit as usize).min(all_trades.len());
    let page = all_trades[start..end].to_vec();

    let provider_market_ref = if let Some(first) = token_ids.first() {
        first.clone()
    } else {
        market_id.to_string()
    };

    Ok(ExternalTradesSnapshot {
        trades: page,
        total,
        limit,
        offset,
        has_more: end < all_trades.len(),
        source: "external_polymarket".to_string(),
        provider: "polymarket".to_string(),
        chain_id: 137,
        provider_market_ref,
        is_synthetic: true,
    })
}
