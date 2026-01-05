use crate::api::ApiError;
use crate::AppState;
use actix_web::HttpRequest;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::StatusCode;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

const DEFAULT_X402_TOKEN_NAME: &str = "USD Coin";
const DEFAULT_X402_TOKEN_VERSION: &str = "2";

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum X402Resource {
    OrderBook,
    Trades,
    McpToolCall,
}

impl X402Resource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrderBook => "orderbook",
            Self::Trades => "trades",
            Self::McpToolCall => "mcp_tool_call",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::OrderBook => "Premium order book depth for the requested market.",
            Self::Trades => "Premium recent trade feed for the requested market.",
            Self::McpToolCall => "Premium MCP market-data tool call.",
        }
    }
}

impl fmt::Display for X402Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct X402ResourceInfo {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct X402PaymentRequirement {
    pub scheme: String,
    pub network: String,
    pub amount: String,
    pub asset: String,
    #[serde(rename = "payTo")]
    pub pay_to: String,
    #[serde(rename = "maxTimeoutSeconds")]
    pub max_timeout_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct X402PaymentRequired {
    #[serde(rename = "x402Version")]
    pub x402_version: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub resource: X402ResourceInfo,
    pub accepts: Vec<X402PaymentRequirement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct X402PaymentPayload {
    #[serde(rename = "x402Version")]
    pub x402_version: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<X402ResourceInfo>,
    pub accepted: X402PaymentRequirement,
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct X402VerifyResponse {
    #[serde(rename = "isValid")]
    pub is_valid: bool,
    #[serde(rename = "invalidReason", skip_serializing_if = "Option::is_none")]
    pub invalid_reason: Option<String>,
    #[serde(rename = "invalidMessage", skip_serializing_if = "Option::is_none")]
    pub invalid_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct X402SettleResponse {
    pub success: bool,
    #[serde(rename = "errorReason", skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
    #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    pub transaction: String,
    pub network: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct X402FacilitatorEnvelope {
    #[serde(rename = "x402Version")]
    x402_version: u8,
    #[serde(rename = "paymentPayload")]
    payment_payload: X402PaymentPayload,
    #[serde(rename = "paymentRequirements")]
    payment_requirements: X402PaymentRequirement,
}

fn required_amount(state: &AppState, resource: X402Resource) -> u64 {
    match resource {
        X402Resource::OrderBook => state.config.x402_orderbook_price_microusdc,
        X402Resource::Trades => state.config.x402_trades_price_microusdc,
        X402Resource::McpToolCall => state.config.x402_mcp_price_microusdc,
    }
}

fn primary_origin(state: &AppState) -> String {
    if let Ok(public_api_url) = std::env::var("PUBLIC_API_URL") {
        let value = public_api_url
            .trim()
            .trim_end_matches('/')
            .trim_end_matches("/v1");
        if !value.is_empty() {
            return value.to_string();
        }
    }

    state
        .config
        .cors_origins
        .iter()
        .find(|entry| entry.starts_with("http://") || entry.starts_with("https://"))
        .cloned()
        .unwrap_or_else(|| "http://localhost:3000".to_string())
}

pub fn api_origin_from_request(state: &AppState, req: &HttpRequest) -> String {
    let proto = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("https");
    let host = req
        .headers()
        .get("x-forwarded-host")
        .or_else(|| req.headers().get("host"))
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty());

    if let Some(host) = host {
        return format!("{proto}://{host}");
    }

    primary_origin(state)
}

fn resource_url_from_request(state: &AppState, req: &HttpRequest) -> String {
    let proto = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("https");
    let host = req
        .headers()
        .get("x-forwarded-host")
        .or_else(|| req.headers().get("host"))
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty());

    if let Some(host) = host {
        let path = req
            .uri()
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or(req.path());
        return format!("{proto}://{host}{path}");
    }

    let origin = primary_origin(state);
    let path = req
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or(req.path());
    format!("{}{}", origin.trim_end_matches('/'), path)
}

fn resource_url_for_quote(origin: &str, resource: X402Resource) -> String {
    format!(
        "{}/v1/payments/x402/quote?resource={}",
        origin.trim_end_matches('/'),
        resource.as_str()
    )
}

fn requirement_for_resource(
    state: &AppState,
    resource: X402Resource,
    resource_url: String,
) -> X402PaymentRequired {
    X402PaymentRequired {
        x402_version: 2,
        error: Some("payment required".to_string()),
        resource: X402ResourceInfo {
            url: resource_url,
            description: Some(resource.description().to_string()),
            mime_type: Some("application/json".to_string()),
        },
        accepts: vec![X402PaymentRequirement {
            scheme: "exact".to_string(),
            network: format!("eip155:{}", state.config.base_chain_id),
            amount: required_amount(state, resource).to_string(),
            asset: state.config.usdc_mint.clone(),
            pay_to: state.config.x402_receiver_address.clone(),
            max_timeout_seconds: state.config.x402_quote_ttl_seconds.max(30),
            extra: Some(json!({
                "name": DEFAULT_X402_TOKEN_NAME,
                "version": DEFAULT_X402_TOKEN_VERSION,
            })),
        }],
        extensions: None,
    }
}

pub fn build_quote_for_origin(
    state: &AppState,
    origin: &str,
    resource: X402Resource,
) -> X402PaymentRequired {
    requirement_for_resource(state, resource, resource_url_for_quote(origin, resource))
}

pub fn build_quote_for_request(
    state: &AppState,
    resource: X402Resource,
    req: &HttpRequest,
) -> X402PaymentRequired {
    let origin = api_origin_from_request(state, req);
    build_quote_for_origin(state, origin.as_str(), resource)
}

fn encode_payment_required_header(payment_required: &X402PaymentRequired) -> String {
    BASE64.encode(serde_json::to_vec(payment_required).unwrap_or_default())
}

pub fn encode_payment_signature_header(payment_payload: &X402PaymentPayload) -> Result<String, ApiError> {
    serde_json::to_vec(payment_payload)
        .map(|bytes| BASE64.encode(bytes))
        .map_err(|_| ApiError::bad_request("INVALID_X402_PAYMENT", "payment payload is invalid"))
}

fn decode_payment_signature_header(header: &str) -> Result<X402PaymentPayload, ApiError> {
    let decoded = BASE64.decode(header).map_err(|_| {
        ApiError::bad_request(
            "INVALID_X402_PAYMENT_HEADER",
            "payment-signature must be base64 encoded JSON",
        )
    })?;

    serde_json::from_slice::<X402PaymentPayload>(&decoded).map_err(|_| {
        ApiError::bad_request(
            "INVALID_X402_PAYMENT_HEADER",
            "payment-signature payload is invalid",
        )
    })
}

fn payment_required_error(
    state: &AppState,
    resource: X402Resource,
    resource_url: String,
    message: &str,
    extra: Option<Value>,
) -> ApiError {
    let payment_required = requirement_for_resource(state, resource, resource_url);
    let encoded = encode_payment_required_header(&payment_required);
    let mut details = json!({
        "paymentRequired": payment_required,
    });

    if let Some(extra) = extra {
        details["context"] = extra;
    }

    ApiError::payment_required_with_headers(message, None::<Value>, vec![
        ("PAYMENT-REQUIRED".to_string(), encoded),
        (
            "WWW-Authenticate".to_string(),
            "X402 realm=\"relay44\", scheme=\"exact\"".to_string(),
        ),
        ("Cache-Control".to_string(), "no-store".to_string()),
    ])
    .with_details(Some(details))
}

async fn facilitator_request<T: DeserializeOwned>(
    state: &AppState,
    path: &str,
    envelope: &X402FacilitatorEnvelope,
) -> Result<(StatusCode, T), ApiError> {
    let url = format!(
        "{}/{}",
        state.config.x402_facilitator_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

    let client = reqwest::Client::new();
    let mut request = client.post(url).json(envelope);
    if !state.config.x402_facilitator_token.trim().is_empty() {
        request = request.bearer_auth(state.config.x402_facilitator_token.as_str());
    }

    let response = request
        .send()
        .await
        .map_err(|_| ApiError::internal("Failed to reach x402 facilitator"))?;
    let status = response.status();
    let payload = response
        .json::<T>()
        .await
        .map_err(|_| ApiError::internal("Invalid x402 facilitator response"))?;
    Ok((status, payload))
}

fn build_envelope(
    state: &AppState,
    resource: X402Resource,
    payment_payload: X402PaymentPayload,
    resource_url: String,
) -> X402FacilitatorEnvelope {
    X402FacilitatorEnvelope {
        x402_version: 2,
        payment_requirements: requirement_for_resource(state, resource, resource_url)
            .accepts
            .into_iter()
            .next()
            .unwrap(),
        payment_payload,
    }
}

pub async fn ensure_payment_for_request(
    state: &AppState,
    req: &HttpRequest,
    resource: X402Resource,
) -> Result<(), ApiError> {
    if !state.config.x402_enabled {
        return Ok(());
    }

    let resource_url = resource_url_from_request(state, req);
    let header = req
        .headers()
        .get("payment-signature")
        .or_else(|| req.headers().get("PAYMENT-SIGNATURE"))
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| payment_required_error(state, resource, resource_url.clone(), "x402 payment required", None))?;

    let payment_payload = decode_payment_signature_header(header).map_err(|_| {
        payment_required_error(
            state,
            resource,
            resource_url.clone(),
            "x402 payment-signature header is invalid",
            None,
        )
    })?;

    ensure_payment_from_payload(state, payment_payload, resource, Some(resource_url)).await
}

pub async fn ensure_payment_from_payload(
    state: &AppState,
    payment_payload: X402PaymentPayload,
    resource: X402Resource,
    resource_url: Option<String>,
) -> Result<(), ApiError> {
    if !state.config.x402_enabled {
        return Ok(());
    }

    let resource_url = resource_url.unwrap_or_else(|| {
        let origin = primary_origin(state);
        resource_url_for_quote(origin.as_str(), resource)
    });
    let envelope = build_envelope(state, resource, payment_payload.clone(), resource_url.clone());

    let (verify_status, verify_response) =
        facilitator_request::<X402VerifyResponse>(state, "/verify", &envelope).await?;
    if !verify_status.is_success() || !verify_response.is_valid {
        return Err(payment_required_error(
            state,
            resource,
            resource_url.clone(),
            verify_response
                .invalid_message
                .as_deref()
                .unwrap_or("x402 payment verification failed"),
            Some(json!({ "verify": verify_response })),
        ));
    }

    let (settle_status, settle_response) =
        facilitator_request::<X402SettleResponse>(state, "/settle", &envelope).await?;
    if !settle_status.is_success() || !settle_response.success {
        return Err(payment_required_error(
            state,
            resource,
            resource_url,
            settle_response
                .error_message
                .as_deref()
                .unwrap_or("x402 payment settlement failed"),
            Some(json!({ "settle": settle_response })),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_payment_signature_roundtrip() {
        let payload = X402PaymentPayload {
            x402_version: 2,
            resource: Some(X402ResourceInfo {
                url: "https://example.com/v1/evm/markets/1/orderbook".to_string(),
                description: Some("Order book".to_string()),
                mime_type: Some("application/json".to_string()),
            }),
            accepted: X402PaymentRequirement {
                scheme: "exact".to_string(),
                network: "eip155:8453".to_string(),
                amount: "2500".to_string(),
                asset: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
                pay_to: "0x1111111111111111111111111111111111111111".to_string(),
                max_timeout_seconds: 300,
                extra: Some(json!({ "name": "USD Coin", "version": "2" })),
            },
            payload: json!({ "authorization": { "from": "0x1" } }),
            extensions: None,
        };

        let encoded = encode_payment_signature_header(&payload).unwrap();
        let decoded = decode_payment_signature_header(encoded.as_str()).unwrap();
        assert_eq!(decoded.x402_version, 2);
        assert_eq!(decoded.accepted.network, "eip155:8453");
    }
}

