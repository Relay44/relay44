use anchor_lang::prelude::*;

pub mod crypto;
pub mod instructions;
pub mod state;
pub mod errors;

use instructions::*;

declare_id!("9QGtHZJvmjMKTME1s3mVfNXtGpEdXDQZJTxsxqve9GsL");

/// Privacy Layer
///
/// Confidential trading via ElGamal encryption, Pedersen commitments,
/// and zero-knowledge proofs. Arcium MXE integration planned.
#[program]
pub mod polyguard_privacy {
    use super::*;

    /// Initialize privacy configuration
    pub fn initialize_privacy_config(
        ctx: Context<InitializePrivacyConfig>,
        mxe_authority: Pubkey,
    ) -> Result<()> {
        crate::instructions::initialize_privacy_config::handler(ctx, mxe_authority)
    }

    /// Create a private account for confidential trading
    pub fn create_private_account(
        ctx: Context<CreatePrivateAccount>,
        elgamal_pubkey: [u8; 32],
    ) -> Result<()> {
        crate::instructions::create_private_account::handler(ctx, elgamal_pubkey)
    }

    /// Deposit funds into private account with proof
    pub fn private_deposit(
        ctx: Context<PrivateDeposit>,
        amount: u64,
        encrypted_amount: [u8; 64],
        deposit_proof: [u8; 64],
    ) -> Result<()> {
        crate::instructions::private_deposit::handler(ctx, amount, encrypted_amount, deposit_proof)
    }

    /// Withdraw funds from private account with ZK balance proof
    pub fn private_withdraw(
        ctx: Context<PrivateWithdraw>,
        amount: u64,
        balance_proof: [u8; 160],
    ) -> Result<()> {
        crate::instructions::private_withdraw::handler(ctx, amount, balance_proof)
    }

    /// Place a private order with hidden amounts
    pub fn place_private_order(
        ctx: Context<PlacePrivateOrder>,
        side: u8,
        outcome: u8,
        price_commitment: [u8; 32],
        quantity_commitment: [u8; 32],
        range_proof: [u8; 128],
    ) -> Result<()> {
        crate::instructions::place_private_order::handler(
            ctx,
            side,
            outcome,
            price_commitment,
            quantity_commitment,
            range_proof,
        )
    }

    /// Settle a private trade via MXE result
    pub fn private_settle(
        ctx: Context<PrivateSettle>,
        buy_order_id: u64,
        sell_order_id: u64,
        mxe_result: [u8; 256],
        settlement_proof: [u8; 128],
    ) -> Result<()> {
        crate::instructions::private_settle::handler(ctx, buy_order_id, sell_order_id, mxe_result, settlement_proof)
    }

    /// Update MXE authority
    pub fn update_mxe_authority(
        ctx: Context<UpdateMxeAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        crate::instructions::update_mxe_authority::handler(ctx, new_authority)
    }
}
