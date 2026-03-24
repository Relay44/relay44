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

    // Transfer collateral from escrow to seller
    let transfer_to_seller_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.escrow_vault.to_account_info(),
            to: ctx.accounts.seller_collateral.to_account_info(),
            authority: ctx.accounts.escrow_authority.to_account_info(),
        },
        signer_seeds,
    );
    token::transfer(transfer_to_seller_ctx, collateral_amount)?;

    // SECURITY FIX: Transfer buyer's refund back to buyer if fill price was lower
    if buyer_refund > 0 {
        let transfer_refund_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.escrow_vault.to_account_info(),
                to: ctx.accounts.buyer_collateral.to_account_info(),
                authority: ctx.accounts.escrow_authority.to_account_info(),
            },
            signer_seeds,
        );
        token::transfer(transfer_refund_ctx, buyer_refund)?;
    }

    // Update orders
    let buy_order = &mut ctx.accounts.buy_order;
    buy_order.remaining_quantity = buy_order.remaining_quantity
        .checked_sub(fill_quantity)
        .ok_or(OrderBookError::ArithmeticOverflow)?;
    buy_order.filled_quantity = buy_order.filled_quantity
        .checked_add(fill_quantity)
        .ok_or(OrderBookError::ArithmeticOverflow)?;
    buy_order.updated_at = clock.unix_timestamp;

    if buy_order.remaining_quantity == 0 {
        buy_order.status = OrderStatus::Filled;
    } else {
        buy_order.status = OrderStatus::PartiallyFilled;
    }

    let sell_order = &mut ctx.accounts.sell_order;
    sell_order.remaining_quantity = sell_order.remaining_quantity
        .checked_sub(fill_quantity)
        .ok_or(OrderBookError::ArithmeticOverflow)?;
    sell_order.filled_quantity = sell_order.filled_quantity
        .checked_add(fill_quantity)
        .ok_or(OrderBookError::ArithmeticOverflow)?;
    sell_order.updated_at = clock.unix_timestamp;

    if sell_order.remaining_quantity == 0 {
        sell_order.status = OrderStatus::Filled;
    } else {
        sell_order.status = OrderStatus::PartiallyFilled;
    }

    // Update positions
    let buyer_position = &mut ctx.accounts.buyer_position;
    buyer_position.locked_collateral = buyer_position.locked_collateral
        .saturating_sub(buyer_locked);
    buyer_position.total_trades = buyer_position.total_trades
        .checked_add(1)
        .ok_or(OrderBookError::ArithmeticOverflow)?;

    // Credit buyer with outcome tokens
    match ctx.accounts.buy_order.outcome {
        OutcomeType::Yes => {
            buyer_position.yes_balance = buyer_position.yes_balance
                .checked_add(fill_quantity)
                .ok_or(OrderBookError::ArithmeticOverflow)?;
        }
        OutcomeType::No => {
            buyer_position.no_balance = buyer_position.no_balance
                .checked_add(fill_quantity)
                .ok_or(OrderBookError::ArithmeticOverflow)?;
        }
    }

    if ctx.accounts.buy_order.status == OrderStatus::Filled {
        buyer_position.open_order_count = buyer_position.open_order_count.saturating_sub(1);
    }

    let seller_position = &mut ctx.accounts.seller_position;
    seller_position.total_trades = seller_position.total_trades
        .checked_add(1)
        .ok_or(OrderBookError::ArithmeticOverflow)?;

    // Debit seller's locked tokens
    match ctx.accounts.sell_order.outcome {
        OutcomeType::Yes => {
            seller_position.locked_yes = seller_position.locked_yes
                .saturating_sub(fill_quantity);
            seller_position.yes_balance = seller_position.yes_balance
                .saturating_sub(fill_quantity);
        }
        OutcomeType::No => {
            seller_position.locked_no = seller_position.locked_no
                .saturating_sub(fill_quantity);
            seller_position.no_balance = seller_position.no_balance
                .saturating_sub(fill_quantity);
        }
    }

    if ctx.accounts.sell_order.status == OrderStatus::Filled {
        seller_position.open_order_count = seller_position.open_order_count.saturating_sub(1);
    }

    // Update global stats
    let config = &mut ctx.accounts.config;
    config.total_trades = config.total_trades
        .checked_add(1)
        .ok_or(OrderBookError::ArithmeticOverflow)?;
    config.total_volume = config.total_volume
        .checked_add(collateral_amount)
        .ok_or(OrderBookError::ArithmeticOverflow)?;

    emit!(TradeFilled {
        buy_order_id: ctx.accounts.buy_order.order_id,
        sell_order_id: ctx.accounts.sell_order.order_id,
        market: ctx.accounts.buy_order.market,
        outcome: ctx.accounts.buy_order.outcome,
        buyer: ctx.accounts.buy_order.owner,
        seller: ctx.accounts.sell_order.owner,
        fill_price_bps,
        fill_quantity,
        collateral_amount,
        buyer_refund,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct TradeFilled {
    pub buy_order_id: u64,
    pub sell_order_id: u64,
    pub market: Pubkey,
    pub outcome: OutcomeType,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub fill_price_bps: u16,
    pub fill_quantity: u64,
    pub collateral_amount: u64,
    pub buyer_refund: u64,
    pub timestamp: i64,
}
