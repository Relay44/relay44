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
    Ok(())
}

fn contains_dangerous_chars(input: &str) -> bool {
    let patterns = [
        "<script",
        "</script",
        "javascript:",
        "onerror=",
        "onload=",
        "onclick=",
        "DROP TABLE",
        "DELETE FROM",
        "INSERT INTO",
        "UPDATE ",
        "UNION SELECT",
        "--",
        "/*",
        "*/",
    ];
    let lower = input.to_lowercase();
    patterns.iter().any(|p| lower.contains(&p.to_lowercase()))
}

pub fn sanitize_string(input: &str, max_length: usize) -> String {
    input.trim().chars().take(max_length).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_wallet_address() {
        // Valid addresses
        assert!(validate_wallet_address("0x71C7656EC7ab88b098defB751B7401B5f6d8976F").is_ok());
        assert!(validate_wallet_address("0xA0b86991c6218b36c1d19d4a2e9Eb0cE3606eb48").is_ok());

        // Invalid addresses
        assert!(validate_wallet_address("").is_err());
        assert!(validate_wallet_address("short").is_err());
        assert!(validate_wallet_address("0xInvalidEvmAddress").is_err());
        assert!(validate_wallet_address("contains spaces here").is_err());
    }

    #[test]
    fn test_validate_order_price() {
        // Valid prices
        assert!(validate_order_price(0.5).is_ok());
        assert!(validate_order_price(0.0001).is_ok());
        assert!(validate_order_price(0.9999).is_ok());

        // Invalid prices
        assert!(validate_order_price(0.0).is_err());
        assert!(validate_order_price(1.0).is_err());
        assert!(validate_order_price(-0.5).is_err());
        assert!(validate_order_price(1.5).is_err());
        assert!(validate_order_price(f64::NAN).is_err());
        assert!(validate_order_price(f64::INFINITY).is_err());
    }

    #[test]
    fn test_validate_order_quantity() {
        // Valid quantities
        assert!(validate_order_quantity(1).is_ok());
        assert!(validate_order_quantity(1000).is_ok());
        assert!(validate_order_quantity(1_000_000_000).is_ok());

        // Invalid quantities
        assert!(validate_order_quantity(0).is_err());
        assert!(validate_order_quantity(1_000_000_001).is_err());
    }

    #[test]
    fn test_validate_market_question() {
        // Valid questions
        assert!(validate_market_question("Will BTC reach $100k by 2025?").is_ok());
        assert!(validate_market_question("Simple question").is_ok());

        // Invalid questions
        assert!(validate_market_question("").is_err());
        assert!(validate_market_question("   ").is_err());
        assert!(validate_market_question("<script>alert('xss')</script>").is_err());
        assert!(validate_market_question("DROP TABLE markets;").is_err());
    }

    #[test]
    fn test_validate_pagination() {
        // Valid pagination
        assert!(validate_pagination(Some(50), Some(0)).is_ok());
        assert!(validate_pagination(None, None).is_ok());
        assert!(validate_pagination(Some(100), Some(100)).is_ok());

        // Invalid pagination
        assert!(validate_pagination(Some(0), None).is_err());
        assert!(validate_pagination(Some(101), None).is_err());
        assert!(validate_pagination(None, Some(-1)).is_err());
    }

    #[test]
    fn test_contains_dangerous_chars() {
        assert!(contains_dangerous_chars("<script>"));
        assert!(contains_dangerous_chars("DROP TABLE users"));
        assert!(contains_dangerous_chars("SELECT * FROM users--"));

        assert!(!contains_dangerous_chars("Normal question about markets"));
        assert!(!contains_dangerous_chars("Will ETH > $5000?"));
    }

    #[test]
    fn test_validate_market_id() {
        // Valid market IDs
        assert!(validate_market_id("market-123").is_ok());
        assert!(validate_market_id("BTC_USD_2025").is_ok());
        assert!(validate_market_id("a").is_ok());
        assert!(validate_market_id("abcdefghij1234567890_-").is_ok());

        // Invalid market IDs
        assert!(validate_market_id("").is_err());
        assert!(validate_market_id("market id with spaces").is_err());
        assert!(validate_market_id("market!@#").is_err());
    }

    #[test]
    fn test_validate_uuid() {
        // Valid UUIDs
        assert!(validate_uuid("550e8400-e29b-41d4-a716-446655440000", "order_id").is_ok());
        assert!(validate_uuid("00000000-0000-0000-0000-000000000000", "id").is_ok());

        // Invalid UUIDs
        assert!(validate_uuid("not-a-uuid", "id").is_err());
        assert!(validate_uuid("", "id").is_err());
        assert!(validate_uuid("550e8400e29b41d4a716446655440000", "id").is_err());
    }

    #[test]
    fn test_validate_description() {
        // Valid descriptions
        assert!(validate_description(None).is_ok());
        assert!(validate_description(Some("A valid description")).is_ok());
        assert!(validate_description(Some("")).is_ok());

        // Invalid descriptions
        assert!(validate_description(Some("<script>alert('xss')</script>")).is_err());
        let long_desc = "a".repeat(limits::MAX_DESCRIPTION_LENGTH + 1);
        assert!(validate_description(Some(&long_desc)).is_err());
    }

    #[test]
    fn test_validate_fee_bps() {
        // Valid fees
        assert!(validate_fee_bps(0).is_ok());
        assert!(validate_fee_bps(100).is_ok());
        assert!(validate_fee_bps(limits::MAX_FEE_BPS).is_ok());

        // Invalid fees
        assert!(validate_fee_bps(limits::MAX_FEE_BPS + 1).is_err());
    }

    #[test]
    fn test_validate_tx_signature() {
        let valid_sig = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert!(validate_tx_signature(valid_sig).is_ok());

        // Invalid signatures
        assert!(validate_tx_signature("short").is_err());
        assert!(validate_tx_signature("").is_err());
    }

    #[test]
    fn test_sanitize_string() {
        assert_eq!(sanitize_string("  hello  ", 10), "hello");
        assert_eq!(sanitize_string("hello world", 5), "hello");
        assert_eq!(sanitize_string("", 10), "");
        assert_eq!(sanitize_string("   ", 10), "");
    }

    #[test]
    fn test_validate_price_precision() {
        // Valid precisions (up to 4 decimal places)
        assert!(validate_order_price(0.1234).is_ok());
        assert!(validate_order_price(0.1).is_ok());

        // Invalid precision (more than 4 decimal places)
        assert!(validate_order_price(0.12345).is_err());
    }

    #[test]
    fn test_limits_constants() {
        // Verify limits are sensible
        assert!(limits::MAX_ORDER_QUANTITY > limits::MIN_ORDER_QUANTITY);
        assert!(limits::MAX_PAGE_LIMIT > 0);
        assert!(limits::DEFAULT_PAGE_LIMIT <= limits::MAX_PAGE_LIMIT);
        assert!(limits::MAX_FEE_BPS <= 10000); // Cannot exceed 100%
        assert!(limits::MAX_TRADING_WINDOW_SECS > limits::MIN_TRADING_WINDOW_SECS);
    }

    #[test]
    fn test_contains_dangerous_chars_case_insensitive() {
        // Should catch regardless of case
        assert!(contains_dangerous_chars("<SCRIPT>"));
        assert!(contains_dangerous_chars("drop table"));
        assert!(contains_dangerous_chars("DELETE FROM"));
        assert!(contains_dangerous_chars("UNION SELECT"));
    }

    #[test]
    fn test_contains_dangerous_chars_sql_comments() {
        assert!(contains_dangerous_chars("SELECT * -- comment"));
        assert!(contains_dangerous_chars("SELECT /* inline */"));
        assert!(contains_dangerous_chars("end of comment */"));
    }
}
