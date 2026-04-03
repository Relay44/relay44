use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::api::ApiError;
use crate::services::aerodrome;
use crate::services::evm_rpc::EvmRpcService;
use crate::services::external::types::{
    clamp_probability, now_rfc3339, price_to_bps, ExternalMarketSnapshot, ExternalOrderBookLevel,
    ExternalOrderBookSnapshot, ExternalOutcome, ExternalTradeSnapshot, ExternalTradesSnapshot,
};

const SWAP_TOPIC: &str = "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67";
const DEFAULT_TRADE_LOOKBACK_BLOCKS: u64 = 50_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AerodromePoolState {
    pub pool_address: String,
    pub token0: String,
    pub token1: String,
    pub fee: u32,
    pub tick_spacing: i32,
    pub liquidity: u128,
    pub sqrt_price_x96: u128,
    pub tick: i32,
    pub token0_symbol: String,
    pub token1_symbol: String,
    pub token0_decimals: u8,
    pub token1_decimals: u8,
    pub is_slipstream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AerodromeQuote {
    pub amount_out: u128,
    pub gas_estimate: u64,
    pub price_impact_bps: i64,
}

impl AerodromePoolState {
    /// Current price of token0 in terms of token1, derived from sqrtPriceX96.
    pub fn price(&self) -> Result<f64, ApiError> {
        if self.sqrt_price_x96 == 0 {
            return Err(ApiError::bad_request(
                "UNINITIALIZED_POOL",
                "pool sqrtPriceX96 is zero (uninitialized)",
            ));
        }
        let sqrt_price = self.sqrt_price_x96 as f64 / (2.0_f64.powi(96));
        let raw_price = sqrt_price * sqrt_price;
        let decimal_adjustment =
            10.0_f64.powi(self.token0_decimals as i32 - self.token1_decimals as i32);
        let price = raw_price * decimal_adjustment;
        if !price.is_finite() || price <= 0.0 {
            return Err(ApiError::bad_request(
                "INVALID_POOL_PRICE",
                &format!(
                    "price calculation produced invalid value: {} (sqrt={}, decimals={}/{})",
                    price, self.sqrt_price_x96, self.token0_decimals, self.token1_decimals
                ),
            ));
        }
        Ok(price)
    }
}

fn parse_hex_u128(hex: &str) -> Result<u128, ApiError> {
    let clean = hex.trim().trim_start_matches("0x");
    if clean.is_empty() {
        return Err(ApiError::bad_request("INVALID_HEX", "empty hex value"));
    }
    u128::from_str_radix(clean, 16).map_err(|_| {
        ApiError::bad_request("INVALID_HEX", &format!("failed to parse hex u128: {}", hex))
    })
}

fn parse_hex_u64(hex: &str) -> Result<u64, ApiError> {
    let clean = hex.trim().trim_start_matches("0x");
    if clean.is_empty() {
        return Err(ApiError::bad_request("INVALID_HEX", "empty hex value"));
    }
    u64::from_str_radix(clean, 16).map_err(|_| {
        ApiError::bad_request("INVALID_HEX", &format!("failed to parse hex u64: {}", hex))
    })
}

fn parse_hex_i32(hex: &str) -> Result<i32, ApiError> {
    let clean = hex.trim().trim_start_matches("0x");
    if clean.is_empty() {
        return Err(ApiError::bad_request("INVALID_HEX", "empty hex value"));
    }
    // Parse as u32 first, then cast to i32 for two's complement
    let raw = u32::from_str_radix(clean, 16).map_err(|_| {
        ApiError::bad_request("INVALID_HEX", &format!("failed to parse hex i32: {}", hex))
    })?;
    Ok(raw as i32)
}

fn parse_hex_u8(hex: &str) -> Result<u8, ApiError> {
    let clean = hex.trim().trim_start_matches("0x");
    if clean.is_empty() {
        return Err(ApiError::bad_request("INVALID_HEX", "empty hex value"));
    }
    u8::from_str_radix(clean, 16).map_err(|_| {
        ApiError::bad_request("INVALID_HEX", &format!("failed to parse hex u8: {}", hex))
    })
}

fn extract_address_from_word(hex: &str) -> Result<String, ApiError> {
    let clean = hex.trim().trim_start_matches("0x");
    if clean.len() < 40 {
        return Err(ApiError::bad_request(
            "INVALID_ABI_ADDRESS",
            &format!("abi word too short for address: {} chars", clean.len()),
        ));
    }
    // Address is the last 40 chars of a 64-char word
    let addr = &clean[clean.len().saturating_sub(40)..];
    if !addr.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request(
            "INVALID_ABI_ADDRESS",
            "address contains non-hex characters",
        ));
    }
    Ok(format!("0x{}", addr))
}

fn parse_string_from_abi(hex: &str) -> Result<String, ApiError> {
    let clean = hex.trim().trim_start_matches("0x");
    if clean.len() < 128 {
        return Err(ApiError::bad_request(
            "INVALID_ABI_STRING",
            &format!("abi string response too short: {} chars", clean.len()),
        ));
    }
    // ABI string: first 32 bytes = offset, next 32 bytes = length, then data
    let len_hex = &clean[64..128];
    let len = usize::from_str_radix(len_hex, 16).map_err(|_| {
        ApiError::bad_request("INVALID_ABI_STRING", "invalid string length encoding")
    })?;
    if len == 0 {
        return Ok(String::new());
    }
    if clean.len() < 128 + len * 2 {
        return Err(ApiError::bad_request(
            "INVALID_ABI_STRING",
            &format!(
                "abi string data truncated: need {} chars, have {}",
                128 + len * 2,
                clean.len()
            ),
        ));
    }
    let data_hex = &clean[128..128 + len * 2];
    let bytes = hex::decode(data_hex).map_err(|_| {
        ApiError::bad_request("INVALID_ABI_STRING", "failed to decode hex string data")
    })?;
    String::from_utf8(bytes)
        .map_err(|_| ApiError::bad_request("INVALID_ABI_STRING", "string data is not valid utf8"))
}

/// Fetch on-chain state for an Aerodrome Slipstream (CL) pool.
pub async fn fetch_pool_state(
    rpc: &EvmRpcService,
    pool_address: &str,
) -> Result<AerodromePoolState, ApiError> {
    // Retry wrapper: 3 attempts with exponential backoff for RPC failures
    let mut last_err = None;
    for attempt in 0..3u64 {
        match fetch_pool_state_inner(rpc, pool_address).await {
            Ok(state) => return Ok(state),
            Err(err) => {
                // Don't retry validation errors (bad data), only RPC failures
                let msg = format!("{}", err);
                if msg.contains("INVALID_") || msg.contains("UNINITIALIZED_") {
                    return Err(err);
                }
                last_err = Some(err);
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_millis(200 * (attempt + 1))).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| ApiError::internal("aerodrome pool fetch failed after retries")))
}

async fn fetch_pool_state_inner(
    rpc: &EvmRpcService,
    pool_address: &str,
) -> Result<AerodromePoolState, ApiError> {
    // Parallel RPC calls for pool properties
    let (slot0_res, liquidity_res, token0_res, token1_res, tick_spacing_res) = tokio::try_join!(
        rpc.eth_call(pool_address, aerodrome::slot0_selector()),
        rpc.eth_call(pool_address, aerodrome::liquidity_selector()),
        rpc.eth_call(pool_address, aerodrome::token0_selector()),
        rpc.eth_call(pool_address, aerodrome::token1_selector()),
        rpc.eth_call(pool_address, aerodrome::tick_spacing_selector()),
    )
    .map_err(|err| ApiError::internal(&format!("aerodrome pool rpc failed: {}", err)))?;

    // Parse slot0: (sqrtPriceX96, tick, observationIndex, observationCardinality, ...)
    let slot0_clean = slot0_res.trim_start_matches("0x");
    if slot0_clean.len() < 128 {
        return Err(ApiError::bad_request(
            "INVALID_POOL_STATE",
            &format!(
                "slot0 response too short ({} chars), pool may not be a CL pool",
                slot0_clean.len()
            ),
        ));
    }
    let sqrt_price_x96 = parse_hex_u128(&slot0_clean[..64])?;
    let tick = parse_hex_i32(&slot0_clean[64..128])?;

    if sqrt_price_x96 == 0 {
        return Err(ApiError::bad_request(
            "UNINITIALIZED_POOL",
            "pool sqrtPriceX96 is zero (not initialized)",
        ));
    }

    let liquidity = parse_hex_u128(&liquidity_res)?;
    let token0 = extract_address_from_word(&token0_res)?;
    let token1 = extract_address_from_word(&token1_res)?;
    let tick_spacing = parse_hex_i32(&tick_spacing_res)?;

    // Fetch token metadata
    let (symbol0_res, decimals0_res, symbol1_res, decimals1_res) = tokio::try_join!(
        rpc.eth_call(token0.as_str(), aerodrome::erc20_symbol_selector()),
        rpc.eth_call(token0.as_str(), aerodrome::erc20_decimals_selector()),
        rpc.eth_call(token1.as_str(), aerodrome::erc20_symbol_selector()),
        rpc.eth_call(token1.as_str(), aerodrome::erc20_decimals_selector()),
    )
    .map_err(|err| ApiError::internal(&format!("aerodrome token metadata rpc failed: {}", err)))?;

    let token0_decimals = parse_hex_u8(&decimals0_res)?;
    let token1_decimals = parse_hex_u8(&decimals1_res)?;

    if token0_decimals > 18 || token1_decimals > 18 {
        return Err(ApiError::bad_request(
            "INVALID_TOKEN_DECIMALS",
            &format!(
                "token decimals out of range: token0={}, token1={}",
                token0_decimals, token1_decimals
            ),
        ));
    }

    let token0_symbol = parse_string_from_abi(&symbol0_res)?;
    let token1_symbol = parse_string_from_abi(&symbol1_res)?;

    if token0_symbol.is_empty() || token1_symbol.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_TOKEN_METADATA",
            "token symbol is empty — pool may reference non-standard tokens",
        ));
    }

    Ok(AerodromePoolState {
        pool_address: pool_address.to_string(),
        token0,
        token1,
        fee: 0, // Slipstream pools use tick_spacing instead of fee tiers
        tick_spacing,
        liquidity,
        sqrt_price_x96,
        tick,
        token0_symbol,
        token1_symbol,
        token0_decimals,
        token1_decimals,
        is_slipstream: true,
    })
}

/// Quote a swap via Aerodrome QuoterV2.
pub async fn quote_swap(
    rpc: &EvmRpcService,
    quoter_address: &str,
    token_in: &str,
    token_out: &str,
    amount_in: u128,
    tick_spacing: i32,
) -> Result<AerodromeQuote, ApiError> {
    if amount_in == 0 {
        return Err(ApiError::bad_request(
            "INVALID_QUOTE_PARAMS",
            "amount_in must be greater than zero",
        ));
    }

    let calldata =
        aerodrome::encode_quote_exact_input_single(token_in, token_out, amount_in, tick_spacing)?;

    // Retry wrapper for RPC failures
    let mut last_err = None;
    for attempt in 0..3u64 {
        match rpc.eth_call(quoter_address, &calldata).await {
            Ok(result) => {
                let clean = result.trim_start_matches("0x");
                if clean.len() < 64 {
                    return Err(ApiError::bad_request(
                        "INVALID_QUOTE_RESPONSE",
                        &format!(
                            "quoter response too short ({} chars), swap may not be possible",
                            clean.len()
                        ),
                    ));
                }
                // QuoterV2 returns: (amountOut, sqrtPriceX96After, initializedTicksCrossed, gasEstimate)
                let amount_out = parse_hex_u128(&clean[..64])?;
                let gas_estimate = if clean.len() >= 256 {
                    parse_hex_u64(&clean[192..256])?
                } else {
                    0
                };

                // Price impact: compare quoted output against theoretical output at current price
                let theoretical = amount_in; // 1:1 for stableswap-like, will be adjusted by caller
                let impact_bps = if theoretical > 0 && amount_out > 0 {
                    let ratio = amount_out as f64 / theoretical as f64;
                    ((1.0 - ratio) * 10_000.0).round() as i64
                } else {
                    0
                };

                return Ok(AerodromeQuote {
                    amount_out,
                    gas_estimate,
                    price_impact_bps: impact_bps,
                });
            }
            Err(err) => {
                last_err = Some(ApiError::internal(&format!(
                    "aerodrome quoter call failed: {}",
                    err
                )));
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_millis(200 * (attempt + 1))).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| ApiError::internal("aerodrome quote failed after retries")))
}

/// Convert an Aerodrome pool state into an `ExternalMarketSnapshot`.
///
/// Maps the token pair to a yes/no prediction market model where:
/// - token0 = outcome token (YES)
/// - token1 = collateral (USDC)
/// - price = probability of YES outcome
///
/// Returns `None` if the pool state produces an invalid price.
pub fn pool_to_market_snapshot(
    pool: &AerodromePoolState,
    market_id: &str,
) -> Option<ExternalMarketSnapshot> {
    let price = match pool.price() {
        Ok(p) => p.clamp(0.01, 0.99),
        Err(_) => return None,
    };
    let question = format!("{}/{} on Aerodrome", pool.token0_symbol, pool.token1_symbol);

    Some(ExternalMarketSnapshot {
        id: format!("aerodrome:{}", market_id),
        question,
        description: format!(
            "Aerodrome Slipstream pool {} ({}/{})",
            pool.pool_address, pool.token0_symbol, pool.token1_symbol
        ),
        category: "defi".to_string(),
        status: if pool.liquidity > 0 {
            "active".to_string()
        } else {
            "inactive".to_string()
        },
        close_time: 0,
        resolved: false,
        outcome: None,
        yes_price: clamp_probability(price),
        no_price: clamp_probability(1.0 - price),
        volume: 0.0, // Not available from on-chain state alone
        source: "external_aerodrome".to_string(),
        provider: "aerodrome".to_string(),
        is_external: true,
        external_url: format!("https://aerodrome.finance/pools/{}", pool.pool_address),
        chain_id: 8453,
        requires_credentials: false,
        execution_users: true,
        execution_agents: true,
        outcomes: vec![
            ExternalOutcome {
                label: "Yes".to_string(),
                probability: clamp_probability(price),
            },
            ExternalOutcome {
                label: "No".to_string(),
                probability: clamp_probability(1.0 - price),
            },
        ],
        provider_market_ref: pool.pool_address.clone(),
    })
}

/// Synthesize an orderbook from the AMM pool state by sampling price impact
/// at multiple quantity levels.
///
/// This creates synthetic bid/ask levels that approximate the AMM's available depth.
pub fn synthesize_orderbook(
    pool: &AerodromePoolState,
    market_id: &str,
    mid_price: f64,
) -> ExternalOrderBookSnapshot {
    let mid = clamp_probability(mid_price);

    // Synthesize bid/ask levels at different depth levels
    // The AMM provides continuous liquidity, so we discretize into levels
    let spread_bps_per_level: &[f64] = &[5.0, 15.0, 30.0, 50.0, 100.0, 200.0];
    let base_quantity = if pool.liquidity > 0 {
        // Approximate quantity at each level based on pool liquidity
        let liq_approx = (pool.liquidity as f64 / 1e18).max(1.0);
        (liq_approx * 0.01).max(1.0).min(1000.0)
    } else {
        0.0
    };

    let mut bids = Vec::new();
    let mut asks = Vec::new();

    for (i, &spread) in spread_bps_per_level.iter().enumerate() {
        let quantity = base_quantity * (1.0 + i as f64 * 0.5);
        let bid_price = clamp_probability(mid * (1.0 - spread / 10_000.0));
        let ask_price = clamp_probability(mid * (1.0 + spread / 10_000.0));

        if bid_price > 0.0 && quantity > 0.0 {
            bids.push(ExternalOrderBookLevel {
                price: bid_price,
                quantity,
                orders: 1,
            });
        }
        if ask_price < 1.0 && quantity > 0.0 {
            asks.push(ExternalOrderBookLevel {
                price: ask_price,
                quantity,
                orders: 1,
            });
        }
    }

    ExternalOrderBookSnapshot {
        market_id: format!("aerodrome:{}", market_id),
        outcome: "yes".to_string(),
        bids,
        asks,
        last_updated: now_rfc3339(),
        source: "external_aerodrome".to_string(),
        provider: "aerodrome".to_string(),
        chain_id: 8453,
        provider_market_ref: pool.pool_address.clone(),
        is_synthetic: true,
    }
}

fn parse_hex_i128(word: &str) -> Result<i128, ApiError> {
    let clean = word.trim().trim_start_matches("0x");
    if clean.len() != 64 || !clean.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request(
            "INVALID_HEX",
            "invalid int256 word in swap log",
        ));
    }

    let bytes = hex::decode(clean)
        .map_err(|_| ApiError::bad_request("INVALID_HEX", "failed to decode swap log integer"))?;
    let sign_extension = if bytes[0] & 0x80 == 0 { 0x00 } else { 0xff };
    if bytes[..16].iter().any(|byte| *byte != sign_extension) {
        return Err(ApiError::bad_request(
            "INVALID_HEX",
            "swap log integer exceeds supported range",
        ));
    }

    let tail: [u8; 16] = bytes[16..32]
        .try_into()
        .map_err(|_| ApiError::bad_request("INVALID_HEX", "invalid swap log integer width"))?;
    Ok(i128::from_be_bytes(tail))
}

fn word_at(data: &str, index: usize) -> Result<&str, ApiError> {
    let clean = data.trim().trim_start_matches("0x");
    let start = index * 64;
    let end = start + 64;
    clean
        .get(start..end)
        .ok_or_else(|| ApiError::bad_request("INVALID_SWAP_LOG", "swap log payload is truncated"))
}

pub async fn fetch_trades(
    rpc: &EvmRpcService,
    pool_address: &str,
    outcome_filter: Option<&str>,
    limit: u64,
    offset: u64,
) -> Result<ExternalTradesSnapshot, ApiError> {
    let pool = fetch_pool_state(rpc, pool_address).await?;
    let latest_block = rpc
        .eth_block_number()
        .await
        .map_err(|err| ApiError::internal(&format!("aerodrome latest block failed: {}", err)))?;
    let from_block = latest_block.saturating_sub(DEFAULT_TRADE_LOOKBACK_BLOCKS);
    let logs = rpc
        .eth_get_logs(pool_address, SWAP_TOPIC, from_block, latest_block)
        .await
        .map_err(|err| ApiError::internal(&format!("aerodrome swap log fetch failed: {}", err)))?;

    let mut block_timestamps = HashMap::<u64, String>::new();
    let mut trades = Vec::new();
    for (index, log) in logs.iter().enumerate() {
        let amount0 = parse_hex_i128(word_at(log.data.as_str(), 0)?)?;
        let amount1 = parse_hex_i128(word_at(log.data.as_str(), 1)?)?;
        let sqrt_price_x96 = parse_hex_u128(word_at(log.data.as_str(), 2)?)?;
        if amount0 == 0 || amount1 == 0 {
            continue;
        }

        let outcome = if amount0 < 0 && amount1 > 0 {
            "yes"
        } else if amount0 > 0 && amount1 < 0 {
            "no"
        } else {
            continue;
        };
        if outcome_filter.is_some_and(|filter| filter != outcome) {
            continue;
        }

        let block_number = log
            .block_number
            .as_deref()
            .map(parse_hex_u64)
            .transpose()?
            .unwrap_or_default();
        let created_at = if let Some(timestamp) = block_timestamps.get(&block_number) {
            timestamp.clone()
        } else {
            let timestamp = rpc
                .eth_get_block_timestamp(block_number)
                .await
                .map_err(|err| {
                    ApiError::internal(&format!("aerodrome block timestamp fetch failed: {}", err))
                })?;
            let rendered = chrono::DateTime::from_timestamp(timestamp as i64, 0)
                .map(|value| value.to_rfc3339())
                .unwrap_or_else(now_rfc3339);
            block_timestamps.insert(block_number, rendered.clone());
            rendered
        };

        let quantity = ((amount0.unsigned_abs() as f64) / 10_f64.powi(pool.token0_decimals as i32))
            .round()
            .max(1.0) as u64;
        let mut trade_pool = pool.clone();
        trade_pool.sqrt_price_x96 = sqrt_price_x96;
        let price = clamp_probability(trade_pool.price()?);
        let tx_hash = log.transaction_hash.clone().unwrap_or_default();
        let log_index = log.log_index.clone().unwrap_or_else(|| index.to_string());
        trades.push(ExternalTradeSnapshot {
            id: format!("aerodrome:{}:{}", tx_hash, log_index),
            market_id: format!("aerodrome:{}", pool_address),
            outcome: outcome.to_string(),
            price,
            price_bps: price_to_bps(price),
            quantity,
            tx_hash,
            block_number,
            created_at,
        });
    }

    trades.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.block_number.cmp(&left.block_number))
    });

    let total = trades.len() as u64;
    let start = (offset as usize).min(trades.len());
    let end = (start + limit as usize).min(trades.len());
    let page = trades[start..end].to_vec();

    Ok(ExternalTradesSnapshot {
        trades: page,
        total,
        limit,
        offset,
        has_more: end < total as usize,
        source: "external_aerodrome".to_string(),
        provider: "aerodrome".to_string(),
        chain_id: 8453,
        provider_market_ref: pool_address.to_string(),
        is_synthetic: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pool() -> AerodromePoolState {
        AerodromePoolState {
            pool_address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            token0: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            token1: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            fee: 0,
            tick_spacing: 200,
            liquidity: 1_000_000_000_000_000_000, // 1e18
            sqrt_price_x96: 62_613_823_051_772_040_000_000_000_000, // ~0.625 price
            tick: -4700,
            token0_symbol: "YES".to_string(),
            token1_symbol: "USDC".to_string(),
            token0_decimals: 18,
            token1_decimals: 6,
            is_slipstream: true,
        }
    }

    #[test]
    fn pool_to_market_snapshot_sets_provider() {
        let pool = sample_pool();
        let snapshot = pool_to_market_snapshot(&pool, "test-pool").unwrap();
        assert_eq!(snapshot.provider, "aerodrome");
        assert_eq!(snapshot.chain_id, 8453);
        assert!(snapshot.is_external);
        assert!(snapshot.execution_agents);
    }

    #[test]
    fn pool_to_market_snapshot_returns_none_for_zero_sqrt_price() {
        let mut pool = sample_pool();
        pool.sqrt_price_x96 = 0;
        assert!(pool_to_market_snapshot(&pool, "test").is_none());
    }

    #[test]
    fn synthesize_orderbook_produces_levels() {
        let pool = sample_pool();
        let orderbook = synthesize_orderbook(&pool, "test-pool", 0.62);
        assert!(!orderbook.bids.is_empty());
        assert!(!orderbook.asks.is_empty());
        assert!(orderbook.is_synthetic);
        for bid in &orderbook.bids {
            assert!(bid.price < 0.62);
        }
        for ask in &orderbook.asks {
            assert!(ask.price > 0.62);
        }
    }

    #[test]
    fn parse_hex_handles_valid_values() {
        assert_eq!(parse_hex_u128("0x0").unwrap(), 0);
        assert_eq!(parse_hex_u128("0x1").unwrap(), 1);
        assert_eq!(parse_hex_u128("0xff").unwrap(), 255);
        assert_eq!(parse_hex_i32("0xffffff9c").unwrap(), -100);
    }

    #[test]
    fn parse_hex_rejects_invalid_input() {
        assert!(parse_hex_u128("").is_err());
        assert!(parse_hex_u128("0x").is_err());
        assert!(parse_hex_u128("0xZZZZ").is_err());
        assert!(parse_hex_u64("not_hex").is_err());
        assert!(parse_hex_i32("").is_err());
        assert!(parse_hex_u8("0xGG").is_err());
    }

    #[test]
    fn extract_address_from_word_works() {
        let word = "0x000000000000000000000000833589fcd6edb6e08f4c7c32d4f71b54bda02913";
        let addr = extract_address_from_word(word).unwrap();
        assert_eq!(addr, "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913");
    }

    #[test]
    fn extract_address_rejects_short_input() {
        assert!(extract_address_from_word("0x1234").is_err());
    }

    #[test]
    fn price_returns_error_for_zero_sqrt_price() {
        let mut pool = sample_pool();
        pool.sqrt_price_x96 = 0;
        assert!(pool.price().is_err());
    }

    #[test]
    fn price_returns_valid_value_for_sample_pool() {
        let pool = sample_pool();
        let price = pool.price().unwrap();
        assert!(price > 0.0 && price.is_finite());
    }

    #[test]
    fn parse_string_from_abi_rejects_truncated() {
        assert!(parse_string_from_abi("0x1234").is_err());
    }
}
