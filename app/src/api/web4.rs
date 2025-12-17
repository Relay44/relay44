use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::api::ApiError;
use crate::services::x402::{
    api_origin_from_request, build_quote_for_origin, encode_payment_signature_header,
    X402PaymentPayload, X402Resource,
};
use crate::services::xmtp_swarm::{self, SwarmListQuery, SwarmSendRequest};
use crate::AppState;

const MCP_METHOD_WINDOW_SECONDS: u64 = 60;
const MCP_TOOL_WINDOW_SECONDS: u64 = 60;
const MCP_DEFAULT_METHOD_LIMIT_PER_WINDOW: i64 = 240;
const MCP_QUERY_METHOD_LIMIT_PER_WINDOW: i64 = 120;
const MCP_TOOL_CALL_METHOD_LIMIT_PER_WINDOW: i64 = 90;
const MCP_DEFAULT_TOOL_LIMIT_PER_WINDOW: i64 = 60;
const MCP_WRITE_TOOL_LIMIT_PER_WINDOW: i64 = 30;
const MCP_SWARM_TOOL_LIMIT_PER_WINDOW: i64 = 20;

fn infer_api_base_url(state: &AppState) -> String {
    if let Ok(public_api_url) = std::env::var("PUBLIC_API_URL") {
        let value = public_api_url.trim().trim_end_matches('/');
        if !value.is_empty() {
            return value.to_string();
        }
    }

    if let Some(origin) = state
        .config
        .cors_origins
        .iter()
        .find(|origin| origin.starts_with("http://") || origin.starts_with("https://"))
    {
        return format!("{}/v1", origin.trim_end_matches('/'));
    }

    if state.config.is_development || state.config.siwe_domain.contains("localhost") {
        return format!("http://{}:{}/v1", state.config.host, state.config.port);
    }

    format!("https://{}/v1", state.config.siwe_domain)
}

fn internal_api_base_url(state: &AppState) -> String {
    let host = if state.config.host == "0.0.0.0" {
        "127.0.0.1"
    } else {
        state.config.host.as_str()
    };
    format!("http://{}:{}/v1", host, state.config.port)
}

fn is_hex_address(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() == 42
        && trimmed.starts_with("0x")
        && trimmed[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn configured_chains(state: &AppState) -> Vec<Value> {
    let mut chains = Vec::new();
    if state.config.evm_enabled {
        chains.push(json!({
            "name": "base",
            "id": state.config.base_chain_id
        }));
    }
    if state.config.solana_enabled {
        chains.push(json!({
            "name": "solana",
            "rpc_url": state.config.solana_rpc_url,
            "market_program_id": state.config.solana_market_program_id,
            "orderbook_program_id": state.config.solana_orderbook_program_id
        }));
    }
    chains
}

#[derive(Debug, Deserialize)]
struct McpJsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct McpToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
struct McpResourceReadParams {
    uri: String,
}

#[derive(Debug, Deserialize)]
struct McpPromptGetParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Serialize)]
struct McpToolContent {
    #[serde(rename = "type")]
    kind: &'static str,
    text: String,
}

fn retryable_status(status: u16) -> bool {
    matches!(status, 402 | 408 | 409 | 425 | 429 | 500 | 502 | 503 | 504)
}

fn web4_error_payload(
    code: &str,
    reason: &str,
    retryable: bool,
    quote: Option<Value>,
    details: Option<Value>,
) -> Value {
    let mut payload = serde_json::Map::new();
    payload.insert("code".to_string(), json!(code));
    payload.insert("reason".to_string(), json!(reason));
    payload.insert("retryable".to_string(), json!(retryable));
    if let Some(quote_payload) = quote {
        payload.insert("quote".to_string(), quote_payload);
    }
    if let Some(extra) = details {
        payload.insert("details".to_string(), extra);
    }
    Value::Object(payload)
}

fn api_error_as_web4_payload(err: &ApiError) -> Value {
    let quote = err
        .details
        .as_ref()
        .and_then(|details| details.get("quote"))
        .cloned();
    let details = err
        .details
        .as_ref()
        .and_then(|value| value.get("details"))
        .cloned();
    web4_error_payload(
        err.code.as_str(),
        err.message.as_str(),
        retryable_status(err.status),
        quote,
        details,
    )
}

fn web4_error_from_downstream(status: u16, payload: &Value) -> Value {
    let code = payload
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("HTTP_{status}"));
    let reason = payload
        .get("error")
        .and_then(|error| error.get("reason"))
        .and_then(|value| value.as_str())
        .or_else(|| {
            payload
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(|value| value.as_str())
        })
        .unwrap_or("downstream request failed")
        .to_string();
    let quote = payload
        .get("error")
        .and_then(|error| error.get("details"))
        .and_then(|details| details.get("quote"))
        .cloned();
    web4_error_payload(
        code.as_str(),
        reason.as_str(),
        retryable_status(status),
        quote,
        Some(payload.clone()),
    )
}

fn sanitize_client_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "anonymous".to_string();
    }
    let mut result = String::new();
    for ch in trimmed.chars().take(96) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':') {
            result.push(ch);
        } else {
            result.push('_');
        }
    }
    if result.is_empty() {
        "anonymous".to_string()
    } else {
        result
    }
}

fn request_client_id(req: &HttpRequest) -> String {
    if let Some(client_id) = req
        .headers()
        .get("x-client-id")
        .and_then(|value| value.to_str().ok())
    {
        return sanitize_client_id(client_id);
    }

    let connection_info = req.connection_info();
    let remote = connection_info.realip_remote_addr().unwrap_or("anonymous");
    sanitize_client_id(remote)
}

fn mcp_method_limit_per_window(method: &str) -> i64 {
    match method {
        "tools/call" => MCP_TOOL_CALL_METHOD_LIMIT_PER_WINDOW,
        "resources/read" | "prompts/get" => MCP_QUERY_METHOD_LIMIT_PER_WINDOW,
        _ => MCP_DEFAULT_METHOD_LIMIT_PER_WINDOW,
    }
}

fn mcp_tool_limit_per_window(tool_name: &str) -> i64 {
    match tool_name {
        "prepareCreateAgentTx"
        | "prepareExecuteAgentTx"
        | "prepareRegisterIdentityTx"
        | "prepareSetIdentityTierTx"
        | "prepareSetIdentityActiveTx"
        | "prepareSubmitReputationOutcomeTx"
        | "prepareValidationRequestTx"
        | "prepareValidationResponseTx"
        | "prepareExternalOrder"
        | "submitExternalOrder"
        | "cancelExternalOrder"
        | "executeExternalAgent" => MCP_WRITE_TOOL_LIMIT_PER_WINDOW,
        "sendSwarmMessage" => MCP_SWARM_TOOL_LIMIT_PER_WINDOW,
        "listSwarmMessages" => MCP_QUERY_METHOD_LIMIT_PER_WINDOW,
        _ => MCP_DEFAULT_TOOL_LIMIT_PER_WINDOW,
    }
}

async fn enforce_rate_limit(
    state: &AppState,
    key: &str,
    limit: i64,
    window_seconds: u64,
) -> Result<(), ApiError> {
    let (count, ttl) = state
        .redis
        .increment_rate_limit(key, window_seconds)
        .await
        .map_err(|_| ApiError::internal("failed to evaluate MCP rate limit"))?;

    if count > limit {
        return Err(ApiError::rate_limited(ttl.max(1) as u64));
    }
    Ok(())
}

async fn enforce_mcp_policy(
    state: &AppState,
    req: &HttpRequest,
    request: &McpJsonRpcRequest,
) -> Result<(), ApiError> {
    let client_id = request_client_id(req);
    let method_limit = mcp_method_limit_per_window(request.method.as_str());
    let method_key = format!(
        "mcp:method:{}:{}",
        request.method.as_str().to_ascii_lowercase(),
        client_id
    );
    enforce_rate_limit(
        state,
        method_key.as_str(),
        method_limit,
        MCP_METHOD_WINDOW_SECONDS,
    )
    .await?;

    if request.method == "tools/call" {
        if let Some(params) = request.params.as_ref() {
            if let Ok(tool_call) = serde_json::from_value::<McpToolCallParams>(params.clone()) {
                let tool_limit = mcp_tool_limit_per_window(tool_call.name.as_str());
                let tool_key = format!(
                    "mcp:tool:{}:{}",
                    tool_call.name.to_ascii_lowercase(),
                    client_id
                );
                enforce_rate_limit(
                    state,
                    tool_key.as_str(),
                    tool_limit,
                    MCP_TOOL_WINDOW_SECONDS,
                )
                .await?;
            }
        }
    }

    Ok(())
}

fn mcp_response_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn mcp_response_error(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut payload = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    });

    if let Some(extra) = data {
        payload["error"]["data"] = extra;
    }
    payload
}

fn tool_result_payload(payload: Value, is_error: bool) -> Value {
    let pretty = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string());
    json!({
        "content": [McpToolContent {
            kind: "text",
            text: pretty,
        }],
        "structuredContent": payload,
        "isError": is_error
    })
}

fn tool_error_payload(status: u16, error: Value) -> Value {
    tool_result_payload(
        json!({
            "status": status,
            "error": error
        }),
        true,
    )
}

fn tool_payment_required_payload(
    state: &AppState,
    api_base: &str,
    resource: X402Resource,
) -> Value {
    let origin = api_base.trim_end_matches("/v1");
    let quote = serde_json::to_value(build_quote_for_origin(state, origin, resource)).ok();
    tool_error_payload(
        402,
        web4_error_payload(
            "PAYMENT_REQUIRED",
            "x402 payment required",
            true,
            quote,
            None,
        ),
    )
}

fn mcp_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "getMarkets",
            "description": "List unified internal and external markets with source/tradable filters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200 },
                    "offset": { "type": "integer", "minimum": 0 },
                    "source": { "type": "string", "enum": ["all", "internal", "limitless", "polymarket"] },
                    "tradable": { "type": "string", "enum": ["all", "user", "agent"] },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "getOrderBook",
            "description": "Fetch order book for a market side (x402 payment required when enabled).",
            "inputSchema": {
                "type": "object",
                "required": ["market_id", "outcome"],
                "properties": {
                    "market_id": { "type": "string", "description": "Numeric internal ID or namespaced ID (limitless:<slug>, polymarket:<id>)" },
                    "outcome": { "type": "string", "enum": ["yes", "no"] },
                    "depth": { "type": "integer", "minimum": 1, "maximum": 100 },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "getTrades",
            "description": "Fetch recent market trades (x402 payment required when enabled).",
            "inputSchema": {
                "type": "object",
                "required": ["market_id"],
                "properties": {
                    "market_id": { "type": "string", "description": "Numeric internal ID or namespaced ID (limitless:<slug>, polymarket:<id>)" },
                    "outcome": { "type": "string", "enum": ["yes", "no"] },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200 },
                    "offset": { "type": "integer", "minimum": 0 },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "getAgents",
            "description": "List active or historical autonomous agents in AgentRuntime.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200 },
                    "offset": { "type": "integer", "minimum": 0 },
                    "owner": { "type": "string" },
                    "market_id": { "type": "integer", "minimum": 1 },
                    "active": { "type": "boolean" },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "prepareExternalOrder",
            "description": "Create external order intent and return preflight + typed-data payload.",
            "inputSchema": {
                "type": "object",
                "required": ["provider", "market_id", "outcome", "side", "price", "quantity"],
                "properties": {
                    "provider": { "type": "string", "enum": ["limitless", "polymarket"] },
                    "market_id": { "type": "string" },
                    "outcome": { "type": "string", "enum": ["yes", "no"] },
                    "side": { "type": "string", "enum": ["buy", "sell"] },
                    "price": { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 1 },
                    "quantity": { "type": "number", "exclusiveMinimum": 0 },
                    "credential_id": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "submitExternalOrder",
            "description": "Submit signed external order intent to venue.",
            "inputSchema": {
                "type": "object",
                "required": ["intent_id", "signed_order"],
                "properties": {
                    "intent_id": { "type": "string" },
                    "signed_order": { "type": "object" },
                    "credential_id": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "cancelExternalOrder",
            "description": "Cancel venue order(s) for authenticated wallet.",
            "inputSchema": {
                "type": "object",
                "required": ["provider", "provider_order_id"],
                "properties": {
                    "provider": { "type": "string", "enum": ["limitless", "polymarket"] },
                    "provider_order_id": { "type": "string" },
                    "credential_id": { "type": "string" },
                    "payload": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "listExternalAgents",
            "description": "List external venue agents for authenticated wallet.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "provider": { "type": "string", "enum": ["limitless", "polymarket"] },
                    "active": { "type": "boolean" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200 },
                    "offset": { "type": "integer", "minimum": 0 }
                }
            }
        }),
        json!({
            "name": "executeExternalAgent",
            "description": "Force execution cycle for external agent.",
            "inputSchema": {
                "type": "object",
                "required": ["agent_id"],
                "properties": {
                    "agent_id": { "type": "string" },
                    "force": { "type": "boolean" },
                    "signed_order": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "prepareCreateAgentTx",
            "description": "Prepare calldata for createAgent wallet execution.",
            "inputSchema": {
                "type": "object",
                "required": ["marketId", "isYes", "priceBps", "size", "cadence", "expiryWindow", "strategy"],
                "properties": {
                    "from": { "type": "string" },
                    "marketId": { "type": "integer", "minimum": 1 },
                    "isYes": { "type": "boolean" },
                    "priceBps": { "type": "integer", "minimum": 1, "maximum": 9999 },
                    "size": { "type": "string" },
                    "cadence": { "type": "integer", "minimum": 1 },
                    "expiryWindow": { "type": "integer", "minimum": 1 },
                    "strategy": { "type": "string", "minLength": 1 },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "prepareExecuteAgentTx",
            "description": "Prepare calldata for executeAgent wallet execution.",
            "inputSchema": {
                "type": "object",
                "required": ["agentId"],
                "properties": {
                    "from": { "type": "string" },
                    "agentId": { "type": "integer", "minimum": 1 },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "prepareRegisterIdentityTx",
            "description": "Prepare calldata for ERC-8004 identity register(address,uint8).",
            "inputSchema": {
                "type": "object",
                "required": ["wallet", "tier"],
                "properties": {
                    "from": { "type": "string" },
                    "wallet": { "type": "string" },
                    "tier": { "type": "integer", "minimum": 0, "maximum": 100 },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "prepareSetIdentityTierTx",
            "description": "Prepare calldata for ERC-8004 identity setTier(address,uint8).",
            "inputSchema": {
                "type": "object",
                "required": ["wallet", "tier"],
                "properties": {
                    "from": { "type": "string" },
                    "wallet": { "type": "string" },
                    "tier": { "type": "integer", "minimum": 0, "maximum": 100 },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "prepareSetIdentityActiveTx",
            "description": "Prepare calldata for ERC-8004 identity setActive(address,bool).",
            "inputSchema": {
                "type": "object",
                "required": ["wallet", "active"],
                "properties": {
                    "from": { "type": "string" },
                    "wallet": { "type": "string" },
                    "active": { "type": "boolean" },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "prepareSubmitReputationOutcomeTx",
            "description": "Prepare calldata for ERC-8004 reputation submitOutcome(address,bool,uint128,uint16).",
            "inputSchema": {
                "type": "object",
                "required": ["wallet", "success", "notionalMicrousdc", "confidenceWeightBps"],
                "properties": {
                    "from": { "type": "string" },
                    "wallet": { "type": "string" },
                    "success": { "type": "boolean" },
                    "notionalMicrousdc": { "type": "string" },
                    "confidenceWeightBps": { "type": "integer", "minimum": 0, "maximum": 10000 },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "prepareValidationRequestTx",
            "description": "Prepare calldata for ERC-8004 validation validationRequest(address,uint256,string,bytes32).",
            "inputSchema": {
                "type": "object",
                "required": ["validator", "agentId", "requestUri"],
                "properties": {
                    "from": { "type": "string" },
                    "validator": { "type": "string" },
                    "agentId": { "type": "string" },
                    "requestUri": { "type": "string" },
                    "requestHash": { "type": "string", "description": "Optional bytes32 hash; if omitted, API derives keccak(requestUri)." },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "prepareValidationResponseTx",
            "description": "Prepare calldata for ERC-8004 validation validationResponse(bytes32,uint8,string,bytes32,bytes32).",
            "inputSchema": {
                "type": "object",
                "required": ["requestHash", "response", "responseUri", "responseHash", "tag"],
                "properties": {
                    "from": { "type": "string" },
                    "requestHash": { "type": "string" },
                    "response": { "type": "integer", "minimum": 0, "maximum": 100 },
                    "responseUri": { "type": "string" },
                    "responseHash": { "type": "string" },
                    "tag": { "type": "string" },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "getX402Quote",
            "description": "Get x402 quote for premium resources.",
            "inputSchema": {
                "type": "object",
                "required": ["resource"],
                "properties": {
                    "resource": { "type": "string", "enum": ["orderbook", "trades", "mcp_tool_call"] }
                }
            }
        }),
        json!({
            "name": "sendSwarmMessage",
            "description": "Send signed XMTP swarm message.",
            "inputSchema": {
                "type": "object",
                "required": ["swarm_id", "sender", "message", "signature"],
                "properties": {
                    "swarm_id": { "type": "string" },
                    "sender": { "type": "string" },
                    "message": { "type": "string" },
                    "signature": { "type": "string" },
                    "nonce": { "type": "string" },
                    "expires_at": { "type": "integer", "minimum": 1 },
                    "metadata": { "type": "object" },
                    "payment": { "type": "object" }
                }
            }
        }),
        json!({
            "name": "listSwarmMessages",
            "description": "List recent XMTP swarm messages.",
            "inputSchema": {
                "type": "object",
                "required": ["swarm_id"],
                "properties": {
                    "swarm_id": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200 },
                    "offset": { "type": "integer", "minimum": 0 },
                    "payment": { "type": "object" }
                }
            }
        }),
    ]
}

fn mcp_resources(api_base: &str) -> Vec<Value> {
    vec![
        json!({
            "uri": "relay44://markets/live",
            "name": "Live markets",
            "description": "Unified internal + external market list."
        }),
        json!({
            "uri": "relay44://agents/active",
            "name": "Active agents",
            "description": "Active AgentRuntime entries with execution readiness."
        }),
        json!({
            "uri": "relay44://runtime/health",
            "name": "Web4 runtime health",
            "description": "Current MCP/x402/XMTP runtime readiness state."
        }),
        json!({
            "uri": "relay44://xmtp/health",
            "name": "XMTP swarm health",
            "description": "XMTP swarm runtime configuration and limits."
        }),
        json!({
            "uri": format!("{}/web4/capabilities", api_base),
            "name": "Web4 capabilities",
            "description": "Protocol feature status."
        }),
    ]
}

fn mcp_prompts() -> Vec<Value> {
    vec![
        json!({
            "name": "market-scan",
            "description": "Scan active markets and return ranked opportunities.",
            "arguments": [
                { "name": "limit", "description": "Number of markets to scan", "required": false },
                { "name": "source", "description": "all | internal | limitless | polymarket", "required": false }
            ]
        }),
        json!({
            "name": "market-analysis",
            "description": "Analyze market structure, liquidity and executable opportunities.",
            "arguments": [
                { "name": "market_id", "description": "Numeric or namespaced market id", "required": true }
            ]
        }),
        json!({
            "name": "agent-launch",
            "description": "Generate agent launch params from risk budget and target outcome.",
            "arguments": [
                { "name": "market_id", "description": "Numeric or namespaced market id", "required": true },
                { "name": "outcome", "description": "yes or no", "required": true },
                { "name": "budget_usdc", "description": "Budget in USDC", "required": true }
            ]
        }),
        json!({
            "name": "swarm-coordination",
            "description": "Coordinate an XMTP swarm plan for executing market agents.",
            "arguments": [
                { "name": "swarm_id", "description": "Swarm channel id", "required": true },
                { "name": "objective", "description": "Mission objective", "required": true }
            ]
        }),
    ]
}

fn append_query(path: &str, key: &str, value: impl ToString) -> String {
    if path.contains('?') {
        format!("{path}&{key}={}", value.to_string())
    } else {
        format!("{path}?{key}={}", value.to_string())
    }
}

fn parse_payment_arg(args: &Value) -> Result<Option<X402PaymentPayload>, ApiError> {
    let Some(payment) = args.get("payment") else {
        return Ok(None);
    };
    let parsed = serde_json::from_value::<X402PaymentPayload>(payment.clone()).map_err(|_| {
        ApiError::bad_request(
            "INVALID_X402_PAYMENT_OBJECT",
            "payment must be an x402 v2 payment payload",
        )
    })?;
    Ok(Some(parsed))
}

async fn call_internal_api(
    state: &AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
    payment: Option<&X402PaymentPayload>,
) -> Result<(u16, Value), ApiError> {
    let base = internal_api_base_url(state);
    let url = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let client = reqwest::Client::new();
    let mut request = client.request(method, url);
    if let Some(payload) = body {
        request = request.json(&payload);
    }
    if let Some(payment) = payment {
        let encoded = encode_payment_signature_header(payment)?;
        request = request.header("payment-signature", encoded);
    }

    let response = request
        .send()
        .await
        .map_err(|_| ApiError::internal("Failed to call internal API for MCP dispatch"))?;
    let status = response.status().as_u16();
    let payload = response
        .json::<Value>()
        .await
        .unwrap_or_else(|_| json!({ "ok": status < 400 }));
    Ok((status, payload))
}

async fn handle_tool_call(
    state: &AppState,
    api_base: &str,
    params: McpToolCallParams,
) -> Result<Value, ApiError> {
    let mut args = params.arguments;
    if args.is_null() {
        args = json!({});
    }

    match params.name.as_str() {
        "getMarkets" => {
            let mut path = "/evm/markets".to_string();
            if let Some(limit) = args.get("limit").and_then(|v| v.as_u64()) {
                path = append_query(path.as_str(), "limit", limit);
            }
            if let Some(offset) = args.get("offset").and_then(|v| v.as_u64()) {
                path = append_query(path.as_str(), "offset", offset);
            }
            if let Some(source) = args.get("source").and_then(|v| v.as_str()) {
                path = append_query(path.as_str(), "source", source);
            }
            if let Some(tradable) = args.get("tradable").and_then(|v| v.as_str()) {
                path = append_query(path.as_str(), "tradable", tradable);
            }
            let (status, payload) =
                call_internal_api(state, reqwest::Method::GET, path.as_str(), None, None).await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "getOrderBook" => {
            let market_id = args
                .get("market_id")
                .and_then(|v| v.as_str().map(ToOwned::to_owned))
                .or_else(|| {
                    args.get("market_id")
                        .and_then(|v| v.as_u64())
                        .map(|v| v.to_string())
                })
                .ok_or_else(|| ApiError::bad_request("INVALID_ARGS", "market_id is required"))?;
            let outcome = args
                .get("outcome")
                .and_then(|v| v.as_str())
                .unwrap_or("yes");
            let mut path = format!("/evm/markets/{}/orderbook?outcome={outcome}", market_id);
            if let Some(depth) = args.get("depth").and_then(|v| v.as_u64()) {
                path = append_query(path.as_str(), "depth", depth);
            }

            let payment = parse_payment_arg(&args)?;
            if state.config.x402_enabled && payment.is_none() {
                return Ok(tool_payment_required_payload(
                    state,
                    api_base,
                    X402Resource::OrderBook,
                ));
            }
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::GET,
                path.as_str(),
                None,
                payment.as_ref(),
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "getTrades" => {
            let market_id = args
                .get("market_id")
                .and_then(|v| v.as_str().map(ToOwned::to_owned))
                .or_else(|| {
                    args.get("market_id")
                        .and_then(|v| v.as_u64())
                        .map(|v| v.to_string())
                })
                .ok_or_else(|| ApiError::bad_request("INVALID_ARGS", "market_id is required"))?;
            let mut path = format!("/evm/markets/{}/trades", market_id);
            if let Some(outcome) = args.get("outcome").and_then(|v| v.as_str()) {
                path = append_query(path.as_str(), "outcome", outcome);
            }
            if let Some(limit) = args.get("limit").and_then(|v| v.as_u64()) {
                path = append_query(path.as_str(), "limit", limit);
            }
            if let Some(offset) = args.get("offset").and_then(|v| v.as_u64()) {
                path = append_query(path.as_str(), "offset", offset);
            }

            let payment = parse_payment_arg(&args)?;
            if state.config.x402_enabled && payment.is_none() {
                return Ok(tool_payment_required_payload(
                    state,
                    api_base,
                    X402Resource::Trades,
                ));
            }
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::GET,
                path.as_str(),
                None,
                payment.as_ref(),
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "getAgents" => {
            let mut path = "/evm/agents".to_string();
            if let Some(limit) = args.get("limit").and_then(|v| v.as_u64()) {
                path = append_query(path.as_str(), "limit", limit);
            }
            if let Some(offset) = args.get("offset").and_then(|v| v.as_u64()) {
                path = append_query(path.as_str(), "offset", offset);
            }
            if let Some(owner) = args.get("owner").and_then(|v| v.as_str()) {
                path = append_query(path.as_str(), "owner", owner);
            }
            if let Some(market_id) = args.get("market_id").and_then(|v| v.as_u64()) {
                path = append_query(path.as_str(), "market_id", market_id);
            }
            if let Some(active) = args.get("active").and_then(|v| v.as_bool()) {
                path = append_query(path.as_str(), "active", active);
            }
            let (status, payload) =
                call_internal_api(state, reqwest::Method::GET, path.as_str(), None, None).await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "prepareExternalOrder" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/external/orders/intent",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "submitExternalOrder" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/external/orders/submit",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "cancelExternalOrder" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/external/orders/cancel",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "listExternalAgents" => {
            let mut path = "/external/agents".to_string();
            if let Some(provider) = args.get("provider").and_then(|v| v.as_str()) {
                path = append_query(path.as_str(), "provider", provider);
            }
            if let Some(active) = args.get("active").and_then(|v| v.as_bool()) {
                path = append_query(path.as_str(), "active", active);
            }
            if let Some(limit) = args.get("limit").and_then(|v| v.as_u64()) {
                path = append_query(path.as_str(), "limit", limit);
            }
            if let Some(offset) = args.get("offset").and_then(|v| v.as_u64()) {
                path = append_query(path.as_str(), "offset", offset);
            }
            let (status, payload) =
                call_internal_api(state, reqwest::Method::GET, path.as_str(), None, None).await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "executeExternalAgent" => {
            let agent_id = args
                .get("agent_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::bad_request("INVALID_ARGS", "agent_id is required"))?;

            let mut payload = args.clone();
            if let Some(obj) = payload.as_object_mut() {
                obj.remove("agent_id");
            }

            let (status, response_payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                format!("/external/agents/{}/execute", agent_id).as_str(),
                Some(payload),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &response_payload),
                ));
            }
            Ok(tool_result_payload(response_payload, false))
        }
        "prepareCreateAgentTx" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/evm/write/agents/create",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "prepareExecuteAgentTx" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/evm/write/agents/execute",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "prepareRegisterIdentityTx" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/evm/write/identity/register",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "prepareSetIdentityTierTx" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/evm/write/identity/tier",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "prepareSetIdentityActiveTx" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/evm/write/identity/active",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "prepareSubmitReputationOutcomeTx" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/evm/write/reputation/outcome",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "prepareValidationRequestTx" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/evm/write/validation/request",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "prepareValidationResponseTx" => {
            let (status, payload) = call_internal_api(
                state,
                reqwest::Method::POST,
                "/evm/write/validation/response",
                Some(args),
                None,
            )
            .await?;
            if status >= 400 {
                return Ok(tool_error_payload(
                    status,
                    web4_error_from_downstream(status, &payload),
                ));
            }
            Ok(tool_result_payload(payload, false))
        }
        "getX402Quote" => {
            let resource = match args.get("resource").and_then(|v| v.as_str()) {
                Some("orderbook") => X402Resource::OrderBook,
                Some("trades") => X402Resource::Trades,
                Some("mcp_tool_call") => X402Resource::McpToolCall,
                _ => {
                    return Ok(tool_error_payload(
                        400,
                        web4_error_payload(
                            "INVALID_X402_RESOURCE",
                            "resource must be one of: orderbook, trades, mcp_tool_call",
                            false,
                            None,
                            None,
                        ),
                    ))
                }
            };
            let origin = api_base.trim_end_matches("/v1");
            Ok(tool_result_payload(
                json!(build_quote_for_origin(state, origin, resource)),
                false,
            ))
        }
        "sendSwarmMessage" => {
            let payload: SwarmSendRequest = serde_json::from_value(args).map_err(|_| {
                ApiError::bad_request("INVALID_SWARM_MESSAGE", "swarm message payload is invalid")
            })?;
            match xmtp_swarm::send_message(state, payload).await {
                Ok(envelope) => Ok(tool_result_payload(json!(envelope), false)),
                Err(err) => Ok(tool_error_payload(
                    err.status,
                    api_error_as_web4_payload(&err),
                )),
            }
        }
        "listSwarmMessages" => {
            let swarm_id = args
                .get("swarm_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::bad_request("INVALID_ARGS", "swarm_id is required"))?;
            let query = SwarmListQuery {
                limit: args.get("limit").and_then(|v| v.as_u64()),
                offset: args.get("offset").and_then(|v| v.as_u64()),
            };
            match xmtp_swarm::list_messages(state, swarm_id, query).await {
                Ok(data) => Ok(tool_result_payload(json!(data), false)),
                Err(err) => Ok(tool_error_payload(
                    err.status,
                    api_error_as_web4_payload(&err),
                )),
            }
        }
        _ => Ok(tool_error_payload(
            404,
            web4_error_payload(
                "UNKNOWN_TOOL",
                format!("Unknown tool: {}", params.name).as_str(),
                false,
                None,
                None,
            ),
        )),
    }
}

async fn handle_mcp_method(
    state: &AppState,
    req: &HttpRequest,
    request: &McpJsonRpcRequest,
) -> Result<Value, ApiError> {
    let id = request.id.clone().unwrap_or(Value::Null);
    let api_base = format!(
        "{}/v1",
        api_origin_from_request(state, req).trim_end_matches('/')
    );

    match request.method.as_str() {
        "initialize" => Ok(mcp_response_result(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false },
                    "prompts": { "listChanged": false }
                },
                "serverInfo": {
                    "name": "relay44-mcp",
                    "version": "1.0.0"
                }
            }),
        )),
        "ping" => Ok(mcp_response_result(id, json!({ "ok": true }))),
        "tools/list" => Ok(mcp_response_result(id, json!({ "tools": mcp_tools() }))),
        "tools/call" => {
            let params: McpToolCallParams =
                serde_json::from_value(request.params.clone().ok_or_else(|| {
                    ApiError::bad_request("INVALID_PARAMS", "tools/call requires params")
                })?)
                .map_err(|_| {
                    ApiError::bad_request("INVALID_PARAMS", "tools/call params are invalid")
                })?;

            let result = handle_tool_call(state, api_base.as_str(), params).await?;
            Ok(mcp_response_result(id, result))
        }
        "resources/list" => Ok(mcp_response_result(
            id,
            json!({
                "resources": mcp_resources(api_base.as_str())
            }),
        )),
        "resources/read" => {
            let params: McpResourceReadParams =
                serde_json::from_value(request.params.clone().ok_or_else(|| {
                    ApiError::bad_request("INVALID_PARAMS", "resources/read requires params")
                })?)
                .map_err(|_| {
                    ApiError::bad_request("INVALID_PARAMS", "resources/read params are invalid")
                })?;

            let resource_payload = match params.uri.as_str() {
                "relay44://markets/live" => {
                    let (_, payload) = call_internal_api(
                        state,
                        reqwest::Method::GET,
                        "/evm/markets?source=all&limit=50",
                        None,
                        None,
                    )
                    .await?;
                    payload
                }
                "relay44://agents/active" => {
                    let (_, payload) = call_internal_api(
                        state,
                        reqwest::Method::GET,
                        "/evm/agents?active=true&limit=50",
                        None,
                        None,
                    )
                    .await?;
                    payload
                }
                "relay44://runtime/health" => {
                    let (_, payload) = call_internal_api(
                        state,
                        reqwest::Method::GET,
                        "/web4/runtime/health",
                        None,
                        None,
                    )
                    .await?;
                    payload
                }
                "relay44://xmtp/health" => xmtp_swarm::health(state),
                _ if params.uri.starts_with("http://") || params.uri.starts_with("https://") => {
                    let url = reqwest::Url::parse(params.uri.as_str()).map_err(|_| {
                        ApiError::bad_request("INVALID_RESOURCE_URI", "resource uri is invalid")
                    })?;
                    let relative = format!(
                        "{}{}",
                        url.path(),
                        url.query().map(|v| format!("?{v}")).unwrap_or_default()
                    );
                    let (_, payload) = call_internal_api(
                        state,
                        reqwest::Method::GET,
                        relative.as_str(),
                        None,
                        None,
                    )
                    .await?;
                    payload
                }
                _ => {
                    return Ok(mcp_response_error(
                        id,
                        -32602,
                        "Unknown resource uri",
                        Some(json!({ "uri": params.uri })),
                    ))
                }
            };

            Ok(mcp_response_result(
                id,
                json!({
                    "contents": [
                        {
                            "uri": params.uri,
                            "mimeType": "application/json",
                            "text": serde_json::to_string_pretty(&resource_payload).unwrap_or_else(|_| resource_payload.to_string())
                        }
                    ]
                }),
            ))
        }
        "prompts/list" => Ok(mcp_response_result(id, json!({ "prompts": mcp_prompts() }))),
        "prompts/get" => {
            let params: McpPromptGetParams =
                serde_json::from_value(request.params.clone().ok_or_else(|| {
                    ApiError::bad_request("INVALID_PARAMS", "prompts/get requires params")
                })?)
                .map_err(|_| {
                    ApiError::bad_request("INVALID_PARAMS", "prompts/get params are invalid")
                })?;

            let prompt_text = match params.name.as_str() {
                "market-scan" => {
                    let limit = params
                        .arguments
                        .get("limit")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(5);
                    let source = params
                        .arguments
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("all");
                    format!("Scan top {limit} active markets from source {source} and return ranked opportunities with: market_id, direction, confidence (0-100), expected edge, invalidation conditions, and execution notes.")
                }
                "market-analysis" => {
                    let market_id = params
                        .arguments
                        .get("market_id")
                        .and_then(|v| v.as_str().map(ToOwned::to_owned))
                        .or_else(|| {
                            params
                                .arguments
                                .get("market_id")
                                .and_then(|v| v.as_u64())
                                .map(|v| v.to_string())
                        })
                        .unwrap_or_else(|| "unknown".to_string());
                    format!("Analyze market {market_id} using order book depth, recent trades, and agent execution windows. Return: thesis, confidence (0-100), risk factors, and execution plan.")
                }
                "agent-launch" => {
                    let market_id = params
                        .arguments
                        .get("market_id")
                        .and_then(|v| v.as_str().map(ToOwned::to_owned))
                        .or_else(|| {
                            params
                                .arguments
                                .get("market_id")
                                .and_then(|v| v.as_u64())
                                .map(|v| v.to_string())
                        })
                        .unwrap_or_else(|| "unknown".to_string());
                    let outcome = params
                        .arguments
                        .get("outcome")
                        .and_then(|v| v.as_str())
                        .unwrap_or("yes");
                    let budget = params
                        .arguments
                        .get("budget_usdc")
                        .and_then(|v| v.as_str())
                        .unwrap_or("0");
                    format!("Given market {market_id}, target side {outcome}, and budget {budget} USDC, propose createAgent params: priceBps, size, cadence, expiryWindow, and strategy rationale.")
                }
                "swarm-coordination" => {
                    let swarm_id = params
                        .arguments
                        .get("swarm_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("default");
                    let objective = params
                        .arguments
                        .get("objective")
                        .and_then(|v| v.as_str())
                        .unwrap_or("execute market ops");
                    format!("Draft XMTP swarm message plan for swarm {swarm_id} to achieve objective: {objective}. Include role assignments, deadlines, and success criteria.")
                }
                _ => {
                    return Ok(mcp_response_error(
                        id,
                        -32602,
                        "Unknown prompt name",
                        Some(json!({ "name": params.name })),
                    ))
                }
            };

            Ok(mcp_response_result(
                id,
                json!({
                    "description": "Generated prompt",
                    "messages": [
                        {
                            "role": "user",
                            "content": {
                                "type": "text",
                                "text": prompt_text
                            }
                        }
                    ]
                }),
            ))
        }
        "notifications/initialized" => Ok(mcp_response_result(id, json!({ "ok": true }))),
        _ => Ok(mcp_response_error(
            id,
            -32601,
            "Method not found",
            Some(json!({ "method": request.method })),
        )),
    }
}

pub async fn handle_mcp_jsonrpc(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<Value>,
) -> impl Responder {
    let payload = body.into_inner();
    let mut responses = Vec::new();

    let requests: Vec<Value> = if payload.is_array() {
        payload.as_array().cloned().unwrap_or_default()
    } else {
        vec![payload]
    };

    if requests.is_empty() {
        return HttpResponse::BadRequest().json(mcp_response_error(
            Value::Null,
            -32600,
            "Invalid Request",
            Some(json!({ "reason": "empty batch" })),
        ));
    }

    for raw in requests {
        let parsed = serde_json::from_value::<McpJsonRpcRequest>(raw.clone());
        let request = match parsed {
            Ok(req) => req,
            Err(_) => {
                responses.push(mcp_response_error(
                    Value::Null,
                    -32600,
                    "Invalid Request",
                    Some(json!({ "payload": raw })),
                ));
                continue;
            }
