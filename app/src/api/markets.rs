use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::sync::Arc;

use super::auth::extract_jwt_user;
use super::jwt::UserRole;
use super::rate_limit::check_market_create_rate_limit;
use super::ApiError;
use crate::models::{
    CreateMarketRequest, ListMarketsQuery, ListTradesQuery, Market, MarketListResponse,
    MarketStatus, OrderBookResponse, Outcome, TradeListResponse,
};
use crate::AppState;

fn ensure_market_read_mode(state: &web::Data<Arc<AppState>>) -> Result<(), ApiError> {
    let evm_reads = state.config.evm_enabled && state.config.evm_reads_enabled;
    let solana_reads = state.config.solana_enabled && state.config.solana_reads_enabled;
    if !evm_reads && !solana_reads {
        return Err(ApiError::bad_request(
            "CHAIN_READ_PATH_DISABLED",
            "Market read path is disabled for all configured chains",
        ));
    }
    Ok(())
}

fn derive_market_address(market_id: &str) -> String {
    let digest = Sha256::digest(market_id.as_bytes());
    format!("0x{}", hex::encode(&digest[..20]))
}

/// List all markets with filtering
pub async fn list_markets(
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListMarketsQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_market_read_mode(&state)?;

    let status = query.status.as_ref().map(|s| match s.as_str() {
        "active" => MarketStatus::Active,
        "closed" => MarketStatus::Closed,
        "resolved" => MarketStatus::Resolved,
        "paused" => MarketStatus::Paused,
        "cancelled" => MarketStatus::Cancelled,
        _ => MarketStatus::Active,
    });

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);

    let (markets, total) = state
        .db
        .get_markets(status, query.category.as_deref(), limit, offset)
        .await
        .map_err(ApiError::from)?;

    Ok(HttpResponse::Ok().json(MarketListResponse {
        markets,
        total,
        limit,
        offset,
    }))
}

/// Get a single market by ID
pub async fn get_market(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    ensure_market_read_mode(&state)?;

    let market_id = path.into_inner();

    let market = state
        .db
        .get_market(&market_id)
        .await
        .map_err(ApiError::from)?;

    match market {
        Some(m) => Ok(HttpResponse::Ok().json(m)),
        None => Err(ApiError::not_found("Market")),
    }
}

/// Create a new market (requires Admin or Keeper role)
pub async fn create_market(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateMarketRequest>,
) -> Result<impl Responder, ApiError> {
    let evm_writes = state.config.evm_enabled && state.config.evm_writes_enabled;
    let solana_writes = state.config.solana_enabled && state.config.solana_writes_enabled;
    if !evm_writes && !solana_writes {
        return Err(ApiError::bad_request(
            "CHAIN_WRITE_PATH_DISABLED",
            "Market write path is disabled for all configured chains",
        ));
    }

    // Require JWT authentication with role
    let user = extract_jwt_user(&req, &state)?;

    // Require Admin or Keeper role
    if !matches!(user.role, UserRole::Admin | UserRole::Keeper) {
        return Err(ApiError::forbidden(
            "Only admins and keepers can create markets",
        ));
    }

    // SECURITY: Per-user rate limit (1 market/hour)
    check_market_create_rate_limit(&user.wallet_address, &state.redis).await?;

    // Validate inputs
    if body.market_id.len() > 64 {
        return Err(ApiError::bad_request(
            "INVALID_MARKET_ID",
            "Market ID too long",
        ));
    }
    if body.question.len() > 256 {
        return Err(ApiError::bad_request(
            "INVALID_QUESTION",
            "Question too long",
        ));
    }
    if body.fee_bps > 1000 {
        return Err(ApiError::bad_request("INVALID_FEE", "Fee must be <= 10%"));
    }

    let now = Utc::now();

    let market = Market {
        id: body.market_id.clone(),
        address: derive_market_address(&body.market_id),
        question: body.question.clone(),
        description: body.description.clone(),
        category: body.category.clone(),
        status: MarketStatus::Active,
        yes_price: 0.5,
        no_price: 0.5,
        yes_supply: 0,
        no_supply: 0,
        volume_24h: 0.0,
        total_volume: 0.0,
        total_collateral: 0,
        fee_bps: body.fee_bps,
        oracle: body.oracle.clone(),
        collateral_mint: body.collateral_mint.clone(),
        yes_mint: String::new(), // Would be derived
        no_mint: String::new(),
        resolution_deadline: body.resolution_deadline,
        trading_end: body.trading_end,
        resolved_outcome: None,
        created_at: now,
        resolved_at: None,
    };

    // Save to database
    state
        .db
        .create_market(&market)
        .await
        .map_err(ApiError::from)?;

    Ok(HttpResponse::Created().json(market))
}

/// Get order book for a market
pub async fn get_orderbook(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<OrderBookQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_market_read_mode(&state)?;

    let market_id = path.into_inner();
    let outcome = match query.outcome.as_deref().unwrap_or("yes") {
        "yes" => Outcome::Yes,
        "no" => Outcome::No,
        _ => Outcome::Yes,
    };
    let depth = query.depth.unwrap_or(20).min(100) as usize;

    let (bids, asks) = state.orderbook.get_depth(&market_id, outcome, depth);

    let best_bid = bids.first().map(|l| l.price).unwrap_or(0.0);
    let best_ask = asks.first().map(|l| l.price).unwrap_or(1.0);
    let spread = best_ask - best_bid;
    let mid_price = (best_bid + best_ask) / 2.0;

    Ok(HttpResponse::Ok().json(OrderBookResponse {
        market_id,
        outcome: match outcome {
            Outcome::Yes => "yes".to_string(),
            Outcome::No => "no".to_string(),
        },
        timestamp: Utc::now(),
        bids,
        asks,
        spread,
        mid_price,
    }))
}

#[derive(serde::Deserialize)]
pub struct OrderBookQuery {
    pub outcome: Option<String>,
    pub depth: Option<i32>,
}

/// Get recent trades for a market
pub async fn get_trades(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<ListTradesQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_market_read_mode(&state)?;

    let market_id = path.into_inner();
    let outcome = query.outcome.as_ref().map(|o| match o.as_str() {
        "yes" => Outcome::Yes,
        "no" => Outcome::No,
        _ => Outcome::Yes,
    });
    let limit = query.limit.unwrap_or(50).min(100);

    let trades = state
        .db
        .get_trades(&market_id, outcome, limit, query.before.as_deref())
        .await
        .map_err(ApiError::from)?;

    let cursor = trades.last().map(|t| t.id.clone());

    Ok(HttpResponse::Ok().json(TradeListResponse { trades, cursor }))
}
