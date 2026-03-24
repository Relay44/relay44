use actix_web::http::header::HeaderMap;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use super::rate_limit::check_order_rate_limit;
use super::{
    validate_market_id, validate_order_price, validate_order_quantity, validate_pagination,
    validate_uuid, ApiError,
};
use crate::models::{
    CancelOrderResponse, ListOrdersQuery, Order, OrderListResponse, OrderSide, OrderStatus,
    Outcome, PlaceOrderRequest, PlaceOrderResponse,
};
use crate::require_auth;
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

    // SECURITY: Per-user rate limit (10 orders/min)
    check_order_rate_limit(&owner, &state.redis).await?;

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

    // Persist to database order book
    if order.remaining_quantity > 0 {
        state
            .db
            .add_orderbook_entry(
                &order.id,
                &order.market_id,
                order.outcome,
                order.side,
                order.price_bps,
                order.remaining_quantity,
                &order.owner,
            )
            .await
            .ok();
    }

    // Process matches
    for matched_trade in &matches {
        // Publish trade event via Redis
        let outcome_str = match order.outcome {
            Outcome::Yes => "yes",
            Outcome::No => "no",
        };
        state
            .redis
            .publish_trade(
                &order.market_id,
                outcome_str,
                matched_trade.fill_price_bps as f64 / 10000.0,
                matched_trade.fill_quantity,
            )
            .await
            .ok();

        // Update persistent order book for matched orders
        state
            .db
            .update_orderbook_entry_quantity(
                &matched_trade.buy_order_id.to_string(),
                0, // Buyer order filled
            )
            .await
            .ok();
    }

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

    // Save to database
    state
        .db
        .create_order(&final_order)
        .await
        .map_err(ApiError::from)?;

    // Publish order book update
    let outcome_str = match body.outcome {
        Outcome::Yes => "yes",
        Outcome::No => "no",
    };
    let side_str = match body.side {
        OrderSide::Buy => "bid",
        OrderSide::Sell => "ask",
    };
    state
        .redis
        .publish_orderbook_update(
            &body.market_id,
            outcome_str,
            side_str,
            body.price,
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
