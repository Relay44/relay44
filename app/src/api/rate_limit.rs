//! Per-endpoint rate limiting
//!
//! Granular rate limits per endpoint type:
//! - Auth: 10/min (brute force protection)
//! - Order: 10/min per user (DoS protection)
//! - MarketCreate: 1/hour per user (spam protection)
//! - Claim: 5/min per user (economic attack protection)
//! - Write: 30/min (general writes)
//! - Read: 120/min (general reads)

#![allow(dead_code)]

use actix_web::HttpRequest;

use super::ApiError;
use crate::services::RedisService;

/// Rate limit tiers for different endpoint types
#[derive(Clone, Copy)]
pub enum RateLimitTier {
    /// Auth endpoints: 10 requests per minute
    Auth,
    /// Order placement: 10 requests per minute per user
    Order,
    /// Market creation: 1 request per hour per user
    MarketCreate,
    /// Claim winnings: 5 requests per minute per user
    Claim,
    /// General write operations: 30 requests per minute
    Write,
    /// Read operations: 120 requests per minute
    Read,
}

impl RateLimitTier {
    pub fn limit(&self) -> i64 {
        match self {
            RateLimitTier::Auth => 10,
            RateLimitTier::Order => 10,
            RateLimitTier::MarketCreate => 1,
            RateLimitTier::Claim => 5,
            RateLimitTier::Write => 30,
            RateLimitTier::Read => 120,
        }
    }

    pub fn window_secs(&self) -> u64 {
        match self {
            RateLimitTier::MarketCreate => 3600, // 1 hour
            _ => 60,                             // 1 minute
        }
    }

    pub fn key_prefix(&self) -> &'static str {
        match self {
            RateLimitTier::Auth => "rl:auth",
            RateLimitTier::Order => "rl:order",
            RateLimitTier::MarketCreate => "rl:market",
            RateLimitTier::Claim => "rl:claim",
            RateLimitTier::Write => "rl:write",
            RateLimitTier::Read => "rl:read",
        }
    }
}

/// Check rate limit for a request. Call at the start of rate-limited handlers.
pub async fn check_rate_limit(
    req: &HttpRequest,
    redis: &RedisService,
    tier: RateLimitTier,
) -> Result<(), ApiError> {
    let client_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();

    let path = req.path();

    // Build rate limit key using tier prefix and IP
    let key = format!("{}:{}", tier.key_prefix(), client_ip);

    let limit = tier.limit();
    let window = tier.window_secs();

    match redis.increment_rate_limit(&key, window).await {
        Ok((count, _ttl)) => {
            if count > limit {
                log::warn!(
                    "Rate limit exceeded for {} on {} (tier: {:?}, count: {}, limit: {})",
                    client_ip,
                    path,
                    tier.key_prefix(),
                    count,
                    limit
                );
                return Err(ApiError::rate_limited(window));
            }
            Ok(())
        }
        Err(e) => {
            // Fail open to avoid blocking legitimate requests on Redis errors
            log::error!("Rate limit check failed (allowing request): {}", e);
            Ok(())
        }
    }
}

/// Check rate limit by user wallet (for authenticated endpoints)
pub async fn check_rate_limit_by_user(
    wallet: &str,
    redis: &RedisService,
    tier: RateLimitTier,
) -> Result<(), ApiError> {
    let key = format!("{}:user:{}", tier.key_prefix(), wallet);
    let limit = tier.limit();
    let window = tier.window_secs();

    match redis.increment_rate_limit(&key, window).await {
        Ok((count, _ttl)) => {
            if count > limit {
                log::warn!(
                    "Rate limit exceeded for user {} (tier: {:?}, count: {}, limit: {})",
                    wallet,
                    tier.key_prefix(),
                    count,
                    limit
                );
                return Err(ApiError::rate_limited(window));
            }
            Ok(())
        }
        Err(e) => {
            log::error!("Rate limit check failed (allowing request): {}", e);
            Ok(())
        }
    }
}

/// Helper to check auth-tier rate limit
pub async fn check_auth_rate_limit(
    req: &HttpRequest,
    redis: &RedisService,
) -> Result<(), ApiError> {
    check_rate_limit(req, redis, RateLimitTier::Auth).await
}

/// Helper to check write-tier rate limit
pub async fn check_write_rate_limit(
    req: &HttpRequest,
    redis: &RedisService,
) -> Result<(), ApiError> {
    check_rate_limit(req, redis, RateLimitTier::Write).await
}

/// Helper to check order-tier rate limit (10/min per user)
pub async fn check_order_rate_limit(wallet: &str, redis: &RedisService) -> Result<(), ApiError> {
    check_rate_limit_by_user(wallet, redis, RateLimitTier::Order).await
}

/// Helper to check market-creation rate limit (1/hour per user)
pub async fn check_market_create_rate_limit(
    wallet: &str,
    redis: &RedisService,
) -> Result<(), ApiError> {
    check_rate_limit_by_user(wallet, redis, RateLimitTier::MarketCreate).await
}
