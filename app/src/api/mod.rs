pub mod agent_service;
pub mod api_key;
pub mod auth;
pub mod compliance;
pub mod copy_trading;
pub mod creator;
pub mod creator_tiers;
pub mod decisions;
pub mod distribution;
pub mod error;
pub mod evm;
pub mod external;
pub mod hackathon;
pub mod health;
pub mod jwt;
pub mod kyc;
pub mod leaderboard;
pub mod markets;
pub mod notifications;
pub mod orders;
pub mod parlays;
pub mod payments;
pub mod pm_scanner;
pub mod positions;
pub mod profiles;
pub mod protocol;
pub mod rate_limit;
pub mod referrals;
pub mod risk_console;
pub mod routing;
pub mod signals;
pub mod social;
pub mod solana;
pub mod user;
pub mod validation;
pub mod wallet;
pub mod web4;
pub mod ws;

pub use error::ApiError;
pub use jwt::JwtService;
pub use rate_limit::check_auth_rate_limit;
pub use validation::{
    validate_market_id, validate_order_price, validate_order_quantity, validate_pagination,
    validate_uuid,
};
pub use ws::ws_handler;

#[allow(unused_imports)]
pub use rate_limit::{
    check_claim_rate_limit, check_market_create_rate_limit, check_order_rate_limit,
    check_order_rate_limit_for, check_write_rate_limit, order_tier_for, read_tier_for,
    RateLimitTier,
};
