//! Market Auto-Creation Pipeline.
//!
//! Background service that scans popular external markets (Limitless, Polymarket)
//! and automatically creates mirrored internal relay44 markets on Base.
//! When a high-volume external market is found without a corresponding internal
//! market, the pipeline:
//!   1. Creates the market on-chain via MarketCore.createRich()
//!   2. Registers a mirror link for the liquidity mirror service
//!   3. Optionally registers bootstrap configuration
//!
//! Gated by MARKET_AUTO_CREATE_ENABLED env var.

use chrono::Utc;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};
use std::sync::Arc;
use std::time::Duration;

use crate::services::evm_signer;
use crate::services::external::{self, ExternalMarketsRequest, ExternalMarketSource, TradableFilter};
use crate::AppState;

const AUTO_CREATE_TICK_INTERVAL_SECS: u64 = 300; // Every 5 minutes
const DEFAULT_MIN_VOLUME_USDC: f64 = 10_000.0;
const DEFAULT_MAX_AUTO_MARKETS: usize = 10;
const DEFAULT_SPREAD_PREMIUM_BPS: i32 = 50;
const DEFAULT_MAX_DEPTH_USDC: f64 = 5000.0;
const MARKET_CORE_CREATE_RICH_SELECTOR: &str = "0xddabefe7";

/// Spawn the market auto-creation background loop.
pub fn spawn_market_creator(state: Arc<AppState>) {
    let enabled = std::env::var("MARKET_AUTO_CREATE_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    if !enabled {
        info!("Market auto-creation disabled (MARKET_AUTO_CREATE_ENABLED=false)");
        return;
    }

    let admin_key = std::env::var("MARKET_CREATOR_PRIVATE_KEY").unwrap_or_default();
    if admin_key.trim().is_empty() {
        warn!("Market auto-creation disabled: MARKET_CREATOR_PRIVATE_KEY not set");
        return;
    }

    if !state.config.evm_enabled || !state.config.evm_writes_enabled {
        info!("Market auto-creation disabled (EVM writes not enabled)");
        return;
    }

    let interval_secs = std::env::var("AUTO_CREATE_TICK_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(AUTO_CREATE_TICK_INTERVAL_SECS)
        .max(60);

    info!(
        "Starting market auto-creation service (interval={}s)",
        interval_secs
    );

    tokio::spawn(async move {
        // Wait for other services to initialize.
        tokio::time::sleep(Duration::from_secs(30)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                info!("Market auto-creation shutting down");
                break;
            }

            match run_auto_create_tick(&state).await {
                Ok(count) => {
                    if count > 0 {
                        info!("Auto-create tick: created {} new markets", count);
                    }
                }
                Err(e) => {
                    warn!("Auto-create tick error: {}", e);
                }
            }
        }
    });
}

async fn run_auto_create_tick(state: &AppState) -> Result<usize, String> {
    let min_volume = std::env::var("MIRROR_AUTO_CREATE_MIN_VOLUME_USDC")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(DEFAULT_MIN_VOLUME_USDC);

    let max_markets = std::env::var("MIRROR_AUTO_CREATE_MAX")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_AUTO_MARKETS);

    let admin_key = std::env::var("MARKET_CREATOR_PRIVATE_KEY").unwrap_or_default();
    let admin_address = evm_signer::address_from_private_key(&admin_key)
        .map_err(|e| format!("Invalid admin key: {}", e))?;

    // Fetch popular external markets.
    let candidates = find_mirror_candidates(state, min_volume, max_markets).await?;
    if candidates.is_empty() {
        return Ok(0);
    }

    let mut created = 0;
    for candidate in &candidates {
        match create_mirrored_market(state, candidate, &admin_key, &admin_address).await {
            Ok(_) => {
                created += 1;
                info!(
                    "Auto-created market for {} ({})",
                    candidate.question, candidate.external_id
                );
            }
            Err(e) => {
                warn!(
                    "Failed to auto-create market for {}: {}",
                    candidate.external_id, e
                );
            }
        }
    }

    Ok(created)
}

struct MirrorCandidate {
    external_id: String,
    provider: String,
    question: String,
    description: String,
    category: String,
    close_time: u64,
    volume: f64,
}

/// Find popular external markets that don't have a corresponding mirror link.
async fn find_mirror_candidates(
    state: &AppState,
    min_volume: f64,
    max: usize,
) -> Result<Vec<MirrorCandidate>, String> {
    // Fetch active external markets from Limitless (primary source).
    let markets = external::fetch_markets(
        &state.config,
        &state.redis,
        ExternalMarketSource::All,
        TradableFilter::All,
        100,
        0,
        ExternalMarketsRequest {
            include_low_liquidity: false,
            allow_limitless: true,
            allow_polymarket: true,
        },
    )
    .await
    .map_err(|e| format!("Fetch external markets: {}", e))?;

    // Get existing mirror links to avoid duplicates.
    let existing: Vec<(String,)> = sqlx::query_as(
        "SELECT external_market_id FROM mirror_market_links",
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| format!("Query existing links: {}", e))?;

    let existing_ids: std::collections::HashSet<String> =
        existing.into_iter().map(|r| r.0).collect();

    let mut candidates = Vec::new();
    for market in &markets {
        if market.volume < min_volume {
            continue;
        }
        if market.resolved {
            continue;
        }
        // Check close_time is in the future.
        let now = Utc::now().timestamp() as u64;
        if market.close_time > 0 && market.close_time < now {
            continue;
        }

        let external_id = format!("{}:{}", market.provider, market.id);
        if existing_ids.contains(&external_id) {
            continue;
        }

        // Also check if an external market with a similar provider_market_ref already exists.
        let provider_ref = &market.provider_market_ref;
        if !provider_ref.is_empty() && existing_ids.iter().any(|e| e.contains(provider_ref)) {
            continue;
        }

        candidates.push(MirrorCandidate {
            external_id,
            provider: market.provider.clone(),
            question: market.question.clone(),
            description: market.description.clone(),
            category: market.category.clone(),
            close_time: market.close_time,
            volume: market.volume,
        });

        if candidates.len() >= max {
            break;
        }
    }

    Ok(candidates)
}

/// Create an internal market on-chain and register a mirror link.
async fn create_mirrored_market(
    state: &AppState,
    candidate: &MirrorCandidate,
    admin_key: &str,
    admin_address: &str,
) -> Result<u64, String> {
    let market_core = &state.config.market_core_address;
    if market_core.trim().is_empty() {
        return Err("MARKET_CORE_ADDRESS not configured".to_string());
    }

    // Build createRich calldata.
    let close_time = if candidate.close_time > 0 {
        candidate.close_time
    } else {
        // Default: 30 days from now
        (Utc::now().timestamp() as u64) + 30 * 86400
    };

    let resolution_source = format!("Mirrored from {}", candidate.provider);
    let calldata = encode_create_rich_calldata(
        &candidate.question,
        &candidate.description,
        &candidate.category,
        &resolution_source,
        close_time,
        admin_address,
    )?;

    // Fetch nonce and gas.
    let nonce = fetch_nonce(state, admin_address).await?;
    let (base_fee, priority_fee) = fetch_gas_prices(state).await?;

    let signed_tx = evm_signer::sign_eip1559_transaction(&evm_signer::Eip1559TxParams {
        chain_id: state.config.base_chain_id,
        nonce,
        max_priority_fee_per_gas: priority_fee,
        max_fee_per_gas: base_fee * 2 + priority_fee,
        gas_limit: 500_000,
        to: market_core.clone(),
        value: 0,
        data: calldata,
        private_key: admin_key.to_string(),
    })
    .map_err(|e| format!("Tx signing: {}", e))?;

    let tx_hash = broadcast_raw_tx(state, &signed_tx).await?;
    info!("Market creation tx: {}", tx_hash);

    // Wait for receipt to get the market ID.
    let market_id = wait_for_market_id(state, &tx_hash).await?;

    // Register mirror link.
    let _ = sqlx::query(
        "INSERT INTO mirror_market_links \
         (internal_market_id, external_market_id, external_provider, \
          spread_premium_bps, max_depth_usdc, hedge_mode) \
         VALUES ($1, $2, $3, $4, $5::NUMERIC, 'auto')",
    )
    .bind(market_id as i64)
    .bind(&candidate.external_id)
    .bind(&candidate.provider)
    .bind(DEFAULT_SPREAD_PREMIUM_BPS)
    .bind(format!("{}", DEFAULT_MAX_DEPTH_USDC))
    .execute(state.db.pool())
    .await
    .map_err(|e| format!("Insert mirror link: {}", e))?;

    Ok(market_id)
}

/// Wait for the MarketCreated event in the tx receipt to extract the new market ID.
async fn wait_for_market_id(state: &AppState, tx_hash: &str) -> Result<u64, String> {
    let client = reqwest::Client::new();

    for _ in 0..30 {
        tokio::time::sleep(Duration::from_secs(2)).await;

        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash]
        });

        let resp = client
            .post(&state.config.base_rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("RPC receipt: {}", e))?;

        let result: Value = resp.json().await.map_err(|e| format!("Parse receipt: {}", e))?;
        let receipt = match result.get("result") {
            Some(Value::Null) | None => continue,
            Some(r) => r,
        };

        // Check status.
        let status = receipt
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0");
        if status != "0x1" {
            return Err(format!("Market creation tx reverted: {}", tx_hash));
        }

        // Find MarketCreated event in logs.
        // MarketCreated topic: topics[1] = marketId
        let market_created_topic =
            "0x550857481380e1875f94e5eac6470eff69ecd368405067d9d5dfdf645d3d1f8e";

        if let Some(logs) = receipt.get("logs").and_then(|v| v.as_array()) {
            for log in logs {
                let topics = log.get("topics").and_then(|v| v.as_array());
                if let Some(topics) = topics {
                    if topics.first().and_then(|t| t.as_str()) == Some(market_created_topic) {
                        if let Some(market_topic) = topics.get(1).and_then(|t| t.as_str()) {
                            let hex = market_topic.trim_start_matches("0x");
                            let trimmed = hex.trim_start_matches('0');
                            if trimmed.is_empty() {
                                return Ok(0);
                            }
                            return u64::from_str_radix(trimmed, 16)
                                .map_err(|e| format!("Parse market ID: {}", e));
                        }
                    }
                }
            }
        }

        return Err(format!("No MarketCreated event in receipt: {}", tx_hash));
    }

    Err(format!("Timed out waiting for tx receipt: {}", tx_hash))
}

// ---- ABI encoding for createRich ----

fn encode_create_rich_calldata(
    question: &str,
    description: &str,
    category: &str,
    resolution_source: &str,
    close_time: u64,
    resolver: &str,
) -> Result<String, String> {
    let question_tail = encode_dynamic_string_tail(question);
    let description_tail = encode_dynamic_string_tail(description);
    let category_tail = encode_dynamic_string_tail(category);
    let source_tail = encode_dynamic_string_tail(resolution_source);

    let resolver_hex = resolver.trim_start_matches("0x").to_ascii_lowercase();
    let resolver_word = format!("{:0>64}", resolver_hex);

    // Head: 6 x 32 bytes (5 dynamic offsets + closeTime + resolver)
    // Actually: question_offset, description_offset, category_offset, source_offset, closeTime, resolver
    let head_len_bytes = 32usize * 6;
    let question_offset = head_len_bytes;
    let description_offset = question_offset + question_tail.len() / 2;
    let category_offset = description_offset + description_tail.len() / 2;
    let source_offset = category_offset + category_tail.len() / 2;

    let calldata = format!(
        "{}{}{}{}{}{}{}{}{}{}{}",
        MARKET_CORE_CREATE_RICH_SELECTOR.trim_start_matches("0x"),
        format!("{:064x}", question_offset),
        format!("{:064x}", description_offset),
        format!("{:064x}", category_offset),
        format!("{:064x}", source_offset),
        format!("{:064x}", close_time),
        resolver_word,
        question_tail,
        description_tail,
        category_tail,
        source_tail,
    );

    Ok(format!("0x{}", calldata))
}

fn encode_dynamic_string_tail(value: &str) -> String {
    let encoded = hex::encode(value.as_bytes());
    let padded_len = if encoded.is_empty() {
        0
    } else {
        ((encoded.len() + 63) / 64) * 64
    };
    let mut padded = encoded;
    if padded.len() < padded_len {
        padded.push_str(&"0".repeat(padded_len - padded.len()));
    }
    format!("{:064x}{}", value.len(), padded)
}

// ---- RPC helpers (shared with hedge engine) ----

async fn fetch_nonce(state: &AppState, address: &str) -> Result<u64, String> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getTransactionCount",
        "params": [address, "pending"]
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(&state.config.base_rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("RPC nonce: {}", e))?;
    let result: Value = resp.json().await.map_err(|e| format!("Parse nonce: {}", e))?;
    let hex = result.get("result").and_then(|v| v.as_str()).ok_or("No nonce")?;
    u64::from_str_radix(hex.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Parse nonce hex: {}", e))
}

async fn fetch_gas_prices(state: &AppState) -> Result<(u128, u128), String> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_gasPrice",
        "params": []
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(&state.config.base_rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("RPC gas: {}", e))?;
    let result: Value = resp.json().await.map_err(|e| format!("Parse gas: {}", e))?;
    let hex = result.get("result").and_then(|v| v.as_str()).ok_or("No gas price")?;
    let base_fee = u128::from_str_radix(hex.trim_start_matches("0x"), 16).unwrap_or(1_000_000_000);
    Ok((base_fee, 100_000_000u128))
}

async fn broadcast_raw_tx(state: &AppState, signed_tx: &str) -> Result<String, String> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_sendRawTransaction",
        "params": [signed_tx]
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(&state.config.base_rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Broadcast: {}", e))?;
    let result: Value = resp.json().await.map_err(|e| format!("Parse broadcast: {}", e))?;
    if let Some(error) = result.get("error") {
        return Err(format!("Broadcast error: {}", error));
    }
    result
        .get("result")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("No tx hash".to_string())
}
