use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{PrivacyConfig, PrivateAccount};
use crate::errors::PrivacyError;
use crate::crypto::{ElGamalCiphertext, BalanceProof, CompactRangeProof};

#[derive(Accounts)]
pub struct PrivateWithdraw<'info> {
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

    /// CHECK: PDA authority for vault
    #[account(
        seeds = [b"privacy_vault_authority"],
        bump
    )]
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

/// Private withdrawal with ZK proof of sufficient balance
///
/// Security:
/// - Verifies balance proof that encrypted_balance >= amount
/// - Uses homomorphic subtraction to update encrypted balance
/// - No plaintext balance ever stored or computed on-chain
pub fn handler(
    ctx: Context<PrivateWithdraw>,
    amount: u64,
    balance_proof: [u8; 160],
) -> Result<()> {
    require!(amount > 0, PrivacyError::InvalidEncryptedAmount);

    // Parse balance proof
    // Format: [difference_commitment: 32] [range_proof: 128]
    let mut diff_commitment = [0u8; 32];
    diff_commitment.copy_from_slice(&balance_proof[0..32]);

    let mut range_proof_bytes = [0u8; CompactRangeProof::SIZE];
    range_proof_bytes.copy_from_slice(&balance_proof[32..160]);

    let proof = BalanceProof {
        difference_commitment: diff_commitment,
        range_proof: CompactRangeProof::from_bytes(&range_proof_bytes)
            .map_err(|_| PrivacyError::InvalidBalanceProof)?,
    };

    // Get current encrypted balance
    let balance_ciphertext = ctx.accounts.private_account.get_encrypted_balance()
        .map_err(|_| PrivacyError::InvalidEncryptedAmount)?;

    // Verify balance proof
    // The proof shows that (balance - amount) is non-negative
    // by proving the difference commitment opens to a value in [0, 2^64)
    //
    // HIGH-009 FIX: Validate that the proof's difference commitment is correctly
    // derived from the account's actual encrypted balance (c2 component)
    let c2_bytes: [u8; 32] = *balance_ciphertext.c2.as_bytes();
    let balance_commitment = crate::crypto::PedersenCommitment::from_bytes(&c2_bytes)
        .map_err(|_| PrivacyError::InvalidBalanceProof)?;

    // Verify the difference commitment in the proof is cryptographically linked
    // to the account's balance commitment (prevents using proofs from other accounts)
    let expected_diff_commitment = balance_commitment.subtract_value(amount)
        .map_err(|_| PrivacyError::InvalidBalanceProof)?;
    require!(
        proof.difference_commitment == expected_diff_commitment.to_bytes(),
        PrivacyError::InvalidBalanceProof
    );

    proof.verify(&balance_commitment, amount)
        .map_err(|_| PrivacyError::InvalidBalanceProof)?
        .then_some(())
        .ok_or(PrivacyError::InsufficientEncryptedBalance)?;

    // Transfer tokens from privacy vault to user
    let seeds = &[b"privacy_vault_authority".as_ref(), &[ctx.bumps.vault_authority]];
    let signer_seeds = &[&seeds[..]];

    let transfer_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.privacy_vault.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        },
        signer_seeds,
    );
    token::transfer(transfer_ctx, amount)?;

    let clock = Clock::get()?;
    let private_account = &mut ctx.accounts.private_account;

    // Create ciphertext for amount being withdrawn
    // Use deterministic randomness derived from the withdrawal to ensure
    // the subtraction is consistent
    let pubkey = private_account.get_elgamal_pubkey()
        .map_err(|_| PrivacyError::InvalidElGamalKey)?;

    // For withdrawals, we use zero randomness since the amount is public anyway
    // (visible in the token transfer). This simplifies the homomorphic operation.
    let amount_ciphertext = pubkey.encrypt_with_randomness(
        amount,
        &curve25519_dalek::scalar::Scalar::ZERO,
    ).map_err(|_| PrivacyError::InvalidEncryptedAmount)?;

    // Homomorphically subtract from encrypted balance
    private_account.subtract_from_balance(&amount_ciphertext)
        .map_err(|_| PrivacyError::ArithmeticOverflow)?;

    // Update encrypted withdrawal total
    let current_withdrawn = ElGamalCiphertext::from_bytes(&private_account.total_withdrawn_encrypted)
        .map_err(|_| PrivacyError::InvalidEncryptedAmount)?;
    let new_withdrawn = current_withdrawn.add(&amount_ciphertext)
        .map_err(|_| PrivacyError::ArithmeticOverflow)?;
    private_account.total_withdrawn_encrypted = new_withdrawn.to_bytes();

    private_account.last_activity = clock.unix_timestamp;

    emit!(PrivateWithdrawn {
        owner: private_account.owner,
        amount,
        new_encrypted_balance: private_account.encrypted_balance,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct PrivateWithdrawn {
    pub owner: Pubkey,
    pub amount: u64,
    pub new_encrypted_balance: [u8; 64],
    pub timestamp: i64,
}
