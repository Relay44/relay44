use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::{Order, OrderSide, OutcomeType, OrderStatus, Position, OrderBookConfig};
use crate::errors::OrderBookError;

#[derive(Accounts)]
pub struct SettleTrade<'info> {
    #[account(
        constraint = keeper.key() == config.keeper @ OrderBookError::UnauthorizedKeeper
    )]
    pub keeper: Signer<'info>,

    #[account(
        mut,
        seeds = [OrderBookConfig::SEED_PREFIX],
        bump = config.bump
    )]
    pub config: Box<Account<'info, OrderBookConfig>>,

    /// CHECK: Market account
    pub market: UncheckedAccount<'info>,

    // Buy order and position (boxed to reduce stack)
    #[account(
        mut,
        seeds = [Order::SEED_PREFIX, market.key().as_ref(), &buy_order.order_id.to_le_bytes()],
        bump = buy_order.bump,
        constraint = buy_order.side == OrderSide::Buy @ OrderBookError::OrdersDoNotMatch
    )]
    pub buy_order: Box<Account<'info, Order>>,

    #[account(
        mut,
        seeds = [Position::SEED_PREFIX, market.key().as_ref(), buy_order.owner.as_ref()],
        bump = buyer_position.bump
    )]
    pub buyer_position: Box<Account<'info, Position>>,

    // Sell order and position (boxed to reduce stack)
    #[account(
        mut,
        seeds = [Order::SEED_PREFIX, market.key().as_ref(), &sell_order.order_id.to_le_bytes()],
        bump = sell_order.bump,
        constraint = sell_order.side == OrderSide::Sell @ OrderBookError::OrdersDoNotMatch
    )]
    pub sell_order: Box<Account<'info, Order>>,

    #[account(
        mut,
        seeds = [Position::SEED_PREFIX, market.key().as_ref(), sell_order.owner.as_ref()],
        bump = seller_position.bump
    )]
    pub seller_position: Box<Account<'info, Position>>,

    // Token accounts (boxed to reduce stack)
    /// SECURITY: Validate escrow vault ownership
    #[account(
        mut,
        constraint = escrow_vault.owner == escrow_authority.key() @ OrderBookError::InvalidEscrowVault
    )]
    pub escrow_vault: Box<Account<'info, TokenAccount>>,

    /// Seller's collateral account to receive payment
    /// SECURITY: Validate seller ownership
    #[account(
        mut,
        constraint = seller_collateral.owner == sell_order.owner @ OrderBookError::UnauthorizedOwner
    )]
    pub seller_collateral: Box<Account<'info, TokenAccount>>,

    /// Buyer's collateral account for refund (if fill price < buy price)
    /// SECURITY: Validate buyer ownership
    #[account(
        mut,
        constraint = buyer_collateral.owner == buy_order.owner @ OrderBookError::InvalidBuyerCollateral
    )]
    pub buyer_collateral: Box<Account<'info, TokenAccount>>,

    /// CHECK: Escrow authority PDA
    #[account(
        seeds = [b"escrow_authority"],
        bump
    )]
    pub escrow_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(
    ctx: Context<SettleTrade>,
    fill_quantity: u64,
    fill_price_bps: u16,
) -> Result<()> {
    let buy_order = &ctx.accounts.buy_order;
    let sell_order = &ctx.accounts.sell_order;
    let clock = Clock::get()?;
    let current_time = clock.unix_timestamp;

    // SECURITY: Check order expiration - expired orders cannot be settled
    if buy_order.expires_at > 0 {
        require!(
            current_time < buy_order.expires_at,
            OrderBookError::OrderExpiredCannotSettle
        );
    }
    if sell_order.expires_at > 0 {
        require!(
            current_time < sell_order.expires_at,
            OrderBookError::OrderExpiredCannotSettle
        );
    }

    // Validate orders can match
    require!(
        buy_order.outcome == sell_order.outcome,
        OrderBookError::OrdersDoNotMatch
    );
    require!(
        buy_order.market == sell_order.market,
        OrderBookError::OrdersDoNotMatch
    );
    require!(
        buy_order.price_bps >= sell_order.price_bps,
        OrderBookError::OrdersDoNotMatch
    );
    require!(
        fill_quantity > 0 && fill_quantity <= buy_order.remaining_quantity && fill_quantity <= sell_order.remaining_quantity,
        OrderBookError::InvalidFillQuantity
    );
    require!(
        fill_price_bps >= sell_order.price_bps && fill_price_bps <= buy_order.price_bps,
        OrderBookError::InvalidFillPrice
    );

    // Calculate collateral amount
    let collateral_amount = (fill_quantity as u128)
        .checked_mul(fill_price_bps as u128)
        .ok_or(OrderBookError::ArithmeticOverflow)?
        .checked_div(10000)
        .ok_or(OrderBookError::ArithmeticOverflow)? as u64;

    // Calculate buyer's refund if fill price < buy price
    let buyer_locked = (fill_quantity as u128)
        .checked_mul(buy_order.price_bps as u128)
        .ok_or(OrderBookError::ArithmeticOverflow)?
        .checked_div(10000)
        .ok_or(OrderBookError::ArithmeticOverflow)? as u64;
    let buyer_refund = buyer_locked.saturating_sub(collateral_amount);

    let seeds = &[b"escrow_authority".as_ref(), &[ctx.bumps.escrow_authority]];
    let signer_seeds = &[&seeds[..]];

