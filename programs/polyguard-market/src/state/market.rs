use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Market {
    /// Unique market identifier
    #[max_len(64)]
    pub market_id: String,

    /// Market question/title
    #[max_len(256)]
    pub question: String,

    /// Market description
    #[max_len(512)]
    pub description: String,

    /// Market category
    #[max_len(32)]
    pub category: String,

    /// Market creator/authority
    pub authority: Pubkey,

    /// Resolution oracle
    pub oracle: Pubkey,

    /// YES outcome token mint
    pub yes_mint: Pubkey,

    /// NO outcome token mint
    pub no_mint: Pubkey,

    /// Collateral vault
    pub vault: Pubkey,

    /// Collateral mint (e.g., USDC)
    pub collateral_mint: Pubkey,

    /// Market status
    pub status: MarketStatus,

    /// Resolution deadline (Unix timestamp)
    pub resolution_deadline: i64,

    /// Trading end time (Unix timestamp)
    pub trading_end: i64,

    /// Resolved outcome (0 = unresolved, 1 = Yes, 2 = No)
    pub resolved_outcome: u8,

    /// Total collateral deposited
    pub total_collateral: u64,

    /// Total YES tokens minted
    pub total_yes_supply: u64,

    /// Total NO tokens minted
    pub total_no_supply: u64,

    /// Total fee in basis points (100 = 1%)
    pub fee_bps: u16,

    /// Protocol's share of fees in basis points (e.g., 2000 = 20% of total fee)
    /// Remaining (10000 - protocol_fee_share_bps) goes to market creator
    pub protocol_fee_share_bps: u16,

    /// Protocol treasury address for fee collection
    pub protocol_treasury: Pubkey,

    /// Accumulated fees (total, before split)
    pub accumulated_fees: u64,

    /// Protocol fees already withdrawn
    pub protocol_fees_withdrawn: u64,

    /// Creator fees already withdrawn
    pub creator_fees_withdrawn: u64,

    /// Bump seed for PDA
    pub bump: u8,

    /// YES mint bump
    pub yes_mint_bump: u8,

    /// NO mint bump
    pub no_mint_bump: u8,

    /// Vault bump
    pub vault_bump: u8,

    /// Creation timestamp
    pub created_at: i64,

    /// Resolution timestamp
    pub resolved_at: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum MarketStatus {
    /// Trading is open
    Active,
    /// Trading temporarily halted
    Paused,
    /// Trading ended, awaiting resolution
    Closed,
    /// Outcome determined, claims available
    Resolved,
    /// Market cancelled, refunds available
    Cancelled,
}

impl Default for MarketStatus {
    fn default() -> Self {
        MarketStatus::Active
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum Outcome {
    Yes,
    No,
}

impl Market {
    pub const SEED_PREFIX: &'static [u8] = b"market";
    pub const YES_MINT_SEED: &'static [u8] = b"yes_mint";
    pub const NO_MINT_SEED: &'static [u8] = b"no_mint";
    pub const VAULT_SEED: &'static [u8] = b"vault";

    /// Default protocol fee share: 20% of total fees
    pub const DEFAULT_PROTOCOL_FEE_SHARE_BPS: u16 = 2000;

    pub fn is_trading_active(&self, current_time: i64) -> bool {
        self.status == MarketStatus::Active && current_time < self.trading_end
    }

    pub fn can_resolve(&self, current_time: i64) -> bool {
        self.status == MarketStatus::Closed && current_time >= self.resolution_deadline
    }

    /// Calculate the protocol's share of accumulated fees
    pub fn calculate_protocol_fees(&self) -> u64 {
        (self.accumulated_fees as u128)
            .checked_mul(self.protocol_fee_share_bps as u128)
            .and_then(|v| v.checked_div(10000))
            .unwrap_or(0) as u64
    }

    /// Calculate the creator's share of accumulated fees
    pub fn calculate_creator_fees(&self) -> u64 {
        let protocol_share = self.calculate_protocol_fees();
        self.accumulated_fees.saturating_sub(protocol_share)
    }

    /// Get available protocol fees (not yet withdrawn)
    pub fn available_protocol_fees(&self) -> u64 {
        self.calculate_protocol_fees()
            .saturating_sub(self.protocol_fees_withdrawn)
    }

    /// Get available creator fees (not yet withdrawn)
    pub fn available_creator_fees(&self) -> u64 {
        self.calculate_creator_fees()
            .saturating_sub(self.creator_fees_withdrawn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            fee_bps: 100, // 1%
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
