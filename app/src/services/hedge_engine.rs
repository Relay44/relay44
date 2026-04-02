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

use chrono::Utc;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};
use std::sync::Arc;
use std::time::Duration;

use crate::services::evm_signer;
use crate::services::external::credentials::decrypt_json;
use crate::AppState;

const HEDGE_TICK_INTERVAL_SECS: u64 = 10;
const HEDGE_REDIS_PREFIX: &str = "hedge:pending:";
const HEDGE_REDIS_TTL: u64 = 300;

// Limitless EIP-712 constants (must match api/external.rs)
const LIMITLESS_SIGNING_NAME: &str = "Limitless CTF Exchange";
const LIMITLESS_SIGNING_VERSION: &str = "1";
const LIMITLESS_CHAIN_ID: u64 = 8453;
const LIMITLESS_SCALE: u128 = 1_000_000;
const LIMITLESS_PRICE_TICK: u128 = 1_000;

/// A fill detected on an internal mirrored market that needs hedging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingHedge {
    pub mirror_link_id: i32,
    pub internal_market_id: i64,
    pub external_market_id: String,
    pub external_provider: String,
    pub direction: String,   // "buy" or "sell" — what the internal user did
    pub side: String,        // "yes" or "no"
    pub outcome: String,     // "yes" or "no"
    pub price: f64,          // 0.0 to 1.0
    pub quantity: f64,       // USDC amount
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

        let outcome_yes = u64::from_str_radix(outcome_yes_word.trim_start_matches('0'), 16)
            .unwrap_or(0)
            != 0;
        let price_bps = u64::from_str_radix(
            price_bps_hex.trim_start_matches('0'),
            16,
        )
        .unwrap_or(0);
        let quantity = u64::from_str_radix(
            quantity_hex.trim_start_matches('0'),
            16,
        )
        .unwrap_or(0);

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
    let _ = state
        .redis
        .set(watermark_key, &to_block, Some(86400))
        .await;

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
    let result: Value = resp.json().await.map_err(|e| format!("Parse block: {}", e))?;
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
    let result: Value = resp.json().await.map_err(|e| format!("Parse logs: {}", e))?;
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
    let taker_amount = if price_aligned > 0 {
        (quantity_int * LIMITLESS_SCALE) / price_aligned
    } else {
        return Err("Price is zero, cannot compute taker amount".to_string());
    };

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
    let struct_hash: [u8; 32] = evm_signer::eip712_struct_hash(
        &type_hash_arr,
        &encode_data,
    );

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
    outcome: &str,
    price: f64,
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

    // For Aerodrome, the external_market_id contains the token address to swap into.
    let amount_in = (quantity * 1_000_000.0).round() as u128; // USDC in 6 decimals
    let amount_out_min = (amount_in * 95) / 100; // 5% slippage tolerance
    let deadline = (Utc::now().timestamp() as u64) + 300; // 5 min

    let nonce = fetch_nonce(state, &wallet_address).await?;
    let (base_fee, priority_fee) = fetch_gas_prices(state).await?;

    let swap_router = &state.config.aerodrome_swap_router_address;
    let calldata = crate::services::aerodrome::encode_swap_exact_input_single(
        &state.config.usdc_mint,
        external_market_id, // token_out address
        200,                // tick_spacing (default for Aerodrome CL)
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
        .ok_or_else(|| format!("Cannot find token ID for outcome '{}' in market data", outcome))
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
    let params = json!(["latest"]);
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
    let base_fee =
        u128::from_str_radix(hex.trim_start_matches("0x"), 16).unwrap_or(1_000_000_000);

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

    let result: Value = resp.json().await.map_err(|e| format!("Parse broadcast: {}", e))?;

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
    let _ = sqlx::query(
        "UPDATE mirror_hedge_log SET hedge_status = $1, hedge_provider_order_id = $2, \
         hedge_tx_hash = $3, pnl_usdc = $4::NUMERIC WHERE id = $5",
    )
    .bind(status)
    .bind(provider_order_id)
    .bind(tx_hash)
    .bind(pnl_usdc.map(|p| format!("{}", p)))
    .bind(hedge_id)
    .execute(state.db.pool())
    .await;

    // Update the mirror link's totals.
    let _ = sqlx::query(
        "UPDATE mirror_market_links SET last_hedge_at = NOW(), hedge_error = NULL, updated_at = NOW() \
         WHERE id = (SELECT mirror_link_id FROM mirror_hedge_log WHERE id = $1)",
    )
    .bind(hedge_id)
    .execute(state.db.pool())
    .await;
}

async fn mark_hedge_failed(state: &AppState, hedge_id: i32, error: &str) {
    let _ = sqlx::query(
        "UPDATE mirror_hedge_log SET hedge_status = 'failed', error_message = $1 WHERE id = $2",
    )
    .bind(error)
    .bind(hedge_id)
    .execute(state.db.pool())
    .await;

    let _ = sqlx::query(
        "UPDATE mirror_market_links SET hedge_error = $1, updated_at = NOW() \
         WHERE id = (SELECT mirror_link_id FROM mirror_hedge_log WHERE id = $2)",
    )
    .bind(error)
    .bind(hedge_id)
    .execute(state.db.pool())
    .await;
}
