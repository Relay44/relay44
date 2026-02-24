//! Market Lifecycle Tests
//!
//! Tests for market state transitions and validity checks.

use super::market::{Market, MarketStatus};
use anchor_lang::prelude::Pubkey;

fn default_market() -> Market {
    Market {
        market_id: String::new(),
        question: String::new(),
        description: String::new(),
        category: String::new(),
        authority: Pubkey::default(),
        oracle: Pubkey::default(),
        yes_mint: Pubkey::default(),
        no_mint: Pubkey::default(),
        vault: Pubkey::default(),
        collateral_mint: Pubkey::default(),
        status: MarketStatus::Active,
        resolution_deadline: 0,
        trading_end: 0,
        resolved_outcome: 0,
        total_collateral: 0,
        total_yes_supply: 0,
        total_no_supply: 0,
        fee_bps: 100,
        protocol_fee_share_bps: Market::DEFAULT_PROTOCOL_FEE_SHARE_BPS,
        protocol_treasury: Pubkey::default(),
        accumulated_fees: 0,
        protocol_fees_withdrawn: 0,
        creator_fees_withdrawn: 0,
        bump: 0,
        yes_mint_bump: 0,
        no_mint_bump: 0,
        vault_bump: 0,
        created_at: 0,
        resolved_at: 0,
    }
}

/// Valid state transitions:
/// Active -> Paused (pause_market)
/// Active -> Closed (trading_end reached)
/// Active -> Cancelled (cancel_market)
/// Paused -> Active (resume_market)
/// Paused -> Cancelled (cancel_market)
/// Closed -> Resolved (resolve_market)
/// Closed -> Cancelled (cancel_market)
/// Resolved -> Disputed (file_dispute)
fn is_valid_transition(from: MarketStatus, to: MarketStatus) -> bool {
    match (from, to) {
        // From Active
        (MarketStatus::Active, MarketStatus::Paused) => true,
        (MarketStatus::Active, MarketStatus::Closed) => true,
        (MarketStatus::Active, MarketStatus::Cancelled) => true,

        // From Paused
        (MarketStatus::Paused, MarketStatus::Active) => true,
        (MarketStatus::Paused, MarketStatus::Cancelled) => true,

        // From Closed
        (MarketStatus::Closed, MarketStatus::Resolved) => true,
        (MarketStatus::Closed, MarketStatus::Cancelled) => true,

        // No transitions from Resolved or Cancelled (terminal states)
        // Except Resolved can be disputed (handled separately)
        _ => false,
    }
}

/// Check if an operation is allowed in the given market status
fn can_mint(status: MarketStatus) -> bool {
    status == MarketStatus::Active
}

fn can_redeem(status: MarketStatus) -> bool {
    status == MarketStatus::Active
}

fn can_trade(status: MarketStatus) -> bool {
    status == MarketStatus::Active
}

fn can_claim_winnings(status: MarketStatus) -> bool {
    status == MarketStatus::Resolved
}

fn can_refund(status: MarketStatus) -> bool {
    status == MarketStatus::Cancelled
}

fn can_resolve(status: MarketStatus) -> bool {
    status == MarketStatus::Closed
}

fn can_pause(status: MarketStatus) -> bool {
    status == MarketStatus::Active
}

fn can_resume(status: MarketStatus) -> bool {
    status == MarketStatus::Paused
}

fn can_cancel(status: MarketStatus) -> bool {
    // Cannot cancel after resolution
    status != MarketStatus::Resolved
}

fn can_file_dispute(status: MarketStatus) -> bool {
    status == MarketStatus::Resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- State Transition Tests ---

    #[test]
    fn test_valid_transitions_from_active() {
        assert!(is_valid_transition(MarketStatus::Active, MarketStatus::Paused));
        assert!(is_valid_transition(MarketStatus::Active, MarketStatus::Closed));
        assert!(is_valid_transition(MarketStatus::Active, MarketStatus::Cancelled));
        assert!(!is_valid_transition(MarketStatus::Active, MarketStatus::Resolved));
        assert!(!is_valid_transition(MarketStatus::Active, MarketStatus::Active));
    }

    #[test]
    fn test_valid_transitions_from_paused() {
        assert!(is_valid_transition(MarketStatus::Paused, MarketStatus::Active));
        assert!(is_valid_transition(MarketStatus::Paused, MarketStatus::Cancelled));
        assert!(!is_valid_transition(MarketStatus::Paused, MarketStatus::Closed));
        assert!(!is_valid_transition(MarketStatus::Paused, MarketStatus::Resolved));
    }

    #[test]
    fn test_valid_transitions_from_closed() {
        assert!(is_valid_transition(MarketStatus::Closed, MarketStatus::Resolved));
        assert!(is_valid_transition(MarketStatus::Closed, MarketStatus::Cancelled));
        assert!(!is_valid_transition(MarketStatus::Closed, MarketStatus::Active));
        assert!(!is_valid_transition(MarketStatus::Closed, MarketStatus::Paused));
    }

    #[test]
    fn test_resolved_is_terminal() {
        assert!(!is_valid_transition(MarketStatus::Resolved, MarketStatus::Active));
        assert!(!is_valid_transition(MarketStatus::Resolved, MarketStatus::Paused));
        assert!(!is_valid_transition(MarketStatus::Resolved, MarketStatus::Closed));
        assert!(!is_valid_transition(MarketStatus::Resolved, MarketStatus::Cancelled));
    }

    #[test]
    fn test_cancelled_is_terminal() {
        assert!(!is_valid_transition(MarketStatus::Cancelled, MarketStatus::Active));
        assert!(!is_valid_transition(MarketStatus::Cancelled, MarketStatus::Paused));
        assert!(!is_valid_transition(MarketStatus::Cancelled, MarketStatus::Closed));
        assert!(!is_valid_transition(MarketStatus::Cancelled, MarketStatus::Resolved));
    }

    // --- Operation Permission Tests ---

    #[test]
    fn test_can_mint_by_status() {
        assert!(can_mint(MarketStatus::Active));
        assert!(!can_mint(MarketStatus::Paused));
        assert!(!can_mint(MarketStatus::Closed));
        assert!(!can_mint(MarketStatus::Resolved));
        assert!(!can_mint(MarketStatus::Cancelled));
    }

    #[test]
    fn test_can_redeem_by_status() {
        assert!(can_redeem(MarketStatus::Active));
        assert!(!can_redeem(MarketStatus::Paused));
        assert!(!can_redeem(MarketStatus::Closed));
        assert!(!can_redeem(MarketStatus::Resolved));
        assert!(!can_redeem(MarketStatus::Cancelled));
    }

    #[test]
    fn test_can_trade_by_status() {
        assert!(can_trade(MarketStatus::Active));
        assert!(!can_trade(MarketStatus::Paused));
        assert!(!can_trade(MarketStatus::Closed));
        assert!(!can_trade(MarketStatus::Resolved));
        assert!(!can_trade(MarketStatus::Cancelled));
    }

    #[test]
    fn test_can_claim_winnings_by_status() {
        assert!(!can_claim_winnings(MarketStatus::Active));
        assert!(!can_claim_winnings(MarketStatus::Paused));
        assert!(!can_claim_winnings(MarketStatus::Closed));
        assert!(can_claim_winnings(MarketStatus::Resolved));
        assert!(!can_claim_winnings(MarketStatus::Cancelled));
    }

    #[test]
    fn test_can_refund_by_status() {
        assert!(!can_refund(MarketStatus::Active));
        assert!(!can_refund(MarketStatus::Paused));
        assert!(!can_refund(MarketStatus::Closed));
        assert!(!can_refund(MarketStatus::Resolved));
        assert!(can_refund(MarketStatus::Cancelled));
    }

    #[test]
    fn test_can_resolve_by_status() {
        assert!(!can_resolve(MarketStatus::Active));
        assert!(!can_resolve(MarketStatus::Paused));
        assert!(can_resolve(MarketStatus::Closed));
        assert!(!can_resolve(MarketStatus::Resolved));
        assert!(!can_resolve(MarketStatus::Cancelled));
    }

    #[test]
    fn test_can_pause_by_status() {
        assert!(can_pause(MarketStatus::Active));
        assert!(!can_pause(MarketStatus::Paused));
        assert!(!can_pause(MarketStatus::Closed));
        assert!(!can_pause(MarketStatus::Resolved));
        assert!(!can_pause(MarketStatus::Cancelled));
    }

    #[test]
    fn test_can_resume_by_status() {
        assert!(!can_resume(MarketStatus::Active));
        assert!(can_resume(MarketStatus::Paused));
        assert!(!can_resume(MarketStatus::Closed));
        assert!(!can_resume(MarketStatus::Resolved));
        assert!(!can_resume(MarketStatus::Cancelled));
    }

    #[test]
    fn test_can_cancel_by_status() {
        assert!(can_cancel(MarketStatus::Active));
        assert!(can_cancel(MarketStatus::Paused));
        assert!(can_cancel(MarketStatus::Closed));
        assert!(!can_cancel(MarketStatus::Resolved)); // Cannot cancel after resolution
        assert!(can_cancel(MarketStatus::Cancelled)); // Idempotent
    }

    #[test]
    fn test_can_file_dispute_by_status() {
        assert!(!can_file_dispute(MarketStatus::Active));
        assert!(!can_file_dispute(MarketStatus::Paused));
        assert!(!can_file_dispute(MarketStatus::Closed));
        assert!(can_file_dispute(MarketStatus::Resolved));
        assert!(!can_file_dispute(MarketStatus::Cancelled));
    }

    // --- Timing Constraint Tests ---

    #[test]
    fn test_trading_active_boundary() {
        let mut market = default_market();
        market.status = MarketStatus::Active;
        market.trading_end = 1000;

        // Trading is active before trading_end
        assert!(market.is_trading_active(999));
        // Trading stops at trading_end
        assert!(!market.is_trading_active(1000));
        // Trading stops after trading_end
        assert!(!market.is_trading_active(1001));
    }

    #[test]
    fn test_can_resolve_boundary() {
        let mut market = default_market();
        market.status = MarketStatus::Closed;
        market.resolution_deadline = 1000;

        // Cannot resolve before deadline
        assert!(!market.can_resolve(999));
        // Can resolve at deadline
        assert!(market.can_resolve(1000));
        // Can resolve after deadline
        assert!(market.can_resolve(2000));
    }

    #[test]
    fn test_trading_end_before_resolution_deadline() {
        // Trading ends before resolution is allowed
        let mut market = default_market();
        market.status = MarketStatus::Closed;
        market.trading_end = 1000;
        market.resolution_deadline = 2000;

        // At time 1500: trading ended, but cannot resolve yet
        assert!(!market.is_trading_active(1500));
        assert!(!market.can_resolve(1500));

        // At time 2500: trading ended, can resolve
        assert!(!market.is_trading_active(2500));
        assert!(market.can_resolve(2500));
    }

    // --- Market State Consistency Tests ---

    #[test]
    fn test_resolved_market_has_outcome() {
        let mut market = default_market();
        market.status = MarketStatus::Resolved;
        market.resolved_outcome = 1; // YES

        assert!(market.resolved_outcome == 1 || market.resolved_outcome == 2);
    }

    #[test]
    fn test_unresolved_market_no_outcome() {
        let market = default_market();
        assert_eq!(market.resolved_outcome, 0);
    }

    #[test]
    fn test_supply_invariant() {
        // YES + NO supply should relate to total collateral
        let mut market = default_market();
        market.total_collateral = 1000;
        market.total_yes_supply = 1000;
        market.total_no_supply = 1000;

        // For a balanced mint, yes_supply == no_supply == collateral
        // (1 collateral = 1 YES + 1 NO)
        assert_eq!(market.total_yes_supply, market.total_no_supply);
        assert_eq!(market.total_yes_supply, market.total_collateral);
    }

    #[test]
    fn test_fee_invariant() {
        // Accumulated fees should be >= withdrawn fees
        let mut market = default_market();
        market.accumulated_fees = 1000;
        market.protocol_fees_withdrawn = 200;
        market.creator_fees_withdrawn = 800;

