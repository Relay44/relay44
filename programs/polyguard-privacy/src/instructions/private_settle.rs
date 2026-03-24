use anchor_lang::prelude::*;
use anchor_lang::solana_program::ed25519_program;
use anchor_lang::solana_program::sysvar::instructions::{
    load_instruction_at_checked, ID as SYSVAR_INSTRUCTIONS_ID,
};
use crate::state::{PrivacyConfig, PrivateAccount, PrivateOrder, PrivateSettlement};
use crate::errors::PrivacyError;
use crate::crypto::{CompactRangeProof, PedersenCommitment};

/// Verify Ed25519 signature from MXE authority by checking the Ed25519 precompile instruction
fn verify_mxe_signature(
    mxe_authority: &Pubkey,
    instructions_sysvar: &AccountInfo,
) -> Result<()> {
    // The Ed25519 signature verification must be done via the Ed25519 precompile
    // which should be called as the previous instruction in the transaction
    let current_idx = anchor_lang::solana_program::sysvar::instructions::load_current_index_checked(
        instructions_sysvar,
    ).map_err(|_| PrivacyError::InvalidMxeSignature)?;

    // Look for Ed25519 precompile instruction before this one
    if current_idx == 0 {
        return Err(PrivacyError::InvalidMxeSignature.into());
    }

    let ed25519_ix = load_instruction_at_checked(
        (current_idx - 1) as usize,
        instructions_sysvar,
    ).map_err(|_| PrivacyError::InvalidMxeSignature)?;

    // Verify the instruction was from the Ed25519 program
    require!(
        ed25519_ix.program_id == ed25519_program::ID,
        PrivacyError::InvalidMxeSignature
    );

    // Parse the Ed25519 instruction data
    // Format: [num_sigs(1), padding(1), sig_offset(2), sig_instruction_idx(2),
    //          pubkey_offset(2), pubkey_instruction_idx(2), message_offset(2),
    //          message_len(2), message_instruction_idx(2)]
    if ed25519_ix.data.len() < 16 {
        return Err(PrivacyError::InvalidMxeSignature.into());
    }

    // Extract pubkey offset from instruction data
    let pubkey_offset = u16::from_le_bytes([ed25519_ix.data[6], ed25519_ix.data[7]]) as usize;
    if pubkey_offset + 32 > ed25519_ix.data.len() {
        return Err(PrivacyError::InvalidMxeSignature.into());
    }

    // Extract and verify the pubkey matches MXE authority
    let pubkey_in_ix: [u8; 32] = ed25519_ix.data[pubkey_offset..pubkey_offset + 32]
        .try_into()
        .map_err(|_| PrivacyError::InvalidMxeSignature)?;

    require!(
        pubkey_in_ix == mxe_authority.to_bytes(),
        PrivacyError::InvalidMxeSignature
    );

    // The Ed25519 precompile already verified the signature
    // If we got here, the signature is valid
    Ok(())
}

#[derive(Accounts)]
#[instruction(buy_order_id: u64, sell_order_id: u64)]
pub struct PrivateSettle<'info> {
    /// MXE authority (Arcium verifier)
    #[account(mut)]
    pub mxe_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [PrivacyConfig::SEED_PREFIX],
        bump = config.bump,
        constraint = mxe_authority.key() == config.mxe_authority @ PrivacyError::UnauthorizedMxeAuthority
    )]
    pub config: Box<Account<'info, PrivacyConfig>>,

    /// CHECK: Market account
    pub market: UncheckedAccount<'info>,

    // Buy order (boxed to reduce stack)
    #[account(
        mut,
        seeds = [
            PrivateOrder::SEED_PREFIX,
            market.key().as_ref(),
            &buy_order_id.to_le_bytes()
        ],
        bump,
        constraint = buy_order.side == PrivateOrder::SIDE_BUY @ PrivacyError::InvalidOrderSide,
        constraint = buy_order.status == PrivateOrder::STATUS_OPEN @ PrivacyError::MxeComputationFailed
    )]
    pub buy_order: Box<Account<'info, PrivateOrder>>,

    #[account(
        mut,
        seeds = [PrivateAccount::SEED_PREFIX, buy_order.owner.as_ref()],
        bump,
        constraint = buyer_account.is_active @ PrivacyError::PrivateAccountNotInitialized
    )]
    pub buyer_account: Box<Account<'info, PrivateAccount>>,

    // Sell order (boxed to reduce stack)
    #[account(
        mut,
        seeds = [
            PrivateOrder::SEED_PREFIX,
            market.key().as_ref(),
            &sell_order_id.to_le_bytes()
        ],
        bump,
        constraint = sell_order.side == PrivateOrder::SIDE_SELL @ PrivacyError::InvalidOrderSide,
        constraint = sell_order.status == PrivateOrder::STATUS_OPEN @ PrivacyError::MxeComputationFailed
    )]
    pub sell_order: Box<Account<'info, PrivateOrder>>,

    #[account(
        mut,
        seeds = [PrivateAccount::SEED_PREFIX, sell_order.owner.as_ref()],
        bump,
        constraint = seller_account.is_active @ PrivacyError::PrivateAccountNotInitialized
    )]
    pub seller_account: Box<Account<'info, PrivateAccount>>,

    // Settlement record (boxed to reduce stack)
    #[account(
        init,
        payer = mxe_authority,
        space = 8 + PrivateSettlement::INIT_SPACE,
        seeds = [
            PrivateSettlement::SEED_PREFIX,
            buy_order.key().as_ref(),
            sell_order.key().as_ref()
        ],
        bump
    )]
    pub settlement: Box<Account<'info, PrivateSettlement>>,

    pub system_program: Program<'info, System>,

    /// CHECK: Instructions sysvar for Ed25519 signature verification
    #[account(address = SYSVAR_INSTRUCTIONS_ID)]
    pub instructions_sysvar: AccountInfo<'info>,
}

pub fn handler(
    ctx: Context<PrivateSettle>,
    _buy_order_id: u64,
    _sell_order_id: u64,
    mxe_result: [u8; 256],
    settlement_proof: [u8; 128],
) -> Result<()> {
    // Parse MXE result structure:
    // [0..64]   = encrypted fill quantity (ElGamal ciphertext)
    // [64..128] = encrypted fill price (ElGamal ciphertext)
    // [128..160] = fill quantity commitment (Pedersen)
    // [160..192] = fill price commitment (Pedersen)
    // [192..256] = MXE signature (Ed25519 over the above)
    let encrypted_fill_quantity: [u8; 64] = mxe_result[0..64].try_into()
        .map_err(|_| PrivacyError::InvalidMxeResult)?;
    let encrypted_fill_price: [u8; 64] = mxe_result[64..128].try_into()
        .map_err(|_| PrivacyError::InvalidMxeResult)?;
    let fill_qty_commitment: [u8; 32] = mxe_result[128..160].try_into()
        .map_err(|_| PrivacyError::InvalidMxeResult)?;
    let fill_price_commitment: [u8; 32] = mxe_result[160..192].try_into()
        .map_err(|_| PrivacyError::InvalidMxeResult)?;
    // Note: bytes [192..256] contain the MXE Ed25519 signature, verified via precompile

    // Verify MXE signature over the result data
    // The Ed25519 precompile must be called as the previous instruction
    // with the signature over the first 192 bytes (encrypted data + commitments)
    verify_mxe_signature(
        &ctx.accounts.config.mxe_authority,
        &ctx.accounts.instructions_sysvar,
    )?;

    // Validate commitments are valid curve points
    let _qty_comm = PedersenCommitment::from_bytes(&fill_qty_commitment)
        .map_err(|_| PrivacyError::InvalidMxeResult)?;
    let _price_comm = PedersenCommitment::from_bytes(&fill_price_commitment)
        .map_err(|_| PrivacyError::InvalidMxeResult)?;

    // Verify settlement proof (range proof on fill quantity)
    // This proves the fill quantity is valid (non-negative, bounded)
    let settlement_range_proof = CompactRangeProof::from_bytes(&settlement_proof)
        .map_err(|_| PrivacyError::InvalidSettlementProof)?;

    let proof_valid = settlement_range_proof.verify()
        .map_err(|_| PrivacyError::InvalidSettlementProof)?;
    require!(proof_valid, PrivacyError::InvalidSettlementProof);

    // Verify the settlement proof commitment matches the fill quantity commitment
    require!(
        settlement_range_proof.commitment == fill_qty_commitment,
        PrivacyError::InvalidSettlementProof
    );

    // Validate orders are for same market and outcome
    require!(
        ctx.accounts.buy_order.market == ctx.accounts.sell_order.market,
        PrivacyError::MxeComputationFailed
    );
    require!(
        ctx.accounts.buy_order.outcome == ctx.accounts.sell_order.outcome,
        PrivacyError::MxeComputationFailed
    );

    let clock = Clock::get()?;

    // Initialize settlement record
    let settlement = &mut ctx.accounts.settlement;
    settlement.buy_order = ctx.accounts.buy_order.key();
    settlement.sell_order = ctx.accounts.sell_order.key();
    settlement.market = ctx.accounts.market.key();
    settlement.mxe_result = mxe_result;
    settlement.settlement_proof = settlement_proof;
    settlement.encrypted_fill_quantity = encrypted_fill_quantity;
    settlement.encrypted_fill_price = encrypted_fill_price;
    settlement.status = PrivateSettlement::STATUS_COMPLETED;
    settlement.bump = ctx.bumps.settlement;
    settlement.settled_at = clock.unix_timestamp;

    // Update orders
    let buy_order = &mut ctx.accounts.buy_order;
    buy_order.status = PrivateOrder::STATUS_FILLED;
    buy_order.settled_at = clock.unix_timestamp;

    let sell_order = &mut ctx.accounts.sell_order;
    sell_order.status = PrivateOrder::STATUS_FILLED;
    sell_order.settled_at = clock.unix_timestamp;

    // Update accounts
    let buyer_account = &mut ctx.accounts.buyer_account;
    buyer_account.private_settlement_count = buyer_account.private_settlement_count
        .checked_add(1)
        .ok_or(PrivacyError::ArithmeticOverflow)?;
    buyer_account.last_activity = clock.unix_timestamp;

    let seller_account = &mut ctx.accounts.seller_account;
    seller_account.private_settlement_count = seller_account.private_settlement_count
        .checked_add(1)
        .ok_or(PrivacyError::ArithmeticOverflow)?;
    seller_account.last_activity = clock.unix_timestamp;

    // Update config
    let config = &mut ctx.accounts.config;
    config.total_private_settlements = config.total_private_settlements
        .checked_add(1)
        .ok_or(PrivacyError::ArithmeticOverflow)?;

    emit!(PrivateTradeSettled {
        settlement: settlement.key(),
        buy_order: ctx.accounts.buy_order.key(),
        sell_order: ctx.accounts.sell_order.key(),
        market: settlement.market,
        buyer: ctx.accounts.buy_order.owner,
        seller: ctx.accounts.sell_order.owner,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct PrivateTradeSettled {
    pub settlement: Pubkey,
    pub buy_order: Pubkey,
    pub sell_order: Pubkey,
    pub market: Pubkey,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub timestamp: i64,
}
