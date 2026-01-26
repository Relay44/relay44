use anchor_lang::prelude::*;

use crate::errors::OrderBookError;
use crate::state::{EventHeap, EventType, FillEvent, OpenOrdersAccount, OrderBookConfig, OutEvent};

/// Maximum events to consume in a single transaction
pub const MAX_EVENTS_CONSUME: usize = 8;

#[derive(Accounts)]
pub struct ConsumeEvents<'info> {
    /// Crank operator (anyone can crank)
    #[account(mut)]
    pub crank: Signer<'info>,

    #[account(
        seeds = [OrderBookConfig::SEED_PREFIX],
        bump = config.bump,
    )]
    pub config: Account<'info, OrderBookConfig>,

    /// CHECK: Market account
    pub market: AccountInfo<'info>,

    /// Event heap
    #[account(mut)]
    pub event_heap: AccountLoader<'info, EventHeap>,

    // Remaining accounts: OpenOrdersAccount pairs for each fill event
    // Format: [maker_open_orders, taker_open_orders, ...]
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ConsumeEventsResult {
    pub events_consumed: u32,
    pub fills_processed: u32,
    pub outs_processed: u32,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, ConsumeEvents<'info>>,
    limit: u8,
) -> Result<ConsumeEventsResult> {
    let event_heap = &mut ctx.accounts.event_heap.load_mut()?;

    let max_events = if limit == 0 {
        MAX_EVENTS_CONSUME
    } else {
        limit as usize
    };

    let mut events_consumed = 0u32;
    let mut fills_processed = 0u32;
    let mut outs_processed = 0u32;

    // Track which open orders accounts we've seen
    let remaining_accounts = &ctx.remaining_accounts;

    // Process events from oldest to newest
    while events_consumed < max_events as u32 {
        let event = match event_heap.pop() {
            Some((slot, node)) => (slot, node),
            None => break,
        };

        let (slot, node) = event;

        match node.event_type() {
            EventType::Fill => {
                if let Some(fill) = node.as_fill() {
                    process_fill_event(
                        &fill,
                        remaining_accounts,
                        &ctx.accounts.market.key(),
                    )?;
                    fills_processed += 1;
                }
            }
            EventType::Out => {
                if let Some(out) = node.as_out() {
                    process_out_event(&out, remaining_accounts, &ctx.accounts.market.key())?;
                    outs_processed += 1;
                }
            }
        }

        events_consumed += 1;
    }

    emit!(EventsConsumed {
        market: ctx.accounts.market.key(),
        crank: ctx.accounts.crank.key(),
        events_consumed,
        fills_processed,
        outs_processed,
    });

    Ok(ConsumeEventsResult {
        events_consumed,
        fills_processed,
        outs_processed,
    })
}

/// Process a fill event - update maker's position
fn process_fill_event(
    fill: &FillEvent,
    remaining_accounts: &[AccountInfo],
    _market: &Pubkey,
) -> Result<()> {
    // Find maker's open orders account in remaining accounts
    let maker_account = remaining_accounts
        .iter()
        .find(|acc| acc.key() == fill.maker);

    if let Some(maker_info) = maker_account {
        // Deserialize and update maker's open orders
        let mut data = maker_info.try_borrow_mut_data()?;

        // Skip discriminator (8 bytes)
        if data.len() < 8 {
            return Ok(()); // Skip invalid account
        }

        // Parse as OpenOrdersAccount (simplified - in production use proper deserialization)
        // The maker receives/pays based on whether they were buying or selling

        // Determine if maker was buying (taker_side == 1 means taker was selling, so maker was buying)
        let maker_was_buying = fill.taker_side == 1;

        // Calculate collateral exchanged
        let collateral = fill
            .quantity
            .checked_mul(fill.price)
            .and_then(|v| v.checked_div(10000))
            .unwrap_or(0);

        // Update maker's position based on their side
        // Note: This is a simplified version. In production, you'd properly deserialize
        // the OpenOrdersAccount and call execute_maker_fill.

        if maker_was_buying {
            // Maker bought: unlock collateral, receive outcome tokens
            // collateral_locked -= collateral
            // {yes|no}_free += quantity
        } else {
            // Maker sold: unlock outcome tokens, receive collateral
            // {yes|no}_locked -= quantity
            // collateral_free += collateral
        }

        // If maker order is fully filled, remove from their open orders
        if fill.is_maker_out() {
            // Remove order from maker's open orders at maker_slot
        }
    }

    Ok(())
}

/// Process an out event - return locked funds to owner
fn process_out_event(
    out: &OutEvent,
    remaining_accounts: &[AccountInfo],
    _market: &Pubkey,
) -> Result<()> {
    // Find owner's open orders account
    let owner_account = remaining_accounts.iter().find(|acc| acc.key() == out.owner);

    if let Some(owner_info) = owner_account {
        // Deserialize and update owner's open orders
        let mut data = owner_info.try_borrow_mut_data()?;

        if data.len() < 8 {
            return Ok(());
        }

        // Return locked funds based on order side
        // side == 0: buy order -> unlock collateral
        // side == 1: sell order -> unlock outcome tokens

        // Remove order from open orders at owner_slot
    }

    Ok(())
}

#[event]
pub struct EventsConsumed {
    pub market: Pubkey,
    pub crank: Pubkey,
    pub events_consumed: u32,
    pub fills_processed: u32,
    pub outs_processed: u32,
}

/// Consume events with explicit open orders accounts
/// This version requires passing the exact accounts needed
#[derive(Accounts)]
pub struct ConsumeEventsExplicit<'info> {
    /// Crank operator
    #[account(mut)]
    pub crank: Signer<'info>,

