//! Limitless Partner Integration.
//!
//! Implements HMAC-authenticated trading via the Limitless Programmatic API.
//! Enables Relay44 agents to trade on Limitless using delegated signing
//! (server-wallet sub-accounts) without requiring end-user private keys.

use chrono::Utc;
use hmac::{Hmac, Mac};
use log::{info, warn};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;

use crate::api::ApiError;
use crate::config::AppConfig;

type HmacSha256 = Hmac<Sha256>;

// ── Configuration ──

/// Partner config loaded from environment.
pub struct LimitlessPartnerConfig {
    pub token_id: String,
    pub secret: String,
    pub profile_id: String,
    pub api_base: String,
}

impl LimitlessPartnerConfig {
    /// Load from env vars. Returns None if not configured.
    pub fn from_config(config: &AppConfig) -> Option<Self> {
        let token_id = std::env::var("LIMITLESS_PARTNER_TOKEN_ID").ok()?;
        let secret = std::env::var("LIMITLESS_PARTNER_SECRET").ok()?;

        if token_id.is_empty() || secret.is_empty() {
            return None;
        }

        let profile_id =
            std::env::var("LIMITLESS_PARTNER_PROFILE_ID").unwrap_or_default();

        Some(Self {
            token_id,
            secret,
            profile_id,
            api_base: config.limitless_api_base.clone(),
        })
    }

    pub fn is_ready(&self) -> bool {
        !self.token_id.is_empty()
            && !self.secret.is_empty()
            && !self.profile_id.is_empty()
    }
}

// ── HMAC Authentication ──

/// Build HMAC-SHA256 authentication headers for Limitless API.
///
/// Headers: lmts-api-key, lmts-timestamp, lmts-signature
fn build_hmac_headers(
    token_id: &str,
    secret: &str,
    method: &str,
    path: &str,
    body: &str,
) -> Result<HeaderMap, ApiError> {
    let timestamp = Utc::now().to_rfc3339();

    // Canonical message: method + path + timestamp + body
    let message = format!("{}\n{}\n{}\n{}", method, path, timestamp, body);

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| ApiError::internal("HMAC key invalid"))?;
    mac.update(message.as_bytes());
    let signature = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        mac.finalize().into_bytes(),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "lmts-api-key",
        HeaderValue::from_str(token_id)
            .map_err(|_| ApiError::internal("Invalid token ID"))?,
    );
    headers.insert(
        "lmts-timestamp",
        HeaderValue::from_str(&timestamp)
            .map_err(|_| ApiError::internal("Invalid timestamp"))?,
    );
    headers.insert(
        "lmts-signature",
        HeaderValue::from_str(&signature)
            .map_err(|_| ApiError::internal("Invalid signature"))?,
    );

    Ok(headers)
}

// ── Sub-Account Management ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerAccount {
    pub profile_id: String,
    pub account: String,
    pub display_name: String,
}

/// Create a server-wallet sub-account for Relay44 agent trading.
pub async fn create_sub_account(
    partner: &LimitlessPartnerConfig,
    display_name: &str,
) -> Result<PartnerAccount, ApiError> {
    let client = reqwest::Client::new();
    let path = "/profiles/partner-accounts";
    let body = json!({
        "displayName": display_name,
        "createServerWallet": true
    });
    let body_str = serde_json::to_string(&body)
        .map_err(|e| ApiError::internal(&format!("JSON serialize: {}", e)))?;

    let headers = build_hmac_headers(
        &partner.token_id,
        &partner.secret,
        "POST",
        path,
        &body_str,
    )?;

    let url = format!(
        "{}{}",
        partner.api_base.trim_end_matches('/'),
        path
    );

    let response = client
        .post(&url)
        .headers(headers)
        .header("Content-Type", "application/json")
        .body(body_str)
        .send()
        .await
        .map_err(|e| ApiError::internal(&format!("Limitless partner API: {}", e)))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| ApiError::internal(&format!("Response read: {}", e)))?;

    if !status.is_success() {
        return Err(ApiError::internal(&format!(
            "Limitless sub-account creation failed ({}): {}",
            status, text
        )));
    }

    let result: Value = serde_json::from_str(&text)
        .map_err(|e| ApiError::internal(&format!("Parse response: {}", e)))?;

    Ok(PartnerAccount {
        profile_id: result
            .get("profileId")
            .or_else(|| result.get("id"))
            .and_then(|v| v.as_str().or_else(|| v.as_u64().map(|_| "")))
            .unwrap_or("")
            .to_string(),
        account: result
            .get("account")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        display_name: display_name.to_string(),
    })
}

// ── Delegated Order Placement ──

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegatedOrderRequest {
    pub market_slug: String,
    pub order_type: String, // "GTC" or "FOK"
    pub token_id: String,
    pub side: String, // "BUY" or "SELL"
    pub price: Option<f64>,   // For GTC
    pub size: Option<f64>,    // For GTC
    pub maker_amount: Option<f64>, // For FOK
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    #[serde(default)]
    pub order_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub maker_matches: Vec<Value>,
}

/// Place an order on Limitless using delegated signing (partner sub-account).
pub async fn place_delegated_order(
    partner: &LimitlessPartnerConfig,
    order: &DelegatedOrderRequest,
) -> Result<OrderResponse, ApiError> {
    if !partner.is_ready() {
        return Err(ApiError::bad_request(
            "LIMITLESS_PARTNER_NOT_CONFIGURED",
            "Partner token/secret/profile not set",
        ));
    }

    let client = reqwest::Client::new();
    let path = "/orders";

    let args = if order.order_type == "FOK" {
        json!({
            "tokenId": order.token_id,
            "side": order.side,
            "makerAmount": order.maker_amount.unwrap_or(0.0)
        })
    } else {
        json!({
            "tokenId": order.token_id,
            "side": order.side,
            "price": order.price.unwrap_or(0.0),
            "size": order.size.unwrap_or(0.0)
        })
    };

    let body = json!({
        "marketSlug": order.market_slug,
        "orderType": order.order_type,
        "onBehalfOf": partner.profile_id,
        "args": args
    });

    let body_str = serde_json::to_string(&body)
        .map_err(|e| ApiError::internal(&format!("JSON serialize: {}", e)))?;

    let headers = build_hmac_headers(
        &partner.token_id,
        &partner.secret,
        "POST",
        path,
        &body_str,
    )?;

    let url = format!(
        "{}{}",
        partner.api_base.trim_end_matches('/'),
        path
    );

    let response = client
        .post(&url)
        .headers(headers)
        .header("Content-Type", "application/json")
        .body(body_str)
        .send()
        .await
        .map_err(|e| ApiError::internal(&format!("Limitless order API: {}", e)))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| ApiError::internal(&format!("Response read: {}", e)))?;

    if !status.is_success() {
        return Err(ApiError::bad_request(
            "LIMITLESS_ORDER_FAILED",
            &format!("Order failed ({}): {}", status, text),
        ));
    }

    let result: Value = serde_json::from_str(&text)
        .map_err(|e| ApiError::internal(&format!("Parse response: {}", e)))?;

    Ok(OrderResponse {
        order_id: result
            .get("orderId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        status: result
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("submitted")
            .to_string(),
        maker_matches: result
            .get("makerMatches")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
    })
}

/// Cancel an order on Limitless using partner credentials.
pub async fn cancel_order(
    partner: &LimitlessPartnerConfig,
    order_id: &str,
) -> Result<(), ApiError> {
    let client = reqwest::Client::new();
    let path = format!("/orders/{}", order_id);

    let headers = build_hmac_headers(
        &partner.token_id,
        &partner.secret,
        "DELETE",
        &path,
        "",
    )?;

    let url = format!(
        "{}{}",
        partner.api_base.trim_end_matches('/'),
        path
    );

    let response = client
        .delete(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| ApiError::internal(&format!("Limitless cancel API: {}", e)))?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(ApiError::bad_request(
            "LIMITLESS_CANCEL_FAILED",
            &format!("Cancel failed: {}", text),
        ));
    }

    Ok(())
}

/// Check partner configuration status. Returns a summary for diagnostics.
pub fn partner_status(config: &AppConfig) -> Value {
    match LimitlessPartnerConfig::from_config(config) {
        Some(partner) => json!({
            "configured": true,
            "ready": partner.is_ready(),
            "hasTokenId": !partner.token_id.is_empty(),
            "hasSecret": true,
            "hasProfileId": !partner.profile_id.is_empty(),
        }),
        None => json!({
            "configured": false,
            "ready": false,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_headers_are_valid() {
        let headers = build_hmac_headers(
            "test-token",
            "test-secret",
            "POST",
            "/orders",
            r#"{"test": true}"#,
        )
        .unwrap();

        assert!(headers.contains_key("lmts-api-key"));
        assert!(headers.contains_key("lmts-timestamp"));
        assert!(headers.contains_key("lmts-signature"));
        assert_eq!(
            headers.get("lmts-api-key").unwrap().to_str().unwrap(),
            "test-token"
        );
    }

    #[test]
    fn partner_config_requires_both_fields() {
        // Can't test from_config without AppConfig, but we test is_ready
        let config = LimitlessPartnerConfig {
            token_id: "tok".to_string(),
            secret: "sec".to_string(),
            profile_id: String::new(),
            api_base: "https://api.limitless.exchange".to_string(),
        };
        assert!(!config.is_ready()); // Missing profile_id

        let ready = LimitlessPartnerConfig {
            profile_id: "123".to_string(),
            ..config
        };
        assert!(ready.is_ready());
    }
}
