use serde::{Deserialize, Serialize};

use crate::api::ApiError;
use crate::services::aerodrome;
use crate::services::evm_rpc::EvmRpcService;
use crate::services::external::types::{
    clamp_probability, now_rfc3339, ExternalMarketSnapshot, ExternalOrderBookLevel,
    ExternalOrderBookSnapshot, ExternalOutcome,
};

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
    pub fn price(&self) -> f64 {
        if self.sqrt_price_x96 == 0 {
            return 0.0;
        }
        let sqrt_price = self.sqrt_price_x96 as f64 / (2.0_f64.powi(96));
        let raw_price = sqrt_price * sqrt_price;
        let decimal_adjustment =
            10.0_f64.powi(self.token0_decimals as i32 - self.token1_decimals as i32);
        raw_price * decimal_adjustment
    }
}

fn parse_hex_u128(hex: &str) -> u128 {
    let clean = hex.trim().trim_start_matches("0x");
    u128::from_str_radix(clean, 16).unwrap_or(0)
}

fn parse_hex_u64(hex: &str) -> u64 {
    let clean = hex.trim().trim_start_matches("0x");
    u64::from_str_radix(clean, 16).unwrap_or(0)
}

fn parse_hex_i32(hex: &str) -> i32 {
    let clean = hex.trim().trim_start_matches("0x");
    // Parse as u32 first, then cast to i32 for two's complement
    let raw = u32::from_str_radix(clean, 16).unwrap_or(0);
    raw as i32
}

fn parse_hex_u8(hex: &str) -> u8 {
    let clean = hex.trim().trim_start_matches("0x");
    u8::from_str_radix(clean, 16).unwrap_or(0)
}

fn extract_address_from_word(hex: &str) -> String {
    let clean = hex.trim().trim_start_matches("0x");
    if clean.len() < 40 {
        return format!("0x{}", clean);
    }
    // Address is the last 40 chars of a 64-char word
    let addr = &clean[clean.len().saturating_sub(40)..];
    format!("0x{}", addr)
}

fn parse_string_from_abi(hex: &str) -> String {
    let clean = hex.trim().trim_start_matches("0x");
    if clean.len() < 128 {
        return String::new();
    }
    // ABI string: first 32 bytes = offset, next 32 bytes = length, then data
    // For a simple return value, offset points to where length starts
    let len_hex = &clean[64..128];
    let len = usize::from_str_radix(len_hex, 16).unwrap_or(0);
    if len == 0 || clean.len() < 128 + len * 2 {
        return String::new();
    }
    let data_hex = &clean[128..128 + len * 2];
    hex::decode(data_hex)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default()
}

/// Fetch on-chain state for an Aerodrome Slipstream (CL) pool.
pub async fn fetch_pool_state(
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
    let sqrt_price_x96 = if slot0_clean.len() >= 64 {
        parse_hex_u128(&slot0_clean[..64])
    } else {
        0
    };
    let tick = if slot0_clean.len() >= 128 {
        parse_hex_i32(&slot0_clean[64..128])
    } else {
        0
    };

    let liquidity = parse_hex_u128(&liquidity_res);
    let token0 = extract_address_from_word(&token0_res);
    let token1 = extract_address_from_word(&token1_res);
    let tick_spacing = parse_hex_i32(&tick_spacing_res);

    // Fetch token metadata
    let (symbol0_res, decimals0_res, symbol1_res, decimals1_res) = tokio::try_join!(
        rpc.eth_call(token0.as_str(), aerodrome::erc20_symbol_selector()),
        rpc.eth_call(token0.as_str(), aerodrome::erc20_decimals_selector()),
        rpc.eth_call(token1.as_str(), aerodrome::erc20_symbol_selector()),
        rpc.eth_call(token1.as_str(), aerodrome::erc20_decimals_selector()),
    )
    .map_err(|err| ApiError::internal(&format!("aerodrome token metadata rpc failed: {}", err)))?;

    Ok(AerodromePoolState {
        pool_address: pool_address.to_string(),
        token0,
        token1,
        fee: 0, // Slipstream pools use tick_spacing instead of fee tiers
        tick_spacing,
        liquidity,
        sqrt_price_x96,
        tick,
        token0_symbol: parse_string_from_abi(&symbol0_res),
        token1_symbol: parse_string_from_abi(&symbol1_res),
        token0_decimals: parse_hex_u8(&decimals0_res),
        token1_decimals: parse_hex_u8(&decimals1_res),
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
    let calldata =
        aerodrome::encode_quote_exact_input_single(token_in, token_out, amount_in, tick_spacing)?;

    let result = rpc
        .eth_call(quoter_address, &calldata)
        .await
        .map_err(|err| ApiError::internal(&format!("aerodrome quoter call failed: {}", err)))?;

    let clean = result.trim_start_matches("0x");
    // QuoterV2 returns: (amountOut, sqrtPriceX96After, initializedTicksCrossed, gasEstimate)
    let amount_out = if clean.len() >= 64 {
        parse_hex_u128(&clean[..64])
    } else {
        0
    };
    let gas_estimate = if clean.len() >= 256 {
        parse_hex_u64(&clean[192..256])
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

    Ok(AerodromeQuote {
        amount_out,
        gas_estimate,
        price_impact_bps: impact_bps,
    })
}

/// Convert an Aerodrome pool state into an `ExternalMarketSnapshot`.
///
/// Maps the token pair to a yes/no prediction market model where:
/// - token0 = outcome token (YES)
/// - token1 = collateral (USDC)
/// - price = probability of YES outcome
pub fn pool_to_market_snapshot(
    pool: &AerodromePoolState,
    market_id: &str,
) -> ExternalMarketSnapshot {
    let price = pool.price().clamp(0.01, 0.99);
    let question = format!(
        "{}/{} on Aerodrome",
        pool.token0_symbol, pool.token1_symbol
    );

    ExternalMarketSnapshot {
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
        external_url: format!(
            "https://aerodrome.finance/pools/{}",
            pool.pool_address
        ),
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
    }
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
        let snapshot = pool_to_market_snapshot(&pool, "test-pool");
        assert_eq!(snapshot.provider, "aerodrome");
        assert_eq!(snapshot.chain_id, 8453);
        assert!(snapshot.is_external);
        assert!(snapshot.execution_agents);
    }

    #[test]
    fn synthesize_orderbook_produces_levels() {
        let pool = sample_pool();
        let orderbook = synthesize_orderbook(&pool, "test-pool", 0.62);
        assert!(!orderbook.bids.is_empty());
        assert!(!orderbook.asks.is_empty());
        assert!(orderbook.is_synthetic);
        // Bids should be below mid price
        for bid in &orderbook.bids {
            assert!(bid.price < 0.62);
        }
        // Asks should be above mid price
        for ask in &orderbook.asks {
            assert!(ask.price > 0.62);
        }
    }

    #[test]
    fn parse_hex_handles_edge_cases() {
        assert_eq!(parse_hex_u128("0x0"), 0);
        assert_eq!(parse_hex_u128("0x1"), 1);
        assert_eq!(parse_hex_u128("0xff"), 255);
        assert_eq!(parse_hex_i32("0xffffff9c"), -100); // -100 in two's complement
    }

    #[test]
    fn extract_address_from_word_works() {
        let word = "0x000000000000000000000000833589fcd6edb6e08f4c7c32d4f71b54bda02913";
        let addr = extract_address_from_word(word);
        assert_eq!(addr, "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913");
    }
}
