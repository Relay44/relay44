use crate::{api::ApiError, config::AppConfig, services::evm_rpc::EvmRpcService};
use actix_web::{HttpRequest, HttpResponseBuilder};
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

fn base_amount(config: &AppConfig, resource: X402Resource) -> u64 {
    match resource {
        X402Resource::OrderBook => config.x402_orderbook_price_microusdc,
        X402Resource::Trades => config.x402_trades_price_microusdc,
        X402Resource::McpToolCall => config.x402_mcp_price_microusdc,
    }
}

fn required_amount(config: &AppConfig, resource: X402Resource) -> u64 {
    base_amount(config, resource)
}

fn required_amount_for_tier(config: &AppConfig, resource: X402Resource, tier: u64) -> u64 {
    super::staking::discounted_amount(base_amount(config, resource), tier)
}

fn primary_origin(config: &AppConfig) -> String {
    if let Ok(public_api_url) = std::env::var("PUBLIC_API_URL") {
        let value = public_api_url
            .trim()
            .trim_end_matches('/')
            .trim_end_matches("/v1");
        if !value.is_empty() {
            return value.to_string();
        }
    }

    config
        .cors_origins
        .iter()
        .find(|entry| entry.starts_with("http://") || entry.starts_with("https://"))
        .cloned()
        .unwrap_or_else(|| "http://localhost:3000".to_string())
}

pub fn api_origin_from_request(config: &AppConfig, req: &HttpRequest) -> String {
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

    primary_origin(config)
}

fn resource_url_from_request(config: &AppConfig, req: &HttpRequest) -> String {
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

    let origin = primary_origin(config);
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
    config: &AppConfig,
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
            network: format!("eip155:{}", config.base_chain_id),
            amount: required_amount(config, resource).to_string(),
            asset: config.usdc_mint.clone(),
            pay_to: config.x402_receiver_address.clone(),
            max_timeout_seconds: config.x402_quote_ttl_seconds.max(30),
            extra: Some(json!({
                "name": DEFAULT_X402_TOKEN_NAME,
                "version": DEFAULT_X402_TOKEN_VERSION,
            })),
        }],
        extensions: None,
    }
}

pub fn build_quote_for_origin(
    config: &AppConfig,
    origin: &str,
    resource: X402Resource,
) -> X402PaymentRequired {
    requirement_for_resource(config, resource, resource_url_for_quote(origin, resource))
}

pub fn build_quote_for_request(
    config: &AppConfig,
    resource: X402Resource,
    req: &HttpRequest,
) -> X402PaymentRequired {
    let origin = api_origin_from_request(config, req);
    build_quote_for_origin(config, origin.as_str(), resource)
}

fn encode_payment_required_header(payment_required: &X402PaymentRequired) -> String {
    BASE64.encode(serde_json::to_vec(payment_required).unwrap_or_default())
}

pub fn encode_payment_signature_header(
    payment_payload: &X402PaymentPayload,
) -> Result<String, ApiError> {
    serde_json::to_vec(payment_payload)
        .map(|bytes| BASE64.encode(bytes))
        .map_err(|_| ApiError::bad_request("INVALID_X402_PAYMENT", "payment payload is invalid"))
}

pub fn encode_payment_response_header(
    settle_response: &X402SettleResponse,
) -> Result<String, ApiError> {
    serde_json::to_vec(settle_response)
        .map(|bytes| BASE64.encode(bytes))
        .map_err(|_| ApiError::internal("x402 settlement response is invalid"))
}

pub fn append_payment_response_header(
    response: &mut HttpResponseBuilder,
    settlement: Option<&X402SettleResponse>,
) -> Result<(), ApiError> {
    if let Some(settle_response) = settlement {
        response.append_header((
            "PAYMENT-RESPONSE",
            encode_payment_response_header(settle_response)?,
        ));
    }

    Ok(())
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
    config: &AppConfig,
    resource: X402Resource,
    resource_url: String,
    message: &str,
    extra: Option<Value>,
) -> ApiError {
    let payment_required = requirement_for_resource(config, resource, resource_url);
    let encoded = encode_payment_required_header(&payment_required);
    let mut details = json!({
        "paymentRequired": payment_required,
    });

    if let Some(extra) = extra {
        details["context"] = extra;
    }

    ApiError::payment_required_with_headers(
        message,
        None::<Value>,
        vec![
            ("PAYMENT-REQUIRED".to_string(), encoded),
            (
                "WWW-Authenticate".to_string(),
                "X402 realm=\"relay44\", scheme=\"exact\"".to_string(),
            ),
            ("Cache-Control".to_string(), "no-store".to_string()),
        ],
    )
    .with_details(Some(details))
}

async fn facilitator_request<T: DeserializeOwned>(
    config: &AppConfig,
    path: &str,
    envelope: &X402FacilitatorEnvelope,
) -> Result<(StatusCode, T), ApiError> {
    let url = format!(
        "{}/{}",
        config.x402_facilitator_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

    let client = reqwest::Client::new();
    let mut request = client.post(url).json(envelope);
    if !config.x402_facilitator_token.trim().is_empty() {
        request = request.bearer_auth(config.x402_facilitator_token.as_str());
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
    config: &AppConfig,
    resource: X402Resource,
    payment_payload: X402PaymentPayload,
    resource_url: String,
) -> X402FacilitatorEnvelope {
    X402FacilitatorEnvelope {
        x402_version: 2,
        payment_requirements: requirement_for_resource(config, resource, resource_url)
            .accepts
            .into_iter()
            .next()
            .unwrap(),
        payment_payload,
    }
}

fn extract_staker_address(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("x-relay-address")
        .or_else(|| req.headers().get("X-Relay-Address"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| s.len() == 42 && s.starts_with("0x"))
}

pub async fn ensure_payment_for_request(
    config: &AppConfig,
    req: &HttpRequest,
    resource: X402Resource,
) -> Result<Option<X402SettleResponse>, ApiError> {
    ensure_payment_for_request_with_rpc(config, req, resource, None).await
}

pub async fn ensure_payment_for_request_with_rpc(
    config: &AppConfig,
    req: &HttpRequest,
    resource: X402Resource,
    rpc: Option<&EvmRpcService>,
) -> Result<Option<X402SettleResponse>, ApiError> {
    if !config.x402_enabled {
        return Ok(None);
    }

    // Check staking tier for fee reduction/bypass
    if let (Some(rpc), Some(staker)) = (rpc, extract_staker_address(req)) {
        let tier = super::staking::get_staking_tier(rpc, &config.relay_staking_address, &staker)
            .await
            .unwrap_or(0);

        if tier >= 2 {
            return Ok(None); // tier 2+ gets free access
        }
    }

    let resource_url = resource_url_from_request(config, req);
    let header = req
        .headers()
        .get("payment-signature")
        .or_else(|| req.headers().get("PAYMENT-SIGNATURE"))
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            payment_required_error(
                config,
                resource,
                resource_url.clone(),
                "x402 payment required",
                None,
            )
        })?;

    let payment_payload = decode_payment_signature_header(header).map_err(|_| {
        payment_required_error(
            config,
            resource,
            resource_url.clone(),
            "x402 payment-signature header is invalid",
            None,
        )
    })?;

    ensure_payment_from_payload(config, payment_payload, resource, Some(resource_url)).await
}

pub async fn ensure_payment_from_payload(
    config: &AppConfig,
    payment_payload: X402PaymentPayload,
    resource: X402Resource,
    resource_url: Option<String>,
) -> Result<Option<X402SettleResponse>, ApiError> {
    if !config.x402_enabled {
        return Ok(None);
    }

    let resource_url = resource_url.unwrap_or_else(|| {
        let origin = primary_origin(config);
        resource_url_for_quote(origin.as_str(), resource)
    });
    let envelope = build_envelope(
        config,
        resource,
        payment_payload.clone(),
        resource_url.clone(),
    );

    let (verify_status, verify_response) =
        facilitator_request::<X402VerifyResponse>(config, "/verify", &envelope).await?;
    if !verify_status.is_success() || !verify_response.is_valid {
        return Err(payment_required_error(
            config,
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
        facilitator_request::<X402SettleResponse>(config, "/settle", &envelope).await?;
    if !settle_status.is_success() || !settle_response.success {
        return Err(payment_required_error(
            config,
            resource,
            resource_url,
            settle_response
                .error_message
                .as_deref()
                .unwrap_or("x402 payment settlement failed"),
            Some(json!({ "settle": settle_response })),
        ));
    }

    Ok(Some(settle_response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use actix_web::{
        http::{header, StatusCode as ActixStatusCode},
        test::TestRequest,
        web, App, HttpRequest, HttpResponse, HttpServer,
    };
    use serde_json::Value;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    const TEST_RECEIVER: &str = "0x1111111111111111111111111111111111111111";
    const TEST_USDC: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
    static TEST_ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[derive(Clone)]
    struct MockFacilitatorConfig {
        verify_status: ActixStatusCode,
        verify_body: Value,
        settle_status: ActixStatusCode,
        settle_body: Value,
        requests: Arc<Mutex<Vec<ObservedRequest>>>,
    }

    #[derive(Clone, Debug)]
    struct ObservedRequest {
        path: String,
        authorization: Option<String>,
        body: Value,
    }

    struct MockFacilitatorServer {
        base_url: String,
        handle: actix_web::dev::ServerHandle,
        requests: Arc<Mutex<Vec<ObservedRequest>>>,
    }

    impl MockFacilitatorServer {
        async fn stop(self) {
            self.handle.stop(true).await;
        }

        fn requests(&self) -> Vec<ObservedRequest> {
            self.requests.lock().expect("requests lock").clone()
        }
    }

    fn with_test_config(facilitator_url: &str) -> AppConfig {
        let _guard = TEST_ENV_MUTEX.lock().expect("env lock");
        let keys = [
            "ENVIRONMENT",
            "PUBLIC_API_URL",
            "CORS_ORIGINS",
            "BASE_CHAIN_ID",
            "USDC_MINT",
            "X402_ENABLED",
            "X402_RECEIVER_ADDRESS",
            "X402_FACILITATOR_URL",
            "X402_FACILITATOR_TOKEN",
            "X402_ORDERBOOK_PRICE_MICROUSDC",
            "X402_TRADES_PRICE_MICROUSDC",
            "X402_MCP_PRICE_MICROUSDC",
        ];
        let saved: Vec<(String, Option<String>)> = keys
            .iter()
            .map(|key| (key.to_string(), std::env::var(key).ok()))
            .collect();

        for key in &keys {
            std::env::remove_var(key);
        }

        std::env::set_var("ENVIRONMENT", "development");
        std::env::set_var("PUBLIC_API_URL", "https://relay44-api.onrender.com");
        std::env::set_var("CORS_ORIGINS", "https://relay44.com");
        std::env::set_var("BASE_CHAIN_ID", "8453");
        std::env::set_var("USDC_MINT", TEST_USDC);
        std::env::set_var("X402_ENABLED", "true");
        std::env::set_var("X402_RECEIVER_ADDRESS", TEST_RECEIVER);
        std::env::set_var("X402_FACILITATOR_URL", facilitator_url);
        std::env::set_var("X402_FACILITATOR_TOKEN", "facilitator-secret");
        std::env::set_var("X402_ORDERBOOK_PRICE_MICROUSDC", "2500");
        std::env::set_var("X402_TRADES_PRICE_MICROUSDC", "3500");
        std::env::set_var("X402_MCP_PRICE_MICROUSDC", "5000");

        let config = AppConfig::from_env();

        for (key, value) in saved {
            if let Some(raw) = value {
                std::env::set_var(key, raw);
            } else {
                std::env::remove_var(key);
            }
        }

        config
    }

    fn test_payment_payload(amount: &str) -> X402PaymentPayload {
        X402PaymentPayload {
            x402_version: 2,
            resource: Some(X402ResourceInfo {
                url: "https://relay44-api.onrender.com/v1/evm/markets/12/orderbook?outcome=yes&depth=5"
                    .to_string(),
                description: Some("Premium order book depth for the requested market.".to_string()),
                mime_type: Some("application/json".to_string()),
            }),
            accepted: X402PaymentRequirement {
                scheme: "exact".to_string(),
                network: "eip155:8453".to_string(),
                amount: amount.to_string(),
                asset: TEST_USDC.to_string(),
                pay_to: TEST_RECEIVER.to_string(),
                max_timeout_seconds: 300,
                extra: Some(json!({ "name": "USD Coin", "version": "2" })),
            },
            payload: json!({ "authorization": { "from": "0xabc" } }),
            extensions: None,
        }
    }

    fn decode_payment_required_header(header: &str) -> X402PaymentRequired {
        serde_json::from_slice(&BASE64.decode(header).expect("decode payment-required"))
            .expect("payment-required payload")
    }

    fn decode_payment_response(header: &str) -> X402SettleResponse {
        serde_json::from_slice(&BASE64.decode(header).expect("decode payment-response"))
            .expect("payment-response payload")
    }

    async fn facilitator_handler(
        req: HttpRequest,
        body: web::Json<Value>,
        config: web::Data<MockFacilitatorConfig>,
    ) -> HttpResponse {
        config
            .requests
            .lock()
            .expect("requests lock")
            .push(ObservedRequest {
                path: req.path().to_string(),
                authorization: req
                    .headers()
                    .get(header::AUTHORIZATION)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string),
                body: body.into_inner(),
            });

        match req.path() {
            "/verify" => HttpResponse::build(config.verify_status).json(config.verify_body.clone()),
            "/settle" => HttpResponse::build(config.settle_status).json(config.settle_body.clone()),
            _ => HttpResponse::NotFound().finish(),
        }
    }

    async fn start_mock_facilitator(
        verify_status: ActixStatusCode,
        verify_body: Value,
        settle_status: ActixStatusCode,
        settle_body: Value,
    ) -> MockFacilitatorServer {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let config = MockFacilitatorConfig {
            verify_status,
            verify_body,
            settle_status,
            settle_body,
            requests: requests.clone(),
        };
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind facilitator");
        let addr = listener.local_addr().expect("facilitator addr");
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(config.clone()))
                .route("/verify", web::post().to(facilitator_handler))
                .route("/settle", web::post().to(facilitator_handler))
        })
        .listen(listener)
        .expect("listen facilitator")
        .run();
        let handle = server.handle();
        actix_rt::spawn(server);

        MockFacilitatorServer {
            base_url: format!("http://{addr}"),
            handle,
            requests,
        }
    }

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

    #[test]
    fn encode_payment_response_header_roundtrip() {
        let response = X402SettleResponse {
            success: true,
            error_reason: None,
            error_message: None,
            payer: Some("0x1111111111111111111111111111111111111111".to_string()),
            transaction: "0xabc".to_string(),
            network: "eip155:8453".to_string(),
            extensions: Some(json!({ "foo": "bar" })),
        };

        let encoded = encode_payment_response_header(&response).unwrap();
        let decoded: X402SettleResponse =
            serde_json::from_slice(&BASE64.decode(encoded).unwrap()).unwrap();
        assert!(decoded.success);
        assert_eq!(decoded.transaction, "0xabc");
        assert_eq!(decoded.network, "eip155:8453");
    }

    #[test]
    fn append_payment_response_header_roundtrip() {
        let settlement = X402SettleResponse {
            success: true,
            error_reason: None,
            error_message: None,
            payer: Some(TEST_RECEIVER.to_string()),
            transaction: "0xsettled".to_string(),
            network: "eip155:8453".to_string(),
            extensions: Some(json!({ "foo": "bar" })),
        };
        let mut response = actix_web::HttpResponse::Ok();

        append_payment_response_header(&mut response, Some(&settlement)).unwrap();

        let response = response.finish();
        let header = response
            .headers()
            .get("PAYMENT-RESPONSE")
            .and_then(|value| value.to_str().ok())
            .expect("payment response header");
        let decoded = decode_payment_response(header);

        assert_eq!(decoded.transaction, "0xsettled");
        assert_eq!(decoded.payer.as_deref(), Some(TEST_RECEIVER));
    }

    #[actix_rt::test]
    async fn ensure_payment_for_request_returns_payment_required_quote_without_signature() {
        let config = with_test_config("http://127.0.0.1:1");
        let req = TestRequest::default()
            .insert_header(("x-forwarded-proto", "https"))
            .insert_header(("x-forwarded-host", "relay44-api.onrender.com"))
            .uri("/v1/evm/markets/12/orderbook?outcome=yes&depth=5")
            .to_http_request();

        let err = ensure_payment_for_request(&config, &req, X402Resource::OrderBook)
            .await
            .expect_err("missing header should fail");
        let payment_required = err
            .headers
            .iter()
            .find(|(key, _)| key == "PAYMENT-REQUIRED")
            .map(|(_, value)| decode_payment_required_header(value))
            .expect("payment-required header");

        assert_eq!(err.status, 402);
        assert_eq!(err.message, "x402 payment required");
        assert_eq!(
            payment_required.resource.url,
            "https://relay44-api.onrender.com/v1/evm/markets/12/orderbook?outcome=yes&depth=5"
        );
        assert_eq!(payment_required.accepts[0].amount, "2500");
    }

    #[actix_rt::test]
    async fn ensure_payment_from_payload_returns_settlement_on_success() {
        let server = start_mock_facilitator(
            ActixStatusCode::OK,
            json!({
                "isValid": true,
                "payer": TEST_RECEIVER,
            }),
            ActixStatusCode::OK,
            json!({
                "success": true,
                "payer": TEST_RECEIVER,
                "transaction": "0xsettled",
                "network": "eip155:8453",
            }),
        )
        .await;
        let config = with_test_config(server.base_url.as_str());

        let settlement = ensure_payment_from_payload(
            &config,
            test_payment_payload("2500"),
            X402Resource::OrderBook,
            None,
        )
        .await
        .expect("payment should succeed")
        .expect("settlement");
        let requests = server.requests();
        server.stop().await;

        assert_eq!(settlement.transaction, "0xsettled");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].path, "/verify");
        assert_eq!(requests[1].path, "/settle");
        assert_eq!(
            requests[0].authorization.as_deref(),
            Some("Bearer facilitator-secret")
        );
        assert_eq!(
            requests[0].body["paymentRequirements"]["amount"],
            serde_json::json!("2500")
        );
        assert_eq!(
            requests[0].body["paymentPayload"]["accepted"]["amount"],
            serde_json::json!("2500")
        );
    }

    #[actix_rt::test]
    async fn ensure_payment_from_payload_returns_payment_required_when_verify_fails() {
        let server = start_mock_facilitator(
            ActixStatusCode::OK,
            json!({
                "isValid": false,
                "invalidReason": "invalid_signature",
                "invalidMessage": "signature mismatch",
            }),
            ActixStatusCode::OK,
            json!({
                "success": true,
                "payer": TEST_RECEIVER,
                "transaction": "0xsettled",
                "network": "eip155:8453",
            }),
        )
        .await;
        let config = with_test_config(server.base_url.as_str());

        let err = ensure_payment_from_payload(
            &config,
            test_payment_payload("2500"),
            X402Resource::OrderBook,
            None,
        )
        .await
        .expect_err("verify failure should be rejected");
        let requests = server.requests();
        server.stop().await;

        assert_eq!(err.status, 402);
        assert_eq!(err.message, "signature mismatch");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            err.details.as_ref().expect("details")["context"]["verify"]["invalidReason"],
            serde_json::json!("invalid_signature")
        );
    }

    #[actix_rt::test]
    async fn ensure_payment_from_payload_returns_payment_required_when_settle_fails() {
        let server = start_mock_facilitator(
            ActixStatusCode::OK,
            json!({
                "isValid": true,
                "payer": TEST_RECEIVER,
            }),
            ActixStatusCode::OK,
            json!({
                "success": false,
                "errorReason": "insufficient_balance",
                "errorMessage": "insufficient balance",
                "transaction": "0xfailed",
                "network": "eip155:8453",
            }),
        )
        .await;
        let config = with_test_config(server.base_url.as_str());

        let err = ensure_payment_from_payload(
            &config,
            test_payment_payload("2500"),
            X402Resource::OrderBook,
            None,
        )
        .await
        .expect_err("settle failure should be rejected");
        let requests = server.requests();
        server.stop().await;

        assert_eq!(err.status, 402);
        assert_eq!(err.message, "insufficient balance");
        assert_eq!(requests.len(), 2);
        assert_eq!(
            err.details.as_ref().expect("details")["context"]["settle"]["errorReason"],
            serde_json::json!("insufficient_balance")
        );
    }
}
