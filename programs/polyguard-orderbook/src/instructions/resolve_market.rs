use anchor_lang::prelude::*;

use crate::errors::OrderBookError;
use crate::state::{
    ComparisonOp, OracleConfig, OraclePrice, OracleType, OrderBookConfig, ResolutionOutcome,
};

/// Maximum staleness for oracle data in slots (~400ms per slot)
pub const DEFAULT_MAX_STALENESS_SLOTS: u64 = 150; // ~60 seconds

/// Switchboard decimal precision
pub const SWITCHBOARD_DECIMALS: u32 = 18;

/// Market state for resolution (simplified - would reference actual market account)
#[account]
pub struct MarketV2 {
    /// Market authority
    pub authority: Pubkey,

    /// Oracle feed pubkey
    pub oracle_feed: Pubkey,

    /// Oracle configuration
    pub oracle_config: OracleConfig,

    /// Current status (0 = Active, 1 = Closed, 2 = Resolved)
    pub status: u8,

    /// Resolved outcome (0 = Unresolved, 1 = Yes, 2 = No, 3 = Invalid)
    pub resolved_outcome: u8,

    /// Resolution timestamp
    pub resolved_at: i64,

    /// Resolution price low 64 bits
    pub resolution_price_lo: u64,

    /// Resolution price high 64 bits (signed)
    pub resolution_price_hi: i64,

    /// Trading end timestamp
    pub trading_end: i64,

    /// Resolution deadline timestamp
    pub resolution_deadline: i64,

    /// Bump seed
    pub bump: u8,

    /// Padding
    pub _padding: [u8; 7],
}

impl anchor_lang::Space for MarketV2 {
    // 32 + 32 + 36 + 1 + 1 + 8 + 8 + 8 + 8 + 8 + 1 + 7 = 150 bytes
    const INIT_SPACE: usize = 32 + 32 + OracleConfig::SIZE + 1 + 1 + 8 + 8 + 8 + 8 + 8 + 1 + 7;
}

impl MarketV2 {
    pub const SEED_PREFIX: &'static [u8] = b"market_v2";

    pub fn is_resolved(&self) -> bool {
        self.status == 2
    }

    pub fn can_resolve(&self, current_time: i64) -> bool {
        self.status == 1 && current_time >= self.resolution_deadline
    }

    pub fn get_resolution_price(&self) -> i128 {
        ((self.resolution_price_hi as i128) << 64) | (self.resolution_price_lo as i128)
    }

    pub fn set_resolution_price(&mut self, price: i128) {
        self.resolution_price_lo = price as u64;
        self.resolution_price_hi = (price >> 64) as i64;
    }
}

#[derive(Accounts)]
pub struct ResolveMarket<'info> {
    /// Anyone can resolve once conditions are met
    #[account(mut)]
    pub resolver: Signer<'info>,

    #[account(
        seeds = [OrderBookConfig::SEED_PREFIX],
        bump = config.bump,
    )]
    pub config: Account<'info, OrderBookConfig>,

    #[account(
        mut,
        seeds = [MarketV2::SEED_PREFIX, market.oracle_feed.as_ref()],
        bump = market.bump,
        constraint = !market.is_resolved() @ OrderBookError::MarketAlreadyResolved,
    )]
    pub market: Account<'info, MarketV2>,

    /// CHECK: Switchboard pull feed account - validated in handler
    #[account(
        constraint = oracle_feed.key() == market.oracle_feed @ OrderBookError::OracleFeedInvalid
    )]
    pub oracle_feed: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ResolveMarketResult {
    pub outcome: u8,
    pub price: i128,
    pub timestamp: i64,
}

pub fn handler(ctx: Context<ResolveMarket>) -> Result<ResolveMarketResult> {
    let market = &mut ctx.accounts.market;
    let clock = Clock::get()?;

    // Verify market can be resolved
    require!(
        market.can_resolve(clock.unix_timestamp),
        OrderBookError::MarketNotReadyForResolution
    );

    // Read oracle price based on oracle type
    let oracle_price = match market.oracle_config.get_oracle_type() {
        OracleType::Switchboard => {
            read_switchboard_price(&ctx.accounts.oracle_feed, clock.slot)?
        }
        OracleType::Manual => {
            // Manual resolution requires authority signature
            return Err(OrderBookError::OracleFeedInvalid.into());
        }
        _ => {
            return Err(OrderBookError::OracleFeedInvalid.into());
        }
    };

    // Check staleness
    let max_staleness = if market.oracle_config.max_staleness > 0 {
        market.oracle_config.max_staleness as u64
    } else {
        DEFAULT_MAX_STALENESS_SLOTS
    };

    require!(
        !oracle_price.is_stale(clock.slot, max_staleness),
        OrderBookError::OracleFeedStale
    );

    // Check confidence if configured
    if market.oracle_config.max_confidence > 0 {
        require!(
            oracle_price.confidence <= market.oracle_config.max_confidence,
            OrderBookError::OraclePriceOutOfRange
        );
    }

    // Determine outcome based on threshold
    let outcome = if market.oracle_config.evaluate_threshold(oracle_price.price) {
        ResolutionOutcome::Yes
    } else {
        ResolutionOutcome::No
    };

    // Update market state
    market.status = 2; // Resolved
    market.resolved_outcome = outcome as u8;
    market.resolved_at = clock.unix_timestamp;
    market.set_resolution_price(oracle_price.price);

    emit!(MarketResolved {
        market: market.key(),
        outcome: outcome as u8,
        price: oracle_price.price,
        timestamp: clock.unix_timestamp,
        resolver: ctx.accounts.resolver.key(),
    });

