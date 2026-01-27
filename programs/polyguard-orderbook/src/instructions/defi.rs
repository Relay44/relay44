use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{YieldVault, YieldSource, MarginAccount, LendingPool, DeFiError};

#[derive(Accounts)]
pub struct InitializeYieldVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Market account
    pub market: UncheckedAccount<'info>,

    pub yield_mint: Account<'info, token::Mint>,

    #[account(
        init,
        payer = authority,
        space = 8 + YieldVault::INIT_SPACE,
        seeds = [YieldVault::SEED_PREFIX, market.key().as_ref()],
        bump
    )]
    pub yield_vault: Account<'info, YieldVault>,

    #[account(
        init,
        payer = authority,
        token::mint = yield_mint,
        token::authority = vault_authority,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    /// CHECK: PDA authority
    #[account(
        seeds = [b"vault_authority", yield_vault.key().as_ref()],
        bump
    )]
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_yield_vault(
    ctx: Context<InitializeYieldVault>,
    yield_source: u8,
    min_harvest_interval: u64,
    protocol_fee_bps: u16,
) -> Result<()> {
    let vault = &mut ctx.accounts.yield_vault;
    let clock = Clock::get()?;

    vault.market = ctx.accounts.market.key();
    vault.yield_mint = ctx.accounts.yield_mint.key();
    vault.vault = ctx.accounts.vault_token_account.key();
    vault.authority = ctx.accounts.vault_authority.key();
    vault.yield_source = yield_source;
    vault.bump = ctx.bumps.yield_vault;
    vault.is_active = true;
    vault._padding = [0; 1];
    vault.total_deposited = 0;
    vault.yield_accrued = 0;
    vault.last_harvest = clock.unix_timestamp;
    vault.last_exchange_rate = YieldVault::RATE_SCALE;
    vault.min_harvest_interval = min_harvest_interval;
    vault.protocol_fee_bps = protocol_fee_bps;
    vault._padding2 = [0; 6];
    vault._reserved = [0; 32];

    Ok(())
}

#[derive(Accounts)]
pub struct DepositToYieldVault<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(
        mut,
        constraint = yield_vault.is_active @ DeFiError::YieldVaultNotActive
    )]
    pub yield_vault: Account<'info, YieldVault>,

    #[account(mut)]
    pub depositor_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = yield_vault.vault
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn deposit_to_yield_vault(
    ctx: Context<DepositToYieldVault>,
    amount: u64,
) -> Result<()> {
    let vault = &mut ctx.accounts.yield_vault;

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.depositor_token_account.to_account_info(),
                to: ctx.accounts.vault_token_account.to_account_info(),
                authority: ctx.accounts.depositor.to_account_info(),
            },
        ),
        amount,
    )?;

    vault.total_deposited = vault.total_deposited.saturating_add(amount);

    Ok(())
}

#[derive(Accounts)]
pub struct HarvestYield<'info> {
    pub harvester: Signer<'info>,

    #[account(
        mut,
        constraint = yield_vault.is_active @ DeFiError::YieldVaultNotActive
    )]
    pub yield_vault: Account<'info, YieldVault>,

    #[account(address = yield_vault.vault)]
    pub vault_token_account: Account<'info, TokenAccount>,
}

pub fn harvest_yield(
    ctx: Context<HarvestYield>,
    current_rate: u64,
) -> Result<()> {
    let vault = &mut ctx.accounts.yield_vault;
    let clock = Clock::get()?;

    require!(vault.can_harvest(clock.unix_timestamp), DeFiError::HarvestTooSoon);

    let current_balance = ctx.accounts.vault_token_account.amount;
    let pending = vault.pending_yield(current_balance, current_rate);

    if pending > 0 {
        let protocol_fee = (pending as u128 * vault.protocol_fee_bps as u128 / 10000) as u64;
        let net_yield = pending.saturating_sub(protocol_fee);

        vault.yield_accrued = vault.yield_accrued.saturating_add(net_yield);
    }

    vault.last_harvest = clock.unix_timestamp;
    vault.last_exchange_rate = current_rate;

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeMarginAccount<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    pub collateral_mint: Account<'info, token::Mint>,

    #[account(
        init,
        payer = owner,
        space = 8 + MarginAccount::INIT_SPACE,
        seeds = [MarginAccount::SEED_PREFIX, owner.key().as_ref()],
        bump
    )]
    pub margin_account: Account<'info, MarginAccount>,

    #[account(
        init,
        payer = owner,
        token::mint = collateral_mint,
        token::authority = margin_account,
    )]
    pub collateral_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_margin_account(
    ctx: Context<InitializeMarginAccount>,
    max_leverage: u8,
    liquidation_threshold_bps: u16,
) -> Result<()> {
    let account = &mut ctx.accounts.margin_account;
    let clock = Clock::get()?;

    account.owner = ctx.accounts.owner.key();
    account.collateral_mint = ctx.accounts.collateral_mint.key();
    account.collateral_vault = ctx.accounts.collateral_vault.key();
    account.bump = ctx.bumps.margin_account;
    account.is_active = true;
    account.max_leverage = max_leverage;
    account._padding = [0; 1];
    account.collateral = 0;
    account.borrowed = 0;
    account.interest_accrued = 0;
    account.health_factor = u16::MAX;
    account.liquidation_threshold_bps = liquidation_threshold_bps;
    account.last_health_update = clock.unix_timestamp;
    account._padding2 = [0; 4];
    account.total_borrowed = 0;
    account.total_interest_paid = 0;
    account.liquidation_count = 0;
    account._padding3 = [0; 6];
    account._reserved = [0; 32];

    Ok(())
}

#[derive(Accounts)]
pub struct DepositCollateral<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [MarginAccount::SEED_PREFIX, owner.key().as_ref()],
        bump = margin_account.bump,
        has_one = owner,
        constraint = margin_account.is_active @ DeFiError::MarginNotActive
    )]
    pub margin_account: Account<'info, MarginAccount>,

    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = margin_account.collateral_vault
    )]
    pub collateral_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn deposit_collateral(
    ctx: Context<DepositCollateral>,
    amount: u64,
) -> Result<()> {
    let account = &mut ctx.accounts.margin_account;
    let clock = Clock::get()?;

