use actix_web::http::header::HeaderMap;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::rate_limit::{check_rate_limit_by_user, order_tier_for};
use super::{
    validate_market_id, validate_order_price, validate_order_quantity, validate_pagination,
    validate_uuid, ApiError,
};
use crate::models::{
    CancelOrderResponse, ListOrdersQuery, MatchedTrade, Order, OrderListResponse, OrderSide,
    OrderStatus, Outcome, PlaceOrderRequest, PlaceOrderResponse, Trade,
};
use crate::require_auth;
use crate::services::database::LocalTradeSettlement;
use crate::AppState;

const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";

fn ensure_order_read_mode(state: &web::Data<Arc<AppState>>) -> Result<(), ApiError> {
    let evm_reads = state.config.evm_enabled && state.config.evm_reads_enabled;
    let solana_reads = state.config.solana_enabled && state.config.solana_reads_enabled;
    if !evm_reads && !solana_reads {
        return Err(ApiError::bad_request(
            "CHAIN_READ_PATH_DISABLED",
            "Order read path is disabled for all configured chains",
        ));
    }
    Ok(())
}

fn ensure_order_write_mode(state: &web::Data<Arc<AppState>>) -> Result<(), ApiError> {
    let evm_writes = state.config.evm_enabled && state.config.evm_writes_enabled;
    let solana_writes = state.config.solana_enabled && state.config.solana_writes_enabled;
    if !evm_writes && !solana_writes {
        return Err(ApiError::bad_request(
            "CHAIN_WRITE_PATH_DISABLED",
            "Order write path is disabled for all configured chains",
        ));
    }
    Ok(())
}

/// Extract idempotency key from request headers
fn get_idempotency_key(headers: &HeaderMap) -> Option<String> {
    headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
}

fn settlement_deltas(outcome: Outcome, quantity: u64) -> (i64, i64, i64, i64) {
    let quantity = quantity as i64;
    match outcome {
        Outcome::Yes => (quantity, 0, 0, quantity),
        Outcome::No => (0, quantity, quantity, 0),
    }
}

fn build_trade_settlement(
    matched_trade: &MatchedTrade,
    created_at: chrono::DateTime<Utc>,
) -> LocalTradeSettlement {
    let trade = Trade {
        id: Uuid::new_v4().to_string(),
        market_id: matched_trade.market_id.clone(),
        buy_order_id: matched_trade.buy_order_id.clone(),
        sell_order_id: matched_trade.sell_order_id.clone(),
        outcome: matched_trade.outcome,
        price: matched_trade.fill_price_bps as f64 / 10_000.0,
        price_bps: matched_trade.fill_price_bps,
        quantity: matched_trade.fill_quantity,
        collateral_amount: matched_trade.fill_quantity,
        buyer: matched_trade.buyer.clone(),
        seller: matched_trade.seller.clone(),
        tx_signature: String::new(),
        created_at,
    };
    let (buyer_yes_delta, buyer_no_delta, seller_yes_delta, seller_no_delta) =
        settlement_deltas(matched_trade.outcome, matched_trade.fill_quantity);

    LocalTradeSettlement {
        trade,
        buyer_yes_delta,
        buyer_no_delta,
        seller_yes_delta,
        seller_no_delta,
    }
}

async fn reload_orderbook_from_database(state: &Arc<AppState>) {
    match state.db.load_orderbook_entries().await {
        Ok(entries) => state.orderbook.replace_from_entries(entries),
        Err(err) => log::error!("failed to resync orderbook from database: {}", err),
    }
}

/// List orders for authenticated user
pub async fn list_orders(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListOrdersQuery>,
) -> Result<impl Responder, ApiError> {
    ensure_order_read_mode(&state)?;

    // SECURITY: Extract authenticated user from request
    let user = require_auth!(&req, &state);
    let owner = &user.wallet_address;

    // Validate market_id if provided
    if let Some(ref market_id) = query.market_id {
        validate_market_id(market_id)?;
    }

    let status = query.status.as_ref().map(|s| match s.as_str() {
        "open" => OrderStatus::Open,
        "filled" => OrderStatus::Filled,
        "cancelled" => OrderStatus::Cancelled,
        "partially_filled" => OrderStatus::PartiallyFilled,
        _ => OrderStatus::Open,
    });

    let (limit, offset) = validate_pagination(query.limit, query.offset)?;

    let (orders, total) = state
        .db
        .get_orders(owner, query.market_id.as_deref(), status, limit, offset)
        .await
        .map_err(ApiError::from)?;

    Ok(HttpResponse::Ok().json(OrderListResponse { orders, total }))
}

/// Get a single order (requires authentication, only owner can view)
pub async fn get_order(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    ensure_order_read_mode(&state)?;

    // SECURITY: Require authentication
    let user = require_auth!(&req, &state);

    let order_id = path.into_inner();

    // Validate order ID format
    validate_uuid(&order_id, "order_id")?;

    let order = state
        .db
        .get_order(&order_id)
        .await
        .map_err(ApiError::from)?;

    match order {
        Some(o) => {
            // SECURITY: Only order owner can view their order
            if o.owner != user.wallet_address {
                return Err(ApiError::forbidden("You can only view your own orders"));
            }
            Ok(HttpResponse::Ok().json(o))
        }
        None => Err(ApiError::not_found("Order")),
    }
}

/// Place a new order
/// Supports Idempotency-Key header to prevent duplicate orders
pub async fn place_order(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<PlaceOrderRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_order_write_mode(&state)?;

    // SECURITY: Extract authenticated user from request
    let user = require_auth!(&req, &state);

    let owner = user.wallet_address;

    // SECURITY: Per-user rate limit (10/min for JWT, 120/min for API key)
    let tier = order_tier_for(&user.auth_method);
    check_rate_limit_by_user(&owner, &state.redis, tier).await?;

    // Check for idempotency key
    let idempotency_key = get_idempotency_key(req.headers());

    if let Some(ref key) = idempotency_key {
        // Validate key format (UUID or similar)
        if key.len() > 64 || key.is_empty() {
            return Err(ApiError::bad_request(
                "INVALID_IDEMPOTENCY_KEY",
                "Idempotency key must be 1-64 characters",
            ));
        }

        // Combine with user wallet to prevent cross-user key collisions
        let full_key = format!("{}:{}", owner, key);

        // Check if we have a cached response
        if let Ok(Some(cached)) = state.redis.check_idempotency_key(&full_key).await {
            log::info!("Returning cached response for idempotency key: {}", key);
            return Ok(HttpResponse::Created()
                .content_type("application/json")
                .body(cached));
        }

        // Try to acquire lock for concurrent request handling
        match state.redis.acquire_idempotency_lock(&full_key).await {
            Ok(true) => {
                // Lock acquired, proceed with order
            }
            Ok(false) => {
                // Another request is processing with same key
                return Err(ApiError::conflict(
                    "DUPLICATE_REQUEST",
                    "Request with this idempotency key is already being processed",
                ));
            }
            Err(e) => {
                log::error!("Failed to acquire idempotency lock: {}", e);
                // Fail open - proceed without idempotency protection
            }
        }
    }

    // Validate all inputs using centralized validation
    validate_market_id(&body.market_id)?;
    validate_order_price(body.price)?;
    validate_order_quantity(body.quantity)?;

    // Validate expiration if provided
    if let Some(expires_at) = body.expires_at {
        let now = Utc::now();
        if expires_at <= now {
            return Err(ApiError::bad_request(
                "INVALID_EXPIRATION",
                "Expiration time must be in the future",
            ));
        }
    }

    let now = Utc::now();
    let order_id = Uuid::new_v4().to_string();
    let price_bps = (body.price * 10000.0) as u16;

    let order = Order {
        id: order_id.clone(),
        order_id: 0, // Would be assigned by on-chain program
        market_id: body.market_id.clone(),
        owner: owner.clone(),
        side: body.side,
        outcome: body.outcome,
        order_type: body.order_type,
        price: body.price,
        price_bps,
        quantity: body.quantity,
        filled_quantity: 0,
        remaining_quantity: body.quantity,
        status: OrderStatus::Open,
        is_private: body.private,
        tx_signature: None,
        created_at: now,
        updated_at: now,
        expires_at: body.expires_at,
    };

    // Add to order book and attempt matching
    let matches = state.orderbook.add_order(&order);

    // Calculate filled amount
    let total_filled: u64 = matches.iter().map(|m| m.fill_quantity).sum();
    let remaining = body.quantity.saturating_sub(total_filled);

    let final_status = if remaining == 0 {
        OrderStatus::Filled
    } else if total_filled > 0 {
        OrderStatus::PartiallyFilled
    } else {
        OrderStatus::Open
    };

    // Update order with fill info
    let mut final_order = order.clone();
    final_order.filled_quantity = total_filled;
    final_order.remaining_quantity = remaining;
    final_order.status = final_status;
    final_order.updated_at = Utc::now();

    let mut maker_fill_totals = HashMap::<String, u64>::new();
    for matched_trade in &matches {
        let maker_order_id = match body.side {
            OrderSide::Buy => matched_trade.sell_order_id.as_str(),
            OrderSide::Sell => matched_trade.buy_order_id.as_str(),
        };
        maker_fill_totals
            .entry(maker_order_id.to_string())
            .and_modify(|total| *total += matched_trade.fill_quantity)
            .or_insert(matched_trade.fill_quantity);
    }

    let mut maker_updates = Vec::with_capacity(maker_fill_totals.len());
    for (maker_order_id, filled_delta) in maker_fill_totals {
        let mut maker_order = match state.db.get_order(maker_order_id.as_str()).await {
            Ok(Some(order)) => order,
            Ok(None) => {
                reload_orderbook_from_database(state.get_ref()).await;
                return Err(ApiError::internal(&format!(
                    "matched resting order {} was missing from storage",
                    maker_order_id
                )));
            }
            Err(err) => {
                reload_orderbook_from_database(state.get_ref()).await;
                return Err(ApiError::internal(&format!(
                    "failed to load matched resting order {}: {}",
                    maker_order_id, err
                )));
            }
        };
        maker_order.filled_quantity = maker_order.filled_quantity.saturating_add(filled_delta);
        maker_order.remaining_quantity =
            maker_order.remaining_quantity.saturating_sub(filled_delta);
        maker_order.status = if maker_order.remaining_quantity == 0 {
            OrderStatus::Filled
        } else {
            OrderStatus::PartiallyFilled
        };
        maker_order.updated_at = Utc::now();
        maker_updates.push(maker_order);
    }

    let settlements = matches
        .iter()
        .map(|matched_trade| build_trade_settlement(matched_trade, Utc::now()))
        .collect::<Vec<_>>();

    if let Err(err) = state
        .db
        .persist_local_order_flow(&final_order, &maker_updates, &settlements)
        .await
    {
        reload_orderbook_from_database(state.get_ref()).await;
        return Err(ApiError::from(err));
    }

    // Process matches after durable persistence
    for matched_trade in &matches {
        let outcome_str = match matched_trade.outcome {
            Outcome::Yes => "yes",
            Outcome::No => "no",
        };
        state
            .redis
            .publish_trade(
                matched_trade.market_id.as_str(),
                outcome_str,
                matched_trade.fill_price_bps as f64 / 10000.0,
                matched_trade.fill_quantity,
            )
            .await
            .ok();
    }
    let outcome_str = match final_order.outcome {
        Outcome::Yes => "yes",
        Outcome::No => "no",
    };
    let side_str = match final_order.side {
        OrderSide::Buy => "bid",
        OrderSide::Sell => "ask",
    };
    state
        .redis
        .publish_orderbook_update(
            &final_order.market_id,
            outcome_str,
            side_str,
            final_order.price,
            remaining,
        )
        .await
        .ok();

    let response = PlaceOrderResponse {
        order_id,
        market_id: body.market_id.clone(),
        side: body.side,
        outcome: body.outcome,
        order_type: body.order_type,
        price: body.price,
        quantity: body.quantity,
        filled_quantity: total_filled,
        status: final_status,
        created_at: now,
        expires_at: body.expires_at,
        tx_signature: None,
    };

    // Store idempotency key if provided
    if let Some(ref key) = idempotency_key {
        let full_key = format!("{}:{}", owner, key);
        if let Ok(json) = serde_json::to_string(&response) {
            state
                .redis
                .store_idempotency_key(&full_key, &json)
                .await
                .ok();
        }
        state.redis.release_idempotency_lock(&full_key).await.ok();
    }

    Ok(HttpResponse::Created().json(response))
}

/// Cancel an open order
pub async fn cancel_order(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    ensure_order_write_mode(&state)?;

    // SECURITY: Extract authenticated user from request
    let user = require_auth!(&req, &state);

    let order_id = path.into_inner();

    // Get the order
    let order = state
        .db
        .get_order(&order_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Order"))?;

    // SECURITY: Verify ownership - only order owner can cancel
    if order.owner != user.wallet_address {
        return Err(ApiError::forbidden("You can only cancel your own orders"));
    }

    // Check if order can be cancelled
    if order.status == OrderStatus::Filled {
        return Err(ApiError::bad_request(
            "ORDER_FILLED",
            "Cannot cancel a filled order",
        ));
    }
    if order.status == OrderStatus::Cancelled {
        return Err(ApiError::bad_request(
            "ORDER_CANCELLED",
            "Order is already cancelled",
        ));
    }

    // Remove from order book
    state
        .orderbook
        .remove_order(&order.market_id, order.outcome, order.side, &order_id);

    // Remove from persistent order book
    state.db.remove_orderbook_entry(&order_id).await.ok();

    // Update database
    state
        .db
        .update_order_status(&order_id, OrderStatus::Cancelled, order.filled_quantity, 0)
        .await
        .map_err(ApiError::from)?;

    let now = Utc::now();

    Ok(HttpResponse::Ok().json(CancelOrderResponse {
        order_id,
        status: OrderStatus::Cancelled,
        cancelled_at: now,
        tx_signature: None,
    }))
}

// ── Batch Order Types ───────────────────────────────────────────────

const MAX_BATCH_SIZE: usize = 20;

#[derive(Debug, Deserialize)]
pub struct BatchPlaceOrderRequest {
    pub orders: Vec<PlaceOrderRequest>,
}

#[derive(Debug, Serialize)]
pub struct BatchPlaceOrderResponse {
    pub results: Vec<PlaceOrderResponse>,
}

#[derive(Debug, Deserialize)]
pub struct BatchCancelRequest {
    pub order_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchCancelResponse {
    pub results: Vec<BatchCancelResult>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum BatchCancelResult {
    Ok(CancelOrderResponse),
    Error {
        order_id: String,
        code: String,
        message: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct ReplaceOrderRequest {
    pub cancel_order_ids: Vec<String>,
    pub place_orders: Vec<PlaceOrderRequest>,
}

#[derive(Debug, Serialize)]
pub struct ReplaceOrderResponse {
    pub cancelled: Vec<BatchCancelResult>,
    pub placed: Vec<PlaceOrderResponse>,
}

// ── Batch Place Orders ──────────────────────────────────────────────

/// Place multiple orders atomically (up to 20).
/// All orders are validated upfront — if any order is invalid, the entire batch is rejected.
pub async fn batch_place_orders(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<BatchPlaceOrderRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_order_write_mode(&state)?;
    let user = require_auth!(&req, &state);
    let owner = user.wallet_address;

    let orders_req = &body.orders;
    if orders_req.is_empty() || orders_req.len() > MAX_BATCH_SIZE {
        return Err(ApiError::bad_request(
            "INVALID_BATCH_SIZE",
            &format!("Batch must contain 1-{} orders", MAX_BATCH_SIZE),
        ));
    }

    // Rate limit: count as N orders
    let tier = order_tier_for(&user.auth_method);
    for _ in 0..orders_req.len() {
        check_rate_limit_by_user(&owner, &state.redis, tier).await?;
    }

    // Idempotency for entire batch
    let idempotency_key = get_idempotency_key(req.headers());
    if let Some(ref key) = idempotency_key {
        if key.len() > 64 || key.is_empty() {
            return Err(ApiError::bad_request(
                "INVALID_IDEMPOTENCY_KEY",
                "Idempotency key must be 1-64 characters",
            ));
        }
        let full_key = format!("batch:{}:{}", owner, key);
        if let Ok(Some(cached)) = state.redis.check_idempotency_key(&full_key).await {
            return Ok(HttpResponse::Created()
                .content_type("application/json")
                .body(cached));
        }
        match state.redis.acquire_idempotency_lock(&full_key).await {
            Ok(true) => {}
            Ok(false) => {
                return Err(ApiError::conflict(
                    "DUPLICATE_REQUEST",
                    "Request with this idempotency key is already being processed",
                ));
            }
            Err(e) => {
                log::error!("Failed to acquire batch idempotency lock: {}", e);
            }
        }
    }

    // Phase 1: Validate all orders upfront
    let now = Utc::now();
    let mut prepared_orders = Vec::with_capacity(orders_req.len());

    for (idx, req_order) in orders_req.iter().enumerate() {
        validate_market_id(&req_order.market_id).map_err(|e| {
            ApiError::bad_request("INVALID_ORDER", &format!("Order {}: {}", idx, e.message))
        })?;
        validate_order_price(req_order.price).map_err(|e| {
            ApiError::bad_request("INVALID_ORDER", &format!("Order {}: {}", idx, e.message))
        })?;
        validate_order_quantity(req_order.quantity).map_err(|e| {
            ApiError::bad_request("INVALID_ORDER", &format!("Order {}: {}", idx, e.message))
        })?;

        if let Some(expires_at) = req_order.expires_at {
            if expires_at <= now {
                return Err(ApiError::bad_request(
                    "INVALID_EXPIRATION",
                    &format!("Order {}: expiration must be in the future", idx),
                ));
            }
        }

        let order_id = Uuid::new_v4().to_string();
        let price_bps = (req_order.price * 10000.0) as u16;

        prepared_orders.push(Order {
            id: order_id,
            order_id: 0,
            market_id: req_order.market_id.clone(),
            owner: owner.clone(),
            side: req_order.side,
            outcome: req_order.outcome,
            order_type: req_order.order_type,
            price: req_order.price,
            price_bps,
            quantity: req_order.quantity,
            filled_quantity: 0,
            remaining_quantity: req_order.quantity,
            status: OrderStatus::Open,
            is_private: req_order.private,
            tx_signature: None,
            created_at: now,
            updated_at: now,
            expires_at: req_order.expires_at,
        });
    }

    // Phase 2: Acquire orderbook lock ONCE, match all orders
    let batch_matches = state.orderbook.add_orders_batch(&prepared_orders);

    // Phase 3: Compute final states, build maker updates and settlements
    let mut all_maker_updates = Vec::new();
    let mut all_settlements = Vec::new();
    let mut final_orders = Vec::with_capacity(prepared_orders.len());
    let mut results = Vec::with_capacity(prepared_orders.len());

    for (mut order, matches) in prepared_orders.into_iter().zip(batch_matches.into_iter()) {
        let total_filled: u64 = matches.iter().map(|m| m.fill_quantity).sum();
        let remaining = order.quantity.saturating_sub(total_filled);

        order.filled_quantity = total_filled;
        order.remaining_quantity = remaining;
        order.status = if remaining == 0 {
            OrderStatus::Filled
        } else if total_filled > 0 {
            OrderStatus::PartiallyFilled
        } else {
            OrderStatus::Open
        };
        order.updated_at = Utc::now();

        // Aggregate maker fill totals
        let mut maker_fill_totals = HashMap::<String, u64>::new();
        for matched_trade in &matches {
            let maker_order_id = match order.side {
                OrderSide::Buy => matched_trade.sell_order_id.as_str(),
                OrderSide::Sell => matched_trade.buy_order_id.as_str(),
            };
            maker_fill_totals
                .entry(maker_order_id.to_string())
                .and_modify(|total| *total += matched_trade.fill_quantity)
                .or_insert(matched_trade.fill_quantity);
        }

        for (maker_order_id, filled_delta) in maker_fill_totals {
            let mut maker_order = match state.db.get_order(maker_order_id.as_str()).await {
                Ok(Some(o)) => o,
                Ok(None) => {
                    reload_orderbook_from_database(state.get_ref()).await;
                    return Err(ApiError::internal(&format!(
                        "matched resting order {} was missing from storage",
                        maker_order_id
                    )));
                }
                Err(err) => {
                    reload_orderbook_from_database(state.get_ref()).await;
                    return Err(ApiError::internal(&format!(
                        "failed to load matched resting order {}: {}",
                        maker_order_id, err
                    )));
                }
            };
            maker_order.filled_quantity = maker_order.filled_quantity.saturating_add(filled_delta);
            maker_order.remaining_quantity =
                maker_order.remaining_quantity.saturating_sub(filled_delta);
            maker_order.status = if maker_order.remaining_quantity == 0 {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };
            maker_order.updated_at = Utc::now();
            all_maker_updates.push(maker_order);
        }

        let settlements: Vec<_> = matches
            .iter()
            .map(|m| build_trade_settlement(m, Utc::now()))
            .collect();
        all_settlements.extend(settlements);

        results.push(PlaceOrderResponse {
            order_id: order.id.clone(),
            market_id: order.market_id.clone(),
            side: order.side,
            outcome: order.outcome,
            order_type: order.order_type,
            price: order.price,
            quantity: order.quantity,
            filled_quantity: total_filled,
            status: order.status,
            created_at: order.created_at,
            expires_at: order.expires_at,
            tx_signature: None,
        });

        final_orders.push(order);
    }

    // Phase 4: Atomic persistence
    if let Err(err) = state
        .db
        .persist_batch_order_flow(&final_orders, &all_maker_updates, &all_settlements)
        .await
    {
        reload_orderbook_from_database(state.get_ref()).await;
        return Err(ApiError::from(err));
    }

    // Phase 5: Publish events after commit
    for settlement in &all_settlements {
        let outcome_str = match settlement.trade.outcome {
            Outcome::Yes => "yes",
            Outcome::No => "no",
        };
        state
            .redis
            .publish_trade(
                settlement.trade.market_id.as_str(),
                outcome_str,
                settlement.trade.price,
                settlement.trade.quantity,
            )
            .await
            .ok();
    }
    for order in &final_orders {
        let outcome_str = match order.outcome {
            Outcome::Yes => "yes",
            Outcome::No => "no",
        };
        let side_str = match order.side {
            OrderSide::Buy => "bid",
            OrderSide::Sell => "ask",
        };
        state
            .redis
            .publish_orderbook_update(
                &order.market_id,
                outcome_str,
                side_str,
                order.price,
                order.remaining_quantity,
            )
            .await
            .ok();
    }

    let response = BatchPlaceOrderResponse { results };

    // Store idempotency result
    if let Some(ref key) = idempotency_key {
        let full_key = format!("batch:{}:{}", owner, key);
        if let Ok(json) = serde_json::to_string(&response) {
            state
                .redis
                .store_idempotency_key(&full_key, &json)
                .await
                .ok();
        }
        state.redis.release_idempotency_lock(&full_key).await.ok();
    }

    Ok(HttpResponse::Created().json(response))
}

// ── Batch Cancel Orders ─────────────────────────────────────────────

/// Cancel multiple orders at once (up to 20). Individual results per order.
pub async fn batch_cancel_orders(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<BatchCancelRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_order_write_mode(&state)?;
    let user = require_auth!(&req, &state);

    if body.order_ids.is_empty() || body.order_ids.len() > MAX_BATCH_SIZE {
        return Err(ApiError::bad_request(
            "INVALID_BATCH_SIZE",
            &format!("Batch must contain 1-{} order IDs", MAX_BATCH_SIZE),
        ));
    }

    // Deduplicate order IDs (preserve first occurrence order)
    let mut seen = std::collections::HashSet::new();
    let unique_order_ids: Vec<&String> = body
        .order_ids
        .iter()
        .filter(|id| seen.insert(id.as_str().to_string()))
        .collect();

    let mut results = Vec::with_capacity(unique_order_ids.len());
    let mut cancels_for_orderbook = Vec::new();
    let now = Utc::now();

    for order_id in &unique_order_ids {
        if validate_uuid(order_id, "order_id").is_err() {
            results.push(BatchCancelResult::Error {
                order_id: order_id.to_string(),
                code: "INVALID_ORDER_ID".into(),
                message: "Invalid order ID format".into(),
            });
            continue;
        }
        match state.db.get_order(order_id).await {
            Ok(Some(order)) => {
                if order.owner != user.wallet_address {
                    results.push(BatchCancelResult::Error {
                        order_id: order_id.to_string(),
                        code: "FORBIDDEN".into(),
                        message: "Not your order".into(),
                    });
                    continue;
                }
                if order.status == OrderStatus::Filled || order.status == OrderStatus::Cancelled {
                    results.push(BatchCancelResult::Error {
                        order_id: order_id.to_string(),
                        code: "CANNOT_CANCEL".into(),
                        message: format!("Order status is {:?}", order.status),
                    });
                    continue;
                }
                cancels_for_orderbook.push((
                    order.market_id.clone(),
                    order.outcome,
                    order.side,
                    order_id.to_string(),
                ));
                results.push(BatchCancelResult::Ok(CancelOrderResponse {
                    order_id: order_id.to_string(),
                    status: OrderStatus::Cancelled,
                    cancelled_at: now,
                    tx_signature: None,
                }));
            }
            Ok(None) => {
                results.push(BatchCancelResult::Error {
                    order_id: order_id.to_string(),
                    code: "NOT_FOUND".into(),
                    message: "Order not found".into(),
                });
            }
            Err(_) => {
                results.push(BatchCancelResult::Error {
                    order_id: order_id.to_string(),
                    code: "INTERNAL_ERROR".into(),
                    message: "Failed to fetch order".into(),
                });
            }
        }
    }

    // Batch remove from orderbook (single lock)
    state.orderbook.remove_orders_batch(&cancels_for_orderbook);

    // Update DB for each cancelled order
    for (_, _, _, order_id) in &cancels_for_orderbook {
        if let Err(e) = state
            .db
            .update_order_status(order_id, OrderStatus::Cancelled, 0, 0)
            .await
        {
            log::error!("Failed to update cancelled order {} in DB: {}", order_id, e);
        }
        if let Err(e) = state.db.remove_orderbook_entry(order_id).await {
            log::error!("Failed to remove orderbook entry for {}: {}", order_id, e);
        }
    }

    Ok(HttpResponse::Ok().json(BatchCancelResponse { results }))
}

// ── Replace Orders (Cancel + Place Atomically) ─────────────────────

/// Cancel existing orders and place new ones in a single operation.
/// Used by market makers to update quotes atomically.
pub async fn replace_orders(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<ReplaceOrderRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_order_write_mode(&state)?;
    let user = require_auth!(&req, &state);
    let owner = user.wallet_address;

    let total_ops = body.cancel_order_ids.len() + body.place_orders.len();
    if total_ops == 0 || total_ops > MAX_BATCH_SIZE {
        return Err(ApiError::bad_request(
            "INVALID_BATCH_SIZE",
            &format!("Total operations must be 1-{}", MAX_BATCH_SIZE),
        ));
    }

    // Rate limit the new orders
    let tier = order_tier_for(&user.auth_method);
    for _ in 0..body.place_orders.len() {
        check_rate_limit_by_user(&owner, &state.redis, tier).await?;
    }

    let now = Utc::now();

    // ── Cancel phase ────────────────────────────────────────────
    let mut seen_cancels = std::collections::HashSet::new();
    let unique_cancel_ids: Vec<&String> = body
        .cancel_order_ids
        .iter()
        .filter(|id| seen_cancels.insert(id.as_str().to_string()))
        .collect();

    let mut cancel_results = Vec::with_capacity(unique_cancel_ids.len());
    let mut cancels_for_orderbook = Vec::new();

    for order_id in &unique_cancel_ids {
        if validate_uuid(order_id, "order_id").is_err() {
            cancel_results.push(BatchCancelResult::Error {
                order_id: order_id.to_string(),
                code: "INVALID_ORDER_ID".into(),
                message: "Invalid order ID format".into(),
            });
            continue;
        }
        match state.db.get_order(order_id).await {
            Ok(Some(order)) => {
                if order.owner != owner {
                    cancel_results.push(BatchCancelResult::Error {
                        order_id: order_id.to_string(),
                        code: "FORBIDDEN".into(),
                        message: "Not your order".into(),
                    });
                    continue;
                }
                if order.status == OrderStatus::Filled || order.status == OrderStatus::Cancelled {
                    cancel_results.push(BatchCancelResult::Error {
                        order_id: order_id.to_string(),
                        code: "CANNOT_CANCEL".into(),
                        message: format!("Order status is {:?}", order.status),
                    });
                    continue;
                }
                cancels_for_orderbook.push((
                    order.market_id.clone(),
                    order.outcome,
                    order.side,
                    order_id.to_string(),
                ));
                cancel_results.push(BatchCancelResult::Ok(CancelOrderResponse {
                    order_id: order_id.to_string(),
                    status: OrderStatus::Cancelled,
                    cancelled_at: now,
                    tx_signature: None,
                }));
            }
            Ok(None) => {
                cancel_results.push(BatchCancelResult::Error {
                    order_id: order_id.to_string(),
                    code: "NOT_FOUND".into(),
                    message: "Order not found".into(),
                });
            }
            Err(_) => {
                cancel_results.push(BatchCancelResult::Error {
                    order_id: order_id.to_string(),
                    code: "INTERNAL_ERROR".into(),
                    message: "Failed to fetch order".into(),
                });
            }
        }
    }

    // Remove cancelled orders from orderbook
    state.orderbook.remove_orders_batch(&cancels_for_orderbook);
    for (_, _, _, order_id) in &cancels_for_orderbook {
        if let Err(e) = state
            .db
            .update_order_status(order_id, OrderStatus::Cancelled, 0, 0)
            .await
        {
            log::error!("Failed to update cancelled order {} in DB: {}", order_id, e);
        }
        if let Err(e) = state.db.remove_orderbook_entry(order_id).await {
            log::error!("Failed to remove orderbook entry for {}: {}", order_id, e);
        }
    }

    // ── Place phase ─────────────────────────────────────────────
    let mut prepared_orders = Vec::with_capacity(body.place_orders.len());

    for (idx, req_order) in body.place_orders.iter().enumerate() {
        validate_market_id(&req_order.market_id).map_err(|e| {
            ApiError::bad_request("INVALID_ORDER", &format!("Order {}: {}", idx, e.message))
        })?;
        validate_order_price(req_order.price).map_err(|e| {
            ApiError::bad_request("INVALID_ORDER", &format!("Order {}: {}", idx, e.message))
        })?;
        validate_order_quantity(req_order.quantity).map_err(|e| {
            ApiError::bad_request("INVALID_ORDER", &format!("Order {}: {}", idx, e.message))
        })?;

        if let Some(expires_at) = req_order.expires_at {
            if expires_at <= now {
                return Err(ApiError::bad_request(
                    "INVALID_EXPIRATION",
                    &format!("Order {}: expiration must be in the future", idx),
                ));
            }
        }

        let order_id = Uuid::new_v4().to_string();
        let price_bps = (req_order.price * 10000.0) as u16;

        prepared_orders.push(Order {
            id: order_id,
            order_id: 0,
            market_id: req_order.market_id.clone(),
            owner: owner.clone(),
            side: req_order.side,
            outcome: req_order.outcome,
            order_type: req_order.order_type,
            price: req_order.price,
            price_bps,
            quantity: req_order.quantity,
            filled_quantity: 0,
            remaining_quantity: req_order.quantity,
            status: OrderStatus::Open,
            is_private: req_order.private,
            tx_signature: None,
            created_at: now,
            updated_at: now,
            expires_at: req_order.expires_at,
        });
    }

    let batch_matches = state.orderbook.add_orders_batch(&prepared_orders);

    let mut all_maker_updates = Vec::new();
    let mut all_settlements = Vec::new();
    let mut final_orders = Vec::with_capacity(prepared_orders.len());
    let mut place_results = Vec::with_capacity(prepared_orders.len());

    for (mut order, matches) in prepared_orders.into_iter().zip(batch_matches.into_iter()) {
        let total_filled: u64 = matches.iter().map(|m| m.fill_quantity).sum();
        let remaining = order.quantity.saturating_sub(total_filled);

        order.filled_quantity = total_filled;
        order.remaining_quantity = remaining;
        order.status = if remaining == 0 {
            OrderStatus::Filled
        } else if total_filled > 0 {
            OrderStatus::PartiallyFilled
        } else {
            OrderStatus::Open
        };
        order.updated_at = Utc::now();

        let mut maker_fill_totals = HashMap::<String, u64>::new();
        for matched_trade in &matches {
            let maker_order_id = match order.side {
                OrderSide::Buy => matched_trade.sell_order_id.as_str(),
                OrderSide::Sell => matched_trade.buy_order_id.as_str(),
            };
            maker_fill_totals
                .entry(maker_order_id.to_string())
                .and_modify(|total| *total += matched_trade.fill_quantity)
                .or_insert(matched_trade.fill_quantity);
        }

        for (maker_order_id, filled_delta) in maker_fill_totals {
            let mut maker_order = match state.db.get_order(maker_order_id.as_str()).await {
                Ok(Some(o)) => o,
                Ok(None) => {
                    reload_orderbook_from_database(state.get_ref()).await;
                    return Err(ApiError::internal(&format!(
                        "matched resting order {} missing",
                        maker_order_id
                    )));
                }
                Err(err) => {
                    reload_orderbook_from_database(state.get_ref()).await;
                    return Err(ApiError::internal(&format!(
                        "failed to load resting order {}: {}",
                        maker_order_id, err
                    )));
                }
            };
            maker_order.filled_quantity = maker_order.filled_quantity.saturating_add(filled_delta);
            maker_order.remaining_quantity =
                maker_order.remaining_quantity.saturating_sub(filled_delta);
            maker_order.status = if maker_order.remaining_quantity == 0 {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };
            maker_order.updated_at = Utc::now();
            all_maker_updates.push(maker_order);
        }

        let settlements: Vec<_> = matches
            .iter()
            .map(|m| build_trade_settlement(m, Utc::now()))
            .collect();
        all_settlements.extend(settlements);

        place_results.push(PlaceOrderResponse {
            order_id: order.id.clone(),
            market_id: order.market_id.clone(),
            side: order.side,
            outcome: order.outcome,
            order_type: order.order_type,
            price: order.price,
            quantity: order.quantity,
            filled_quantity: total_filled,
            status: order.status,
            created_at: order.created_at,
            expires_at: order.expires_at,
            tx_signature: None,
        });

        final_orders.push(order);
    }

    if !final_orders.is_empty() {
        if let Err(err) = state
            .db
            .persist_batch_order_flow(&final_orders, &all_maker_updates, &all_settlements)
            .await
        {
            reload_orderbook_from_database(state.get_ref()).await;
            return Err(ApiError::from(err));
        }
    }

    // Publish events
    for settlement in &all_settlements {
        let outcome_str = match settlement.trade.outcome {
            Outcome::Yes => "yes",
            Outcome::No => "no",
        };
        state
            .redis
            .publish_trade(
                settlement.trade.market_id.as_str(),
                outcome_str,
                settlement.trade.price,
                settlement.trade.quantity,
            )
            .await
            .ok();
    }

    Ok(HttpResponse::Ok().json(ReplaceOrderResponse {
        cancelled: cancel_results,
        placed: place_results,
    }))
}
