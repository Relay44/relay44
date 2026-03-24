use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::Market;
use crate::errors::MarketError;

/// Fee recipient type for split withdrawals
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum FeeRecipient {
    /// Protocol treasury withdrawal
    Protocol,
    /// Market creator withdrawal
    Creator,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    /// Caller - must be either market authority (creator) or protocol treasury owner
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        mut,
        constraint = market.accumulated_fees > 0 @ MarketError::NoFeesToWithdraw
    )]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [Market::VAULT_SEED, market.key().as_ref()],
        bump = market.vault_bump
    )]
    pub vault: Account<'info, TokenAccount>,

    /// Recipient account to receive fees
    #[account(
        mut,
        constraint = recipient.mint == market.collateral_mint @ MarketError::InvalidCollateral
    )]
    pub recipient: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

/// Withdraw fees with protocol/creator split
///
/// Security:
/// - Protocol fees: Only protocol treasury owner can withdraw
/// - Creator fees: Only market authority (creator) can withdraw
/// - Tracks separate withdrawal amounts to prevent double withdrawal
pub fn handler(ctx: Context<WithdrawFees>, recipient_type: FeeRecipient) -> Result<()> {
    let market = &ctx.accounts.market;
    let caller = ctx.accounts.caller.key();

    // Validate caller and calculate available amount based on recipient type
    let withdraw_amount = match recipient_type {
        FeeRecipient::Protocol => {
            // Recipient token account must be owned by the protocol treasury
            // This ensures fees go to the correct address
            require!(
                ctx.accounts.recipient.owner == market.protocol_treasury,
                MarketError::UnauthorizedWithdrawal
            );
            market.available_protocol_fees()
        }
        FeeRecipient::Creator => {
            // Only market authority can withdraw creator fees
            // and recipient must be owned by the authority
            require!(
                caller == market.authority,
                MarketError::UnauthorizedOracle
            );
            require!(
                ctx.accounts.recipient.owner == market.authority,
                MarketError::UnauthorizedWithdrawal
            );
            market.available_creator_fees()
        }
    };

    require!(withdraw_amount > 0, MarketError::NoFeesToWithdraw);

    // Verify vault has sufficient balance before transfer
    require!(
        ctx.accounts.vault.amount >= withdraw_amount,
        MarketError::InsufficientVaultBalance
    );

    // Transfer fees from vault to recipient
    let seeds = &[
        Market::SEED_PREFIX,
        market.market_id.as_bytes(),
        &[market.bump],
    ];
    let signer_seeds = &[&seeds[..]];

    let transfer_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.recipient.to_account_info(),
            authority: ctx.accounts.market.to_account_info(),
        },
        signer_seeds,
    );
    token::transfer(transfer_ctx, withdraw_amount)?;

    // Update market state
    let market = &mut ctx.accounts.market;

    match recipient_type {
        FeeRecipient::Protocol => {
            market.protocol_fees_withdrawn = market
                .protocol_fees_withdrawn
                .checked_add(withdraw_amount)
                .ok_or(MarketError::ArithmeticOverflow)?;
        }
        FeeRecipient::Creator => {
            market.creator_fees_withdrawn = market
                .creator_fees_withdrawn
                .checked_add(withdraw_amount)
                .ok_or(MarketError::ArithmeticOverflow)?;
        }
    }

    // Decrease total_collateral since fees are part of vault balance
    market.total_collateral = market
        .total_collateral
        .checked_sub(withdraw_amount)
        .ok_or(MarketError::ArithmeticOverflow)?;

    emit!(FeesWithdrawn {
        market: market.key(),
        recipient_type,
        recipient: ctx.accounts.recipient.key(),
        amount: withdraw_amount,
        protocol_fees_withdrawn: market.protocol_fees_withdrawn,
        creator_fees_withdrawn: market.creator_fees_withdrawn,
    });

    Ok(())
}

#[event]
pub struct FeesWithdrawn {
    pub market: Pubkey,
    pub recipient_type: FeeRecipient,
    pub recipient: Pubkey,
    pub amount: u64,
    pub protocol_fees_withdrawn: u64,
    pub creator_fees_withdrawn: u64,
}
