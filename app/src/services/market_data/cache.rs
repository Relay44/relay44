//! Redis-backed caches for L2 events.
//!
//! Two views of the same stream:
//! - `r44:l2:top:{venue}:{market_key}`  — best bid/ask, sized for hot-path reads.
//! - `r44:l2:book:{venue}:{market_key}` — full snapshot (bids+asks), for depth consumers.
//!
//! Both have a 30s sliding TTL; a silent venue expires and callers fall back
//! to direct fetches.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{L2Level, Venue};
use crate::services::RedisService;

pub const TOP_OF_BOOK_TTL_SECS: u64 = 30;
pub const FULL_BOOK_TTL_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopOfBook {
    pub best_bid: Option<L2Level>,
    pub best_ask: Option<L2Level>,
    pub last_trade: Option<f64>,
    pub seq: u64,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullBook {
    pub bids: Vec<L2Level>,
    pub asks: Vec<L2Level>,
    pub last_trade: Option<f64>,
    pub seq: u64,
    pub observed_at: DateTime<Utc>,
}

fn top_key(venue: Venue, market_key: &str) -> String {
    format!("r44:l2:top:{}:{}", venue.as_str(), market_key)
}

fn book_key(venue: Venue, market_key: &str) -> String {
    format!("r44:l2:book:{}:{}", venue.as_str(), market_key)
}

pub async fn write_top(
    redis: &RedisService,
    venue: Venue,
    market_key: &str,
    top: &TopOfBook,
) -> Result<()> {
    redis
        .set(&top_key(venue, market_key), top, Some(TOP_OF_BOOK_TTL_SECS))
        .await
}

pub async fn read_top(
    redis: &RedisService,
    venue: Venue,
    market_key: &str,
) -> Result<Option<TopOfBook>> {
    redis.get(&top_key(venue, market_key)).await
}

pub async fn write_book(
    redis: &RedisService,
    venue: Venue,
    market_key: &str,
    book: &FullBook,
) -> Result<()> {
    redis
        .set(&book_key(venue, market_key), book, Some(FULL_BOOK_TTL_SECS))
        .await
}

pub async fn read_book(
    redis: &RedisService,
    venue: Venue,
    market_key: &str,
) -> Result<Option<FullBook>> {
    redis.get(&book_key(venue, market_key)).await
}
