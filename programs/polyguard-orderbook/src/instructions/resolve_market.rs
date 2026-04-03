use anchor_lang::prelude::*;

use crate::errors::OrderBookError;
use crate::state::{
    ComparisonOp, OracleConfig, OraclePrice, OracleType, OrderBookConfig, ResolutionOutcome,
};

/// Maximum staleness for oracle data in slots (~400ms per slot)
pub const DEFAULT_MAX_STALENESS_SLOTS: u64 = 150; // ~60 seconds

/// Switchboard decimal precision
pub const SWITCHBOARD_DECIMALS: u32 = 18;

/// Internal price precision (18 decimals)
pub const PRICE_DECIMALS: i32 = 18;

/// Pyth Solana Receiver program (owns PriceUpdateV2 accounts)
pub const PYTH_RECEIVER_PROGRAM_ID: Pubkey =
    anchor_lang::solana_program::pubkey!("rec5EKMGg6MxZYo2VKDBb7Gth8N8wdjqeeHYYZHdCN8");

/// Pyth PriceUpdateV2 account layout offsets (after 8-byte anchor discriminator)
mod pyth_offsets {
    pub const DISCRIMINATOR: usize = 0;
    pub const DISCRIMINATOR_LEN: usize = 8;
    pub const PRICE: usize = 73;        // i64 — raw price
    pub const CONF: usize = 81;         // u64 — confidence interval
    pub const EXPONENT: usize = 89;     // i32 — price exponent
    pub const PUBLISH_TIME: usize = 93;  // i64 — unix timestamp
    pub const POSTED_SLOT: usize = 125;  // u64 — slot when posted
    pub const MIN_ACCOUNT_LEN: usize = 133;
}

/// Acceptable exponent range for Pyth feeds (typically -5 to -12 for crypto)
const PYTH_MIN_EXPONENT: i32 = -18;
const PYTH_MAX_EXPONENT: i32 = 0;

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

    /// CHECK: Oracle feed account (Switchboard or Pyth) - validated in handler
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
        OracleType::Pyth => {
            read_pyth_price(&ctx.accounts.oracle_feed, clock.slot)?
        }
        OracleType::Manual => {
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

    Ok(ResolveMarketResult {
        outcome: outcome as u8,
        price: oracle_price.price,
        timestamp: clock.unix_timestamp,
    })
}

/// Read price from Switchboard pull feed
fn read_switchboard_price(feed_account: &AccountInfo, _current_slot: u64) -> Result<OraclePrice> {
    // Switchboard on-demand uses a specific account structure
    // The feed data is stored in the account data after a discriminator

    let data = feed_account.try_borrow_data()?;

    // Minimum data length check
    if data.len() < 128 {
        return Err(OrderBookError::OracleFeedInvalid.into());
    }

    // Switchboard pull feed structure (simplified):
    // - 8 bytes: discriminator
    // - 32 bytes: queue pubkey
    // - 8 bytes: created_at
    // - 16 bytes: result (i128)
    // - 8 bytes: max_variance
    // - 4 bytes: min_responses
    // - ... more fields
    // - 8 bytes: last_update_slot

    // Read the result value (i128 at offset ~48-64, varies by version)
    // For switchboard-on-demand, we read the aggregated result

    // Note: In production, use the official switchboard-on-demand crate's
    // deserialization. This is a simplified example.

    // Read price from typical offset
    let price_offset = 48;
    let price_bytes: [u8; 16] = data[price_offset..price_offset + 16]
        .try_into()
        .map_err(|_| OrderBookError::OracleFeedInvalid)?;
    let price = i128::from_le_bytes(price_bytes);

    // Read slot from end of commonly used area
    let slot_offset = data.len().saturating_sub(16);
    let slot_bytes: [u8; 8] = data[slot_offset..slot_offset + 8]
        .try_into()
        .map_err(|_| OrderBookError::OracleFeedInvalid)?;
    let slot = u64::from_le_bytes(slot_bytes);

    // Read timestamp (typically 8 bytes before slot)
    let ts_offset = slot_offset.saturating_sub(8);
    let ts_bytes: [u8; 8] = data[ts_offset..ts_offset + 8]
        .try_into()
        .map_err(|_| OrderBookError::OracleFeedInvalid)?;
    let timestamp = i64::from_le_bytes(ts_bytes);

    // Confidence is typically stored nearby
    let confidence = 0u64; // Simplified

    Ok(OraclePrice {
        price,
        confidence,
        slot,
        timestamp,
    })
}

/// Read price from Pyth pull oracle (PriceUpdateV2 account)
fn read_pyth_price(feed_account: &AccountInfo, _current_slot: u64) -> Result<OraclePrice> {
    require!(
        feed_account.owner == &PYTH_RECEIVER_PROGRAM_ID,
        OrderBookError::OracleFeedInvalid
    );

    let data = feed_account.try_borrow_data()?;

    if data.len() < pyth_offsets::MIN_ACCOUNT_LEN {
        return Err(OrderBookError::OracleFeedInvalid.into());
    }

    let raw_price = read_i64(&data, pyth_offsets::PRICE)?;
    let raw_conf = read_u64(&data, pyth_offsets::CONF)?;
    let exponent = read_i32(&data, pyth_offsets::EXPONENT)?;
    let timestamp = read_i64(&data, pyth_offsets::PUBLISH_TIME)?;
    let posted_slot = read_u64(&data, pyth_offsets::POSTED_SLOT)?;

    require!(raw_price > 0, OrderBookError::OraclePriceOutOfRange);
    require!(
        exponent >= PYTH_MIN_EXPONENT && exponent <= PYTH_MAX_EXPONENT,
        OrderBookError::OracleFeedInvalid
    );

    let price = normalize_pyth_price(raw_price as i128, exponent)?;
    let confidence = normalize_pyth_confidence(raw_conf, exponent);

    Ok(OraclePrice {
        price,
        confidence,
        slot: posted_slot,
        timestamp,
    })
}

fn normalize_pyth_price(raw: i128, exponent: i32) -> Result<i128> {
    let scale_exp = PRICE_DECIMALS + exponent;
    if scale_exp >= 0 {
        let scale = 10i128
            .checked_pow(scale_exp as u32)
            .ok_or(OrderBookError::OraclePriceOutOfRange)?;
        raw.checked_mul(scale)
            .ok_or_else(|| OrderBookError::OraclePriceOutOfRange.into())
    } else {
        let divisor = 10i128
            .checked_pow((-scale_exp) as u32)
            .ok_or(OrderBookError::OraclePriceOutOfRange)?;
        Ok(raw / divisor)
    }
}

fn normalize_pyth_confidence(raw: u64, exponent: i32) -> u64 {
    let scale_exp = PRICE_DECIMALS + exponent;
    if scale_exp >= 0 {
        let scale = match 10u128.checked_pow(scale_exp as u32) {
            Some(s) => s,
            None => return u64::MAX,
        };
        match (raw as u128).checked_mul(scale) {
            Some(v) if v <= u64::MAX as u128 => v as u64,
            _ => u64::MAX,
        }
    } else {
        let divisor = match 10u128.checked_pow((-scale_exp) as u32) {
            Some(d) => d,
            None => return 0,
        };
        ((raw as u128) / divisor) as u64
    }
}

fn read_i64(data: &[u8], offset: usize) -> Result<i64> {
    let end = offset.checked_add(8).ok_or(OrderBookError::OracleFeedInvalid)?;
    let bytes: [u8; 8] = data.get(offset..end)
        .ok_or(OrderBookError::OracleFeedInvalid)?
        .try_into()
        .map_err(|_| OrderBookError::OracleFeedInvalid)?;
    Ok(i64::from_le_bytes(bytes))
}

fn read_u64(data: &[u8], offset: usize) -> Result<u64> {
    let end = offset.checked_add(8).ok_or(OrderBookError::OracleFeedInvalid)?;
    let bytes: [u8; 8] = data.get(offset..end)
        .ok_or(OrderBookError::OracleFeedInvalid)?
        .try_into()
        .map_err(|_| OrderBookError::OracleFeedInvalid)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_i32(data: &[u8], offset: usize) -> Result<i32> {
    let end = offset.checked_add(4).ok_or(OrderBookError::OracleFeedInvalid)?;
    let bytes: [u8; 4] = data.get(offset..end)
        .ok_or(OrderBookError::OracleFeedInvalid)?
        .try_into()
        .map_err(|_| OrderBookError::OracleFeedInvalid)?;
    Ok(i32::from_le_bytes(bytes))
}

/// Manual resolution by authority
#[derive(Accounts)]
pub struct ResolveMarketManual<'info> {
    #[account(
        mut,
        constraint = authority.key() == market.authority @ OrderBookError::UnauthorizedAdmin
    )]
    pub authority: Signer<'info>,

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
        constraint = market.oracle_config.get_oracle_type() == OracleType::Manual @ OrderBookError::OracleFeedInvalid,
    )]
    pub market: Account<'info, MarketV2>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct ManualResolutionParams {
    pub outcome: u8, // 1 = Yes, 2 = No
}

pub fn handler_manual(
    ctx: Context<ResolveMarketManual>,
    params: ManualResolutionParams,
) -> Result<()> {
    let market = &mut ctx.accounts.market;
    let clock = Clock::get()?;

    // Validate outcome
    require!(
        params.outcome == 1 || params.outcome == 2,
        OrderBookError::InvalidResolutionOutcome
    );

    // Update market state
    market.status = 2; // Resolved
    market.resolved_outcome = params.outcome;
    market.resolved_at = clock.unix_timestamp;
    market.set_resolution_price(0); // No oracle price for manual

    emit!(MarketResolved {
        market: market.key(),
        outcome: params.outcome,
        price: 0,
        timestamp: clock.unix_timestamp,
        resolver: ctx.accounts.authority.key(),
    });

    Ok(())
}

#[event]
pub struct MarketResolved {
    pub market: Pubkey,
    pub outcome: u8,
    pub price: i128,
    pub timestamp: i64,
    pub resolver: Pubkey,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolution_outcome_conversion() {
        assert_eq!(ResolutionOutcome::from(0), ResolutionOutcome::Unresolved);
        assert_eq!(ResolutionOutcome::from(1), ResolutionOutcome::Yes);
        assert_eq!(ResolutionOutcome::from(2), ResolutionOutcome::No);
        assert_eq!(ResolutionOutcome::from(3), ResolutionOutcome::Invalid);
        assert_eq!(ResolutionOutcome::from(99), ResolutionOutcome::Unresolved);
    }

    #[test]
    fn test_pyth_config_constructor() {
        let threshold = 50_000_000_000_000_000_000_000i128;
        let config = OracleConfig::new_pyth(
            threshold,
            ComparisonOp::GreaterThan,
            300,
            1_000_000_000_000_000_000,
        );
        assert_eq!(config.get_oracle_type(), OracleType::Pyth);
        assert_eq!(config.get_threshold(), threshold);
        assert_eq!(config.max_confidence, 1_000_000_000_000_000_000);
        assert_eq!(config.max_staleness, 300);
    }

    // -- normalize_pyth_price --

    #[test]
    fn test_normalize_btc_exp_neg8() {
        // BTC $65,432.12, exponent -8 → raw = 6_543_212_000_000
        let price = normalize_pyth_price(6_543_212_000_000, -8).unwrap();
        assert_eq!(price, 65_432_120_000_000_000_000_000i128);
    }

    #[test]
    fn test_normalize_eth_exp_neg8() {
        // ETH $3,200.50, exponent -8 → raw = 320_050_000_000
        let price = normalize_pyth_price(320_050_000_000, -8).unwrap();
        assert_eq!(price, 3_200_500_000_000_000_000_000i128);
    }

    #[test]
    fn test_normalize_sol_exp_neg5() {
        // SOL $145.23, exponent -5 → raw = 14_523_000
        let price = normalize_pyth_price(14_523_000, -5).unwrap();
        // 14_523_000 * 10^13 = 145_230_000_000_000_000_000
        assert_eq!(price, 145_230_000_000_000_000_000i128);
    }

    #[test]
    fn test_normalize_exp_neg18() {
        // Edge: exponent == -18 → scale_exp = 0, no scaling
        let price = normalize_pyth_price(42, -18).unwrap();
        assert_eq!(price, 42);
    }

    #[test]
    fn test_normalize_exp_zero() {
        // exponent = 0 → scale_exp = 18
        let price = normalize_pyth_price(100, 0).unwrap();
        assert_eq!(price, 100_000_000_000_000_000_000i128);
    }

    // -- normalize_pyth_confidence --

    #[test]
    fn test_confidence_scales_with_price() {
        // conf = 500 with exp -8 → 500 * 10^10 = 5_000_000_000_000
        let conf = normalize_pyth_confidence(500, -8);
        assert_eq!(conf, 5_000_000_000_000);
    }

    #[test]
    fn test_confidence_saturates_on_overflow() {
        let conf = normalize_pyth_confidence(u64::MAX, 0);
        assert_eq!(conf, u64::MAX);
    }

    #[test]
    fn test_confidence_zero() {
        assert_eq!(normalize_pyth_confidence(0, -8), 0);
    }

    // -- threshold evaluation --

    #[test]
    fn test_oracle_config_threshold() {
        let threshold = 50_000_000_000_000_000_000_000i128;
        let config = OracleConfig::new_switchboard(threshold, ComparisonOp::GreaterThan, 150);

        assert!(config.evaluate_threshold(55_000_000_000_000_000_000_000i128));
        assert!(!config.evaluate_threshold(45_000_000_000_000_000_000_000i128));
        assert!(!config.evaluate_threshold(50_000_000_000_000_000_000_000i128));
    }

    #[test]
    fn test_pyth_threshold_with_confidence_gate() {
        let config = OracleConfig::new_pyth(
            50_000_000_000_000_000_000_000i128,
            ComparisonOp::GreaterThan,
            300,
            1_000_000_000_000_000_000, // max $1 confidence
        );

        // price passes, confidence within limit
        let price = OraclePrice {
            price: 55_000_000_000_000_000_000_000i128,
            confidence: 500_000_000_000_000_000, // $0.50
            slot: 100,
            timestamp: 1000,
        };
        assert!(config.evaluate_threshold(price.price));
        assert!(price.confidence <= config.max_confidence);

        // price passes, confidence too wide → should be rejected by caller
        let noisy = OraclePrice {
            price: 55_000_000_000_000_000_000_000i128,
            confidence: 5_000_000_000_000_000_000, // $5
            slot: 100,
            timestamp: 1000,
        };
        assert!(noisy.confidence > config.max_confidence);
    }

    // -- read helpers --

    #[test]
    fn test_read_i64() {
        let val: i64 = -12345;
        let data = val.to_le_bytes();
        assert_eq!(read_i64(&data, 0).unwrap(), val);
    }

    #[test]
    fn test_read_u64() {
        let val: u64 = 9999999;
        let data = val.to_le_bytes();
        assert_eq!(read_u64(&data, 0).unwrap(), val);
    }

    #[test]
    fn test_read_i32() {
        let val: i32 = -8;
        let data = val.to_le_bytes();
        assert_eq!(read_i32(&data, 0).unwrap(), val);
    }

    // -- full byte-level parsing --

    fn build_pyth_account_data(price: i64, conf: u64, exponent: i32, publish_time: i64, posted_slot: u64) -> Vec<u8> {
        let mut data = vec![0u8; pyth_offsets::MIN_ACCOUNT_LEN];
        data[pyth_offsets::PRICE..pyth_offsets::PRICE + 8].copy_from_slice(&price.to_le_bytes());
        data[pyth_offsets::CONF..pyth_offsets::CONF + 8].copy_from_slice(&conf.to_le_bytes());
        data[pyth_offsets::EXPONENT..pyth_offsets::EXPONENT + 4].copy_from_slice(&exponent.to_le_bytes());
        data[pyth_offsets::PUBLISH_TIME..pyth_offsets::PUBLISH_TIME + 8].copy_from_slice(&publish_time.to_le_bytes());
        data[pyth_offsets::POSTED_SLOT..pyth_offsets::POSTED_SLOT + 8].copy_from_slice(&posted_slot.to_le_bytes());
        data
    }

    #[test]
    fn test_parse_pyth_fields_from_bytes() {
        let data = build_pyth_account_data(6_543_212_000_000, 500, -8, 1_700_000_000, 42);

        assert_eq!(read_i64(&data, pyth_offsets::PRICE).unwrap(), 6_543_212_000_000);
        assert_eq!(read_u64(&data, pyth_offsets::CONF).unwrap(), 500);
        assert_eq!(read_i32(&data, pyth_offsets::EXPONENT).unwrap(), -8);
        assert_eq!(read_i64(&data, pyth_offsets::PUBLISH_TIME).unwrap(), 1_700_000_000);
        assert_eq!(read_u64(&data, pyth_offsets::POSTED_SLOT).unwrap(), 42);
    }

    #[test]
    fn test_full_normalization_pipeline() {
        // ETH $3,200.50, exp -8, conf 100 ($0.000001)
        let raw_price: i64 = 320_050_000_000;
        let raw_conf: u64 = 100;
        let exp: i32 = -8;

        let price = normalize_pyth_price(raw_price as i128, exp).unwrap();
        let conf = normalize_pyth_confidence(raw_conf, exp);

        assert_eq!(price, 3_200_500_000_000_000_000_000i128);
        assert_eq!(conf, 1_000_000_000_000); // 100 * 10^10
    }

    #[test]
    fn test_account_too_short_rejected() {
        let data = vec![0u8; 50]; // way too short
        assert!(read_u64(&data, pyth_offsets::POSTED_SLOT).is_err());
    }
}
