use anchor_lang::prelude::*;

/// Maximum markets an agent can whitelist
pub const MAX_ALLOWED_MARKETS: usize = 16;

/// Position sizing strategy
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum PositionSizing {
    /// Fixed size per trade
    #[default]
    Fixed = 0,
    /// Kelly criterion fraction (scaled by 10000)
    Kelly = 1,
    /// Proportional to bankroll (risk_bps per trade)
    Proportional = 2,
}

/// Risk parameters for the agent
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
#[repr(C)]
pub struct RiskParams {
    /// Maximum drawdown before stopping (basis points)
    pub max_drawdown_bps: u16,

    /// Maximum daily loss in collateral units
    pub max_daily_loss: u64,

    /// Minimum probability edge to trade (basis points)
    pub min_edge_bps: u16,

    /// Position sizing strategy
    pub position_sizing: u8,

    /// Position sizing parameter (meaning depends on strategy)
    /// - Fixed: absolute size
    /// - Kelly: fraction * 10000
    /// - Proportional: risk_bps per trade
    pub sizing_param: u64,

    /// Padding
    pub _padding: [u8; 5],
}

impl anchor_lang::Space for RiskParams {
    const INIT_SPACE: usize = 2 + 8 + 2 + 1 + 8 + 5; // 26 bytes
}

impl RiskParams {
    pub const SIZE: usize = 2 + 8 + 2 + 1 + 8 + 5;

    pub fn get_sizing_strategy(&self) -> PositionSizing {
        match self.position_sizing {
            0 => PositionSizing::Fixed,
            1 => PositionSizing::Kelly,
            2 => PositionSizing::Proportional,
            _ => PositionSizing::Fixed,
        }
    }

    /// Calculate position size based on strategy
    pub fn calculate_size(&self, bankroll: u64, edge_bps: u16, win_prob_bps: u16) -> u64 {
        match self.get_sizing_strategy() {
            PositionSizing::Fixed => self.sizing_param,
            PositionSizing::Kelly => {
                // Kelly fraction = (p*b - q) / b where b = odds, p = win prob, q = 1-p
                // Simplified: fraction * edge / 10000
                let fraction = self.sizing_param;
                let edge = edge_bps as u64;
                bankroll
                    .checked_mul(fraction)
                    .and_then(|v| v.checked_mul(edge))
                    .and_then(|v| v.checked_div(10000 * 10000))
                    .unwrap_or(0)
            }
            PositionSizing::Proportional => {
                // Risk a fixed percentage of bankroll
                let risk_bps = self.sizing_param;
                bankroll
                    .checked_mul(risk_bps)
                    .and_then(|v| v.checked_div(10000))
                    .unwrap_or(0)
            }
        }
    }
}

/// Agent status
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum AgentStatus {
    /// Agent is active and can trade
    #[default]
    Active = 0,
    /// Agent is paused (manual or risk limit)
    Paused = 1,
    /// Agent is stopped permanently
    Stopped = 2,
}

/// Trading agent account
#[account]
#[derive(InitSpace)]
pub struct TradingAgent {
    /// Owner of the agent (can withdraw, update params)
    pub owner: Pubkey,

    /// Delegate authorized to execute trades
    pub delegate: Pubkey,

    /// Agent name (max 32 bytes)
    #[max_len(32)]
    pub name: String,

    /// Bump seed
    pub bump: u8,

    /// Agent status
    pub status: u8,

    /// Version
    pub version: u8,

    /// Padding
    pub _padding: [u8; 1],

    // === Constraints ===
    /// Maximum position size per market
    pub max_position_size: u64,

    /// Maximum total exposure across all markets
    pub max_total_exposure: u64,

    /// Risk parameters
    pub risk_params: RiskParams,

    // === Balances ===
    /// Total collateral deposited
    pub total_deposited: u64,

    /// Current available balance
    pub available_balance: u64,

    /// Total locked in positions
    pub locked_balance: u64,

    // === Performance ===
    /// Total realized PnL (can be negative, stored as i64)
    pub total_pnl: i64,

    /// High water mark for drawdown calculation
    pub high_water_mark: u64,

    /// Current drawdown from high water mark
    pub current_drawdown: u64,

    /// Daily loss tracking (reset daily)
    pub daily_loss: u64,

    /// Last day tracked (unix timestamp / 86400)
    pub last_day: u64,

    // === Statistics ===
    /// Number of active positions
    pub active_positions: u16,

    /// Total trades executed
    pub trades_count: u64,

    /// Winning trades
    pub win_count: u64,

    /// Total volume traded
    pub volume_traded: u64,

    // === Timestamps ===
    /// Creation timestamp
    pub created_at: i64,

    /// Last trade timestamp
    pub last_trade_at: i64,

    // === Market whitelist ===
    /// Number of whitelisted markets (0 = all allowed)
    pub allowed_markets_count: u8,

    /// Reserved
    pub _reserved: [u8; 7],

    /// Whitelisted markets (if count > 0)
    #[max_len(16)]
    pub allowed_markets: Vec<Pubkey>,
}

impl TradingAgent {
    pub const SEED_PREFIX: &'static [u8] = b"trading_agent";

    pub fn get_status(&self) -> AgentStatus {
        match self.status {
            0 => AgentStatus::Active,
            1 => AgentStatus::Paused,
            2 => AgentStatus::Stopped,
            _ => AgentStatus::Stopped,
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == AgentStatus::Active as u8
    }

    /// Check if market is allowed for this agent
    pub fn is_market_allowed(&self, market: &Pubkey) -> bool {
        if self.allowed_markets_count == 0 {
            return true; // All markets allowed
        }
        self.allowed_markets.iter().any(|m| m == market)
    }

    /// Check if trade passes risk checks
    pub fn check_risk(&self, size: u64, current_price: u64) -> Result<()> {
        // Check position size limit
        require!(
            size <= self.max_position_size,
            AgentError::PositionSizeExceeded
        );

        // Check total exposure
        let new_exposure = self.locked_balance.saturating_add(size);
        require!(
            new_exposure <= self.max_total_exposure,
            AgentError::ExposureLimitExceeded
        );
