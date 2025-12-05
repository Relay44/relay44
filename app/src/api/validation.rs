//! Input validation utilities for API requests.
//! Some functions are infrastructure for future use.

#![allow(dead_code)]

use super::ApiError;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref EVM_ADDRESS_REGEX: Regex = Regex::new(r"^0x[0-9a-fA-F]{40}$").unwrap();
    static ref MARKET_ID_REGEX: Regex = Regex::new(r"^[a-zA-Z0-9\-_]{1,64}$").unwrap();
    static ref UUID_REGEX: Regex = Regex::new(
        r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
    )
    .unwrap();
    static ref TX_SIGNATURE_REGEX: Regex = Regex::new(r"^0x[0-9a-fA-F]{64}$").unwrap();
}

pub mod limits {
    pub const MAX_ORDER_QUANTITY: u64 = 1_000_000_000;
    pub const MIN_ORDER_QUANTITY: u64 = 1;
    pub const MAX_QUESTION_LENGTH: usize = 500;
    pub const MAX_DESCRIPTION_LENGTH: usize = 2000;
    pub const MAX_RESOLUTION_DETAILS_LENGTH: usize = 1000;
    pub const MAX_PAGE_LIMIT: i64 = 100;
    pub const DEFAULT_PAGE_LIMIT: i64 = 50;
    pub const MAX_FEE_BPS: u16 = 5000;
    pub const MIN_TRADING_WINDOW_SECS: i64 = 3600;
    pub const MAX_TRADING_WINDOW_SECS: i64 = 365 * 24 * 3600;
}

pub fn validate_wallet_address(address: &str) -> Result<(), ApiError> {
    if address.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "Wallet address cannot be empty",
        ));
    }
    if !EVM_ADDRESS_REGEX.is_match(address) {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "Invalid EVM wallet address format",
        ));
    }
    Ok(())
}

pub fn validate_market_id(market_id: &str) -> Result<(), ApiError> {
    if market_id.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_MARKET_ID",
            "Market ID cannot be empty",
        ));
    }
    if !MARKET_ID_REGEX.is_match(market_id) {
        return Err(ApiError::bad_request(
            "INVALID_MARKET_ID",
            "Invalid market ID format",
        ));
    }
    Ok(())
}

pub fn validate_uuid(id: &str, field_name: &str) -> Result<(), ApiError> {
    if !UUID_REGEX.is_match(id) {
        return Err(ApiError::bad_request(
            "INVALID_ID",
            &format!("Invalid {} format", field_name),
        ));
    }
    Ok(())
}

pub fn validate_order_price(price: f64) -> Result<(), ApiError> {
    if price.is_nan() || price.is_infinite() {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "Price must be a valid number",
        ));
    }
    if price <= 0.0 || price >= 1.0 {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "Price must be between 0 and 1",
        ));
    }
    // Max 4 decimal places (1 bps precision)
    let bps = (price * 10000.0).round() as u16;
    if (price - (bps as f64 / 10000.0)).abs() > 0.00001 {
        return Err(ApiError::bad_request(
            "INVALID_PRICE",
            "Max 4 decimal places",
        ));
    }
    Ok(())
}

pub fn validate_order_quantity(quantity: u64) -> Result<(), ApiError> {
    if quantity < limits::MIN_ORDER_QUANTITY {
        return Err(ApiError::bad_request(
            "INVALID_QUANTITY",
            "Quantity must be at least 1",
        ));
    }
    if quantity > limits::MAX_ORDER_QUANTITY {
        return Err(ApiError::bad_request(
            "INVALID_QUANTITY",
            "Quantity exceeds maximum",
        ));
    }
    Ok(())
}

pub fn validate_market_question(question: &str) -> Result<(), ApiError> {
    let trimmed = question.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_QUESTION",
            "Question cannot be empty",
        ));
    }
    if trimmed.len() > limits::MAX_QUESTION_LENGTH {
        return Err(ApiError::bad_request(
            "INVALID_QUESTION",
            "Question too long",
        ));
    }
    if contains_dangerous_chars(trimmed) {
        return Err(ApiError::bad_request(
            "INVALID_QUESTION",
            "Invalid characters",
        ));
    }
    Ok(())
}

pub fn validate_description(description: Option<&str>) -> Result<(), ApiError> {
    if let Some(desc) = description {
        if desc.len() > limits::MAX_DESCRIPTION_LENGTH {
            return Err(ApiError::bad_request(
                "INVALID_DESCRIPTION",
                "Description too long",
            ));
        }
        if contains_dangerous_chars(desc) {
            return Err(ApiError::bad_request(
                "INVALID_DESCRIPTION",
                "Invalid characters",
            ));
        }
    }
    Ok(())
}

pub fn validate_fee_bps(fee_bps: u16) -> Result<(), ApiError> {
    if fee_bps > limits::MAX_FEE_BPS {
        return Err(ApiError::bad_request("INVALID_FEE", "Fee exceeds maximum"));
    }
    Ok(())
}

pub fn validate_trading_end(trading_end: i64) -> Result<(), ApiError> {
    let now = chrono::Utc::now().timestamp();
    if trading_end <= now {
        return Err(ApiError::bad_request(
            "INVALID_TRADING_END",
            "Must be in the future",
        ));
    }
    let duration = trading_end - now;
    if duration < limits::MIN_TRADING_WINDOW_SECS {
        return Err(ApiError::bad_request(
            "INVALID_TRADING_END",
            "Window too short",
        ));
    }
    if duration > limits::MAX_TRADING_WINDOW_SECS {
        return Err(ApiError::bad_request(
            "INVALID_TRADING_END",
            "Window too long",
        ));
    }
    Ok(())
}

pub fn validate_pagination(
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<(i64, i64), ApiError> {
    let limit = limit.unwrap_or(limits::DEFAULT_PAGE_LIMIT);
    let offset = offset.unwrap_or(0);
    if limit <= 0 {
        return Err(ApiError::bad_request(
            "INVALID_LIMIT",
            "Limit must be positive",
        ));
    }
    if limit > limits::MAX_PAGE_LIMIT {
        return Err(ApiError::bad_request(
            "INVALID_LIMIT",
            "Limit exceeds maximum",
        ));
    }
    if offset < 0 {
        return Err(ApiError::bad_request(
            "INVALID_OFFSET",
            "Offset cannot be negative",
        ));
    }
    Ok((limit, offset))
}

pub fn validate_tx_signature(signature: &str) -> Result<(), ApiError> {
    if !TX_SIGNATURE_REGEX.is_match(signature) {
        return Err(ApiError::bad_request(
            "INVALID_SIGNATURE",
            "Invalid signature format",
        ));
    }
