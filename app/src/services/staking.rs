use crate::services::evm_rpc::EvmRpcService;

// keccak256("getTier(address)")[:4] = 0xb45aae52
const GET_TIER_SELECTOR: &str = "0xb45aae52";

pub async fn get_staking_tier(
    rpc: &EvmRpcService,
    staking_address: &str,
    user_address: &str,
) -> Result<u64, String> {
    if staking_address.is_empty() {
        return Ok(0);
    }

    let addr = user_address.trim_start_matches("0x").to_ascii_lowercase();
    if addr.len() != 40 {
        return Ok(0);
    }

    let calldata = format!("{}{:0>64}", GET_TIER_SELECTOR, addr);
    let result = rpc
        .eth_call(staking_address, &calldata)
        .await
        .map_err(|e| format!("staking tier lookup failed: {e}"))?;

    let trimmed = result.trim_start_matches("0x");
    if trimmed.len() < 64 {
        return Ok(0);
    }

    let normalized = trimmed[..64].trim_start_matches('0');
    if normalized.is_empty() {
        return Ok(0);
    }

    u64::from_str_radix(normalized, 16).map_err(|_| "invalid tier response".to_string())
}

/// Tier-based discount: 0=none, 1=25%, 2=50%, 3+=75%
pub fn tier_discount_bps(tier: u64) -> u64 {
    match tier {
        0 => 0,
        1 => 2_500,
        2 => 5_000,
        _ => 7_500,
    }
}

/// Apply tier discount to a price. Returns 0 for tier 2+ (free access).
pub fn discounted_amount(base_amount: u64, tier: u64) -> u64 {
    if tier >= 2 {
        return 0;
    }
    let discount = tier_discount_bps(tier);
    if discount == 0 {
        return base_amount;
    }
    base_amount - (base_amount * discount) / 10_000
}
