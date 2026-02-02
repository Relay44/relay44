use anchor_lang::prelude::*;
use crate::state::PrivacyConfig;

#[derive(Accounts)]
pub struct InitializePrivacyConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + PrivacyConfig::INIT_SPACE,
        seeds = [PrivacyConfig::SEED_PREFIX],
        bump
    )]
    pub config: Account<'info, PrivacyConfig>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitializePrivacyConfig>, mxe_authority: Pubkey) -> Result<()> {
    let config = &mut ctx.accounts.config;
    let clock = Clock::get()?;

    config.admin = ctx.accounts.admin.key();
    config.mxe_authority = mxe_authority;
    config.total_private_accounts = 0;
    config.total_private_orders = 0;
    config.total_private_settlements = 0;
    config.enabled = true;
    config.bump = ctx.bumps.config;
    config.created_at = clock.unix_timestamp;

    emit!(PrivacyConfigInitialized {
        admin: config.admin,
        mxe_authority: config.mxe_authority,
        created_at: config.created_at,
    });

    Ok(())
}

#[event]
pub struct PrivacyConfigInitialized {
    pub admin: Pubkey,
    pub mxe_authority: Pubkey,
    pub created_at: i64,
}
