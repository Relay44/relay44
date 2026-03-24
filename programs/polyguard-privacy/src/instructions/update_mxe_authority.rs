use anchor_lang::prelude::*;
use crate::state::PrivacyConfig;
use crate::errors::PrivacyError;

#[derive(Accounts)]
pub struct UpdateMxeAuthority<'info> {
    #[account(
        constraint = admin.key() == config.admin @ PrivacyError::UnauthorizedAdmin
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [PrivacyConfig::SEED_PREFIX],
        bump = config.bump
    )]
    pub config: Account<'info, PrivacyConfig>,
}

pub fn handler(ctx: Context<UpdateMxeAuthority>, new_authority: Pubkey) -> Result<()> {
    let config = &mut ctx.accounts.config;
    let old_authority = config.mxe_authority;
    config.mxe_authority = new_authority;

    emit!(MxeAuthorityUpdated {
        old_authority,
        new_authority,
        updated_by: ctx.accounts.admin.key(),
        updated_at: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct MxeAuthorityUpdated {
    pub old_authority: Pubkey,
    pub new_authority: Pubkey,
    pub updated_by: Pubkey,
    pub updated_at: i64,
}
