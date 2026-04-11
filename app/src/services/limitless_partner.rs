//! Limitless Partner Integration.
//!
//! Uses the existing LIMITLESS_API_KEY with partner scopes (trading,
//! account_creation, delegated_signing) enabled by Limitless on our wallet.

use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;

use crate::api::ApiError;
use crate::config::AppConfig;

type HmacSha256 = Hmac<Sha256>;

const PARTNER_WALLET: &str = "0x6dA01f124f9a2979BFcF4F5326f4E4330ea99Ba1";

pub struct LimitlessPartnerConfig {
    pub api_key: String,
    pub api_base: String,
    pub wallet: String,
}

impl LimitlessPartnerConfig {
    pub fn from_config(config: &AppConfig) -> Option<Self> {
        if config.limitless_api_key.trim().is_empty() {
            return None;
        }
        Some(Self {
            api_key: config.limitless_api_key.clone(),
            api_base: config.limitless_api_base.clone(),
            wallet: PARTNER_WALLET.to_string(),
        })
    }

    pub fn is_ready(&self) -> bool {
        !self.api_key.is_empty()
    }
}

fn build_auth_headers(api_key: &str) -> Result<HeaderMap, ApiError> {
    let timestamp = Utc::now().to_rfc3339();
    let mut headers = HeaderMap::new();
    headers.insert(
        "X-API-Key",
        HeaderValue::from_str(api_key).map_err(|_| ApiError::internal("Invalid API key"))?,
    );
    headers.insert(
        "X-Timestamp",
        HeaderValue::from_str(&timestamp).map_err(|_| ApiError::internal("Invalid timestamp"))?,
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    Ok(headers)
}

fn build_hmac_headers(
    api_key: &str,
    method: &str,
    path: &str,
    body: &str,
) -> Result<HeaderMap, ApiError> {
    let timestamp = Utc::now().to_rfc3339();
    let message = format!("{}\n{}\n{}\n{}", method, path, timestamp, body);

    let mut mac = HmacSha256::new_from_slice(api_key.as_bytes())
        .map_err(|_| ApiError::internal("HMAC key invalid"))?;
    mac.update(message.as_bytes());
    let signature = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        mac.finalize().into_bytes(),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "lmts-api-key",
        HeaderValue::from_str(api_key).map_err(|_| ApiError::internal("Invalid API key"))?,
    );
    headers.insert(
        "lmts-timestamp",
        HeaderValue::from_str(&timestamp).map_err(|_| ApiError::internal("Invalid timestamp"))?,
    );
    headers.insert(
        "lmts-signature",
        HeaderValue::from_str(&signature).map_err(|_| ApiError::internal("Invalid signature"))?,
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    Ok(headers)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerAccount {
    pub profile_id: String,
    pub account: String,
    pub display_name: String,
}

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
        .map_err(|e| ApiError::internal(&format!("JSON serialize: {e}")))?;

    let headers = build_hmac_headers(&partner.api_key, "POST", path, &body_str)?;
    let url = format!("{}{}", partner.api_base.trim_end_matches('/'), path);

    let response = client
        .post(&url)
        .headers(headers)
        .body(body_str)
        .send()
        .await
        .map_err(|e| ApiError::internal(&format!("Limitless partner API: {e}")))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| ApiError::internal(&format!("Response read: {e}")))?;

    if !status.is_success() {
        return Err(ApiError::internal(&format!(
            "Limitless sub-account creation failed ({status}): {text}"
        )));
    }

    let result: Value = serde_json::from_str(&text)
        .map_err(|e| ApiError::internal(&format!("Parse response: {e}")))?;

    Ok(PartnerAccount {
        profile_id: result
            .get("profileId")
            .or_else(|| result.get("id"))
            .and_then(|v| v.as_str())
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegatedOrderRequest {
    pub market_slug: String,
    pub order_type: String,
    pub token_id: String,
    pub side: String,
    pub price: Option<f64>,
    pub size: Option<f64>,
    pub maker_amount: Option<f64>,
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

pub async fn place_delegated_order(
    partner: &LimitlessPartnerConfig,
    order: &DelegatedOrderRequest,
) -> Result<OrderResponse, ApiError> {
    if !partner.is_ready() {
        return Err(ApiError::bad_request(
            "LIMITLESS_PARTNER_NOT_CONFIGURED",
            "Partner API key not set",
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
        "onBehalfOf": partner.wallet,
        "args": args
    });

    let body_str = serde_json::to_string(&body)
        .map_err(|e| ApiError::internal(&format!("JSON serialize: {e}")))?;

    let headers = build_hmac_headers(&partner.api_key, "POST", path, &body_str)?;
    let url = format!("{}{}", partner.api_base.trim_end_matches('/'), path);

    let response = client
        .post(&url)
        .headers(headers)
        .body(body_str)
        .send()
        .await
        .map_err(|e| ApiError::internal(&format!("Limitless order API: {e}")))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| ApiError::internal(&format!("Response read: {e}")))?;

    if !status.is_success() {
        return Err(ApiError::bad_request(
            "LIMITLESS_ORDER_FAILED",
            &format!("Order failed ({status}): {text}"),
        ));
    }

    let result: Value = serde_json::from_str(&text)
        .map_err(|e| ApiError::internal(&format!("Parse response: {e}")))?;

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

pub async fn cancel_order(
    partner: &LimitlessPartnerConfig,
    order_id: &str,
) -> Result<(), ApiError> {
    let client = reqwest::Client::new();
    let path = format!("/orders/{order_id}");

    let headers = build_hmac_headers(&partner.api_key, "DELETE", &path, "")?;
    let url = format!("{}{}", partner.api_base.trim_end_matches('/'), path);

    let response = client
        .delete(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| ApiError::internal(&format!("Limitless cancel API: {e}")))?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(ApiError::bad_request(
            "LIMITLESS_CANCEL_FAILED",
            &format!("Cancel failed: {text}"),
        ));
    }

    Ok(())
}

pub fn partner_status(config: &AppConfig) -> Value {
    match LimitlessPartnerConfig::from_config(config) {
        Some(partner) => json!({
            "configured": true,
            "ready": partner.is_ready(),
            "wallet": partner.wallet,
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
        let headers =
            build_hmac_headers("test-key", "POST", "/orders", r#"{"test": true}"#).unwrap();

        assert!(headers.contains_key("lmts-api-key"));
        assert!(headers.contains_key("lmts-timestamp"));
        assert!(headers.contains_key("lmts-signature"));
        assert_eq!(
            headers.get("lmts-api-key").unwrap().to_str().unwrap(),
            "test-key"
        );
    }

    #[test]
    fn partner_config_requires_api_key() {
        let config = LimitlessPartnerConfig {
            api_key: String::new(),
            api_base: "https://api.limitless.exchange".to_string(),
            wallet: PARTNER_WALLET.to_string(),
        };
        assert!(!config.is_ready());

        let ready = LimitlessPartnerConfig {
            api_key: "some-key".to_string(),
            ..config
        };
        assert!(ready.is_ready());
    }
}
