//! Liquidity Mirror Service.
//!
//! Background tokio task that reads external orderbooks (Limitless, Polymarket,
//! Aerodrome) and synthesizes mirrored quotes for internal relay44 markets.
//! Mirror quotes are stored in Redis and merged into the internal orderbook
//! display, giving native markets real depth backed by external venues.

use chrono::Utc;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use crate::services::external::types::ExternalMarketId;
use crate::services::external::{self};
use crate::AppState;

const DEFAULT_TICK_INTERVAL_SECS: u64 = 5;
const DEFAULT_SPREAD_PREMIUM_BPS: u64 = 50;
const DEFAULT_MAX_DEPTH_USDC: f64 = 5000.0;
const DEFAULT_MAX_MARKETS: usize = 50;
const MIRROR_REDIS_PREFIX: &str = "mirror:quotes:";
const MIRROR_REDIS_TTL_SECONDS: u64 = 15;

/// A mirrored quote level ready to merge into the internal orderbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorQuote {
    pub price_bps: u64,
    pub quantity: u64, // micro-USDC (6 decimals)
    pub source: String,
}

/// Snapshot of mirrored depth for a single market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorDepthSnapshot {
    pub market_id: u64,
    pub external_market_id: String,
    pub provider: String,
    pub yes_bids: Vec<MirrorQuote>,
    pub no_bids: Vec<MirrorQuote>,
    pub spread_premium_bps: u64,
    pub total_depth_usdc: f64,
    pub updated_at: String,
}

/// Database row for a mirror link.
#[derive(Debug, Clone)]
pub struct MirrorMarketLink {
    pub id: i32,
    pub internal_market_id: i64,
    pub external_market_id: String,
    pub external_provider: String,
    pub spread_premium_bps: i32,
    pub max_depth_usdc: f64,
    pub hedge_mode: String,
    pub hedge_credential_id: Option<String>,
    pub active: bool,
}

/// Spawn the liquidity mirror background loop.
pub fn spawn_liquidity_mirror(state: Arc<AppState>) {
    let enabled = std::env::var("LIQUIDITY_MIRROR_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    if !enabled {
        info!("Liquidity mirror disabled (LIQUIDITY_MIRROR_ENABLED=false)");
        return;
    }

    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        info!("Liquidity mirror disabled (EVM reads not enabled)");
        return;
    }

    let interval_secs = std::env::var("MIRROR_TICK_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TICK_INTERVAL_SECS)
        .max(2);

    info!(
        "Starting liquidity mirror service (interval={}s)",
        interval_secs
    );

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(8)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                info!("Liquidity mirror shutting down");
                break;
            }

            match run_mirror_tick(&state).await {
                Ok(count) => {
                    if count > 0 {
                        info!("Mirror tick: updated {} markets", count);
                    }
                }
                Err(e) => {
                    warn!("Mirror tick error: {}", e);
                }
            }
        }
    });
}

/// Run one mirror tick: refresh quotes for all active mirror links.
async fn run_mirror_tick(state: &AppState) -> Result<usize, String> {
    let max_markets = std::env::var("MIRROR_MAX_MARKETS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_MARKETS);

    let links = load_active_mirror_links(state, max_markets)
        .await
        .map_err(|e| format!("Failed to load mirror links: {}", e))?;

    if links.is_empty() {
        return Ok(0);
    }

    let mut updated = 0;
    for link in &links {
        match mirror_single_market(state, link).await {
            Ok(snapshot) => {
                if let Err(e) = store_mirror_snapshot(state, link.internal_market_id as u64, &snapshot).await {
                    warn!(
                        "Failed to store mirror snapshot for market {}: {}",
                        link.internal_market_id, e
                    );
                    update_mirror_error(state, link.id, &e).await;
                } else {
                    updated += 1;
                    clear_mirror_error(state, link.id).await;
                }
            }
            Err(e) => {
                warn!(
                    "Mirror failed for {} -> {}: {}",
                    link.internal_market_id, link.external_market_id, e
                );
                update_mirror_error(state, link.id, &e).await;
            }
        }
    }

    Ok(updated)
}

/// Fetch external orderbook and transform into mirrored quotes.
async fn mirror_single_market(
    state: &AppState,
    link: &MirrorMarketLink,
) -> Result<MirrorDepthSnapshot, String> {
    let external_id = ExternalMarketId::parse(&link.external_market_id)
        .map_err(|e| format!("Invalid external market id: {}", e))?;

    let spread_bps = if link.spread_premium_bps > 0 {
        link.spread_premium_bps as u64
    } else {
        DEFAULT_SPREAD_PREMIUM_BPS
    };

    let max_depth = if link.max_depth_usdc > 0.0 {
        link.max_depth_usdc
    } else {
        DEFAULT_MAX_DEPTH_USDC
    };

    // Fetch YES orderbook from external venue.
    let yes_snapshot =
        external::fetch_orderbook(&state.config, &state.redis, &external_id, "yes", 20)
            .await
            .map_err(|e| format!("Fetch yes orderbook: {}", e))?;

    let mut yes_bids: Vec<MirrorQuote> = Vec::new();
    let mut no_bids: Vec<MirrorQuote> = Vec::new();
    let mut total_depth = 0.0;

    // Mirror YES bids: external YES bids at price P → internal YES bid at P - spread/2.
    for entry in &yes_snapshot.bids {
        if total_depth >= max_depth {
            break;
        }
        let price_bps = (entry.price * 10_000.0).round() as u64;
        let adjusted = price_bps.saturating_sub(spread_bps / 2);
        if adjusted < 100 || adjusted >= 9900 {
            continue;
        }
        let quantity_micro = (entry.quantity * 1_000_000.0).round() as u64;
        if quantity_micro == 0 {
            continue;
        }
        total_depth += entry.quantity;
        yes_bids.push(MirrorQuote {
            price_bps: adjusted,
            quantity: quantity_micro,
            source: link.external_provider.clone(),
        });
    }

    // Mirror YES asks → become NO bids at complementary price.
    // External YES ask at P means someone sells YES at P.
    // This is equivalent to a NO bid at (10000 - P).
    for entry in &yes_snapshot.asks {
        if total_depth >= max_depth {
            break;
        }
        let price_bps = (entry.price * 10_000.0).round() as u64;
        let no_price = 10_000u64.saturating_sub(price_bps);
        let adjusted = no_price.saturating_sub(spread_bps / 2);
        if adjusted < 100 || adjusted >= 9900 {
            continue;
        }
        let quantity_micro = (entry.quantity * 1_000_000.0).round() as u64;
        if quantity_micro == 0 {
            continue;
        }
        total_depth += entry.quantity;
        no_bids.push(MirrorQuote {
            price_bps: adjusted,
            quantity: quantity_micro,
            source: link.external_provider.clone(),
        });
    }

    Ok(MirrorDepthSnapshot {
        market_id: link.internal_market_id as u64,
        external_market_id: link.external_market_id.clone(),
        provider: link.external_provider.clone(),
        yes_bids,
        no_bids,
        spread_premium_bps: spread_bps,
        total_depth_usdc: total_depth,
        updated_at: Utc::now().to_rfc3339(),
    })
}

/// Store mirror snapshot in Redis for the orderbook endpoint to read.
async fn store_mirror_snapshot(
    state: &AppState,
    market_id: u64,
    snapshot: &MirrorDepthSnapshot,
) -> Result<(), String> {
    let key = format!("{}{}", MIRROR_REDIS_PREFIX, market_id);
    state
        .redis
        .set(&key, snapshot, Some(MIRROR_REDIS_TTL_SECONDS))
        .await
        .map_err(|e| format!("Redis set: {}", e))
}

/// Load mirror snapshot from Redis (called by orderbook endpoint).
pub async fn load_mirror_snapshot(
    state: &AppState,
    market_id: u64,
) -> Option<MirrorDepthSnapshot> {
    let key = format!("{}{}", MIRROR_REDIS_PREFIX, market_id);
    state
        .redis
        .get::<MirrorDepthSnapshot>(&key)
        .await
        .ok()
        .flatten()
}

/// Convert mirror quotes into BTreeMap<price_bps, quantity> for merging with
/// the on-chain orderbook (same format as bootstrap synthetic book).
pub fn mirror_to_level_map(quotes: &[MirrorQuote]) -> BTreeMap<u64, u64> {
    let mut map = BTreeMap::new();
    for q in quotes {
        *map.entry(q.price_bps).or_insert(0) += q.quantity;
    }
    map
}

// ---- Database helpers ----

async fn load_active_mirror_links(
    state: &AppState,
    limit: usize,
) -> Result<Vec<MirrorMarketLink>, String> {
    let rows = sqlx::query(
        "SELECT id, internal_market_id, external_market_id, external_provider,
                spread_premium_bps, max_depth_usdc::text as max_depth_text, hedge_mode,
                hedge_credential_id, active
         FROM mirror_market_links
         WHERE active = true
         ORDER BY id
         LIMIT $1",
    )
    .bind(limit as i64)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| format!("Query mirror links: {}", e))?;

    Ok(rows
        .iter()
        .map(|row| {
            let max_depth_str: String = row
                .try_get::<String, _>("max_depth_text")
                .unwrap_or_else(|_| "5000".to_string());
            MirrorMarketLink {
                id: row.get("id"),
                internal_market_id: row.get("internal_market_id"),
                external_market_id: row.get("external_market_id"),
                external_provider: row.get("external_provider"),
                spread_premium_bps: row.get("spread_premium_bps"),
                max_depth_usdc: max_depth_str.parse::<f64>().unwrap_or(DEFAULT_MAX_DEPTH_USDC),
                hedge_mode: row.get("hedge_mode"),
                hedge_credential_id: row.try_get("hedge_credential_id").ok(),
                active: row.get("active"),
            }
        })
        .collect())
}

async fn update_mirror_error(state: &AppState, link_id: i32, error: &str) {
    let _ = sqlx::query(
        "UPDATE mirror_market_links SET mirror_error = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(error)
    .bind(link_id)
    .execute(state.db.pool())
    .await;
}

async fn clear_mirror_error(state: &AppState, link_id: i32) {
    let _ = sqlx::query(
        "UPDATE mirror_market_links SET mirror_error = NULL, last_mirror_at = NOW(), updated_at = NOW() WHERE id = $1",
    )
    .bind(link_id)
    .execute(state.db.pool())
    .await;
}
