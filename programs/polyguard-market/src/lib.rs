use anchor_lang::prelude::*;

pub mod instructions;
pub mod state;
pub mod errors;

use instructions::*;
use instructions::withdraw_fees::FeeRecipient;
use instructions::manage_oracle::OracleAction;
use state::*;

declare_id!("98jqxMe88XGjXzCY3bwV1Kuqzj32fcwdhPZa193RUffQ");

#[program]
pub mod polyguard_market {
    use super::*;

    /// Creates a new prediction market
    pub fn create_market(
        ctx: Context<CreateMarket>,
        market_id: String,
        question: String,
        description: String,
        category: String,
        resolution_deadline: i64,
        trading_end: i64,
        fee_bps: u16,
    ) -> Result<()> {
        crate::instructions::create_market::handler(
            ctx,
            market_id,
            question,
            description,
            category,
            resolution_deadline,
            trading_end,
            fee_bps,
        )
    }

    /// Resolves a market with the final outcome
    pub fn resolve_market(ctx: Context<ResolveMarket>, outcome: Outcome) -> Result<()> {
        crate::instructions::resolve_market::handler(ctx, outcome)
    }

    /// Pauses trading on a market
    pub fn pause_market(ctx: Context<PauseMarket>) -> Result<()> {
        crate::instructions::pause_market::handler(ctx)
    }

    /// Resumes trading on a paused market
    pub fn resume_market(ctx: Context<ResumeMarket>) -> Result<()> {
        crate::instructions::resume_market::handler(ctx)
    }

    /// Cancels a market (emergency only)
    pub fn cancel_market(ctx: Context<CancelMarket>) -> Result<()> {
        crate::instructions::cancel_market::handler(ctx)
    }

    /// Mints outcome tokens (YES/NO) in exchange for collateral
    pub fn mint_outcome_tokens(ctx: Context<MintOutcomeTokens>, amount: u64) -> Result<()> {
        crate::instructions::mint_outcome_tokens::handler(ctx, amount)
    }

    /// Redeems outcome tokens for collateral (before resolution)
    pub fn redeem_outcome_tokens(ctx: Context<RedeemOutcomeTokens>, amount: u64) -> Result<()> {
        crate::instructions::redeem_outcome_tokens::handler(ctx, amount)
    }

    /// Claims winnings after market resolution
    pub fn claim_winnings(ctx: Context<ClaimWinnings>) -> Result<()> {
        crate::instructions::claim_winnings::handler(ctx)
    }

    /// Refunds collateral for cancelled markets (burns paired YES+NO tokens)
    pub fn refund_cancelled(ctx: Context<RefundCancelled>) -> Result<()> {
        crate::instructions::refund_cancelled::handler(ctx)
    }

    /// Withdraws accumulated fees with protocol/creator split
    pub fn withdraw_fees(ctx: Context<WithdrawFees>, recipient_type: FeeRecipient) -> Result<()> {
        crate::instructions::withdraw_fees::handler(ctx, recipient_type)
    }

    /// Initialize the oracle registry
    pub fn initialize_oracle_registry(
        ctx: Context<InitializeOracleRegistry>,
        enforce_validation: bool,
    ) -> Result<()> {
        crate::instructions::initialize_oracle_registry::handler(ctx, enforce_validation)
    }

    /// Add or remove an oracle from the registry
    pub fn manage_oracle(
        ctx: Context<ManageOracle>,
        oracle: Pubkey,
        action: OracleAction,
    ) -> Result<()> {
        crate::instructions::manage_oracle::handler(ctx, oracle, action)
    }

    /// Update the oracle registry authority
    pub fn update_registry_authority(
        ctx: Context<UpdateRegistryAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        crate::instructions::manage_oracle::update_authority_handler(ctx, new_authority)
    }

    /// Enable or disable oracle validation enforcement
    pub fn set_enforce_validation(
        ctx: Context<SetEnforceValidation>,
        enforce: bool,
    ) -> Result<()> {
        crate::instructions::manage_oracle::set_enforce_validation_handler(ctx, enforce)
    }

    // =========================================================================
    // Multisig Operations
    // =========================================================================

