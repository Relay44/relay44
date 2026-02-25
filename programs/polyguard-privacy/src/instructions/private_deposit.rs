use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{PrivacyConfig, PrivateAccount};
use crate::errors::PrivacyError;
use crate::crypto::{ElGamalCiphertext, DepositProof};

#[derive(Accounts)]
pub struct PrivateDeposit<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [PrivacyConfig::SEED_PREFIX],
        bump = config.bump,
        constraint = config.enabled @ PrivacyError::PrivateAccountNotInitialized
    )]
    pub config: Account<'info, PrivacyConfig>,

    #[account(
        mut,
        seeds = [PrivateAccount::SEED_PREFIX, owner.key().as_ref()],
        bump = private_account.bump,
        constraint = private_account.owner == owner.key() @ PrivacyError::UnauthorizedAdmin,
        constraint = private_account.is_active @ PrivacyError::PrivateAccountNotInitialized,
        constraint = private_account.is_crypto_enabled() @ PrivacyError::InvalidElGamalKey
    )]
    pub private_account: Account<'info, PrivateAccount>,

    #[account(
        mut,
        constraint = user_token_account.owner == owner.key()
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub privacy_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

/// Private deposit with proof that encrypted_amount correctly encrypts `amount`
///
/// Security:
/// - Verifies deposit proof linking plaintext amount to ciphertext
/// - Uses homomorphic addition to update encrypted balance
/// - No plaintext balance stored on-chain
pub fn handler(
    ctx: Context<PrivateDeposit>,
    amount: u64,
    encrypted_amount: [u8; 64],
    deposit_proof: [u8; 64],
) -> Result<()> {
    require!(amount > 0, PrivacyError::InvalidEncryptedAmount);

    // Parse encrypted amount
    let ciphertext = ElGamalCiphertext::from_bytes(&encrypted_amount)
        .map_err(|_| PrivacyError::InvalidEncryptedAmount)?;

    // Parse and verify deposit proof
    let proof = DepositProof {
        challenge: {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&deposit_proof[0..32]);
            arr
        },
        response: {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&deposit_proof[32..64]);
            arr
        },
    };

    // Get public key and verify proof
    let pubkey = ctx.accounts.private_account.get_elgamal_pubkey()
        .map_err(|_| PrivacyError::InvalidElGamalKey)?;

    proof.verify(&pubkey, amount, &ciphertext)
        .map_err(|_| PrivacyError::ProofVerificationFailed)?
        .then_some(())
        .ok_or(PrivacyError::ProofVerificationFailed)?;

    // Transfer tokens to privacy vault
    let transfer_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.privacy_vault.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        },
    );
    token::transfer(transfer_ctx, amount)?;

    let clock = Clock::get()?;
    let private_account = &mut ctx.accounts.private_account;

    // Homomorphically add to encrypted balance
    private_account.add_to_balance(&ciphertext)
        .map_err(|_| PrivacyError::ArithmeticOverflow)?;

    // Update encrypted deposit total (also homomorphic)
    let current_deposited = ElGamalCiphertext::from_bytes(&private_account.total_deposited_encrypted)
        .map_err(|_| PrivacyError::InvalidEncryptedAmount)?;
    let new_deposited = current_deposited.add(&ciphertext)
        .map_err(|_| PrivacyError::ArithmeticOverflow)?;
    private_account.total_deposited_encrypted = new_deposited.to_bytes();

    private_account.last_activity = clock.unix_timestamp;

    emit!(PrivateDeposited {
        owner: private_account.owner,
        // Note: amount is public since token transfer reveals it
        amount,
        new_encrypted_balance: private_account.encrypted_balance,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct PrivateDeposited {
    pub owner: Pubkey,
    pub amount: u64,
    pub new_encrypted_balance: [u8; 64],
    pub timestamp: i64,
}
