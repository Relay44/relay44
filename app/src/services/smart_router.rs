//! Smart Order Router.
//!
//! Compares prices across multiple venues for the same market event and
//! selects the best execution venue. Also runs a background arbitrage
//! scanner that detects cross-venue price discrepancies.

use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use crate::services::external::types::ExternalMarketId;
use crate::services::external::{self};
use crate::services::market_data::{cache as l2_cache, TopOfBook, Venue};
use crate::AppState;

fn venue_for_provider(provider: &str) -> Option<Venue> {
    match provider {
        "polymarket" => Some(Venue::Polymarket),
        "limitless" => Some(Venue::Limitless),
        "aerodrome" => Some(Venue::Aerodrome),
        _ => None,
    }
}

fn cache_market_key(venue: Venue, provider_market_id: &str, outcome: &str) -> String {
    match venue {
        Venue::Limitless => format!("{}:{}", provider_market_id, outcome),
        _ => provider_market_id.to_string(),
    }
}

fn quote_from_top(link: &VenueLink, top: &TopOfBook) -> Option<VenueQuote> {
    if top.best_bid.is_none() && top.best_ask.is_none() {
        return None;
    }
    let best_bid = top.best_bid.as_ref().map(|l| l.price).filter(|&p| p > 0.0);
    let best_ask = top.best_ask.as_ref().map(|l| l.price).filter(|&p| p > 0.0);
    let fee_mult = link.fee_bps as f64 / 10_000.0;
    Some(VenueQuote {
        provider: link.provider.clone(),
        provider_market_id: link.provider_market_id.clone(),
        best_bid,
        best_ask,
        bid_depth_usdc: top
            .best_bid
            .as_ref()
            .map(|l| l.price * l.size)
            .unwrap_or(0.0),
        ask_depth_usdc: top
            .best_ask
            .as_ref()
            .map(|l| l.price * l.size)
            .unwrap_or(0.0),
        fee_bps: link.fee_bps,
        effective_buy: best_ask.map(|p| p * (1.0 + fee_mult)),
        effective_sell: best_bid.map(|p| p * (1.0 - fee_mult)),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenueQuote {
    pub provider: String,
    pub provider_market_id: String,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub bid_depth_usdc: f64,
    pub ask_depth_usdc: f64,
    pub fee_bps: i32,
    /// Effective buy price = best_ask + fee
    pub effective_buy: Option<f64>,
    /// Effective sell price = best_bid - fee
    pub effective_sell: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub chosen: VenueQuote,
    pub alternatives: Vec<VenueQuote>,
    pub savings_bps: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub market_slug: String,
    pub outcome: String,
    pub buy_provider: String,
    pub buy_price: f64,
    pub sell_provider: String,
    pub sell_price: f64,
    pub spread_bps: i32,
    pub max_size_usdc: f64,
}

struct VenueLink {
    provider: String,
    provider_market_id: String,
    fee_bps: i32,
}

/// Fetch quotes from all linked venues for a market and pick best execution.
pub async fn route_order(
    state: &AppState,
    market_slug: &str,
    outcome: &str,
    side: &str, // "buy" or "sell"
    quantity: f64,
) -> Result<RoutingDecision, String> {
    let links = load_venue_links(state, market_slug).await?;
    if links.is_empty() {
        return Err(format!("No venue links for market {}", market_slug));
    }

    let mut quotes = Vec::with_capacity(links.len());
    for link in &links {
        match fetch_venue_quote(state, link, outcome).await {
            Ok(q) => quotes.push(q),
            Err(e) => {
                warn!("Route: skip {} for {}: {}", link.provider, market_slug, e);
            }
        }
    }

    if quotes.is_empty() {
        return Err("No venues returned valid quotes".to_string());
    }

    // Sort by effective price: best buy = lowest effective_buy, best sell = highest effective_sell.
    let is_buy = side == "buy";
    quotes.sort_by(|a, b| {
        let pa = if is_buy {
            a.effective_buy
        } else {
            a.effective_sell
        };
        let pb = if is_buy {
            b.effective_buy
        } else {
            b.effective_sell
        };
        match (pa, pb) {
            (Some(a), Some(b)) => {
                if is_buy {
                    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    b.partial_cmp(&a).unwrap_or(std::cmp::Ordering::Equal)
                }
            }
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    let chosen = quotes.remove(0);

    // Savings vs second-best venue.
    let savings_bps = if let Some(second) = quotes.first() {
        let chosen_price = if is_buy {
            chosen.effective_buy
        } else {
            chosen.effective_sell
        };
        let second_price = if is_buy {
            second.effective_buy
        } else {
            second.effective_sell
        };
        match (chosen_price, second_price) {
            (Some(c), Some(s)) if s.abs() > f64::EPSILON => {
                ((s - c).abs() / s * 10_000.0).round() as i32
            }
            _ => 0,
        }
    } else {
        0
    };

    // Log the routing decision to DB.
    if let Err(e) = sqlx::query(
        "INSERT INTO routing_decisions \
         (market_slug, outcome, side, quantity, chosen_provider, chosen_price, alternatives, savings_bps) \
         VALUES ($1, $2, $3, $4::NUMERIC, $5, $6::NUMERIC, $7, $8)",
    )
    .bind(market_slug)
    .bind(outcome)
    .bind(side)
    .bind(format!("{}", quantity))
    .bind(&chosen.provider)
    .bind(format!(
        "{}",
        if is_buy {
            chosen.effective_buy.unwrap_or(0.0)
        } else {
            chosen.effective_sell.unwrap_or(0.0)
        }
    ))
    .bind(json!(&quotes))
    .bind(savings_bps)
    .execute(state.db.pool())
    .await
    {
        warn!("Failed to log routing decision: {}", e);
    }

    Ok(RoutingDecision {
        chosen,
        alternatives: quotes,
        savings_bps,
    })
}

/// Scan all multi-venue markets for arbitrage opportunities.
pub async fn scan_arbitrage(state: &AppState) -> Result<Vec<ArbitrageOpportunity>, String> {
    let slugs: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT market_slug FROM market_venue_links \
         WHERE active = true \
         GROUP BY market_slug HAVING COUNT(*) >= 2",
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| format!("Query arb slugs: {}", e))?;

    let mut opportunities = Vec::new();

    for (slug,) in &slugs {
        for outcome in &["yes", "no"] {
            match find_arb_for_market(state, slug, outcome).await {
                Ok(Some(opp)) => {
                    log_arb_opportunity(state, &opp).await;
                    opportunities.push(opp);
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("Arb scan error for {}:{}: {}", slug, outcome, e);
                }
            }
        }
    }

    Ok(opportunities)
}

async fn find_arb_for_market(
    state: &AppState,
    market_slug: &str,
    outcome: &str,
) -> Result<Option<ArbitrageOpportunity>, String> {
    let links = load_venue_links(state, market_slug).await?;
    if links.len() < 2 {
        return Ok(None);
    }

    let mut quotes = Vec::new();
    for link in &links {
        if let Ok(q) = fetch_venue_quote(state, &link, outcome).await {
            quotes.push(q);
        }
    }
    if quotes.len() < 2 {
        return Ok(None);
    }

    // Find best buy (lowest ask) and best sell (highest bid).
    let best_buy = quotes
        .iter()
        .filter_map(|q| q.effective_buy.map(|p| (p, q)))
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let best_sell = quotes
        .iter()
        .filter_map(|q| q.effective_sell.map(|p| (p, q)))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let (buy_price, buyer) = match best_buy {
        Some((p, q)) => (p, q),
        None => return Ok(None),
    };
    let (sell_price, seller) = match best_sell {
        Some((p, q)) => (p, q),
        None => return Ok(None),
    };

    // Arb exists when sell_price > buy_price (can buy low, sell high).
    if sell_price <= buy_price {
        return Ok(None);
    }

    // Same venue doesn't count.
    if buyer.provider == seller.provider {
        return Ok(None);
    }

    let spread_bps = ((sell_price - buy_price) / buy_price * 10_000.0).round() as i32;
    let max_size = buyer.ask_depth_usdc.min(seller.bid_depth_usdc);

    // Only report if spread is meaningful (> 10 bps) and there's executable size.
    if spread_bps < 10 || max_size < 1.0 {
        return Ok(None);
    }

    Ok(Some(ArbitrageOpportunity {
        market_slug: market_slug.to_string(),
        outcome: outcome.to_string(),
        buy_provider: buyer.provider.clone(),
        buy_price,
        sell_provider: seller.provider.clone(),
        sell_price,
        spread_bps,
        max_size_usdc: max_size,
    }))
}

async fn log_arb_opportunity(state: &AppState, opp: &ArbitrageOpportunity) {
    info!(
        "Arb detected: {} {} buy@{} on {} sell@{} on {} spread={}bps size=${:.0}",
        opp.market_slug,
        opp.outcome,
        opp.buy_price,
        opp.buy_provider,
        opp.sell_price,
        opp.sell_provider,
        opp.spread_bps,
        opp.max_size_usdc,
    );
    if let Err(e) = sqlx::query(
        "INSERT INTO arbitrage_opportunities \
         (market_slug, outcome, buy_provider, buy_price, sell_provider, sell_price, spread_bps, max_size_usdc) \
         VALUES ($1, $2, $3, $4::NUMERIC, $5, $6::NUMERIC, $7, $8::NUMERIC)",
    )
    .bind(&opp.market_slug)
    .bind(&opp.outcome)
    .bind(&opp.buy_provider)
    .bind(format!("{}", opp.buy_price))
    .bind(&opp.sell_provider)
    .bind(format!("{}", opp.sell_price))
    .bind(opp.spread_bps)
    .bind(format!("{}", opp.max_size_usdc))
    .execute(state.db.pool())
    .await
    {
        warn!("Failed to log arb opportunity: {}", e);
    }
}

// ---- Background scanner ----

/// Spawn the arbitrage scanner background loop.
pub fn spawn_arb_scanner(state: Arc<AppState>) {
    if !state.config.routing_enabled {
        info!("Arb scanner disabled (ROUTING_ENABLED=false)");
        return;
    }

    let enabled = std::env::var("ARB_SCANNER_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    if !enabled {
        info!("Arb scanner disabled (ARB_SCANNER_ENABLED=false)");
        return;
    }

    let interval_secs = std::env::var("ARB_SCANNER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30)
        .max(10);

    info!("Starting arb scanner (interval={}s)", interval_secs);

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(15)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                info!("Arb scanner shutting down");
                break;
            }

            match scan_arbitrage(&state).await {
                Ok(opps) => {
                    if !opps.is_empty() {
                        info!("Arb scan: found {} opportunities", opps.len());
                    }
                }
                Err(e) => {
                    warn!("Arb scan error: {}", e);
                }
            }
        }
    });
}

// ---- Helpers ----

async fn load_venue_links(state: &AppState, market_slug: &str) -> Result<Vec<VenueLink>, String> {
    let rows: Vec<(String, String, i32)> = sqlx::query_as(
        "SELECT provider, provider_market_id, fee_bps \
         FROM market_venue_links WHERE market_slug = $1 AND active = true",
    )
    .bind(market_slug)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| format!("Query venue links: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|(provider, provider_market_id, fee_bps)| VenueLink {
            provider,
            provider_market_id,
            fee_bps,
        })
        .collect())
}

async fn fetch_venue_quote(
    state: &AppState,
    link: &VenueLink,
    outcome: &str,
) -> Result<VenueQuote, String> {
    if let Some(venue) = venue_for_provider(&link.provider) {
        let key = cache_market_key(venue, &link.provider_market_id, outcome);
        match l2_cache::read_top(&state.redis, venue, &key).await {
            Ok(Some(top)) => {
                if let Some(q) = quote_from_top(link, &top) {
                    log::debug!(
                        "smart_router cache hit {}:{} outcome={}",
                        link.provider,
                        key,
                        outcome
                    );
                    return Ok(q);
                }
            }
            Ok(None) => {}
            Err(e) => log::debug!("l2 cache read failed ({}): {}", link.provider, e),
        }
    }

    let namespaced = format!("{}:{}", link.provider, link.provider_market_id);
    let market_id = ExternalMarketId::parse(&namespaced)
        .map_err(|e| format!("Bad market id: {}", e.message))?;

    let book = external::fetch_orderbook(&state.config, &state.redis, &market_id, outcome, 10)
        .await
        .map_err(|e| format!("Fetch orderbook: {}", e.message))?;

    if book.bids.is_empty() && book.asks.is_empty() {
        return Err(format!(
            "Empty orderbook for {} on {}",
            link.provider, outcome
        ));
    }

    let best_bid = book.bids.first().map(|l| l.price).filter(|&p| p > 0.0);
    let best_ask = book.asks.first().map(|l| l.price).filter(|&p| p > 0.0);

    let bid_depth: f64 = book.bids.iter().map(|l| l.price * l.quantity).sum();
    let ask_depth: f64 = book.asks.iter().map(|l| l.price * l.quantity).sum();

    let fee_mult = link.fee_bps as f64 / 10_000.0;
    let effective_buy = best_ask.map(|p| p * (1.0 + fee_mult));
    let effective_sell = best_bid.map(|p| p * (1.0 - fee_mult));

    Ok(VenueQuote {
        provider: link.provider.clone(),
        provider_market_id: link.provider_market_id.clone(),
        best_bid,
        best_ask,
        bid_depth_usdc: bid_depth,
        ask_depth_usdc: ask_depth,
        fee_bps: link.fee_bps,
        effective_buy,
        effective_sell,
    })
}
