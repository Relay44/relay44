use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::errors::OrderBookError;
use crate::state::{
    BookSide, EventHeap, FillEvent, OpenOrdersAccount, OrderBookConfig, OutEvent, PRICE_SCALE,
    MAX_ORDER_QUANTITY,
};

/// Maximum orders to match in a single transaction
pub const MAX_MATCHES: usize = 8;

/// Maximum expired orders to clean up per transaction
pub const MAX_EXPIRED_CLEANUP: usize = 5;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum OrderSideV2 {
    Buy,
    Sell,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeV2 {
    Yes,
    No,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum OrderTypeV2 {
    Limit,
    Market,
    PostOnly,
    ImmediateOrCancel,
    FillOrKill,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PlaceOrderParams {
    pub side: OrderSideV2,
    pub outcome: OutcomeV2,
    pub price: u64,           // In basis points (1-9999)
    pub quantity: u64,        // Number of outcome tokens
    pub order_type: OrderTypeV2,
    pub client_order_id: u64,
    pub time_in_force: u16,   // Seconds until expiry (0 = no expiry)
    pub limit: u8,            // Max orders to match (default: 8)
}

#[derive(Accounts)]
pub struct PlaceOrderV2<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [OrderBookConfig::SEED_PREFIX],
        bump = config.bump,
        constraint = !config.paused @ OrderBookError::MarketNotActive
    )]
    pub config: Account<'info, OrderBookConfig>,

    /// CHECK: Market account - validated by open_orders seeds
    pub market: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [OpenOrdersAccount::SEED_PREFIX, market.key().as_ref(), owner.key().as_ref()],
        bump = open_orders.bump,
        constraint = open_orders.owner == owner.key() @ OrderBookError::UnauthorizedOwner
    )]
    pub open_orders: Account<'info, OpenOrdersAccount>,

    /// Bids bookside
    #[account(mut)]
    pub bids: AccountLoader<'info, BookSide>,

    /// Asks bookside
    #[account(mut)]
    pub asks: AccountLoader<'info, BookSide>,

    /// Event heap for settlement
    #[account(mut)]
    pub event_heap: AccountLoader<'info, EventHeap>,

    /// User's collateral token account
    #[account(
        mut,
        constraint = user_collateral.owner == owner.key() @ OrderBookError::UnauthorizedOwner
    )]
    pub user_collateral: Account<'info, TokenAccount>,

    /// Market's collateral vault
    #[account(mut)]
    pub market_vault: Account<'info, TokenAccount>,

    /// CHECK: Market authority PDA for vault transfers
    #[account(
        seeds = [b"market_authority", market.key().as_ref()],
        bump
    )]
    pub market_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PlaceOrderResult {
    pub order_id: Option<u128>,
    pub posted_quantity: u64,
    pub filled_quantity: u64,
    pub total_cost: u64,
}

pub fn handler(ctx: Context<PlaceOrderV2>, params: PlaceOrderParams) -> Result<PlaceOrderResult> {
    // Validate inputs
    require!(
        params.price >= 1 && params.price <= 9999,
        OrderBookError::InvalidPrice
    );
    require!(params.quantity > 0, OrderBookError::InvalidQuantity);
    require!(
        params.quantity <= MAX_ORDER_QUANTITY,
        OrderBookError::QuantityTooLarge
    );

    let clock = Clock::get()?;
    let now = clock.unix_timestamp;
    let limit = if params.limit == 0 {
        MAX_MATCHES
    } else {
        params.limit as usize
    };

    let mut bids = ctx.accounts.bids.load_mut()?;
    let mut asks = ctx.accounts.asks.load_mut()?;
    let mut event_heap = ctx.accounts.event_heap.load_mut()?;
    let open_orders = &mut ctx.accounts.open_orders;

    // Determine which book to match against and which to post to
    let (matching_book, posting_book) = if params.side == OrderSideV2::Buy {
        (&mut *asks, &mut *bids)
    } else {
        (&mut *bids, &mut *asks)
    };

    let mut remaining_quantity = params.quantity;
    let mut total_filled = 0u64;
    let mut total_cost = 0u64;
    let mut matches_count = 0usize;
    let mut expired_cleanup = 0usize;

    // Track orders to remove after matching (to avoid iterator invalidation)
    let mut orders_to_remove: Vec<u128> = Vec::with_capacity(MAX_MATCHES);
    let mut orders_to_update: Vec<(u128, u64)> = Vec::with_capacity(MAX_MATCHES);

    // Match against opposing orders
    loop {
        if remaining_quantity == 0 || matches_count >= limit {
            break;
        }

        let best = match matching_book.get_best() {
            Some(order) => order,
            None => break,
        };

        // Skip and remove expired orders
        if best.timestamp > 0 {
            // Check expiry based on time_in_force if implemented
            // For now, just match
        }

        // Check if price is acceptable
        let maker_price = best.price();
        if !is_price_acceptable(params.side, params.price, maker_price) {
            break;
        }

        // Post-only check
        if params.order_type == OrderTypeV2::PostOnly {
            // Would match, so fail
            return Ok(PlaceOrderResult {
                order_id: None,
                posted_quantity: 0,
                filled_quantity: 0,
                total_cost: 0,
            });
        }

        // Calculate fill quantity
        let fill_quantity = remaining_quantity.min(best.quantity);
        let fill_cost = calculate_cost(fill_quantity, maker_price);

        // Check taker has sufficient funds
        if params.side == OrderSideV2::Buy {
            require!(
                open_orders.collateral_free >= fill_cost,
                OrderBookError::InsufficientCollateral
            );
        } else {
            // Selling: need outcome tokens
            match params.outcome {
                OutcomeV2::Yes => {
                    require!(
                        open_orders.yes_free >= fill_quantity,
                        OrderBookError::InsufficientBalance
                    );
                }
                OutcomeV2::No => {
                    require!(
                        open_orders.no_free >= fill_quantity,
                        OrderBookError::InsufficientBalance
                    );
                }
            }
        }

        // Create fill event
        let fill = FillEvent::new(
            if params.side == OrderSideV2::Buy {
                0
            } else {
                1
            },
            fill_quantity == best.quantity, // maker_out
            best.owner_slot,
            now,
            event_heap.seq_num,
            best.owner,
            open_orders.owner,
            maker_price,
            fill_quantity,
            best.client_order_id,
            params.client_order_id,
            if params.outcome == OutcomeV2::Yes {
                0
            } else {
                1
            },
        );
