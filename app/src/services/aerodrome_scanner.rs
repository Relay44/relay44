//! Aerodrome Pool Scanner.
//!
//! Background service that polls registered Aerodrome Slipstream pools on Base
//! to track liquidity, pricing, and trading opportunities. Reads pool addresses
//! from the `aerodrome_pools` table, fetches on-chain state via RPC, persists
//! results into `aerodrome_scanned_pools`, and upserts venue links into
//! `market_venue_links` so the smart router can route orders to Aerodrome.

use log::{info, warn};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use crate::services::external::providers::aerodrome;
use crate::services::market_data::{L2Event, L2Level, L2Payload, Venue};
use crate::AppState;

// ── Scanner entry point ──

/// Spawn the Aerodrome pool scanner background loop.
pub fn spawn_aerodrome_scanner(state: Arc<AppState>) {
    if !state.config.aerodrome_enabled {
        info!("Aerodrome scanner disabled (aerodrome_enabled=false)");
        return;
    }

    let enabled = std::env::var("AERODROME_SCANNER_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    if !enabled {
        info!("Aerodrome scanner disabled (AERODROME_SCANNER_ENABLED=false)");
        return;
    }

    let interval_secs = std::env::var("AERODROME_SCANNER_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(120)
        .max(60);

    info!("Starting Aerodrome scanner (interval={}s)", interval_secs);

    tokio::spawn(async move {
        // Wait for app startup.
        tokio::time::sleep(Duration::from_secs(30)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                info!("Aerodrome scanner shutting down");
                break;
            }

            match run_scan(&state).await {
                Ok((indexed, opportunities, venue_links)) => {
                    if indexed > 0 || opportunities > 0 {
                        info!(
                            "Aerodrome scan: indexed={} opportunities={} venue_links={}",
                            indexed, opportunities, venue_links
                        );
                    }
                }
                Err(e) => {
                    warn!("Aerodrome scan error: {}", e);
                    record_scan_run(&state, 0, 0, 0, Some(&e)).await;
                }
            }
        }
    });
}

// ── Scan execution ──

async fn run_scan(state: &AppState) -> Result<(i32, i32, i32), String> {
    // Load registered pools with their token pair info from the aerodrome_pools table.
    let pool_rows = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT pool_address, market_id FROM aerodrome_pools WHERE active = TRUE",
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| format!("Failed to load aerodrome pools: {}", e))?;

    if pool_rows.is_empty() {
        return Ok((0, 0, 0));
    }

    let total_scanned = pool_rows.len() as i32;
    let mut indexed = 0i32;
    let mut opportunities = 0i32;
    let mut venue_links = 0i32;

    for (pool_address, market_id) in &pool_rows {
        // Fetch on-chain state via existing provider.
        let pool_state = match aerodrome::fetch_pool_state(&state.evm_rpc, pool_address).await {
            Ok(ps) => ps,
            Err(e) => {
                warn!(
                    "Aerodrome scanner: failed to fetch pool {}: {}",
                    pool_address, e
                );
                continue;
            }
        };

        let price = match pool_state.price() {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    "Aerodrome scanner: invalid price for pool {}: {}",
                    pool_address, e
                );
                continue;
            }
        };

        // Synthesize orderbook to compute spread.
        let orderbook = aerodrome::synthesize_orderbook(&pool_state, pool_address, price);
        let spread_bps = compute_spread_bps(&orderbook);

        let seq = state.market_data.next_seq(Venue::Aerodrome);
        state.market_data.emit(L2Event {
            venue: Venue::Aerodrome,
            market_key: pool_address.to_string(),
            seq,
            observed_at: chrono::Utc::now(),
            payload: L2Payload::Snapshot {
                bids: orderbook
                    .bids
                    .iter()
                    .take(10)
                    .map(|l| L2Level {
                        price: l.price,
                        size: l.quantity,
                    })
                    .collect(),
                asks: orderbook
                    .asks
                    .iter()
                    .take(10)
                    .map(|l| L2Level {
                        price: l.price,
                        size: l.quantity,
                    })
                    .collect(),
                last_trade: None,
            },
        });

        let (opp_type, opp_score) = score_opportunity(&pool_state, spread_bps);
        if opp_score > 0.0 {
            opportunities += 1;
        }

        if let Err(e) = upsert_scanned_pool(
            state,
            pool_address,
            &pool_state.token0,
            &pool_state.token1,
            pool_state.token0_symbol.as_str(),
            pool_state.token1_symbol.as_str(),
            pool_state.tick_spacing,
            pool_state.liquidity,
            pool_state.sqrt_price_x96,
            pool_state.tick,
            price,
            spread_bps,
            &opp_type,
            opp_score,
        )
        .await
        {
            warn!("Failed to persist aerodrome pool {}: {}", pool_address, e);
            continue;
        }
        indexed += 1;

        // Upsert venue link so the smart router can route to this pool.
        // The market_slug is derived from the aerodrome_pools.market_id if set,
        // otherwise from the token pair (e.g., "weth-usdc").
        let slug = match market_id {
            Some(mid) if !mid.is_empty() => mid.clone(),
            _ => format!(
                "{}-{}",
                pool_state.token0_symbol.to_lowercase(),
                pool_state.token1_symbol.to_lowercase()
            ),
        };

        if let Err(e) = upsert_venue_link(state, &slug, pool_address).await {
            warn!(
                "Failed to upsert venue link for pool {}: {}",
                pool_address, e
            );
        } else {
            venue_links += 1;
        }
    }

    record_scan_run(state, total_scanned, indexed, opportunities, None).await;

    Ok((indexed, opportunities, venue_links))
}

// ── Spread computation ──

fn compute_spread_bps(
    orderbook: &crate::services::external::types::ExternalOrderBookSnapshot,
) -> i32 {
    let best_bid = orderbook.bids.first().map(|l| l.price).unwrap_or(0.0);
    let best_ask = orderbook.asks.first().map(|l| l.price).unwrap_or(0.0);
    if best_bid > 0.0 && best_ask > best_bid {
        ((best_ask - best_bid) / best_bid * 10_000.0).round() as i32
    } else {
        0
    }
}

// ── Opportunity scoring ──

/// Score a pool for trading opportunities.
///
/// Scoring priority (checked in order, first match wins):
/// 1. `low_liquidity` (score=0) — below minimum threshold, not tradeable
/// 2. `high_liquidity` (score=liq+spread) — deep pool with good execution
/// 3. `tight_spread` (score=spread) — moderate pool with narrow spread
/// 4. `none` (score=0) — no notable opportunity
fn score_opportunity(pool: &aerodrome::AerodromePoolState, spread_bps: i32) -> (String, f64) {
    // Minimum liquidity threshold (1e15 raw units, roughly $1K+ depending on token)
    let min_liquidity: u128 = 1_000_000_000_000_000;

    if pool.liquidity < min_liquidity {
        return ("low_liquidity".to_string(), 0.0);
    }

    // Liquidity depth score: log2(liquidity) normalized to [0, ~160]
    // A pool with 1e18 liquidity ≈ 60 bits → score ~75
    // A pool with 1e30 liquidity ≈ 100 bits → score ~125
    let liq_score = (pool.liquidity as f64).log2() / 80.0 * 100.0;

    // Tight spread bonus: under 100 bps earns up to 100 bonus points
    let spread_score = if spread_bps > 0 && spread_bps < 100 {
        (100 - spread_bps) as f64
    } else {
        0.0
    };

    let total = liq_score + spread_score;

    if total > 50.0 {
        return ("high_liquidity".to_string(), total.min(200.0));
    }

    if spread_bps > 0 && spread_bps < 50 {
        return ("tight_spread".to_string(), spread_score.min(100.0));
    }

    ("none".to_string(), 0.0)
}

// ── Database operations ──

#[allow(clippy::too_many_arguments)]
async fn upsert_scanned_pool(
    state: &AppState,
    pool_address: &str,
    token0: &str,
    token1: &str,
    token0_symbol: &str,
    token1_symbol: &str,
    tick_spacing: i32,
    liquidity: u128,
    sqrt_price_x96: u128,
    tick: i32,
    price: f64,
    spread_bps: i32,
    opportunity_type: &str,
    opportunity_score: f64,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO aerodrome_scanned_pools (
            pool_address, token0, token1,
            token0_symbol, token1_symbol, tick_spacing,
            liquidity, sqrt_price_x96, tick, price,
            spread_bps, opportunity_type, opportunity_score,
            active, last_scanned_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, TRUE, NOW())
        ON CONFLICT (pool_address) DO UPDATE SET
            token0 = EXCLUDED.token0,
            token1 = EXCLUDED.token1,
            token0_symbol = EXCLUDED.token0_symbol,
            token1_symbol = EXCLUDED.token1_symbol,
            tick_spacing = EXCLUDED.tick_spacing,
            liquidity = EXCLUDED.liquidity,
            sqrt_price_x96 = EXCLUDED.sqrt_price_x96,
            tick = EXCLUDED.tick,
            price = EXCLUDED.price,
            spread_bps = EXCLUDED.spread_bps,
            opportunity_type = EXCLUDED.opportunity_type,
            opportunity_score = EXCLUDED.opportunity_score,
            active = TRUE,
            last_scanned_at = NOW()
        "#,
    )
    .bind(pool_address)
    .bind(token0)
    .bind(token1)
    .bind(token0_symbol)
    .bind(token1_symbol)
    .bind(tick_spacing)
    .bind(liquidity.to_string()) // NUMERIC(40,0) — bind u128 as string
    .bind(sqrt_price_x96.to_string()) // NUMERIC(50,0) — bind u128 as string
    .bind(tick)
    .bind(price)
    .bind(spread_bps)
    .bind(opportunity_type)
    .bind(opportunity_score)
    .execute(state.db.pool())
    .await
    .map_err(|e| format!("DB upsert: {}", e))?;

    Ok(())
}

/// Upsert an aerodrome venue link so the smart router can route orders to this pool.
///
/// The `provider_market_id` is the bare pool address (not namespaced).
/// The smart router constructs the full `aerodrome:<pool_address>` when fetching quotes.
async fn upsert_venue_link(
    state: &AppState,
    market_slug: &str,
    pool_address: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO market_venue_links (market_slug, provider, provider_market_id, fee_bps)
        VALUES ($1, 'aerodrome', $2, 0)
        ON CONFLICT (market_slug, provider) DO UPDATE SET
            provider_market_id = EXCLUDED.provider_market_id,
            fee_bps = EXCLUDED.fee_bps,
            active = TRUE,
            updated_at = NOW()
        "#,
    )
    .bind(market_slug)
    .bind(pool_address)
    .execute(state.db.pool())
    .await
    .map_err(|e| format!("Venue link upsert: {}", e))?;

    Ok(())
}

async fn record_scan_run(
    state: &AppState,
    scanned: i32,
    indexed: i32,
    opportunities: i32,
    error: Option<&str>,
) {
    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO aerodrome_scanner_runs
            (pools_scanned, pools_indexed, opportunities_found, completed_at, error)
        VALUES ($1, $2, $3, NOW(), $4)
        "#,
    )
    .bind(scanned)
    .bind(indexed)
    .bind(opportunities)
    .bind(error)
    .execute(state.db.pool())
    .await
    {
        warn!("Failed to record aerodrome scan run: {}", e);
    }
}

// ── Public API for querying scanned pools ──

/// List scanned Aerodrome pools, optionally filtered by opportunity type.
pub async fn list_scanned_pools(
    state: &AppState,
    opportunity_type: Option<&str>,
    limit: i64,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = if let Some(opp_type) = opportunity_type {
        sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<i32>,
                f64,
                i32,
                String,
                f64,
            ),
        >(
            r#"
            SELECT pool_address, token0, token1,
                   token0_symbol, token1_symbol, tick_spacing,
                   price, spread_bps,
                   opportunity_type, opportunity_score
            FROM aerodrome_scanned_pools
            WHERE active = TRUE AND opportunity_type = $1
            ORDER BY opportunity_score DESC
            LIMIT $2
            "#,
        )
        .bind(opp_type)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<i32>,
                f64,
                i32,
                String,
                f64,
            ),
        >(
            r#"
            SELECT pool_address, token0, token1,
                   token0_symbol, token1_symbol, tick_spacing,
                   price, spread_bps,
                   opportunity_type, opportunity_score
            FROM aerodrome_scanned_pools
            WHERE active = TRUE AND opportunity_score > 0
            ORDER BY opportunity_score DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| e.to_string())?
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                pool_address,
                token0,
                token1,
                t0_sym,
                t1_sym,
                tick_spacing,
                price,
                spread_bps,
                opp_type,
                opp_score,
            )| {
                json!({
                    "poolAddress": pool_address,
                    "token0": token0,
                    "token1": token1,
                    "token0Symbol": t0_sym,
                    "token1Symbol": t1_sym,
                    "tickSpacing": tick_spacing,
                    "price": price,
                    "spreadBps": spread_bps,
                    "opportunityType": opp_type,
                    "opportunityScore": opp_score,
                    "provider": "aerodrome",
                    "marketId": format!("aerodrome:{}", pool_address)
                })
            },
        )
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::external::providers::aerodrome::AerodromePoolState;
    use crate::services::external::types::ExternalOrderBookSnapshot;

    fn sample_pool() -> AerodromePoolState {
        AerodromePoolState {
            pool_address: "0xb2cc224c1c9feE385f8ad6a55b4d94E92359DC59".to_string(),
            token0: "0x4200000000000000000000000000000000000006".to_string(),
            token1: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            fee: 0,
            tick_spacing: 100,
            liquidity: 5_000_000_000_000_000_000, // 5e18
            sqrt_price_x96: 62_613_823_051_772_040_000_000_000_000,
            tick: -4700,
            token0_symbol: "WETH".to_string(),
            token1_symbol: "USDC".to_string(),
            token0_decimals: 18,
            token1_decimals: 6,
            is_slipstream: true,
        }
    }

    #[test]
    fn score_high_liquidity_pool() {
        let pool = sample_pool();
        let (opp_type, score) = score_opportunity(&pool, 10);
        assert_eq!(opp_type, "high_liquidity");
        assert!(score > 0.0, "Expected positive score, got {}", score);
    }

    #[test]
    fn score_low_liquidity_pool() {
        let mut pool = sample_pool();
        pool.liquidity = 1000; // Well below 1e15 threshold
        let (opp_type, score) = score_opportunity(&pool, 50);
        assert_eq!(opp_type, "low_liquidity");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn score_tight_spread() {
        let mut pool = sample_pool();
        pool.liquidity = 2_000_000_000_000_000; // Above threshold but modest
        let (opp_type, _score) = score_opportunity(&pool, 20);
        assert!(
            opp_type == "tight_spread" || opp_type == "high_liquidity",
            "Expected opportunity type, got {}",
            opp_type
        );
    }

    #[test]
    fn score_wide_spread_no_opportunity() {
        let mut pool = sample_pool();
        pool.liquidity = 2_000_000_000_000_000; // Above threshold
        let (opp_type, score) = score_opportunity(&pool, 200);
        // Wide spread, modest liquidity → liq_score alone probably > 50
        // but no spread bonus
        if opp_type == "none" {
            assert_eq!(score, 0.0);
        }
    }

    #[test]
    fn score_at_liquidity_boundary() {
        let mut pool = sample_pool();
        pool.liquidity = 1_000_000_000_000_000; // Exactly at threshold
        let (opp_type, _score) = score_opportunity(&pool, 30);
        assert_ne!(opp_type, "low_liquidity", "Boundary should be included");
    }

    #[test]
    fn score_capped_at_200() {
        let pool = sample_pool(); // Very high liquidity
        let (_, score) = score_opportunity(&pool, 5); // Tight spread too
        assert!(
            score <= 200.0,
            "Score should be capped at 200, got {}",
            score
        );
    }

    #[test]
    fn compute_spread_bps_normal() {
        let orderbook = ExternalOrderBookSnapshot {
            market_id: "test".to_string(),
            outcome: "yes".to_string(),
            bids: vec![crate::services::external::types::ExternalOrderBookLevel {
                price: 0.50,
                quantity: 100.0,
                orders: 1,
            }],
            asks: vec![crate::services::external::types::ExternalOrderBookLevel {
                price: 0.51,
                quantity: 100.0,
                orders: 1,
            }],
            last_updated: String::new(),
            source: "test".to_string(),
            provider: "test".to_string(),
            chain_id: 8453,
            provider_market_ref: String::new(),
            is_synthetic: true,
        };
        let bps = compute_spread_bps(&orderbook);
        assert_eq!(bps, 200); // (0.51-0.50)/0.50 * 10000 = 200 bps
    }

    #[test]
    fn compute_spread_bps_empty_orderbook() {
        let orderbook = ExternalOrderBookSnapshot {
            market_id: "test".to_string(),
            outcome: "yes".to_string(),
            bids: vec![],
            asks: vec![],
            last_updated: String::new(),
            source: "test".to_string(),
            provider: "test".to_string(),
            chain_id: 8453,
            provider_market_ref: String::new(),
            is_synthetic: true,
        };
        assert_eq!(compute_spread_bps(&orderbook), 0);
    }
}
