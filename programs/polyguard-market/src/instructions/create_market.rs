use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use crate::state::{Market, MarketStatus, OracleRegistry, OracleRegistryError};
use crate::errors::MarketError;

#[derive(Accounts)]
#[instruction(market_id: String)]
pub struct CreateMarket<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Oracle account that will resolve the market (validated against registry)
    pub oracle: UncheckedAccount<'info>,

    /// Oracle registry for validation (required)
    #[account(
        seeds = [OracleRegistry::SEED_PREFIX],
        bump = oracle_registry.bump
    )]
    pub oracle_registry: Account<'info, OracleRegistry>,

    #[account(
        init,
        payer = authority,
        space = 8 + Market::INIT_SPACE,
        seeds = [Market::SEED_PREFIX, market_id.as_bytes()],
        bump
    )]
    pub market: Account<'info, Market>,

    #[account(
        init,
        payer = authority,
        mint::decimals = 6,
        mint::authority = market,
        seeds = [Market::YES_MINT_SEED, market.key().as_ref()],
        bump
    )]
    pub yes_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        mint::decimals = 6,
        mint::authority = market,
        seeds = [Market::NO_MINT_SEED, market.key().as_ref()],
        bump
    )]
    pub no_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        token::mint = collateral_mint,
        token::authority = market,
        seeds = [Market::VAULT_SEED, market.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, TokenAccount>,

    pub collateral_mint: Account<'info, Mint>,

    /// CHECK: Protocol treasury address for fee collection (validated as valid pubkey)
    pub protocol_treasury: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<CreateMarket>,
    market_id: String,
    question: String,
    description: String,
    category: String,
    resolution_deadline: i64,
    trading_end: i64,
    fee_bps: u16,
) -> Result<()> {
    // Validate inputs
    require!(market_id.len() <= 64, MarketError::MarketIdTooLong);
    require!(question.len() <= 256, MarketError::QuestionTooLong);
    require!(description.len() <= 512, MarketError::DescriptionTooLong);
    require!(category.len() <= 32, MarketError::CategoryTooLong);
    require!(fee_bps <= 1000, MarketError::InvalidFee); // Max 10%

    // Validate oracle against registry (required)
    require!(
        ctx.accounts.oracle_registry.is_approved(&ctx.accounts.oracle.key()),
        OracleRegistryError::OracleNotApproved
    );

    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    require!(
        resolution_deadline > current_time,
        MarketError::InvalidResolutionDeadline
    );
    // SECURITY: Ensure trading_end is in the future (prevents immediately-closed markets)
    require!(
        trading_end > current_time,
        MarketError::InvalidTradingEnd
    );
    require!(
        trading_end < resolution_deadline,
        MarketError::TradingEndAfterResolution
    );

    let market = &mut ctx.accounts.market;

    market.market_id = market_id;
    market.question = question;
    market.description = description;
    market.category = category;
    market.authority = ctx.accounts.authority.key();
