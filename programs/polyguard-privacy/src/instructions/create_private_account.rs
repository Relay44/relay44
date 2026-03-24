use anchor_lang::prelude::*;
use crate::state::{PrivacyConfig, PrivateAccount};
use crate::errors::PrivacyError;
use crate::crypto::ElGamalPubkey;

#[derive(Accounts)]
pub struct CreatePrivateAccount<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [PrivacyConfig::SEED_PREFIX],
        bump = config.bump,
        constraint = config.enabled @ PrivacyError::PrivateAccountNotInitialized
    )]
    pub config: Account<'info, PrivacyConfig>,

    #[account(
        init,
        payer = owner,
        space = 8 + PrivateAccount::INIT_SPACE,
        seeds = [PrivateAccount::SEED_PREFIX, owner.key().as_ref()],
        bump
    )]
    pub private_account: Account<'info, PrivateAccount>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreatePrivateAccount>, elgamal_pubkey: [u8; 32]) -> Result<()> {
    // SECURITY: Validate ElGamal public key is a valid Ristretto point
    ElGamalPubkey::from_bytes(&elgamal_pubkey)
        .map_err(|_| PrivacyError::InvalidElGamalKey)?;

    let clock = Clock::get()?;
    let private_account = &mut ctx.accounts.private_account;

    private_account.owner = ctx.accounts.owner.key();
    private_account.elgamal_pubkey = elgamal_pubkey;
    private_account.version = PrivateAccount::CURRENT_VERSION;
    private_account.is_active = true;
    private_account.bump = ctx.bumps.private_account;
    private_account.created_at = clock.unix_timestamp;
    private_account.last_activity = clock.unix_timestamp;
    private_account.private_order_count = 0;
    private_account.private_settlement_count = 0;

    // Initialize encrypted balances to encryption of zero
    // This uses the identity point which decrypts to 0
    private_account.initialize_zero_balance()
        .map_err(|_| PrivacyError::InvalidElGamalKey)?;

    // Update config
    let config = &mut ctx.accounts.config;
    config.total_private_accounts = config.total_private_accounts
        .checked_add(1)
        .ok_or(PrivacyError::ArithmeticOverflow)?;

    emit!(PrivateAccountCreated {
        owner: private_account.owner,
        elgamal_pubkey,
        version: private_account.version,
        created_at: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct PrivateAccountCreated {
    pub owner: Pubkey,
    pub elgamal_pubkey: [u8; 32],
    pub version: u8,
    pub created_at: i64,
}
