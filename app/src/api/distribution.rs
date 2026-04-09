use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use std::sync::Arc;

use super::auth::extract_jwt_user;
use super::jwt::UserRole;
use super::ApiError;
use crate::require_auth;
use crate::services::distribution::{CurvePoint, DistributionMarketState};
use crate::services::websocket::{DistMarketUpdate, DistResolveUpdate, DistTradeUpdate};
use crate::AppState;

use super::rate_limit::{check_rate_limit, check_rate_limit_by_user, RateLimitTier};
use super::notifications::{create_notification, NewNotification, NotificationType};

// ---------------------------------------------------------------------------
// Status mapping helpers
// ---------------------------------------------------------------------------

// --- Market status codes ---
fn market_status_to_int(s: &str) -> i16 {
    match s {
        "active" => 0,
        "paused" => 1,
        "closed" => 2,
        "resolved" => 3,
        "cancelled" => 4,
        _ => 0,
    }
}

fn int_to_market_status(i: i16) -> String {
    match i {
        0 => "active",
        1 => "paused",
        2 => "closed",
        3 => "resolved",
        4 => "cancelled",
        _ => "active",
    }
    .to_string()
}

// --- Position status codes ---
// 0=Open, 1=Closed(early exit), 2=Resolved(payout calculated), 3=Claimed
fn int_to_position_status(i: i16) -> String {
    match i {
        0 => "open",
        1 => "closed",
        2 => "resolved",
        3 => "claimed",
        _ => "open",
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDistMarketRequest {
    pub market_id: String,
    pub question: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub outcome_min: f64,
    pub outcome_max: f64,
    pub outcome_unit: Option<String>,
    pub liquidity_param: f64,
    pub collateral_token: String,
    pub fee_bps: Option<i16>,
    pub resolver: Option<String>,
    pub use_oracle: Option<bool>,
    pub oracle_feed_id: Option<String>,
    pub trading_end: Option<chrono::DateTime<Utc>>,
    pub resolution_deadline: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeRequest {
    pub mu: f64,
    pub sigma: f64,
    pub size: i64,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteQuery {
    pub mu: f64,
    pub sigma: f64,
    pub size: Option<i64>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDistMarketsQuery {
    pub status: Option<String>,
    pub category: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DistMarketResponse {
    pub id: String,
    pub question: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub status: String,
    pub outcome_min: f64,
    pub outcome_max: f64,
    pub outcome_unit: Option<String>,
    pub liquidity_param: f64,
    pub market_mu: Option<f64>,
    pub market_sigma: Option<f64>,
    pub stiffness: Option<f64>,
    pub peak_density: Option<f64>,
    pub headroom_pct: Option<f64>,
    pub lambda: Option<f64>,
    pub collateral_token: String,
    pub total_collateral: i64,
    pub total_volume: i64,
    pub volume_24h: i64,
    pub fee_bps: i16,
    pub resolver: Option<String>,
    pub use_oracle: bool,
    pub oracle_feed_id: Option<String>,
    pub resolved_value: Option<f64>,
    pub trading_end: Option<chrono::DateTime<Utc>>,
    pub resolution_deadline: Option<chrono::DateTime<Utc>>,
    pub created_at: chrono::DateTime<Utc>,
    pub resolved_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DistPositionResponse {
    pub id: i32,
    pub position_id: i64,
    pub market_id: String,
    pub owner: String,
    pub mu: f64,
    pub sigma: f64,
    pub size: i64,
    pub collateral: i64,
    pub cost_basis: Option<f64>,
    pub status: String,
    pub payout: Option<i64>,
    pub pnl: Option<f64>,
    pub created_at: chrono::DateTime<Utc>,
    pub closed_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    pub cost: f64,
    pub collateral_token: String,
    pub new_market_mu: f64,
    pub new_market_sigma: f64,
    pub delta_mu: f64,
    pub delta_sigma: f64,
    pub stiffness: f64,
    pub peak_density: f64,
    pub headroom_pct: f64,
    pub lambda: f64,
    pub fees: f64,
    pub min_fx: f64,
    pub arg_min_x: f64,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveRequest {
    pub value: f64,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurveQuery {
    pub proposal_mu: Option<f64>,
    pub proposal_sigma: Option<f64>,
}

// ---------------------------------------------------------------------------
// SQL column constants (avoid repetition, single source of truth)
// ---------------------------------------------------------------------------

const MARKET_COLUMNS: &str = "id, question, description, category, status, outcome_min, outcome_max, \
    outcome_unit, liquidity_param, market_mu, market_sigma, collateral_token, \
    total_collateral, total_volume, volume_24h, fee_bps, resolver, use_oracle, \
    oracle_feed_id, resolved_value, trading_end, resolution_deadline, created_at, \
    resolved_at";

const POSITION_COLUMNS: &str = "id, position_id, market_id, owner, mu, sigma, size, collateral, \
    cost_basis, status, payout, pnl, created_at, closed_at";

// ---------------------------------------------------------------------------
// Internal row types for sqlx queries
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct MarketRow {
    id: String,
    question: String,
    description: Option<String>,
    category: Option<String>,
    status: i16,
    outcome_min: f64,
    outcome_max: f64,
    outcome_unit: Option<String>,
    liquidity_param: f64,
    market_mu: Option<f64>,
    market_sigma: Option<f64>,
    collateral_token: String,
    total_collateral: i64,
    total_volume: i64,
    volume_24h: i64,
    fee_bps: i16,
    resolver: Option<String>,
    use_oracle: bool,
    oracle_feed_id: Option<String>,
    resolved_value: Option<f64>,
    trading_end: Option<chrono::DateTime<Utc>>,
    resolution_deadline: Option<chrono::DateTime<Utc>>,
    created_at: chrono::DateTime<Utc>,
    resolved_at: Option<chrono::DateTime<Utc>>,
}

impl MarketRow {
    fn into_response(self) -> DistMarketResponse {
        let (stiffness, peak_density, headroom_pct, lambda) =
            match (self.market_mu, self.market_sigma) {
                (Some(mu), Some(sigma)) if sigma > 0.0 => {
                    let state = DistributionMarketState {
                        mu,
                        sigma,
                        liquidity_b: self.liquidity_param,
                        outcome_min: self.outcome_min,
                        outcome_max: self.outcome_max,
                    };
                    let stiffness = state.stiffness();
                    let peak = DistributionMarketState::pdf(mu, mu, sigma);
                    let range = self.outcome_max - self.outcome_min;
                    let headroom = 1.0 - (sigma / (range / 2.0));
                    let lam = self.liquidity_param * peak;
                    (Some(stiffness), Some(peak), Some(headroom), Some(lam))
                }
                _ => (None, None, None, None),
            };

        DistMarketResponse {
            id: self.id,
            question: self.question,
            description: self.description,
            category: self.category,
            status: int_to_market_status(self.status),
            outcome_min: self.outcome_min,
            outcome_max: self.outcome_max,
            outcome_unit: self.outcome_unit,
            liquidity_param: self.liquidity_param,
            market_mu: self.market_mu,
            market_sigma: self.market_sigma,
            stiffness,
            peak_density,
            headroom_pct,
            lambda,
            collateral_token: self.collateral_token,
            total_collateral: self.total_collateral,
            total_volume: self.total_volume,
            volume_24h: self.volume_24h,
            fee_bps: self.fee_bps,
            resolver: self.resolver,
            use_oracle: self.use_oracle,
            oracle_feed_id: self.oracle_feed_id,
            resolved_value: self.resolved_value,
            trading_end: self.trading_end,
            resolution_deadline: self.resolution_deadline,
            created_at: self.created_at,
            resolved_at: self.resolved_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PositionRow {
    id: i32,
    position_id: i64,
    market_id: String,
    owner: String,
    mu: f64,
    sigma: f64,
    size: i64,
    collateral: i64,
    cost_basis: Option<f64>,
    status: i16,
    payout: Option<i64>,
    pnl: Option<f64>,
    created_at: chrono::DateTime<Utc>,
    closed_at: Option<chrono::DateTime<Utc>>,
}

impl PositionRow {
    fn into_response(self) -> DistPositionResponse {
        DistPositionResponse {
            id: self.id,
            position_id: self.position_id,
            market_id: self.market_id,
            owner: self.owner,
            mu: self.mu,
            sigma: self.sigma,
            size: self.size,
            collateral: self.collateral,
            cost_basis: self.cost_basis,
            status: int_to_position_status(self.status),
            payout: self.payout,
            pnl: self.pnl,
            created_at: self.created_at,
            closed_at: self.closed_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /distribution/markets — public, list with filters
pub async fn list_dist_markets(
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListDistMarketsQuery>,
) -> Result<impl Responder, ApiError> {
    let limit = query.limit.unwrap_or(20).max(1).min(100);
    let offset = query.offset.unwrap_or(0).max(0);

    // Build dynamic query to avoid repeating column list 4 times
    let mut conditions = Vec::new();
    let mut param_idx = 1u32;

    if query.status.is_some() {
        conditions.push(format!("status = ${param_idx}"));
        param_idx += 1;
    }
    if query.category.is_some() {
        conditions.push(format!("category = ${param_idx}"));
        param_idx += 1;
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT {MARKET_COLUMNS} FROM distribution_markets{where_clause} \
         ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
        param_idx,
        param_idx + 1,
    );

    let mut q = sqlx::query_as::<_, MarketRow>(&sql);
    if let Some(status) = &query.status {
        q = q.bind(market_status_to_int(status));
    }
    if let Some(category) = &query.category {
        q = q.bind(category.as_str());
    }
    q = q.bind(limit).bind(offset);

    let rows: Vec<MarketRow> = q
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let markets: Vec<DistMarketResponse> = rows.into_iter().map(|r| r.into_response()).collect();
    Ok(HttpResponse::Ok().json(markets))
}

/// GET /distribution/markets/{id} — public, get single market
pub async fn get_dist_market(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let market_id = path.into_inner();

    let row: Option<MarketRow> = sqlx::query_as(&format!(
        "SELECT {MARKET_COLUMNS} FROM distribution_markets WHERE id = $1"
    ))
    .bind(&market_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    match row {
        Some(r) => Ok(HttpResponse::Ok().json(r.into_response())),
        None => Err(ApiError::not_found("Distribution market")),
    }
}

/// POST /distribution/markets — any authenticated user
pub async fn create_dist_market(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateDistMarketRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;

    // Rate limit: 1 market creation per hour per user
    check_rate_limit_by_user(&user.wallet_address, &state.redis, RateLimitTier::MarketCreate).await?;

    // --- Market ID ---
    if body.market_id.is_empty() || body.market_id.len() > 64 {
        return Err(ApiError::bad_request(
            "INVALID_MARKET_ID",
            "Market ID must be 1-64 characters",
        ));
    }
    if !body.market_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(ApiError::bad_request(
            "INVALID_MARKET_ID",
            "Market ID must contain only alphanumeric characters, hyphens, and underscores",
        ));
    }

    // --- Question ---
    let question = body.question.trim();
    if question.len() < 10 || question.len() > 256 {
        return Err(ApiError::bad_request(
            "INVALID_QUESTION",
            "Question must be 10-256 characters",
        ));
    }
    if !question.ends_with('?') {
        return Err(ApiError::bad_request(
            "INVALID_QUESTION",
            "Question must end with ?",
        ));
    }
    if question.contains('<') || question.contains('>') {
        return Err(ApiError::bad_request(
            "INVALID_QUESTION",
            "Question must not contain HTML tags",
        ));
    }

    // --- Description ---
    if let Some(desc) = &body.description {
        if desc.len() > 5000 {
            return Err(ApiError::bad_request(
                "DESCRIPTION_TOO_LONG",
                "Description must be at most 5000 characters",
            ));
        }
        if desc.contains('<') || desc.contains('>') {
            return Err(ApiError::bad_request(
                "INVALID_DESCRIPTION",
                "Description must not contain HTML tags",
            ));
        }
    }

    // --- Outcome range ---
    if !body.outcome_min.is_finite() || !body.outcome_max.is_finite() {
        return Err(ApiError::bad_request(
            "INVALID_OUTCOME",
            "Outcome values must be finite numbers",
        ));
    }
    if body.outcome_min >= body.outcome_max {
        return Err(ApiError::bad_request(
            "INVALID_RANGE",
            "outcome_min must be less than outcome_max",
        ));
    }
    let range = body.outcome_max - body.outcome_min;
    if range < 0.01 {
        return Err(ApiError::bad_request(
            "RANGE_TOO_SMALL",
            "Outcome range must be at least 0.01 to avoid numerical instability",
        ));
    }
    if range > 1e15 {
        return Err(ApiError::bad_request(
            "RANGE_TOO_LARGE",
            "Outcome range must be at most 1e15",
        ));
    }

    // --- Liquidity parameter ---
    if !body.liquidity_param.is_finite() || body.liquidity_param <= 0.0 {
        return Err(ApiError::bad_request(
            "INVALID_LIQUIDITY",
            "liquidity_param must be a positive finite number",
        ));
    }
    if body.liquidity_param > 1e10 {
        return Err(ApiError::bad_request(
            "LIQUIDITY_TOO_LARGE",
            "liquidity_param must be at most 1e10",
        ));
    }

    // --- Fee ---
    let fee_bps = body.fee_bps.unwrap_or(100);
    if fee_bps < 0 || fee_bps > 1000 {
        return Err(ApiError::bad_request("INVALID_FEE", "Fee must be 0-1000 bps"));
    }

    // --- Collateral token (must be valid hex address) ---
    let token = body.collateral_token.trim();
    if token.len() != 42
        || !token.starts_with("0x")
        || !token[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(ApiError::bad_request(
            "INVALID_TOKEN",
            "collateral_token must be a valid address (0x + 40 hex chars)",
        ));
    }

    // --- Resolver address (optional, must be valid if provided) ---
    if let Some(resolver) = &body.resolver {
        let addr = resolver.trim();
        if !addr.is_empty()
            && (addr.len() != 42
                || !addr.starts_with("0x")
                || !addr[2..].chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(ApiError::bad_request(
                "INVALID_RESOLVER",
                "Resolver must be a valid address (0x + 40 hex chars) or empty",
            ));
        }
    }

    // --- Oracle feed ID (required if use_oracle is true) ---
    let use_oracle = body.use_oracle.unwrap_or(false);
    if use_oracle {
        let feed_id = body.oracle_feed_id.as_deref().unwrap_or("");
        if feed_id.len() != 66
            || !feed_id.starts_with("0x")
            || !feed_id[2..].chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(ApiError::bad_request(
                "INVALID_FEED_ID",
                "oracle_feed_id must be a valid Pyth feed ID (0x + 64 hex chars)",
            ));
        }
    }

    // --- Trading end / resolution deadline ---
    let now = Utc::now();
    if let Some(trading_end) = body.trading_end {
        if trading_end <= now {
            return Err(ApiError::bad_request(
                "INVALID_DATE",
                "trading_end must be in the future",
            ));
        }
    }
    if let Some(deadline) = body.resolution_deadline {
        if deadline <= now {
            return Err(ApiError::bad_request(
                "INVALID_DATE",
                "resolution_deadline must be in the future",
            ));
        }
        if let Some(trading_end) = body.trading_end {
            if deadline < trading_end {
                return Err(ApiError::bad_request(
                    "INVALID_DATE",
                    "resolution_deadline must be after trading_end",
                ));
            }
        }
    }

    let now = Utc::now();
    let initial_mu = (body.outcome_min + body.outcome_max) / 2.0;
    let initial_sigma = (body.outcome_max - body.outcome_min) / 6.0; // 3-sigma rule, matches contract
    let use_oracle = body.use_oracle.unwrap_or(false);

    sqlx::query(
        "INSERT INTO distribution_markets \
         (id, question, description, category, status, outcome_min, outcome_max, outcome_unit, \
          liquidity_param, market_mu, market_sigma, collateral_token, total_collateral, \
          total_volume, volume_24h, fee_bps, resolver, use_oracle, oracle_feed_id, \
          trading_end, resolution_deadline, created_at) \
         VALUES ($1, $2, $3, $4, 0, $5, $6, $7, $8, $9, $10, $11, 0, 0, 0, $12, $13, $14, $15, $16, $17, $18)",
    )
    .bind(&body.market_id)
    .bind(&body.question)
    .bind(&body.description)
    .bind(&body.category)
    .bind(body.outcome_min)
    .bind(body.outcome_max)
    .bind(&body.outcome_unit)
    .bind(body.liquidity_param)
    .bind(initial_mu)
    .bind(initial_sigma)
    .bind(&body.collateral_token)
    .bind(fee_bps)
    .bind(&body.resolver)
    .bind(use_oracle)
    .bind(&body.oracle_feed_id)
    .bind(body.trading_end)
    .bind(body.resolution_deadline)
    .bind(now)
    .execute(state.db.pool())
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key") || e.to_string().contains("unique") {
            ApiError::conflict("DUPLICATE_MARKET", "A market with this ID already exists")
        } else {
            ApiError::internal(&e.to_string())
        }
    })?;

    // Return the created market
    let row: MarketRow = sqlx::query_as(&format!(
        "SELECT {MARKET_COLUMNS} FROM distribution_markets WHERE id = $1"
    ))
    .bind(&body.market_id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Created().json(row.into_response()))
}

/// GET /distribution/markets/{id}/quote?mu=X&sigma=Y&size=Z — public, preview trade cost
pub async fn get_quote(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<QuoteQuery>,
) -> Result<impl Responder, ApiError> {
    check_rate_limit(&req, &state.redis, RateLimitTier::DistQuote).await?;
    let market_id = path.into_inner();

    let row: Option<MarketRow> = sqlx::query_as(&format!(
        "SELECT {MARKET_COLUMNS} FROM distribution_markets WHERE id = $1"
    ))
    .bind(&market_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let market = row.ok_or_else(|| ApiError::not_found("Distribution market"))?;

    if market.status != 0 {
        return Err(ApiError::bad_request(
            "MARKET_NOT_ACTIVE",
            "Market is not active for trading",
        ));
    }

    let current_mu = market.market_mu.unwrap_or((market.outcome_min + market.outcome_max) / 2.0);
    let current_sigma =
        market
            .market_sigma
            .unwrap_or((market.outcome_max - market.outcome_min) / 4.0);

    if query.mu < market.outcome_min || query.mu > market.outcome_max {
        return Err(ApiError::bad_request(
            "INVALID_MU",
            &format!(
                "mu must be between {} and {}",
                market.outcome_min, market.outcome_max
            ),
        ));
    }
    if query.sigma <= 0.0 {
        return Err(ApiError::bad_request(
            "INVALID_SIGMA",
            "sigma must be positive",
        ));
    }

    let ms = DistributionMarketState {
        mu: current_mu,
        sigma: current_sigma,
        liquidity_b: market.liquidity_param,
        outcome_min: market.outcome_min,
        outcome_max: market.outcome_max,
    };

    let trade_result = ms.trade_cost(query.mu, query.sigma);
    let size = query.size.unwrap_or(1);
    let scaled_cost = trade_result.cost * size as f64;
    let fee_rate = market.fee_bps as f64 / 10_000.0;
    let fees = scaled_cost.abs() * fee_rate;

    // Compute the minimum PDF value in the outcome range and the x where it occurs.
    // For a Gaussian the minimum of pdf over [a,b] is at the boundary farthest from mu.
    let pdf_at_min = DistributionMarketState::pdf(market.outcome_min, query.mu, query.sigma);
    let pdf_at_max = DistributionMarketState::pdf(market.outcome_max, query.mu, query.sigma);
    let (min_fx, arg_min_x) = if pdf_at_min < pdf_at_max {
        (pdf_at_min, market.outcome_min)
    } else {
        (pdf_at_max, market.outcome_max)
    };

    Ok(HttpResponse::Ok().json(QuoteResponse {
        cost: scaled_cost,
        collateral_token: market.collateral_token,
        new_market_mu: trade_result.new_mu,
        new_market_sigma: trade_result.new_sigma,
        delta_mu: trade_result.delta_mu,
        delta_sigma: trade_result.delta_sigma,
        stiffness: trade_result.stiffness,
        peak_density: trade_result.peak_density,
        headroom_pct: trade_result.headroom_pct,
        lambda: trade_result.lambda,
        fees,
        min_fx,
        arg_min_x,
    }))
}

/// POST /distribution/markets/{id}/trade — authenticated, open position
pub async fn open_position(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<TradeRequest>,
) -> Result<impl Responder, ApiError> {
    let user = require_auth!(&req, &state);
    check_rate_limit(&req, &state.redis, RateLimitTier::DistTrade).await?;
    let market_id = path.into_inner();

    let row: Option<MarketRow> = sqlx::query_as(&format!(
        "SELECT {MARKET_COLUMNS} FROM distribution_markets WHERE id = $1"
    ))
    .bind(&market_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let market = row.ok_or_else(|| ApiError::not_found("Distribution market"))?;

    if market.status != 0 {
        return Err(ApiError::bad_request(
            "MARKET_NOT_ACTIVE",
            "Market is not active for trading",
        ));
    }

    // Check trading deadline
    if let Some(trading_end) = market.trading_end {
        if Utc::now() >= trading_end {
            return Err(ApiError::bad_request(
                "TRADING_ENDED",
                "Trading period has ended for this market",
            ));
        }
    }

    // Validate mu is within outcome range
    if body.mu < market.outcome_min || body.mu > market.outcome_max {
        return Err(ApiError::bad_request(
            "INVALID_MU",
            &format!(
                "mu must be between {} and {}",
                market.outcome_min, market.outcome_max
            ),
        ));
    }

    // Validate sigma
    let min_sigma = (market.outcome_max - market.outcome_min) * 0.001; // 0.1% of range
    if body.sigma <= 0.0 {
        return Err(ApiError::bad_request(
            "INVALID_SIGMA",
            "sigma must be positive",
        ));
    }
    if body.sigma < min_sigma {
        return Err(ApiError::bad_request(
            "SIGMA_TOO_SMALL",
            &format!("sigma must be at least {:.6} (0.1% of outcome range)", min_sigma),
        ));
    }
    let max_sigma = (market.outcome_max - market.outcome_min) / 2.0;
    if body.sigma > max_sigma {
        return Err(ApiError::bad_request(
            "SIGMA_TOO_LARGE",
            &format!("sigma must be at most {:.6} (half the outcome range)", max_sigma),
        ));
    }

    if body.size <= 0 {
        return Err(ApiError::bad_request(
            "INVALID_SIZE",
            "size must be positive",
        ));
    }
    if body.size > 1_000_000_000 {
        return Err(ApiError::bad_request(
            "SIZE_TOO_LARGE",
            "size must be at most 1,000,000,000",
        ));
    }

    let current_mu = market.market_mu.unwrap_or((market.outcome_min + market.outcome_max) / 2.0);
    let current_sigma =
        market
            .market_sigma
            .unwrap_or((market.outcome_max - market.outcome_min) / 4.0);

    let ms = DistributionMarketState {
        mu: current_mu,
        sigma: current_sigma,
        liquidity_b: market.liquidity_param,
        outcome_min: market.outcome_min,
        outcome_max: market.outcome_max,
    };

    let trade_result = ms.trade_cost(body.mu, body.sigma);
    let scaled_cost = trade_result.cost * body.size as f64;
    let fee_rate = market.fee_bps as f64 / 10_000.0;
    let fees = scaled_cost.abs() * fee_rate;
    let total_cost = scaled_cost + fees;
    let collateral = total_cost.ceil().max(1.0) as i64; // minimum 1 unit collateral

    let now = Utc::now();

    // Use a transaction for atomicity — position_id assignment + insert + trade log + market update
    let mut tx = state
        .db
        .pool()
        .begin()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Assign position_id with FOR UPDATE lock to prevent race condition
    let next_pos_id: (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(position_id), 0) + 1 FROM distribution_positions \
         WHERE market_id = $1 FOR UPDATE",
    )
    .bind(&market_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Insert position
    let inserted: PositionRow = sqlx::query_as(&format!(
        "INSERT INTO distribution_positions \
         (position_id, market_id, owner, mu, sigma, size, collateral, cost_basis, status, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 0, $9) \
         RETURNING {POSITION_COLUMNS}"
    ))
    .bind(next_pos_id.0)
    .bind(&market_id)
    .bind(&user.wallet_address)
    .bind(body.mu)
    .bind(body.sigma)
    .bind(body.size)
    .bind(collateral)
    .bind(scaled_cost)
    .bind(now)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Insert trade record
    sqlx::query(
        "INSERT INTO distribution_trades \
         (market_id, position_id, owner, trade_type, mu, sigma, size, cost, fees, \
          new_market_mu, new_market_sigma, created_at) \
         VALUES ($1, $2, $3, 'open', $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(&market_id)
    .bind(next_pos_id.0)
    .bind(&user.wallet_address)
    .bind(body.mu)
    .bind(body.sigma)
    .bind(body.size)
    .bind(scaled_cost)
    .bind(fees)
    .bind(trade_result.new_mu)
    .bind(trade_result.new_sigma)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Update market aggregate mu/sigma and totals
    sqlx::query(
        "UPDATE distribution_markets \
         SET market_mu = $1, market_sigma = $2, \
             total_collateral = total_collateral + $3, \
             total_volume = total_volume + $4 \
         WHERE id = $5",
    )
    .bind(trade_result.new_mu)
    .bind(trade_result.new_sigma)
    .bind(collateral)
    .bind(collateral)
    .bind(&market_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Broadcast WebSocket events (fire-and-forget after commit)
    let ts = now.timestamp();
    state
        .ws_hub
        .broadcast_dist_trade(DistTradeUpdate {
            market_id: market_id.clone(),
            position_id: next_pos_id.0,
            owner: user.wallet_address.clone(),
            trade_type: "open".to_string(),
            mu: body.mu,
            sigma: body.sigma,
            size: body.size,
            cost: scaled_cost,
            timestamp: ts,
        })
        .await;
    state
        .ws_hub
        .broadcast_dist_market(DistMarketUpdate {
            market_id: market_id.clone(),
            market_mu: trade_result.new_mu,
            market_sigma: trade_result.new_sigma,
            stiffness: ms.stiffness(),
            peak_density: DistributionMarketState::pdf(trade_result.new_mu, trade_result.new_mu, trade_result.new_sigma),
            total_collateral: market.total_collateral + collateral,
            timestamp: ts,
        })
        .await;

    // Notification (fire-and-forget)
    let _ = create_notification(
        &state,
        NewNotification {
            owner: user.wallet_address.clone(),
            kind: NotificationType::DistributionTradeConfirmed,
            title: "Distribution trade opened".to_string(),
            message: format!(
                "Position opened: \u{03BC}={:.2}, \u{03C3}={:.2}, size={}",
                body.mu, body.sigma, body.size
            ),
            market_id: Some(market_id),
            order_id: None,
            decision_cell_id: None,
            metadata: serde_json::json!({}),
        },
    )
    .await;

    Ok(HttpResponse::Created().json(inserted.into_response()))
}

/// GET /distribution/positions — authenticated, user's positions
pub async fn list_positions(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = require_auth!(&req, &state);

    let rows: Vec<PositionRow> = sqlx::query_as(&format!(
        "SELECT {POSITION_COLUMNS} FROM distribution_positions WHERE owner = $1 \
         ORDER BY created_at DESC"
    ))
    .bind(&user.wallet_address)
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let positions: Vec<DistPositionResponse> =
        rows.into_iter().map(|r| r.into_response()).collect();
    Ok(HttpResponse::Ok().json(positions))
}

/// DELETE /distribution/positions/{id} — authenticated, owner only, close position
pub async fn close_position(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<i32>,
) -> Result<impl Responder, ApiError> {
    let user = require_auth!(&req, &state);
    check_rate_limit(&req, &state.redis, RateLimitTier::DistTrade).await?;
    let position_id = path.into_inner();

    let row: Option<PositionRow> = sqlx::query_as(&format!(
        "SELECT {POSITION_COLUMNS} FROM distribution_positions WHERE id = $1"
    ))
    .bind(position_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let position = row.ok_or_else(|| ApiError::not_found("Position"))?;

    if position.owner != user.wallet_address {
        return Err(ApiError::forbidden("You can only close your own positions"));
    }
    if position.status != 0 {
        return Err(ApiError::bad_request(
            "POSITION_NOT_ACTIVE",
            "Position is not active",
        ));
    }

    // Check that the market is still active and trading is open
    let market_row: Option<(i16, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
        "SELECT status, trading_end FROM distribution_markets WHERE id = $1",
    )
    .bind(&position.market_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    if let Some((ms, trading_end)) = market_row {
        if ms != 0 {
            return Err(ApiError::bad_request(
                "MARKET_NOT_ACTIVE",
                "Cannot close position: market is not active",
            ));
        }
        if let Some(end) = trading_end {
            if Utc::now() >= end {
                return Err(ApiError::bad_request(
                    "TRADING_ENDED",
                    "Cannot close position: trading period has ended",
                ));
            }
        }
    }

    let now = Utc::now();

    // Use transaction for atomicity
    let mut tx = state
        .db
        .pool()
        .begin()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Mark position as closed (status 1) — atomically check it's still open
    let result = sqlx::query(
        "UPDATE distribution_positions SET status = 1, closed_at = $1 WHERE id = $2 AND status = 0",
    )
    .bind(now)
    .bind(position_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::bad_request(
            "POSITION_NOT_ACTIVE",
            "Position is no longer active (concurrent modification)",
        ));
    }

    // Reduce market collateral
    sqlx::query(
        "UPDATE distribution_markets SET total_collateral = total_collateral - $1 WHERE id = $2",
    )
    .bind(position.collateral)
    .bind(&position.market_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Record the close trade
    sqlx::query(
        "INSERT INTO distribution_trades \
         (market_id, position_id, owner, trade_type, mu, sigma, size, cost, fees, created_at) \
         VALUES ($1, $2, $3, 'close', $4, $5, $6, 0, 0, $7)",
    )
    .bind(&position.market_id)
    .bind(position.position_id)
    .bind(&user.wallet_address)
    .bind(position.mu)
    .bind(position.sigma)
    .bind(position.size)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Broadcast close trade event
    state
        .ws_hub
        .broadcast_dist_trade(DistTradeUpdate {
            market_id: position.market_id.clone(),
            position_id: position.position_id,
            owner: user.wallet_address.clone(),
            trade_type: "close".to_string(),
            mu: position.mu,
            sigma: position.sigma,
            size: position.size,
            cost: 0.0,
            timestamp: now.timestamp(),
        })
        .await;

    // Return updated position
    let updated: PositionRow = sqlx::query_as(&format!(
        "SELECT {POSITION_COLUMNS} FROM distribution_positions WHERE id = $1"
    ))
    .bind(position_id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(updated.into_response()))
}

/// POST /distribution/markets/{id}/resolve — admin only
pub async fn resolve_market(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<ResolveRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;

    if !matches!(user.role, UserRole::Admin) {
        return Err(ApiError::forbidden(
            "Only admins can resolve distribution markets",
        ));
    }

    let market_id = path.into_inner();

    let row: Option<MarketRow> = sqlx::query_as(&format!(
        "SELECT {MARKET_COLUMNS} FROM distribution_markets WHERE id = $1"
    ))
    .bind(&market_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let market = row.ok_or_else(|| ApiError::not_found("Distribution market"))?;

    if market.status == 3 {
        return Err(ApiError::bad_request(
            "ALREADY_RESOLVED",
            "Market is already resolved",
        ));
    }
    if market.status == 4 {
        return Err(ApiError::bad_request(
            "MARKET_CANCELLED",
            "Cannot resolve a cancelled market",
        ));
    }

    let resolved_value = body.value;
    if resolved_value < market.outcome_min || resolved_value > market.outcome_max {
        return Err(ApiError::bad_request(
            "VALUE_OUT_OF_RANGE",
            &format!(
                "Resolved value must be between {} and {}",
                market.outcome_min, market.outcome_max
            ),
        ));
    }

    let now = Utc::now();

    let current_mu = market.market_mu.unwrap_or((market.outcome_min + market.outcome_max) / 2.0);
    let current_sigma =
        market
            .market_sigma
            .unwrap_or((market.outcome_max - market.outcome_min) / 4.0);

    let ms = DistributionMarketState {
        mu: current_mu,
        sigma: current_sigma,
        liquidity_b: market.liquidity_param,
        outcome_min: market.outcome_min,
        outcome_max: market.outcome_max,
    };

    // Use transaction — resolve + compute all payouts atomically
    let mut tx = state
        .db
        .pool()
        .begin()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Atomically update market to resolved (check it's not already resolved)
    let result = sqlx::query(
        "UPDATE distribution_markets \
         SET status = 3, resolved_value = $1, resolved_at = $2 \
         WHERE id = $3 AND status != 3 AND status != 4",
    )
    .bind(resolved_value)
    .bind(now)
    .bind(&market_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::bad_request(
            "ALREADY_RESOLVED",
            "Market was already resolved or cancelled (concurrent modification)",
        ));
    }

    // Calculate payouts for all open positions
    let positions: Vec<PositionRow> = sqlx::query_as(&format!(
        "SELECT {POSITION_COLUMNS} FROM distribution_positions WHERE market_id = $1 AND status = 0"
    ))
    .bind(&market_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Track total payouts to cap against pool
    let total_pool = market.total_collateral as f64;
    let mut total_gross_paid = 0.0;

    for pos in &positions {
        let payout_result = ms.calculate_payout(
            pos.mu,
            pos.sigma,
            pos.collateral as f64,
            resolved_value,
            market.fee_bps as u32,
            0, // discount applied at claim time per-user
        );

        // Guard against NaN/Infinity from degenerate density ratios
        let mut gross = payout_result.gross_payout;
        if !gross.is_finite() || gross < 0.0 {
            gross = 0.0;
        }
        if total_gross_paid + gross > total_pool {
            gross = (total_pool - total_gross_paid).max(0.0);
        }
        total_gross_paid += gross;

        let fee = gross * (market.fee_bps as f64) / 10_000.0;
        let net = (gross - fee).max(0.0);
        let payout = net.ceil() as i64;
        let pnl = payout as f64 - pos.collateral as f64;

        sqlx::query(
            "UPDATE distribution_positions \
             SET status = 2, payout = $1, pnl = $2, closed_at = $3 \
             WHERE id = $4",
        )
        .bind(payout)
        .bind(pnl)
        .bind(now)
        .bind(pos.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Broadcast resolution event
    state
        .ws_hub
        .broadcast_dist_resolve(DistResolveUpdate {
            market_id: market_id.clone(),
            resolved_value,
            timestamp: now.timestamp(),
        })
        .await;

    // Notify all position holders that market resolved and payouts are ready
    let owners: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT owner FROM distribution_positions WHERE market_id = $1 AND status = 2",
    )
    .bind(&market_id)
    .fetch_all(state.db.pool())
    .await
    .unwrap_or_default();

    for (owner,) in owners {
        let _ = create_notification(
            &state,
            NewNotification {
                owner,
                kind: NotificationType::DistributionMarketResolved,
                title: "Distribution market resolved".to_string(),
                message: format!("Resolved at {:.4}. Claim your payout.", resolved_value),
                market_id: Some(market_id.clone()),
                order_id: None,
                decision_cell_id: None,
                metadata: serde_json::json!({ "resolvedValue": resolved_value }),
            },
        )
        .await;
    }

    // Return updated market
    let updated: MarketRow = sqlx::query_as(&format!(
        "SELECT {MARKET_COLUMNS} FROM distribution_markets WHERE id = $1"
    ))
    .bind(&market_id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(updated.into_response()))
}

/// POST /distribution/positions/{id}/claim — authenticated, owner only
pub async fn claim_payout(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<i32>,
) -> Result<impl Responder, ApiError> {
    let user = require_auth!(&req, &state);
    check_rate_limit(&req, &state.redis, RateLimitTier::Claim).await?;
    let position_id = path.into_inner();

    let row: Option<PositionRow> = sqlx::query_as(&format!(
        "SELECT {POSITION_COLUMNS} FROM distribution_positions WHERE id = $1"
    ))
    .bind(position_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let position = row.ok_or_else(|| ApiError::not_found("Position"))?;

    if position.owner != user.wallet_address {
        return Err(ApiError::forbidden("You can only claim your own positions"));
    }

    // Position must be resolved (status 2) to claim
    if position.status != 2 {
        return Err(ApiError::bad_request(
            "NOT_RESOLVED",
            "Position must be resolved before claiming payout",
        ));
    }

    // Payout must have been computed during resolution
    if position.payout.is_none() {
        return Err(ApiError::internal(
            "Payout not computed for resolved position — contact support",
        ));
    }

    let now = Utc::now();

    // Atomically set status to claimed (3) — prevents double-claim via WHERE clause
    let result = sqlx::query(
        "UPDATE distribution_positions SET status = 3, closed_at = $1 WHERE id = $2 AND status = 2",
    )
    .bind(now)
    .bind(position_id)
    .execute(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::bad_request(
            "ALREADY_CLAIMED",
            "Payout has already been claimed",
        ));
    }

    // Record the claim trade
    sqlx::query(
        "INSERT INTO distribution_trades \
         (market_id, position_id, owner, trade_type, mu, sigma, size, cost, fees, created_at) \
         VALUES ($1, $2, $3, 'claim', $4, $5, $6, $7, 0, $8)",
    )
    .bind(&position.market_id)
    .bind(position.position_id)
    .bind(&user.wallet_address)
    .bind(position.mu)
    .bind(position.sigma)
    .bind(position.size)
    .bind(position.payout.unwrap_or(0) as f64)
    .bind(now)
    .execute(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    // Return updated position
    let updated: PositionRow = sqlx::query_as(
        "SELECT id, position_id, market_id, owner, mu, sigma, size, collateral, cost_basis, \
         status, payout, pnl, created_at, closed_at \
         FROM distribution_positions WHERE id = $1",
    )
    .bind(position_id)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    Ok(HttpResponse::Ok().json(updated.into_response()))
}

/// GET /distribution/markets/{id}/curve?proposalMu=X&proposalSigma=Y — public
pub async fn get_curve(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<CurveQuery>,
) -> Result<impl Responder, ApiError> {
    let market_id = path.into_inner();

    let row: Option<MarketRow> = sqlx::query_as(&format!(
        "SELECT {MARKET_COLUMNS} FROM distribution_markets WHERE id = $1"
    ))
    .bind(&market_id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

    let market = row.ok_or_else(|| ApiError::not_found("Distribution market"))?;

    let current_mu = market.market_mu.unwrap_or((market.outcome_min + market.outcome_max) / 2.0);
    let current_sigma =
        market
            .market_sigma
            .unwrap_or((market.outcome_max - market.outcome_min) / 4.0);

    let ms = DistributionMarketState {
        mu: current_mu,
        sigma: current_sigma,
        liquidity_b: market.liquidity_param,
        outcome_min: market.outcome_min,
        outcome_max: market.outcome_max,
    };

    let points: Vec<CurvePoint> =
        ms.generate_curve(200, query.proposal_mu, query.proposal_sigma);

    Ok(HttpResponse::Ok().json(points))
}

// ---------------------------------------------------------------------------
// GET /distribution/markets/{id}/history — curve snapshot history
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub limit: Option<i64>,
    pub since: Option<String>,
}

pub async fn get_curve_history(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<HistoryQuery>,
) -> Result<impl Responder, ApiError> {
    let market_id = path.into_inner();
    let limit = query.limit.unwrap_or(100).min(500);

    let since_filter = query
        .since
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let rows: Vec<(f64, f64, i64, i32, chrono::DateTime<Utc>)> = if let Some(since) = since_filter
    {
        sqlx::query_as(
            "SELECT market_mu, market_sigma, total_collateral, position_count, captured_at \
             FROM distribution_curve_snapshots \
             WHERE market_id = $1 AND captured_at >= $2 \
             ORDER BY captured_at ASC LIMIT $3",
        )
        .bind(&market_id)
        .bind(since)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    } else {
        sqlx::query_as(
            "SELECT market_mu, market_sigma, total_collateral, position_count, captured_at \
             FROM distribution_curve_snapshots \
             WHERE market_id = $1 \
             ORDER BY captured_at DESC LIMIT $2",
        )
        .bind(&market_id)
        .bind(limit)
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    };

    let snapshots: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(mu, sigma, collateral, positions, ts)| {
            serde_json::json!({
                "marketMu": mu,
                "marketSigma": sigma,
                "totalCollateral": collateral,
                "positionCount": positions,
                "capturedAt": ts.to_rfc3339(),
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(snapshots))
}
