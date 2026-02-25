//! Settlement Arithmetic Tests
//!
//! Tests for claim winnings, redemption, and refund calculations.

/// Fee calculation: amount * fee_bps / 10_000
fn calculate_fee(amount: u64, fee_bps: u16) -> Option<u64> {
    (amount as u128)
        .checked_mul(fee_bps as u128)?
        .checked_div(10_000)
        .map(|v| v as u64)
}

/// Net payout after fee
fn calculate_payout(amount: u64, fee_bps: u16) -> Option<u64> {
    let fee = calculate_fee(amount, fee_bps)?;
    amount.checked_sub(fee)
}

/// Cancelled market refund calculation
/// Paired tokens: 1:1 collateral
/// Unpaired tokens: 0.5 collateral each
fn calculate_refund(yes_amount: u64, no_amount: u64) -> Option<u64> {
    let paired = yes_amount.min(no_amount);
    let unpaired_yes = yes_amount.saturating_sub(paired);
    let unpaired_no = no_amount.saturating_sub(paired);
    let unpaired_total = unpaired_yes.checked_add(unpaired_no)?;
    let unpaired_refund = unpaired_total / 2;
    paired.checked_add(unpaired_refund)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Fee Calculation Tests ---

    #[test]
    fn test_fee_zero_amount() {
        assert_eq!(calculate_fee(0, 100), Some(0));
    }

    #[test]
    fn test_fee_zero_bps() {
        assert_eq!(calculate_fee(1_000_000, 0), Some(0));
    }

    #[test]
    fn test_fee_standard_1_percent() {
        // 1% = 100 bps
        assert_eq!(calculate_fee(10_000, 100), Some(100));
        assert_eq!(calculate_fee(1_000_000, 100), Some(10_000));
    }

    #[test]
    fn test_fee_2_percent() {
        // 2% = 200 bps
        assert_eq!(calculate_fee(10_000, 200), Some(200));
    }

    #[test]
    fn test_fee_half_percent() {
        // 0.5% = 50 bps
        assert_eq!(calculate_fee(10_000, 50), Some(50));
    }

    #[test]
    fn test_fee_100_percent() {
        // 100% = 10000 bps
        assert_eq!(calculate_fee(10_000, 10000), Some(10_000));
    }

    #[test]
    fn test_fee_rounding_down() {
        // 1% of 99 = 0.99 -> rounds to 0
        assert_eq!(calculate_fee(99, 100), Some(0));
        // 1% of 100 = 1
        assert_eq!(calculate_fee(100, 100), Some(1));
        // 1% of 150 = 1.5 -> rounds to 1
        assert_eq!(calculate_fee(150, 100), Some(1));
    }

    #[test]
    fn test_fee_large_amount() {
        // u64::MAX with 1% fee
        let fee = calculate_fee(u64::MAX, 100);
        assert!(fee.is_some());
        // Should be approximately 1% of u64::MAX
        let expected = (u64::MAX as u128 * 100 / 10_000) as u64;
        assert_eq!(fee.unwrap(), expected);
    }

    #[test]
    fn test_fee_basis_point_precision() {
        // 1 basis point = 0.01%
        // 1 bps of 1_000_000 = 100
        assert_eq!(calculate_fee(1_000_000, 1), Some(100));
        // 1 bps of 10_000 = 1
        assert_eq!(calculate_fee(10_000, 1), Some(1));
        // 1 bps of 9_999 = 0 (rounds down)
        assert_eq!(calculate_fee(9_999, 1), Some(0));
    }

    // --- Payout Calculation Tests ---

    #[test]
    fn test_payout_no_fee() {
        assert_eq!(calculate_payout(10_000, 0), Some(10_000));
    }

    #[test]
    fn test_payout_with_1_percent_fee() {
        assert_eq!(calculate_payout(10_000, 100), Some(9_900));
    }

    #[test]
    fn test_payout_with_5_percent_fee() {
        assert_eq!(calculate_payout(10_000, 500), Some(9_500));
    }

    #[test]
    fn test_payout_100_percent_fee() {
        assert_eq!(calculate_payout(10_000, 10000), Some(0));
    }

    #[test]
    fn test_payout_small_amount_rounds_favorably() {
        // 99 tokens with 1% fee
        // Fee: 99 * 100 / 10000 = 0 (rounds down)
        // Payout: 99 - 0 = 99 (user keeps everything)
        assert_eq!(calculate_payout(99, 100), Some(99));
    }

    // --- Cancelled Market Refund Tests ---

    #[test]
    fn test_refund_paired_only() {
        // Equal YES and NO tokens = full 1:1 refund
        assert_eq!(calculate_refund(100, 100), Some(100));
        assert_eq!(calculate_refund(1000, 1000), Some(1000));
    }

    #[test]
    fn test_refund_only_yes() {
        // Only YES tokens = 50% refund
        assert_eq!(calculate_refund(100, 0), Some(50));
        assert_eq!(calculate_refund(1000, 0), Some(500));
    }

    #[test]
    fn test_refund_only_no() {
        // Only NO tokens = 50% refund
        assert_eq!(calculate_refund(0, 100), Some(50));
        assert_eq!(calculate_refund(0, 1000), Some(500));
    }

    #[test]
    fn test_refund_mixed_more_yes() {
        // 150 YES, 100 NO
        // Paired: 100 -> 100 collateral
        // Unpaired: 50 YES -> 25 collateral
        // Total: 125
        assert_eq!(calculate_refund(150, 100), Some(125));
    }

    #[test]
    fn test_refund_mixed_more_no() {
        // 100 YES, 150 NO
        // Paired: 100 -> 100 collateral
        // Unpaired: 50 NO -> 25 collateral
        // Total: 125
        assert_eq!(calculate_refund(100, 150), Some(125));
    }

    #[test]
    fn test_refund_odd_unpaired() {
        // 101 YES, 100 NO
        // Paired: 100 -> 100 collateral
        // Unpaired: 1 YES -> 0 collateral (rounds down)
        // Total: 100
        assert_eq!(calculate_refund(101, 100), Some(100));
    }

    #[test]
    fn test_refund_both_odd() {
        // 101 YES, 100 NO
        // 102 YES, 99 NO
        // Paired: 99, Unpaired: 3 YES -> 1 (3/2=1)
        // Total: 100
        assert_eq!(calculate_refund(102, 99), Some(100));
    }

    #[test]
    fn test_refund_single_token() {
        // 1 YES only -> 0 collateral (1/2 = 0)
        assert_eq!(calculate_refund(1, 0), Some(0));
        // 2 YES only -> 1 collateral
        assert_eq!(calculate_refund(2, 0), Some(1));
    }

    #[test]
    fn test_refund_zero_tokens() {
        assert_eq!(calculate_refund(0, 0), Some(0));
    }

    #[test]
    fn test_refund_large_amounts() {
        // Near u64::MAX
        let half_max = u64::MAX / 2;
        let result = calculate_refund(half_max, half_max);
        assert_eq!(result, Some(half_max));
    }

    #[test]
    fn test_refund_asymmetric_large() {
        // One side has max tokens
        let result = calculate_refund(u64::MAX, 0);
        // Should be MAX / 2
        assert_eq!(result, Some(u64::MAX / 2));
    }

    // --- Combined Settlement Scenarios ---

    #[test]
    fn test_full_redemption_flow() {
        // User has 1000 YES and 1000 NO, wants to redeem
        let amount = 1000;
        let fee_bps = 100; // 1%

        let fee = calculate_fee(amount, fee_bps).unwrap();
        let net = calculate_payout(amount, fee_bps).unwrap();

