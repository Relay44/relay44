use crate::api::ApiError;

// Aerodrome Slipstream (concentrated liquidity) function selectors
const SLOT0_SELECTOR: &str = "0x3850c7bd";
const LIQUIDITY_SELECTOR: &str = "0x1a686502";
const TOKEN0_SELECTOR: &str = "0x0dfe1681";
const TOKEN1_SELECTOR: &str = "0xd21220a7";
const FEE_SELECTOR: &str = "0xddca3f43";
const TICK_SPACING_SELECTOR: &str = "0xd0c93a7c";

// ERC-20 metadata
const ERC20_SYMBOL_SELECTOR: &str = "0x95d89b41";
const ERC20_DECIMALS_SELECTOR: &str = "0x313ce567";

// QuoterV2
const QUOTE_EXACT_INPUT_SINGLE_SELECTOR: &str = "0xc6a5026a";

// SwapRouter (Slipstream exactInputSingle)
const EXACT_INPUT_SINGLE_SELECTOR: &str = "0x04e45aaf";

// NonfungiblePositionManager
const NFPM_MINT_SELECTOR: &str = "0x88316456";
const NFPM_INCREASE_LIQUIDITY_SELECTOR: &str = "0x219f5d17";
const NFPM_DECREASE_LIQUIDITY_SELECTOR: &str = "0x0c49ccbe";

fn encode_u256(value: u128) -> String {
    format!("{:064x}", value)
}

fn encode_i256(value: i64) -> String {
    if value >= 0 {
        format!("{:064x}", value as u128)
    } else {
        // Two's complement: negative value in 256-bit = all f's prefix + lower bits
        // For values that fit in i64, the upper 192 bits are all 1s
        let lower = value as u64; // reinterpret as unsigned (two's complement)
        format!("{:0>48}{:016x}", "f".repeat(48), lower)
    }
}

fn encode_address(address: &str) -> Result<String, ApiError> {
    let clean = address.trim().to_ascii_lowercase();
    if clean.len() != 42 || !clean.starts_with("0x") {
        return Err(ApiError::bad_request(
            "INVALID_ADDRESS",
            "address must be a valid 0x EVM address",
        ));
    }
    let hex_part = &clean[2..];
    if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request(
            "INVALID_ADDRESS",
            "address contains non-hex characters",
        ));
    }
    Ok(format!("{:0>64}", hex_part))
}

/// Encode `exactInputSingle` calldata for Aerodrome Slipstream SwapRouter.
///
/// Parameters match the `ExactInputSingleParams` struct:
/// (tokenIn, tokenOut, tickSpacing, recipient, deadline, amountIn, amountOutMinimum, sqrtPriceLimitX96)
pub fn encode_swap_exact_input_single(
    token_in: &str,
    token_out: &str,
    tick_spacing: i32,
    recipient: &str,
    deadline: u64,
    amount_in: u128,
    amount_out_minimum: u128,
) -> Result<String, ApiError> {
    if amount_in == 0 {
        return Err(ApiError::bad_request(
            "INVALID_SWAP_PARAMS",
            "amount_in must be greater than zero",
        ));
    }
    if amount_out_minimum == 0 {
        return Err(ApiError::bad_request(
            "INVALID_SWAP_PARAMS",
            "amount_out_minimum must be > 0 to protect against sandwich attacks",
        ));
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if deadline <= now {
        return Err(ApiError::bad_request(
            "INVALID_DEADLINE",
            "deadline must be in the future",
        ));
    }

    Ok(format!(
        "{}{}{}{}{}{}{}{}{}",
        EXACT_INPUT_SINGLE_SELECTOR,
        encode_address(token_in)?,
        encode_address(token_out)?,
        encode_i256(tick_spacing as i64),
        encode_address(recipient)?,
        encode_u256(deadline as u128),
        encode_u256(amount_in),
        encode_u256(amount_out_minimum),
        encode_u256(0), // sqrtPriceLimitX96 = 0 (no limit)
    ))
}

/// Encode `quoteExactInputSingle` calldata for Aerodrome QuoterV2.
///
/// Parameters: (tokenIn, tokenOut, amountIn, tickSpacing, sqrtPriceLimitX96)
pub fn encode_quote_exact_input_single(
    token_in: &str,
    token_out: &str,
    amount_in: u128,
    tick_spacing: i32,
) -> Result<String, ApiError> {
    if amount_in == 0 {
        return Err(ApiError::bad_request(
            "INVALID_QUOTE_PARAMS",
            "amount_in must be greater than zero",
        ));
    }
    // QuoterV2 uses a struct param: QuoteExactInputSingleParams
    // (address tokenIn, address tokenOut, uint256 amountIn, int24 tickSpacing, uint160 sqrtPriceLimitX96)
    Ok(format!(
        "{}{}{}{}{}{}",
        QUOTE_EXACT_INPUT_SINGLE_SELECTOR,
        encode_address(token_in)?,
        encode_address(token_out)?,
        encode_u256(amount_in),
        encode_i256(tick_spacing as i64),
        encode_u256(0), // sqrtPriceLimitX96 = 0
    ))
}

/// Encode NonfungiblePositionManager `mint` calldata.
///
/// MintParams struct:
/// (token0, token1, int24 tickSpacing, int24 tickLower, int24 tickUpper,
///  uint256 amount0Desired, uint256 amount1Desired,
///  uint256 amount0Min, uint256 amount1Min, address recipient, uint256 deadline)
pub fn encode_nfpm_mint(
    token0: &str,
    token1: &str,
    tick_spacing: i32,
    tick_lower: i32,
    tick_upper: i32,
    amount0_desired: u128,
    amount1_desired: u128,
    amount0_min: u128,
    amount1_min: u128,
    recipient: &str,
    deadline: u64,
) -> Result<String, ApiError> {
    if tick_lower >= tick_upper {
        return Err(ApiError::bad_request(
            "INVALID_TICK_RANGE",
            &format!(
                "tick_lower ({}) must be less than tick_upper ({})",
                tick_lower, tick_upper
            ),
        ));
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if deadline <= now {
        return Err(ApiError::bad_request(
            "INVALID_DEADLINE",
            "deadline must be in the future",
        ));
    }

    Ok(format!(
        "{}{}{}{}{}{}{}{}{}{}{}{}",
        NFPM_MINT_SELECTOR,
        encode_address(token0)?,
        encode_address(token1)?,
        encode_i256(tick_spacing as i64),
        encode_i256(tick_lower as i64),
        encode_i256(tick_upper as i64),
        encode_u256(amount0_desired),
        encode_u256(amount1_desired),
        encode_u256(amount0_min),
        encode_u256(amount1_min),
        encode_address(recipient)?,
        encode_u256(deadline as u128),
    ))
}

/// Encode NonfungiblePositionManager `increaseLiquidity` calldata.
///
/// IncreaseLiquidityParams: (uint256 tokenId, uint256 amount0Desired, uint256 amount1Desired,
///                           uint256 amount0Min, uint256 amount1Min, uint256 deadline)
pub fn encode_nfpm_increase_liquidity(
    token_id: u128,
    amount0_desired: u128,
    amount1_desired: u128,
    amount0_min: u128,
    amount1_min: u128,
    deadline: u64,
) -> String {
    format!(
        "{}{}{}{}{}{}{}",
        NFPM_INCREASE_LIQUIDITY_SELECTOR,
        encode_u256(token_id),
        encode_u256(amount0_desired),
        encode_u256(amount1_desired),
        encode_u256(amount0_min),
        encode_u256(amount1_min),
        encode_u256(deadline as u128),
    )
}

/// Encode NonfungiblePositionManager `decreaseLiquidity` calldata.
///
/// DecreaseLiquidityParams: (uint256 tokenId, uint128 liquidity,
///                           uint256 amount0Min, uint256 amount1Min, uint256 deadline)
pub fn encode_nfpm_decrease_liquidity(
    token_id: u128,
    liquidity: u128,
    amount0_min: u128,
    amount1_min: u128,
    deadline: u64,
) -> String {
    format!(
        "{}{}{}{}{}{}",
        NFPM_DECREASE_LIQUIDITY_SELECTOR,
        encode_u256(token_id),
        encode_u256(liquidity),
        encode_u256(amount0_min),
        encode_u256(amount1_min),
        encode_u256(deadline as u128),
    )
}

/// Convert a price ratio to a Uniswap V3 / Slipstream tick.
/// price = 1.0001^tick, so tick = log(price) / log(1.0001)
pub fn price_to_tick(price: f64, tick_spacing: i32) -> Result<i32, ApiError> {
    if price <= 0.0 || !price.is_finite() {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "price must be positive and finite",
        ));
    }
    if tick_spacing <= 0 {
        return Err(ApiError::bad_request(
            "INVALID_TICK_SPACING",
            "tick_spacing must be positive",
        ));
    }
    let raw_tick = (price.ln() / 1.0001_f64.ln()).round();
    if raw_tick > i32::MAX as f64 || raw_tick < i32::MIN as f64 {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "price results in out-of-range tick",
        ));
    }
    let raw_tick_i32 = raw_tick as i32;
    // Snap to tick spacing
    Ok((raw_tick_i32 / tick_spacing) * tick_spacing)
}

/// Convert a Uniswap V3 / Slipstream tick to a price.
/// price = 1.0001^tick
pub fn tick_to_price(tick: i32) -> f64 {
    1.0001_f64.powi(tick)
}

/// Read selectors used by the provider module to fetch on-chain pool state.
pub fn slot0_selector() -> &'static str {
    SLOT0_SELECTOR
}
pub fn liquidity_selector() -> &'static str {
    LIQUIDITY_SELECTOR
}
pub fn token0_selector() -> &'static str {
    TOKEN0_SELECTOR
}
pub fn token1_selector() -> &'static str {
    TOKEN1_SELECTOR
}
pub fn fee_selector() -> &'static str {
    FEE_SELECTOR
}
pub fn tick_spacing_selector() -> &'static str {
    TICK_SPACING_SELECTOR
}
pub fn erc20_symbol_selector() -> &'static str {
    ERC20_SYMBOL_SELECTOR
}
pub fn erc20_decimals_selector() -> &'static str {
    ERC20_DECIMALS_SELECTOR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_to_tick_roundtrips() {
        let price = 0.62;
        let tick = price_to_tick(price, 200).unwrap();
        let recovered = tick_to_price(tick);
        assert!((recovered - price).abs() < 0.02);
    }

    #[test]
    fn price_to_tick_rejects_invalid_inputs() {
        assert!(price_to_tick(0.0, 200).is_err());
        assert!(price_to_tick(-1.0, 200).is_err());
        assert!(price_to_tick(f64::NAN, 200).is_err());
        assert!(price_to_tick(f64::INFINITY, 200).is_err());
        assert!(price_to_tick(0.5, 0).is_err());
        assert!(price_to_tick(0.5, -1).is_err());
    }

    #[test]
    fn encode_address_pads_correctly() {
        let result = encode_address("0xBE6D8f0d05cC4be24d5167a3eF062215bE6D18a5").unwrap();
        assert_eq!(result.len(), 64);
        assert!(result.starts_with("000000000000000000000000"));
    }

    #[test]
    fn encode_address_rejects_non_hex() {
        assert!(encode_address("0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ").is_err());
    }

    #[test]
    fn encode_swap_rejects_zero_amount_out_minimum() {
        let result = encode_swap_exact_input_single(
            "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            "0x0000000000000000000000000000000000000001",
            200,
            "0x0000000000000000000000000000000000000002",
            u64::MAX, // far future deadline
            1_000_000,
            0, // zero slippage protection
        );
        assert!(result.is_err());
    }

    #[test]
    fn encode_swap_rejects_past_deadline() {
        let result = encode_swap_exact_input_single(
            "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            "0x0000000000000000000000000000000000000001",
            200,
            "0x0000000000000000000000000000000000000002",
            1000, // way in the past
            1_000_000,
            900_000,
        );
        assert!(result.is_err());
    }

    #[test]
    fn encode_swap_produces_valid_calldata() {
        let calldata = encode_swap_exact_input_single(
            "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            "0x0000000000000000000000000000000000000001",
            200,
            "0x0000000000000000000000000000000000000002",
            u64::MAX,
            1_000_000, // 1 USDC (6 decimals)
            900_000,
        )
        .unwrap();
        assert!(calldata.starts_with(EXACT_INPUT_SINGLE_SELECTOR));
    }

    #[test]
    fn encode_nfpm_mint_rejects_invalid_tick_range() {
        let result = encode_nfpm_mint(
            "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            "0x0000000000000000000000000000000000000001",
            200,
            100,  // tick_lower
            100,  // tick_upper == tick_lower
            1000,
            1000,
            0,
            0,
            "0x0000000000000000000000000000000000000002",
            u64::MAX,
        );
        assert!(result.is_err());
    }

    #[test]
    fn negative_tick_encodes_as_twos_complement() {
        let encoded = encode_i256(-100);
        assert_eq!(encoded.len(), 64);
        assert!(encoded.starts_with("fffff"));
    }
}
