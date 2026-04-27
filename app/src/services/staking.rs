use crate::services::evm_rpc::EvmRpcService;

// keccak256("getTier(address)")[:4] = 0xb45aae52
const GET_TIER_SELECTOR: &str = "0xb45aae52";
// keccak256("totalStaked()")[:4] = 0x817b1cd2
const TOTAL_STAKED_SELECTOR: &str = "0x817b1cd2";

pub const RELAY_DECIMALS: u8 = 18;
pub const X402_BYPASS_TIER: u64 = 2;

/// Tier metadata mirroring `evm/src/RelayStaking.sol::getTier` and
/// `evm/src/OrderBook.sol::TIERn_THRESHOLD`. Order is significant: index = tier id.
pub const TIERS: &[StakingTier] = &[
    StakingTier {
        tier: 0,
        name: "Bronze",
        min_relay_wei: "0",
        fee_discount_bps: 0,
        x402_bypass: false,
    },
    StakingTier {
        tier: 1,
        name: "Silver",
        min_relay_wei: "1000000000000000000000",
        fee_discount_bps: 2_500,
        x402_bypass: false,
    },
    StakingTier {
        tier: 2,
        name: "Gold",
        min_relay_wei: "10000000000000000000000",
        fee_discount_bps: 5_000,
        x402_bypass: true,
    },
    StakingTier {
        tier: 3,
        name: "Diamond",
        min_relay_wei: "100000000000000000000000",
        fee_discount_bps: 7_500,
        x402_bypass: true,
    },
];

#[derive(Debug, Clone, Copy)]
pub struct StakingTier {
    pub tier: u64,
    pub name: &'static str,
    pub min_relay_wei: &'static str,
    pub fee_discount_bps: u64,
    pub x402_bypass: bool,
}

pub async fn get_total_staked_hex(
    rpc: &EvmRpcService,
    staking_address: &str,
) -> Result<String, String> {
    if staking_address.is_empty() {
        return Err("staking address not configured".to_string());
    }
    rpc.eth_call(staking_address, TOTAL_STAKED_SELECTOR)
        .await
        .map_err(|e| format!("totalStaked call failed: {e}"))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_metadata_matches_solidity_thresholds() {
        // Mirror evm/src/OrderBook.sol TIER1/2/3_THRESHOLD constants.
        assert_eq!(TIERS.len(), 4);
        assert_eq!(TIERS[0].tier, 0);
        assert_eq!(TIERS[0].min_relay_wei, "0");
        assert_eq!(TIERS[0].fee_discount_bps, 0);
        assert!(!TIERS[0].x402_bypass);

        assert_eq!(TIERS[1].min_relay_wei, "1000000000000000000000"); // 1_000e18
        assert_eq!(TIERS[1].fee_discount_bps, 2_500);
        assert!(!TIERS[1].x402_bypass);

        assert_eq!(TIERS[2].min_relay_wei, "10000000000000000000000"); // 10_000e18
        assert_eq!(TIERS[2].fee_discount_bps, 5_000);
        assert!(TIERS[2].x402_bypass);

        assert_eq!(TIERS[3].min_relay_wei, "100000000000000000000000"); // 100_000e18
        assert_eq!(TIERS[3].fee_discount_bps, 7_500);
        assert!(TIERS[3].x402_bypass);
    }

    #[test]
    fn tier_discount_bps_matches_orderbook() {
        // Mirror evm/src/OrderBook.sol::getDiscountBps tier branches.
        assert_eq!(tier_discount_bps(0), 0);
        assert_eq!(tier_discount_bps(1), 2_500);
        assert_eq!(tier_discount_bps(2), 5_000);
        assert_eq!(tier_discount_bps(3), 7_500);
        assert_eq!(tier_discount_bps(99), 7_500);
    }

    #[test]
    fn discounted_amount_applies_tier_correctly() {
        let base = 10_000_u64;
        assert_eq!(discounted_amount(base, 0), 10_000);
        assert_eq!(discounted_amount(base, 1), 7_500);
        assert_eq!(discounted_amount(base, 2), 0);
        assert_eq!(discounted_amount(base, 3), 0);
        assert_eq!(discounted_amount(base, 99), 0);
    }

    #[test]
    fn x402_bypass_threshold_matches_constant() {
        for tier in &[0_u64, 1] {
            assert!(*tier < X402_BYPASS_TIER);
            assert!(!TIERS[*tier as usize].x402_bypass);
        }
        for tier in &[2_u64, 3] {
            assert!(*tier >= X402_BYPASS_TIER);
            assert!(TIERS[*tier as usize].x402_bypass);
        }
    }
}
