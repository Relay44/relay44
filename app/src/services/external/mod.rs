pub mod credentials;
pub mod paper;
pub mod providers;
pub mod types;

use futures::stream::{FuturesUnordered, StreamExt};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::api::ApiError;
use crate::config::AppConfig;
use crate::services::redis::RedisService;

use self::providers::{limitless, polymarket};
use self::types::{
    ExternalMarketId, ExternalMarketSnapshot, ExternalOrderBookSnapshot, ExternalProvider,
    ExternalTradesSnapshot,
};

const MARKET_CACHE_TTL_SECONDS: u64 = 30;
const ORDERBOOK_CACHE_TTL_SECONDS: u64 = 5;
const TRADES_CACHE_TTL_SECONDS: u64 = 10;
const LIMITLESS_MIN_VOLUME_USDC_DEFAULT: f64 = 25_000.0;
const LIMITLESS_MIN_DEPTH_USDC_DEFAULT: f64 = 2_500.0;
const LIMITLESS_LIQUIDITY_CONCURRENCY_DEFAULT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalMarketSource {
    All,
    Internal,
    Limitless,
    Polymarket,
}

impl ExternalMarketSource {
    pub fn from_query(value: Option<&str>) -> Result<Self, ApiError> {
        match value.unwrap_or("all").trim().to_ascii_lowercase().as_str() {
            "all" => Ok(Self::All),
            "internal" => Ok(Self::Internal),
            "limitless" => Ok(Self::Limitless),
            "polymarket" => Ok(Self::Polymarket),
            _ => Err(ApiError::bad_request(
                "INVALID_SOURCE",
                "source must be one of: all, internal, limitless, polymarket",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradableFilter {
    All,
    User,
    Agent,
}

#[derive(Debug, Clone, Copy)]
pub struct ExternalMarketsRequest {
    pub include_low_liquidity: bool,
    pub allow_limitless: bool,
    pub allow_polymarket: bool,
}

impl Default for ExternalMarketsRequest {
    fn default() -> Self {
        Self {
            include_low_liquidity: false,
            allow_limitless: true,
            allow_polymarket: true,
        }
    }
}

impl TradableFilter {
    pub fn from_query(value: Option<&str>) -> Result<Self, ApiError> {
        match value.unwrap_or("all").trim().to_ascii_lowercase().as_str() {
            "all" => Ok(Self::All),
            "user" => Ok(Self::User),
            "agent" => Ok(Self::Agent),
            _ => Err(ApiError::bad_request(
                "INVALID_TRADABLE_FILTER",
                "tradable must be one of: all, user, agent",
            )),
        }
    }
}

fn http_client() -> Result<Client, ApiError> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| {
            ApiError::internal(&format!("failed to build external http client: {}", err))
        })
}

fn include_market(filter: TradableFilter, market: &ExternalMarketSnapshot) -> bool {
    match filter {
        TradableFilter::All => true,
        TradableFilter::User => market.execution_users,
        TradableFilter::Agent => market.execution_agents,
    }
}

fn parse_positive_f64_env(key: &str, fallback: f64) -> f64 {
    let raw = std::env::var(key).ok();
    let Some(raw) = raw else {
        return fallback;
    };
    match raw.parse::<f64>() {
        Ok(value) if value > 0.0 && value.is_finite() => value,
        _ => fallback,
    }
}

fn parse_positive_usize_env(key: &str, fallback: usize) -> usize {
    let raw = std::env::var(key).ok();
    let Some(raw) = raw else {
        return fallback;
    };
    match raw.parse::<usize>() {
        Ok(value) if value > 0 => value,
        _ => fallback,
    }
}

fn min_limitless_volume_usdc() -> f64 {
    parse_positive_f64_env(
        "LIMITLESS_MARKET_MIN_24H_VOLUME_USDC",
        LIMITLESS_MIN_VOLUME_USDC_DEFAULT,
    )
}

fn min_limitless_depth_usdc() -> f64 {
    parse_positive_f64_env(
        "LIMITLESS_MARKET_MIN_ORDERBOOK_DEPTH_USDC",
        LIMITLESS_MIN_DEPTH_USDC_DEFAULT,
    )
}

fn limitless_liquidity_check_concurrency() -> usize {
    parse_positive_usize_env(
        "LIMITLESS_LIQUIDITY_CHECK_CONCURRENCY",
        LIMITLESS_LIQUIDITY_CONCURRENCY_DEFAULT,
    )
}

fn orderbook_depth_usdc(snapshot: &ExternalOrderBookSnapshot) -> f64 {
    let mut notional = 0.0;
    for level in &snapshot.bids {
        notional += level.price.max(0.0) * level.quantity.max(0.0);
    }
    for level in &snapshot.asks {
        notional += level.price.max(0.0) * level.quantity.max(0.0);
    }
    notional
}

fn filter_by_volume_threshold(
    markets: Vec<ExternalMarketSnapshot>,
    min_volume: f64,
) -> Vec<ExternalMarketSnapshot> {
    markets
        .into_iter()
        .filter(|market| market.volume >= min_volume)
        .collect()
}

async fn filter_limitless_liquidity(
    client: &Client,
    config: &AppConfig,
    markets: Vec<ExternalMarketSnapshot>,
    include_low_liquidity: bool,
) -> Vec<ExternalMarketSnapshot> {
    if include_low_liquidity || markets.is_empty() {
        return markets;
    }

    let min_volume = min_limitless_volume_usdc();
    let min_depth = min_limitless_depth_usdc();
    let by_volume = filter_by_volume_threshold(markets, min_volume);

    if by_volume.is_empty() || min_depth <= 0.0 {
        return by_volume;
    }

    let max_concurrency = limitless_liquidity_check_concurrency()
        .max(1)
        .min(by_volume.len());
    let limiter = Arc::new(Semaphore::new(max_concurrency));
    let mut jobs = FuturesUnordered::new();

    for market in by_volume {
        let limiter = limiter.clone();
        let api_base = config.limitless_api_base.clone();
        let client = client.clone();
        jobs.push(async move {
            let _permit = limiter.acquire_owned().await.ok()?;
            let slug = market
                .id
                .split_once(':')
                .map(|(_, value)| value)
                .unwrap_or(market.id.as_str())
                .trim();
            if slug.is_empty() {
                return None;
            }

            let orderbook = limitless::fetch_orderbook(&client, api_base.as_str(), slug, "yes", 20)
                .await
                .ok()?;
            if orderbook_depth_usdc(&orderbook) >= min_depth {
                Some(market)
            } else {
                None
            }
        });
    }

    let mut accepted = Vec::new();
    while let Some(result) = jobs.next().await {
        if let Some(market) = result {
            accepted.push(market);
        }
    }

    accepted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::external::types::{ExternalOrderBookLevel, ExternalOutcome};

    fn sample_market(id: &str, volume: f64) -> ExternalMarketSnapshot {
        ExternalMarketSnapshot {
            id: id.to_string(),
            question: "q".to_string(),
            description: "d".to_string(),
            category: "c".to_string(),
            status: "active".to_string(),
            close_time: 0,
            resolved: false,
            outcome: None,
            yes_price: 0.5,
            no_price: 0.5,
            volume,
            source: "external_limitless".to_string(),
            provider: "limitless".to_string(),
            is_external: true,
            external_url: "https://example.com".to_string(),
            chain_id: 8453,
            requires_credentials: true,
            execution_users: true,
            execution_agents: true,
            outcomes: vec![
                ExternalOutcome {
                    label: "Yes".to_string(),
                    probability: 0.5,
                },
                ExternalOutcome {
                    label: "No".to_string(),
                    probability: 0.5,
                },
            ],
            provider_market_ref: "ref".to_string(),
        }
    }

    #[test]
    fn volume_threshold_filters_low_liquidity_markets() {
        let markets = vec![sample_market("a", 100.0), sample_market("b", 50000.0)];
        let filtered = filter_by_volume_threshold(markets, 25_000.0);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "b");
    }

    #[test]
    fn orderbook_depth_is_total_notional() {
        let snapshot = ExternalOrderBookSnapshot {
            market_id: "limitless:test".to_string(),
            outcome: "yes".to_string(),
            bids: vec![ExternalOrderBookLevel {
                price: 0.6,
                quantity: 1000.0,
                orders: 2,
            }],
            asks: vec![ExternalOrderBookLevel {
                price: 0.4,
                quantity: 500.0,
                orders: 1,
            }],
            last_updated: "2025-01-01T00:00:00Z".to_string(),
            source: "external_limitless".to_string(),
            provider: "limitless".to_string(),
            chain_id: 8453,
            provider_market_ref: "ref".to_string(),
            is_synthetic: false,
        };

        assert!((orderbook_depth_usdc(&snapshot) - 800.0).abs() < f64::EPSILON);
    }
}

fn with_retries<T, F, Fut>(mut action: F) -> impl std::future::Future<Output = Result<T, ApiError>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ApiError>>,
{
    async move {
        let mut last_error: Option<ApiError> = None;
        for attempt in 0..3 {
            match action().await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    last_error = Some(err);
                    if attempt < 2 {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            200 * (attempt + 1) as u64,
                        ))
                        .await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ApiError::internal("external provider request failed")))
    }
}

pub async fn fetch_markets(
    config: &AppConfig,
    redis: &RedisService,
    source: ExternalMarketSource,
    tradable_filter: TradableFilter,
    limit: u64,
    offset: u64,
    request: ExternalMarketsRequest,
) -> Result<Vec<ExternalMarketSnapshot>, ApiError> {
    if !config.external_markets_enabled {
        return Ok(Vec::new());
    }

    let cache_key = format!(
        "external:markets:{}:{}:{}:{}:{}",
        match source {
            ExternalMarketSource::All => "all",
            ExternalMarketSource::Internal => "internal",
            ExternalMarketSource::Limitless => "limitless",
            ExternalMarketSource::Polymarket => "polymarket",
        },
        match tradable_filter {
            TradableFilter::All => "all",
            TradableFilter::User => "user",
            TradableFilter::Agent => "agent",
        },
        limit,
        offset,
        if request.include_low_liquidity {
            "include_low_liquidity"
        } else {
            "liquidity_filtered"
        }
    );

    if let Ok(Some(cached)) = redis.get::<Vec<ExternalMarketSnapshot>>(&cache_key).await {
        return Ok(cached);
    }

    let client = http_client()?;
    let mut markets = Vec::new();

    if matches!(
        source,
        ExternalMarketSource::All | ExternalMarketSource::Limitless
    ) && config.limitless_enabled
        && request.allow_limitless
    {
        let fetched = with_retries(|| {
            limitless::fetch_active_markets(
                &client,
                config.limitless_api_base.as_str(),
                limit.max(1),
                offset,
            )
        })
        .await?;
        let fetched = fetched
            .into_iter()
            .filter(|entry| include_market(tradable_filter, entry))
            .collect::<Vec<_>>();
        let filtered =
            filter_limitless_liquidity(&client, config, fetched, request.include_low_liquidity)
                .await;
        markets.extend(filtered);
    }

    if matches!(
        source,
        ExternalMarketSource::All | ExternalMarketSource::Polymarket
    ) && config.polymarket_enabled
        && request.allow_polymarket
    {
        let fetched = with_retries(|| {
            polymarket::fetch_active_markets(
                &client,
                config.polymarket_gamma_api_base.as_str(),
                limit.max(1),
                offset,
            )
        })
        .await?;
        markets.extend(
            fetched
                .into_iter()
                .filter(|entry| include_market(tradable_filter, entry)),
        );
    }

    markets.sort_by(|a, b| {
        b.close_time
            .cmp(&a.close_time)
            .then_with(|| a.id.cmp(&b.id))
    });

    let _ = redis
        .set(&cache_key, &markets, Some(MARKET_CACHE_TTL_SECONDS))
        .await;

    Ok(markets)
}

pub async fn fetch_market_by_id(
    config: &AppConfig,
    market_id: &ExternalMarketId,
) -> Result<ExternalMarketSnapshot, ApiError> {
    if !config.external_markets_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_MARKETS_DISABLED",
            "external market integration is disabled",
        ));
    }

    let client = http_client()?;
    match market_id.provider {
        ExternalProvider::Limitless => {
            if !config.limitless_enabled {
                return Err(ApiError::bad_request(
                    "LIMITLESS_DISABLED",
                    "Limitless integration is disabled",
                ));
            }
            with_retries(|| {
                limitless::fetch_market_by_slug(
                    &client,
                    config.limitless_api_base.as_str(),
                    market_id.value.as_str(),
                )
            })
            .await
        }
        ExternalProvider::Polymarket => {
            if !config.polymarket_enabled {
                return Err(ApiError::bad_request(
                    "POLYMARKET_DISABLED",
                    "Polymarket integration is disabled",
                ));
            }
            with_retries(|| {
                polymarket::fetch_market_by_id(
                    &client,
                    config.polymarket_gamma_api_base.as_str(),
                    market_id.value.as_str(),
                )
            })
            .await
        }
    }
}

pub async fn fetch_orderbook(
    config: &AppConfig,
    redis: &RedisService,
    market_id: &ExternalMarketId,
    outcome: &str,
    depth: u64,
) -> Result<ExternalOrderBookSnapshot, ApiError> {
    if !config.external_markets_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_MARKETS_DISABLED",
            "external market integration is disabled",
        ));
    }

    let cache_key = format!(
        "external:orderbook:{}:{}:{}",
        market_id.full_id(),
        outcome,
        depth
    );
    if let Ok(Some(cached)) = redis.get::<ExternalOrderBookSnapshot>(&cache_key).await {
        return Ok(cached);
    }

    let client = http_client()?;
    let value = match market_id.provider {
        ExternalProvider::Limitless => {
            with_retries(|| {
                limitless::fetch_orderbook(
                    &client,
                    config.limitless_api_base.as_str(),
                    market_id.value.as_str(),
                    outcome,
                    depth,
                )
            })
            .await?
        }
        ExternalProvider::Polymarket => {
            with_retries(|| {
                polymarket::fetch_orderbook(
                    &client,
                    config.polymarket_gamma_api_base.as_str(),
                    config.polymarket_clob_api_base.as_str(),
                    market_id.value.as_str(),
                    outcome,
                    depth,
                )
            })
            .await?
        }
    };

    let _ = redis
        .set(&cache_key, &value, Some(ORDERBOOK_CACHE_TTL_SECONDS))
        .await;

    Ok(value)
}

pub async fn fetch_trades(
    config: &AppConfig,
    redis: &RedisService,
    market_id: &ExternalMarketId,
    outcome: Option<&str>,
    limit: u64,
    offset: u64,
) -> Result<ExternalTradesSnapshot, ApiError> {
    if !config.external_markets_enabled {
        return Err(ApiError::bad_request(
            "EXTERNAL_MARKETS_DISABLED",
            "external market integration is disabled",
        ));
    }

    let cache_key = format!(
        "external:trades:{}:{}:{}:{}",
        market_id.full_id(),
        outcome.unwrap_or("all"),
        limit,
        offset
    );
    if let Ok(Some(cached)) = redis.get::<ExternalTradesSnapshot>(&cache_key).await {
        return Ok(cached);
    }

    let client = http_client()?;
    let value = match market_id.provider {
        ExternalProvider::Limitless => {
            with_retries(|| {
                limitless::fetch_trades(
                    &client,
                    config.limitless_api_base.as_str(),
                    market_id.value.as_str(),
                    outcome,
                    limit,
                    offset,
                )
            })
            .await?
        }
        ExternalProvider::Polymarket => {
            with_retries(|| {
                polymarket::fetch_trades(
                    &client,
                    config.polymarket_gamma_api_base.as_str(),
                    config.polymarket_clob_api_base.as_str(),
                    market_id.value.as_str(),
                    outcome,
                    limit,
                    offset,
                )
            })
            .await?
        }
    };

    let _ = redis
        .set(&cache_key, &value, Some(TRADES_CACHE_TTL_SECONDS))
        .await;

    Ok(value)
}
