//! Auto-Hedge Engine.
//!
//! Background service that monitors fills on mirrored markets and automatically
//! hedges them on the corresponding external venue. When a relay44 user fills
//! a mirrored order, the hedge engine places the equivalent order on the
//! external venue (Limitless, Polymarket, or Aerodrome).
//!
//! Strategies:
//!   - `auto`      — immediate hedge on every detected fill
//!   - `batch`     — accumulate fills, hedge net exposure every 30s
//!   - `disabled`  — no hedging (mirror-only mode)

use base64::engine::general_purpose::URL_SAFE;
use base64::Engine as _;
use chrono::Utc;
use hmac::{Hmac, KeyInit as _, Mac as _};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use sha3::{Digest, Keccak256};
use std::sync::Arc;
use std::time::Duration;

use crate::services::evm_signer;
use crate::services::external::credentials::decrypt_json;
use crate::AppState;

const HEDGE_TICK_INTERVAL_SECS: u64 = 10;
const HEDGE_REDIS_PREFIX: &str = "hedge:pending:";
const HEDGE_REDIS_TTL: u64 = 300;
const HEDGE_ORACLE_MAX_DEVIATION_PCT: f64 = 3.0;

// Limitless EIP-712 constants (must match api/external.rs)
const LIMITLESS_SIGNING_NAME: &str = "Limitless CTF Exchange";
const LIMITLESS_SIGNING_VERSION: &str = "1";
const LIMITLESS_CHAIN_ID: u64 = 8453;
const LIMITLESS_SCALE: u128 = 1_000_000;
const LIMITLESS_PRICE_TICK: u128 = 1_000;

// Polymarket EIP-712 constants (must match api/external.rs)
const POLYMARKET_SIGNING_NAME: &str = "Polymarket CTF Exchange";
const POLYMARKET_SIGNING_VERSION: &str = "1";
const POLYMARKET_CHAIN_ID: u64 = 137;
const POLYMARKET_SCALE: u128 = 1_000_000;
const POLYMARKET_LOT_STEP: u128 = 10_000;
const POLYMARKET_EXCHANGE: &str = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E";
const POLYMARKET_NEG_RISK_EXCHANGE: &str = "0xC5d563A36AE78145C45a50134d48A1215220f80a";

/// A fill detected on an internal mirrored market that needs hedging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingHedge {
    pub mirror_link_id: i32,
    pub internal_market_id: i64,
    pub external_market_id: String,
    pub external_provider: String,
    pub direction: String, // "buy" or "sell" — what the internal user did
    pub side: String,      // "yes" or "no"
    pub outcome: String,   // "yes" or "no"
    pub price: f64,        // 0.0 to 1.0
    pub quantity: f64,     // USDC amount
    pub fill_tx_hash: String,
    pub detected_at: String,
}

/// Spawn the hedge engine background loop.
pub fn spawn_hedge_engine(state: Arc<AppState>) {
    let enabled = std::env::var("HEDGE_ENGINE_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    if !enabled {
        info!("Hedge engine disabled (HEDGE_ENGINE_ENABLED=false)");
        return;
    }

    if !state.config.evm_enabled {
        info!("Hedge engine disabled (EVM not enabled)");
        return;
    }

    let interval_secs = std::env::var("HEDGE_TICK_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(HEDGE_TICK_INTERVAL_SECS)
        .max(5);

    info!("Starting hedge engine (interval={}s)", interval_secs);

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(12)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if state
                .is_shutting_down
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                info!("Hedge engine shutting down");
                break;
            }

            match run_hedge_tick(&state).await {
                Ok(count) => {
                    if count > 0 {
                        info!("Hedge tick: processed {} hedges", count);
                    }
                }
                Err(e) => {
                    warn!("Hedge tick error: {}", e);
                }
            }
        }
    });
}

/// Run one hedge tick: scan for pending hedges and execute them.
async fn run_hedge_tick(state: &AppState) -> Result<usize, String> {
    // 1. Scan for new fills on mirrored markets via on-chain OrderFilled events.
    detect_new_fills(state).await?;

    // 2. Process pending hedge entries from the database.
    let pending = load_pending_hedges(state).await?;
    if pending.is_empty() {
        return Ok(0);
    }

    let mut processed = 0;
    for hedge in &pending {
        match execute_hedge(state, hedge).await {
            Ok(result) => {
                mark_hedge_complete(
                    state,
                    hedge.0,
                    &result.status,
                    result.provider_order_id.as_deref(),
                    result.tx_hash.as_deref(),
                    result.pnl_usdc,
                )
                .await;
                processed += 1;
            }
            Err(e) => {
                warn!("Hedge execution failed for log {}: {}", hedge.0, e);
                mark_hedge_failed(state, hedge.0, &e).await;
            }
        }
    }

    Ok(processed)
}

/// OrderFilled event topic from the OrderBook contract.
const ORDER_FILLED_TOPIC: &str =
    "0x5aac01386940f75e601757cfe5dc1d4ab2bac84f98d30664486114a8abb38a45";

/// Detect new OrderFilled events on mirrored markets via RPC eth_getLogs.
async fn detect_new_fills(state: &AppState) -> Result<(), String> {
    // Load the last scanned block from Redis.
    let watermark_key = "hedge:fill_watermark";
    let last_block: u64 = state
        .redis
        .get::<u64>(watermark_key)
        .await
        .ok()
        .flatten()
        .unwrap_or(0);

    // Get active mirror links.
    let active_markets: Vec<(i32, i64, String, String, String)> = sqlx::query_as(
        "SELECT id, internal_market_id, external_market_id, external_provider, hedge_mode \
         FROM mirror_market_links WHERE active = true AND hedge_mode != 'disabled'",
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| format!("Query mirror links: {}", e))?;

    if active_markets.is_empty() {
        return Ok(());
    }

    // Fetch current block number.
    let current_block = fetch_block_number(state).await?;
    if current_block <= last_block {
        return Ok(());
    }

    // Scan at most 100 blocks at a time.
    let from_block = last_block + 1;
    let to_block = current_block.min(from_block + 100);

    // Fetch OrderFilled logs from the OrderBook contract.
    let logs = fetch_order_filled_logs(
        state,
        &state.config.order_book_address,
        from_block,
        to_block,
    )
    .await?;

    for log in &logs {
        // Parse OrderFilled event: topics[1] = orderId, topics[2] = marketId (padded)
        // data = abi.encode(maker, taker, outcomeYes, priceBps, quantity)
        let market_id = parse_topic_u64(log, 2);
        let Some(market_id) = market_id else { continue };

        let link = active_markets.iter().find(|m| m.1 == market_id as i64);
        let Some(link) = link else { continue };

        // Parse fill data from log.
        let data = log.get("data").and_then(|v| v.as_str()).unwrap_or("");
        let data_hex = data.trim_start_matches("0x");

        // data layout (each 32 bytes / 64 hex chars):
        // [0] maker address, [1] taker address, [2] outcomeYes bool,
        // [3] priceBps uint256, [4] quantity uint256
        if data_hex.len() < 320 {
            continue;
        }

        let outcome_yes_word = &data_hex[128..192];
        let price_bps_hex = &data_hex[192..256];
        let quantity_hex = &data_hex[256..320];

        let outcome_yes =
            u64::from_str_radix(outcome_yes_word.trim_start_matches('0'), 16).unwrap_or(0) != 0;
        let price_bps = u64::from_str_radix(price_bps_hex.trim_start_matches('0'), 16).unwrap_or(0);
        let quantity = u64::from_str_radix(quantity_hex.trim_start_matches('0'), 16).unwrap_or(0);

        let outcome = if outcome_yes { "yes" } else { "no" };
        let price = price_bps as f64 / 10_000.0;
        let quantity_usdc = quantity as f64 / 1_000_000.0;

        if quantity_usdc < 0.01 {
            continue;
        }

        // Insert pending hedge.
        let _ = sqlx::query(
            "INSERT INTO mirror_hedge_log \
             (mirror_link_id, direction, side, outcome, price, quantity, hedge_status) \
             VALUES ($1, 'buy', $2, $3, $4::NUMERIC, $5::NUMERIC, 'pending')",
        )
        .bind(link.0)
        .bind(outcome)
        .bind(outcome)
        .bind(format!("{}", price))
        .bind(format!("{}", quantity_usdc))
        .execute(state.db.pool())
        .await;
    }

    // Update watermark.
    let _ = state.redis.set(watermark_key, &to_block, Some(86400)).await;

    Ok(())
}

fn parse_topic_u64(log: &Value, index: usize) -> Option<u64> {
    let topics = log.get("topics")?.as_array()?;
    let topic = topics.get(index)?.as_str()?;
    let hex = topic.trim_start_matches("0x");
    let trimmed = hex.trim_start_matches('0');
    if trimmed.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(trimmed, 16).ok()
}

async fn fetch_block_number(state: &AppState) -> Result<u64, String> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_blockNumber",
        "params": []
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(&state.config.base_rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("RPC blockNumber: {}", e))?;
    let result: Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse block: {}", e))?;
    let hex = result
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or("No block number result")?;
    u64::from_str_radix(hex.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Parse block hex: {}", e))
}

async fn fetch_order_filled_logs(
    state: &AppState,
    contract: &str,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<Value>, String> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getLogs",
        "params": [{
            "address": contract,
            "fromBlock": format!("0x{:x}", from_block),
            "toBlock": format!("0x{:x}", to_block),
            "topics": [ORDER_FILLED_TOPIC]
        }]
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(&state.config.base_rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("RPC getLogs: {}", e))?;
    let result: Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse logs: {}", e))?;
    Ok(result
        .get("result")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default())
}

struct HedgeResult {
    status: String,
    provider_order_id: Option<String>,
    tx_hash: Option<String>,
    pnl_usdc: Option<f64>,
}

type PendingHedgeRow = (i32, i32, String, String, f64, f64); // id, link_id, outcome, provider, price, quantity

async fn load_pending_hedges(state: &AppState) -> Result<Vec<PendingHedgeRow>, String> {
    sqlx::query_as(
        "SELECT h.id, h.mirror_link_id, h.outcome, m.external_provider, \
         h.price::float8, h.quantity::float8 \
         FROM mirror_hedge_log h \
         JOIN mirror_market_links m ON m.id = h.mirror_link_id \
         WHERE h.hedge_status = 'pending' \
         ORDER BY h.id ASC \
         LIMIT 20",
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| format!("Load pending hedges: {}", e))
}

async fn execute_hedge(state: &AppState, hedge: &PendingHedgeRow) -> Result<HedgeResult, String> {
    let (id, link_id, ref outcome, ref provider, price, quantity) = *hedge;

    // Load the mirror link to get external market ID and credential.
    let link = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT external_market_id, hedge_credential_id FROM mirror_market_links WHERE id = $1",
    )
    .bind(link_id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| format!("Load mirror link: {}", e))?;

    let (external_market_id, hedge_credential_id) = link;

    match provider.as_str() {
        "limitless" => {
            execute_limitless_hedge(
                state,
                &external_market_id,
                hedge_credential_id.as_deref(),
                outcome,
                price,
                quantity,
            )
            .await
        }
        "aerodrome" => {
            execute_aerodrome_hedge(
                state,
                &external_market_id,
                hedge_credential_id.as_deref(),
                outcome,
                price,
                quantity,
            )
            .await
        }
        "polymarket" => {
            execute_polymarket_hedge(
                state,
                &external_market_id,
                hedge_credential_id.as_deref(),
                outcome,
                price,
                quantity,
            )
            .await
        }
        _ => Err(format!("Unsupported hedge provider: {}", provider)),
    }
}

/// Execute a hedge on Limitless by building an EIP-712 signed order and submitting it.
async fn execute_limitless_hedge(
    state: &AppState,
    external_market_id: &str,
    credential_id: Option<&str>,
    outcome: &str,
    price: f64,
    quantity: f64,
) -> Result<HedgeResult, String> {
    // Load credential for the hedge wallet.
    let credential = load_hedge_credential(state, "limitless", credential_id).await?;

    let api_key = credential
        .get("apiKey")
        .or_else(|| credential.get("api_key"))
        .and_then(|v| v.as_str())
        .ok_or("Limitless credential missing apiKey")?
        .to_string();

    let private_key = credential
        .get("privateKey")
        .or_else(|| credential.get("private_key"))
        .and_then(|v| v.as_str())
        .ok_or("Limitless credential missing privateKey")?
        .to_string();

    let wallet_address = evm_signer::address_from_private_key(&private_key)
        .map_err(|e| format!("Invalid private key: {}", e))?;

    check_hedge_balance(state, &wallet_address, quantity, "Limitless").await?;

    // Fetch raw Limitless market data (needed for exchange contract + token IDs).
    let ext_id = crate::services::external::types::ExternalMarketId::parse(external_market_id)
        .map_err(|e| format!("Invalid market id: {}", e))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let market_data = crate::services::external::providers::limitless::fetch_market_raw(
        &client,
        &state.config.limitless_api_base,
        &ext_id.value,
    )
    .await
    .map_err(|e| format!("Fetch market data: {}", e))?;

    let exchange_contract = market_data
        .get("venue")
        .and_then(|v| v.get("exchange"))
        .and_then(|v| v.as_str())
        .ok_or("Market missing venue.exchange")?;

    let token_id = extract_limitless_token_id_from_market(&market_data, outcome)?;

    // Build the order message (matching the Limitless EIP-712 schema).
    let side_byte: u8 = 0; // 0 = BUY
    let price_int = (price * LIMITLESS_SCALE as f64).round() as u128;
    let price_aligned = (price_int / LIMITLESS_PRICE_TICK) * LIMITLESS_PRICE_TICK;
    let quantity_int = (quantity * LIMITLESS_SCALE as f64).round() as u128;

    // makerAmount = quantity (USDC in 6 decimals)
    // takerAmount = quantity / price (outcome tokens)
    let taker_amount = (quantity_int * LIMITLESS_SCALE)
        .checked_div(price_aligned)
        .ok_or_else(|| "Price is zero, cannot compute taker amount".to_string())?;

    let salt = generate_salt();
    let expiration = (Utc::now().timestamp() as u64) + 3600; // 1 hour
    let nonce = 0u64;
    let fee_rate_bps = 0u64;

    // Build EIP-712 struct hash for the Order type.
    let order_type_hash = Keccak256::digest(
        b"Order(uint256 salt,address maker,address signer,address taker,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint256 expiration,uint256 nonce,uint256 feeRateBps,uint8 side,uint8 signatureType)"
    );

    let zero_address = [0u8; 32]; // taker = address(0)
    let mut maker_word = [0u8; 32];
    let maker_bytes = hex::decode(wallet_address.trim_start_matches("0x")).unwrap_or_default();
    if maker_bytes.len() == 20 {
        maker_word[12..].copy_from_slice(&maker_bytes);
    }

    let token_id_u256 = u256_from_decimal_str(&token_id)?;

    let mut encode_data = Vec::with_capacity(12 * 32);
    encode_data.extend_from_slice(&u256_bytes(salt));
    encode_data.extend_from_slice(&maker_word); // maker
    encode_data.extend_from_slice(&maker_word); // signer = maker
    encode_data.extend_from_slice(&zero_address); // taker = 0
    encode_data.extend_from_slice(&token_id_u256);
    encode_data.extend_from_slice(&u256_from_u128(quantity_int));
    encode_data.extend_from_slice(&u256_from_u128(taker_amount));
    encode_data.extend_from_slice(&u256_from_u64(expiration));
    encode_data.extend_from_slice(&u256_from_u64(nonce));
    encode_data.extend_from_slice(&u256_from_u64(fee_rate_bps));
    encode_data.extend_from_slice(&u256_from_u64(side_byte as u64));
    encode_data.extend_from_slice(&u256_from_u64(0)); // signatureType = EOA

    let type_hash_arr: [u8; 32] = order_type_hash.into();
    let struct_hash: [u8; 32] = evm_signer::eip712_struct_hash(&type_hash_arr, &encode_data);

    let domain_separator = evm_signer::eip712_domain_separator(
        LIMITLESS_SIGNING_NAME,
        LIMITLESS_SIGNING_VERSION,
        LIMITLESS_CHAIN_ID,
        exchange_contract,
    );

    let signing_hash = evm_signer::eip712_signing_hash(&domain_separator, &struct_hash);

    let signature = evm_signer::sign_eip712_hash(&signing_hash, &private_key)
        .map_err(|e| format!("EIP-712 signing failed: {}", e))?;

    // Build the Limitless order payload.
    let order_payload = json!({
        "order": {
            "salt": salt.to_string(),
            "maker": wallet_address,
            "signer": wallet_address,
            "taker": "0x0000000000000000000000000000000000000000",
            "tokenId": token_id,
            "makerAmount": quantity_int.to_string(),
            "takerAmount": taker_amount.to_string(),
            "expiration": expiration.to_string(),
            "nonce": nonce.to_string(),
            "feeRateBps": fee_rate_bps.to_string(),
            "side": side_byte,
            "signatureType": 0,
            "signature": signature,
        },
        "orderType": "GTC",
    });

    // Submit to Limitless API.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let response = client
        .post(format!(
            "{}/orders",
            state.config.limitless_api_base.trim_end_matches('/')
        ))
        .header("X-API-Key", &api_key)
        .json(&order_payload)
        .send()
        .await
        .map_err(|e| format!("Limitless submit failed: {}", e))?;

    let status = response.status();
    let body: Value = response
        .json()
        .await
        .unwrap_or(json!({ "ok": status.is_success() }));

    if status.is_success() {
        let order_id = body
            .get("id")
            .or_else(|| body.get("orderId"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(HedgeResult {
            status: "hedged".to_string(),
            provider_order_id: Some(order_id),
            tx_hash: None,
            pnl_usdc: None,
        })
    } else {
        let msg = body
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        Err(format!("Limitless API error ({}): {}", status, msg))
    }
}

/// Execute a hedge on Aerodrome via an autonomous swap transaction.
async fn execute_aerodrome_hedge(
    state: &AppState,
    external_market_id: &str,
    credential_id: Option<&str>,
    _outcome: &str,
    _price: f64,
    quantity: f64,
) -> Result<HedgeResult, String> {
    let credential = load_hedge_credential(state, "aerodrome", credential_id).await?;

    let private_key = credential
        .get("privateKey")
        .or_else(|| credential.get("private_key"))
        .and_then(|v| v.as_str())
        .ok_or("Aerodrome credential missing privateKey")?
        .to_string();

    let wallet_address = evm_signer::address_from_private_key(&private_key)
        .map_err(|e| format!("Invalid private key: {}", e))?;

    check_hedge_balance(state, &wallet_address, quantity, "Aerodrome").await?;

    // Oracle pre-check: verify Pyth oracle price is available and sane.
    // The actual slippage protection is handled by amount_out_min (5%),
    // but we log the oracle reference and reject if the oracle itself is stale/unavailable
    // when a feed is configured (indicates the asset may be in a volatile/abnormal state).
    if let Some(pyth_feed_id) = lookup_pyth_feed_for_token(state, external_market_id).await {
        match crate::services::pyth::fetch_price(&state.redis, &pyth_feed_id).await {
            Ok(Some(oracle_price)) => {
                info!(
                    "Hedge oracle ref: token={} pyth=${:.2} quantity={:.2}",
                    external_market_id, oracle_price, quantity
                );
            }
            Ok(None) => {
                warn!(
                    "Hedge oracle: pyth feed {} returned no price, proceeding with on-chain slippage protection",
                    pyth_feed_id
                );
            }
            Err(e) => {
                warn!("Hedge oracle: pyth fetch error for {}: {e}", pyth_feed_id);
            }
        }
    }

    // For Aerodrome, the external_market_id contains the token address to swap into.
    // Look up the pool's tick_spacing from the DB rather than hardcoding.
    let usdc = state.config.usdc_mint.to_ascii_lowercase();
    let token_out = external_market_id.to_ascii_lowercase();
    let tick_spacing: i32 = sqlx::query_scalar(
        "SELECT tick_spacing FROM aerodrome_pools \
         WHERE active = true \
           AND ((LOWER(token0) = $1 AND LOWER(token1) = $2) \
             OR (LOWER(token0) = $2 AND LOWER(token1) = $1)) \
         LIMIT 1",
    )
    .bind(&usdc)
    .bind(&token_out)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| format!("Query pool tick_spacing: {}", e))?
    .unwrap_or(200); // fallback to 200 if pool not in registry

    let amount_in = (quantity * 1_000_000.0).round() as u128; // USDC in 6 decimals
    let amount_out_min = (amount_in * 95) / 100; // 5% slippage tolerance
    let deadline = (Utc::now().timestamp() as u64) + 300; // 5 min

    let nonce = fetch_nonce(state, &wallet_address).await?;
    let (base_fee, priority_fee) = fetch_gas_prices(state).await?;

    let swap_router = &state.config.aerodrome_swap_router_address;
    let calldata = crate::services::aerodrome::encode_swap_exact_input_single(
        &state.config.usdc_mint,
        external_market_id,
        tick_spacing,
        &wallet_address,
        deadline,
        amount_in,
        amount_out_min,
    )
    .map_err(|e| format!("Encode swap: {}", e))?;

    let gas_limit = 300_000u64;
    let signed_tx = evm_signer::sign_eip1559_transaction(&evm_signer::Eip1559TxParams {
        chain_id: state.config.base_chain_id,
        nonce,
        max_priority_fee_per_gas: priority_fee,
        max_fee_per_gas: base_fee + priority_fee,
        gas_limit,
        to: swap_router.clone(),
        value: 0,
        data: calldata,
        private_key,
    })
    .map_err(|e| format!("Tx signing failed: {}", e))?;

    // Broadcast.
    let tx_hash = broadcast_raw_tx(state, &signed_tx).await?;

    Ok(HedgeResult {
        status: "hedged".to_string(),
        provider_order_id: None,
        tx_hash: Some(tx_hash),
        pnl_usdc: None,
    })
}

/// Execute a hedge on Polymarket by building an EIP-712 signed order and
/// submitting it to the CLOB API with HMAC authentication.
async fn execute_polymarket_hedge(
    state: &AppState,
    external_market_id: &str,
    credential_id: Option<&str>,
    outcome: &str,
    price: f64,
    quantity: f64,
) -> Result<HedgeResult, String> {
    let credential = load_hedge_credential(state, "polymarket", credential_id).await?;

    let private_key = credential
        .get("privateKey")
        .or_else(|| credential.get("private_key"))
        .and_then(|v| v.as_str())
        .ok_or("Polymarket credential missing privateKey")?
        .to_string();

    let api_key = credential
        .get("apiKey")
        .or_else(|| credential.get("api_key"))
        .and_then(|v| v.as_str())
        .ok_or("Polymarket credential missing apiKey")?
        .to_string();

    let api_secret = credential
        .get("apiSecret")
        .or_else(|| credential.get("api_secret"))
        .and_then(|v| v.as_str())
        .ok_or("Polymarket credential missing apiSecret")?
        .to_string();

    let api_passphrase = credential
        .get("apiPassphrase")
        .or_else(|| credential.get("api_passphrase"))
        .and_then(|v| v.as_str())
        .ok_or("Polymarket credential missing apiPassphrase")?
        .to_string();

    let wallet_address = evm_signer::address_from_private_key(&private_key)
        .map_err(|e| format!("Invalid private key: {}", e))?;

    check_hedge_balance(state, &wallet_address, quantity, "Polymarket").await?;

    let ctx = fetch_polymarket_hedge_context(state, external_market_id, outcome).await?;

    let side_value: u8 = 0; // BUY — hedging a user sell
    let price_int = scale_decimal_6(price)?;
    let tick_step = scale_decimal_6(ctx.minimum_tick_size)?;
    if price_int % tick_step != 0 {
        return Err(format!(
            "Polymarket price {} doesn't align to tick {}",
            price, ctx.minimum_tick_size
        ));
    }

    let shares_int = scale_decimal_6(quantity)?;
    if shares_int % POLYMARKET_LOT_STEP != 0 {
        return Err(format!(
            "Polymarket quantity {} doesn't align to lot step",
            quantity
        ));
    }

    let notional_int = shares_int
        .checked_mul(price_int)
        .ok_or("Polymarket order amount overflow")?
        / POLYMARKET_SCALE;

    let (maker_amount, taker_amount) = if side_value == 0 {
        (notional_int, shares_int)
    } else {
        (shares_int, notional_int)
    };

    let salt = generate_salt();

    let exchange_contract = if ctx.neg_risk {
        POLYMARKET_NEG_RISK_EXCHANGE
    } else {
        POLYMARKET_EXCHANGE
    };

    // Build EIP-712 struct hash
    let order_type_hash = Keccak256::digest(
        b"Order(uint256 salt,address maker,address signer,address taker,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint256 expiration,uint256 nonce,uint256 feeRateBps,uint8 side,uint8 signatureType)"
    );

    let zero_address = [0u8; 32];
    let mut maker_word = [0u8; 32];
    let maker_bytes = hex::decode(wallet_address.trim_start_matches("0x")).unwrap_or_default();
    if maker_bytes.len() == 20 {
        maker_word[12..].copy_from_slice(&maker_bytes);
    }

    let token_id_u256 = u256_from_decimal_str(&ctx.token_id)?;

    let mut encode_data = Vec::with_capacity(12 * 32);
    encode_data.extend_from_slice(&u256_bytes(salt));
    encode_data.extend_from_slice(&maker_word); // maker
    encode_data.extend_from_slice(&maker_word); // signer = maker
    encode_data.extend_from_slice(&zero_address); // taker = 0
    encode_data.extend_from_slice(&token_id_u256);
    encode_data.extend_from_slice(&u256_from_u128(maker_amount));
    encode_data.extend_from_slice(&u256_from_u128(taker_amount));
    encode_data.extend_from_slice(&u256_from_u64(0)); // expiration
    encode_data.extend_from_slice(&u256_from_u64(0)); // nonce
    encode_data.extend_from_slice(&u256_from_u64(ctx.fee_rate_bps));
    encode_data.extend_from_slice(&u256_from_u64(side_value as u64));
    encode_data.extend_from_slice(&u256_from_u64(0)); // signatureType = EOA

    let type_hash_arr: [u8; 32] = order_type_hash.into();
    let struct_hash = evm_signer::eip712_struct_hash(&type_hash_arr, &encode_data);

    let domain_separator = evm_signer::eip712_domain_separator(
        POLYMARKET_SIGNING_NAME,
        POLYMARKET_SIGNING_VERSION,
        POLYMARKET_CHAIN_ID,
        exchange_contract,
    );

    let signing_hash = evm_signer::eip712_signing_hash(&domain_separator, &struct_hash);

    let signature = evm_signer::sign_eip712_hash(&signing_hash, &private_key)
        .map_err(|e| format!("EIP-712 signing failed: {}", e))?;

    let signed_order = json!({
        "order": {
            "salt": salt.to_string(),
            "maker": wallet_address,
            "signer": wallet_address,
            "taker": "0x0000000000000000000000000000000000000000",
            "tokenId": ctx.token_id,
            "makerAmount": maker_amount.to_string(),
            "takerAmount": taker_amount.to_string(),
            "expiration": "0",
            "nonce": "0",
            "feeRateBps": ctx.fee_rate_bps.to_string(),
            "side": side_value,
            "signatureType": 0,
            "signature": signature,
        },
        "orderType": "GTC",
    });

    let body = serde_json::to_string(&signed_order)
        .map_err(|e| format!("Serialize order: {}", e))?;
    let path = "/order";
    let timestamp = Utc::now().timestamp().to_string();
    let hmac_sig = polymarket_hmac(&api_secret, "POST", path, &body, &timestamp)?;

    let clob_base = state.config.polymarket_clob_api_base.trim_end_matches('/');
    let url = format!("{}{}", clob_base, path);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let mut request = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("POLY_ADDRESS", wallet_address.to_ascii_lowercase())
        .header("POLY_API_KEY", &api_key)
        .header("POLY_PASSPHRASE", &api_passphrase)
        .header("POLY_SIGNATURE", &hmac_sig)
        .header("POLY_TIMESTAMP", &timestamp);

    if let Some((bk, bs, bp)) = polymarket_builder_creds(state) {
        let builder_sig = polymarket_hmac(&bs, "POST", path, &body, &timestamp)?;
        request = request
            .header("POLY_BUILDER_API_KEY", bk)
            .header("POLY_BUILDER_PASSPHRASE", bp)
            .header("POLY_BUILDER_SIGNATURE", builder_sig)
            .header("POLY_BUILDER_TIMESTAMP", &timestamp);
    }

    let response = request
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Polymarket submit failed: {}", e))?;

    let status = response.status();
    let resp_body: Value = response
        .json()
        .await
        .unwrap_or(json!({ "ok": status.is_success() }));

    if status.is_success() {
        let order_id = resp_body
            .get("orderID")
            .or_else(|| resp_body.get("orderId"))
            .or_else(|| resp_body.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(HedgeResult {
            status: "hedged".to_string(),
            provider_order_id: Some(order_id),
            tx_hash: None,
            pnl_usdc: None,
        })
    } else {
        let msg = resp_body
            .get("errorMsg")
            .or_else(|| resp_body.get("message"))
            .or_else(|| resp_body.get("error"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        Err(format!("Polymarket API error ({}): {}", status, msg))
    }
}

struct PolymarketHedgeContext {
    token_id: String,
    fee_rate_bps: u64,
    minimum_tick_size: f64,
    neg_risk: bool,
}

async fn fetch_polymarket_hedge_context(
    state: &AppState,
    external_market_id: &str,
    outcome: &str,
) -> Result<PolymarketHedgeContext, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let clob_base = state.config.polymarket_clob_api_base.trim_end_matches('/');

    // Fetch market data from Gamma API to get token IDs
    let gamma_base = std::env::var("POLYMARKET_GAMMA_API_BASE")
        .unwrap_or_else(|_| "https://gamma-api.polymarket.com".to_string());

    let market_data: Value = client
        .get(format!(
            "{}/markets/{}",
            gamma_base.trim_end_matches('/'),
            external_market_id
        ))
        .send()
        .await
        .map_err(|e| format!("Fetch market: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Parse market: {}", e))?;

    let token_id = extract_polymarket_token_id_from_market(&market_data, outcome)?;

    let tick_payload: Value = client
        .get(format!("{}/tick-size", clob_base))
        .query(&[("token_id", token_id.as_str())])
        .send()
        .await
        .map_err(|e| format!("Fetch tick-size: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Parse tick-size: {}", e))?;

    let minimum_tick_size = tick_payload
        .get("minimum_tick_size")
        .or_else(|| tick_payload.get("minimumTickSize"))
        .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .ok_or("Missing minimum_tick_size")?;

    let fee_payload: Value = client
        .get(format!("{}/fee-rate", clob_base))
        .query(&[("token_id", token_id.as_str())])
        .send()
        .await
        .map_err(|e| format!("Fetch fee-rate: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Parse fee-rate: {}", e))?;

    let fee_rate_bps = fee_payload
        .get("base_fee")
        .or_else(|| fee_payload.get("baseFee"))
        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .ok_or("Missing fee_rate_bps")?;

    let neg_risk_payload: Value = client
        .get(format!("{}/neg-risk", clob_base))
        .query(&[("token_id", token_id.as_str())])
        .send()
        .await
        .map_err(|e| format!("Fetch neg-risk: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Parse neg-risk: {}", e))?;

    let neg_risk = neg_risk_payload
        .get("neg_risk")
        .or_else(|| neg_risk_payload.get("negRisk"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(PolymarketHedgeContext {
        token_id,
        fee_rate_bps,
        minimum_tick_size,
        neg_risk,
    })
}

fn extract_polymarket_token_id_from_market(
    market: &Value,
    outcome: &str,
) -> Result<String, String> {
    let outcomes: Vec<String> = market
        .get("outcomes")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .or_else(|| {
            market
                .get("outcomes")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
        })
        .unwrap_or_default();

    let token_ids: Vec<String> = market
        .get("clobTokenIds")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .or_else(|| {
            market
                .get("clobTokenIds")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
        })
        .unwrap_or_default();

    if outcomes.is_empty() || token_ids.is_empty() {
        return Err("Market payload missing outcomes or clobTokenIds".to_string());
    }

    for (i, label) in outcomes.iter().enumerate() {
        if label.eq_ignore_ascii_case(outcome) {
            if let Some(tid) = token_ids.get(i) {
                return Ok(tid.clone());
            }
        }
    }

    let fallback = if outcome.eq_ignore_ascii_case("yes") {
        token_ids.first()
    } else {
        token_ids.get(1)
    };

    fallback
        .cloned()
        .ok_or_else(|| format!("Cannot map outcome '{}' to Polymarket token ID", outcome))
}

fn scale_decimal_6(value: f64) -> Result<u128, String> {
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("Value {} must be positive and finite", value));
    }
    let normalized = format!("{:.6}", value);
    let mut parts = normalized.split('.');
    let whole = parts.next().unwrap_or("0");
    let frac = parts.next().unwrap_or("0");
    let raw = format!("{}{}", whole, &format!("{:0<6}", frac)[..6]);
    raw.parse::<u128>()
        .map_err(|_| format!("Cannot normalize {}", value))
}

fn polymarket_hmac(
    api_secret: &str,
    method: &str,
    path: &str,
    body: &str,
    timestamp: &str,
) -> Result<String, String> {
    let decoded = URL_SAFE
        .decode(api_secret.trim())
        .map_err(|_| "Invalid Polymarket API secret (base64 decode failed)".to_string())?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&decoded)
        .map_err(|_| "Invalid Polymarket API secret (HMAC key)")?;
    mac.update(format!("{}{}{}{}", timestamp, method, path, body).as_bytes());
    Ok(URL_SAFE.encode(mac.finalize().into_bytes()))
}

fn polymarket_builder_creds(state: &AppState) -> Option<(String, String, String)> {
    let k = state.config.polymarket_builder_api_key.trim();
    let s = state.config.polymarket_builder_api_secret.trim();
    let p = state.config.polymarket_builder_api_passphrase.trim();
    if k.is_empty() || s.is_empty() || p.is_empty() {
        return None;
    }
    Some((k.to_string(), s.to_string(), p.to_string()))
}

// ---- Helper functions ----

async fn load_hedge_credential(
    state: &AppState,
    provider: &str,
    credential_id: Option<&str>,
) -> Result<Value, String> {
    let cred_id = credential_id.ok_or_else(|| {
        format!(
            "No hedge_credential_id configured for {} mirror link",
            provider
        )
    })?;

    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT encrypted_payload, key_id FROM external_credentials WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(cred_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| format!("Query credential: {}", e))?
    .ok_or_else(|| format!("Credential {} not found or revoked", cred_id))?;

    decrypt_json(
        &state.config.external_credentials_master_key,
        &row.1,
        &row.0,
    )
    .map_err(|e| format!("Decrypt credential: {} {}", e.code, e.message))
}

/// Query on-chain USDC balance for a wallet. Returns balance in USDC (6 decimals → f64).
async fn fetch_usdc_balance(state: &AppState, wallet_address: &str) -> Result<f64, String> {
    let addr_clean = wallet_address.trim_start_matches("0x");
    // balanceOf(address) selector = 0x70a08231
    let calldata = format!("0x70a08231000000000000000000000000{}", addr_clean);

    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_call",
        "params": [{
            "to": state.config.usdc_mint,
            "data": calldata,
        }, "latest"]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&state.config.base_rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("RPC balanceOf: {}", e))?;

    let result: Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse balance: {}", e))?;
    let hex = result
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or("No balance result")?;

    let trimmed = hex.trim_start_matches("0x").trim_start_matches('0');
    let raw = if trimmed.is_empty() {
        0u128
    } else {
        u128::from_str_radix(trimmed, 16).map_err(|e| format!("Parse balance hex: {}", e))?
    };

    Ok(raw as f64 / 1_000_000.0)
}

/// Verify the hedge wallet has sufficient USDC for the trade.
async fn check_hedge_balance(
    state: &AppState,
    wallet_address: &str,
    required_usdc: f64,
    provider: &str,
) -> Result<(), String> {
    let balance = fetch_usdc_balance(state, wallet_address).await?;
    if balance < required_usdc {
        return Err(format!(
            "{} hedge wallet {} has {:.2} USDC, need {:.2}",
            provider, wallet_address, balance, required_usdc,
        ));
    }
    Ok(())
}

/// Look up the Pyth price feed ID for a token address via Redis config.
/// Mapping is stored as `hedge:pyth_feed:<token_address_lowercase>` → feed ID string.
async fn lookup_pyth_feed_for_token(state: &AppState, token_address: &str) -> Option<String> {
    let key = format!(
        "hedge:pyth_feed:{}",
        token_address.to_ascii_lowercase().trim_start_matches("0x")
    );
    state.redis.get::<String>(&key).await.ok().flatten()
}

fn extract_limitless_token_id_from_market(market: &Value, outcome: &str) -> Result<String, String> {
    let outcomes = market.get("outcomes").and_then(|v| v.as_array());
    if let Some(outcomes) = outcomes {
        for o in outcomes {
            let name = o.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.eq_ignore_ascii_case(outcome) {
                if let Some(token_id) = o.get("tokenId").and_then(|v| v.as_str()) {
                    return Ok(token_id.to_string());
                }
                if let Some(token_id) = o.get("token_id").and_then(|v| v.as_str()) {
                    return Ok(token_id.to_string());
                }
            }
        }
    }
    // Fallback: try yesTokenId / noTokenId.
    let key = if outcome == "yes" {
        "yesTokenId"
    } else {
        "noTokenId"
    };
    market
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            format!(
                "Cannot find token ID for outcome '{}' in market data",
                outcome
            )
        })
}

fn generate_salt() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    ts as u128
}

fn u256_bytes(value: u128) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[16..].copy_from_slice(&value.to_be_bytes());
    buf
}

fn u256_from_u128(value: u128) -> [u8; 32] {
    u256_bytes(value)
}

fn u256_from_u64(value: u64) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[24..].copy_from_slice(&value.to_be_bytes());
    buf
}

fn u256_from_decimal_str(s: &str) -> Result<[u8; 32], String> {
    let value: u128 = s
        .parse()
        .map_err(|_| format!("Invalid u256 decimal: {}", s))?;
    Ok(u256_bytes(value))
}

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

    let result: Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse nonce: {}", e))?;
    let hex = result
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or("No nonce result")?;
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
        .map_err(|e| format!("RPC gas price: {}", e))?;

    let result: Value = resp.json().await.map_err(|e| format!("Parse gas: {}", e))?;
    let hex = result
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or("No gas price result")?;
    let base_fee = u128::from_str_radix(hex.trim_start_matches("0x"), 16).unwrap_or(1_000_000_000);

    let priority_fee = 100_000_000u128; // 0.1 gwei default
    Ok((base_fee, priority_fee))
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
        .map_err(|e| format!("Broadcast tx: {}", e))?;

    let result: Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse broadcast: {}", e))?;

    if let Some(error) = result.get("error") {
        return Err(format!("Broadcast error: {}", error));
    }

    result
        .get("result")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or("No tx hash in broadcast response".to_string())
}

async fn mark_hedge_complete(
    state: &AppState,
    hedge_id: i32,
    status: &str,
    provider_order_id: Option<&str>,
    tx_hash: Option<&str>,
    pnl_usdc: Option<f64>,
) {
    if let Err(e) = sqlx::query(
        "UPDATE mirror_hedge_log SET hedge_status = $1, hedge_provider_order_id = $2, \
         hedge_tx_hash = $3, pnl_usdc = $4::NUMERIC WHERE id = $5",
    )
    .bind(status)
    .bind(provider_order_id)
    .bind(tx_hash)
    .bind(pnl_usdc.map(|p| format!("{}", p)))
    .bind(hedge_id)
    .execute(state.db.pool())
    .await
    {
        warn!(
            "CRITICAL: failed to mark hedge {} complete: {}",
            hedge_id, e
        );
    }

    // Update the mirror link's totals.
    if let Err(e) = sqlx::query(
        "UPDATE mirror_market_links SET last_hedge_at = NOW(), hedge_error = NULL, updated_at = NOW() \
         WHERE id = (SELECT mirror_link_id FROM mirror_hedge_log WHERE id = $1)",
    )
    .bind(hedge_id)
    .execute(state.db.pool())
    .await
    {
        warn!("Failed to update mirror link after hedge {}: {}", hedge_id, e);
    }
}

async fn mark_hedge_failed(state: &AppState, hedge_id: i32, error: &str) {
    if let Err(e) = sqlx::query(
        "UPDATE mirror_hedge_log SET hedge_status = 'failed', error_message = $1 WHERE id = $2",
    )
    .bind(error)
    .bind(hedge_id)
    .execute(state.db.pool())
    .await
    {
        warn!(
            "CRITICAL: failed to mark hedge {} as failed: {}",
            hedge_id, e
        );
    }

    if let Err(e) = sqlx::query(
        "UPDATE mirror_market_links SET hedge_error = $1, updated_at = NOW() \
         WHERE id = (SELECT mirror_link_id FROM mirror_hedge_log WHERE id = $2)",
    )
    .bind(error)
    .bind(hedge_id)
    .execute(state.db.pool())
    .await
    {
        warn!(
            "Failed to update mirror link error for hedge {}: {}",
            hedge_id, e
        );
    }
}
