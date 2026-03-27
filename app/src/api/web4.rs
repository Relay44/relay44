use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::api::ApiError;
use crate::config::ExternalExecutionMode;
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

fn polymarket_trading_ready(
    polymarket_enabled: bool,
    external_markets_enabled: bool,
    external_trading_enabled: bool,
    execution_mode: ExternalExecutionMode,
    gamma_api_base: &str,
    clob_api_base: &str,
) -> bool {
    polymarket_enabled
        && external_markets_enabled
        && external_trading_enabled
        && !execution_mode.is_paper()
        && !gamma_api_base.trim().is_empty()
        && !clob_api_base.trim().is_empty()
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
        };

        if request.jsonrpc != "2.0" {
            responses.push(mcp_response_error(
                request.id.unwrap_or(Value::Null),
                -32600,
                "Invalid Request: jsonrpc must be '2.0'",
                None,
            ));
            continue;
        }

        if request.id.is_none() {
            if let Err(err) = enforce_mcp_policy(&state, &req, &request).await {
                let _ = mcp_response_error(
                    Value::Null,
                    -32000,
                    "MCP request rejected by policy",
                    Some(json!({
                        "status": err.status,
                        "error": api_error_as_web4_payload(&err)
                    })),
                );
                continue;
            }
            let _ = handle_mcp_method(&state, &req, &request).await;
            continue;
        }

        if let Err(err) = enforce_mcp_policy(&state, &req, &request).await {
            responses.push(mcp_response_error(
                request.id.unwrap_or(Value::Null),
                -32000,
                "MCP request rejected by policy",
                Some(json!({
                    "status": err.status,
                    "error": api_error_as_web4_payload(&err)
                })),
            ));
            continue;
        }

        match handle_mcp_method(&state, &req, &request).await {
            Ok(response) => responses.push(response),
            Err(err) => responses.push(mcp_response_error(
                request.id.unwrap_or(Value::Null),
                -32000,
                "MCP method failed",
                Some(json!({
                    "status": err.status,
                    "error": api_error_as_web4_payload(&err)
                })),
            )),
        }
    }

    if responses.is_empty() {
        HttpResponse::NoContent().finish()
    } else if responses.len() == 1 {
        HttpResponse::Ok().json(responses.remove(0))
    } else {
        HttpResponse::Ok().json(responses)
    }
}

pub async fn get_web4_capabilities(state: web::Data<Arc<AppState>>) -> impl Responder {
    let api_base = infer_api_base_url(&state);
    let chains = configured_chains(&state);
    let limitless_trading_ready = state.config.limitless_enabled
        && state.config.external_trading_enabled
        && !state.config.limitless_api_key.trim().is_empty();
    let polymarket_trading_ready = polymarket_trading_ready(
        state.config.polymarket_enabled,
        state.config.external_markets_enabled,
        state.config.external_trading_enabled,
        state.config.external_execution_mode,
        state.config.polymarket_gamma_api_base.as_str(),
        state.config.polymarket_clob_api_base.as_str(),
    );
    let beta = (state.config.limitless_enabled && !limitless_trading_ready)
        || (state.config.polymarket_enabled && !polymarket_trading_ready);
    let erc8004_ready = is_hex_address(state.config.erc8004_identity_registry_address.as_str())
        && is_hex_address(state.config.erc8004_reputation_registry_address.as_str())
        && is_hex_address(state.config.erc8004_validation_registry_address.as_str());
    let x402_status = if state.config.x402_enabled {
        "implemented"
    } else {
        "disabled"
    };
    let x402_description = if state.config.x402_enabled {
        "Facilitator-backed x402 premium gating for orderbook, trades, and MCP premium calls."
    } else {
        "x402 is disabled by config."
    };
    let (xmtp_status, xmtp_description) = if !state.config.xmtp_swarm_enabled {
        ("disabled", "XMTP swarm messaging is disabled by config.")
    } else if state.config.xmtp_swarm_transport == "xmtp_http" {
        (
            "implemented",
            "XMTP swarm transport routed to XMTP HTTP bridge.",
        )
    } else {
        (
            "partial",
            "Redis relay mode enabled; full XMTP network bridge is not active.",
        )
    };

    HttpResponse::Ok().json(json!({
        "project": "relay44",
        "mode": "web4-agent-market-network",
        "chain_mode": state.config.chain_mode,
        "chains": chains,
        "api_base": api_base,
        "runtime": {
            "market_core_configured": !state.config.market_core_address.trim().is_empty(),
            "order_book_configured": !state.config.order_book_address.trim().is_empty(),
            "agent_runtime_configured": !state.config.agent_runtime_address.trim().is_empty(),
            "solana_programs_configured": !state.config.solana_market_program_id.trim().is_empty() && !state.config.solana_orderbook_program_id.trim().is_empty(),
            "evm_reads_enabled": state.config.evm_reads_enabled,
            "evm_writes_enabled": state.config.evm_writes_enabled,
            "solana_reads_enabled": state.config.solana_reads_enabled,
            "solana_writes_enabled": state.config.solana_writes_enabled,
            "external_markets_enabled": state.config.external_markets_enabled,
            "external_trading_enabled": state.config.external_trading_enabled,
            "external_agents_enabled": state.config.external_agents_enabled,
            "limitless_enabled": state.config.limitless_enabled,
            "polymarket_enabled": state.config.polymarket_enabled
        },
        "wallet": {
            "read_enabled": (state.config.evm_enabled && state.config.evm_reads_enabled)
                || (state.config.solana_enabled && state.config.solana_reads_enabled),
            "deposit_enabled": state.config.evm_enabled && state.config.evm_writes_enabled,
            "withdraw_enabled": state.config.evm_enabled && state.config.evm_writes_enabled,
            "claim_enabled": state.config.evm_enabled && state.config.evm_writes_enabled,
            "deposit_mode": if state.config.evm_enabled && state.config.evm_writes_enabled {
                "chain"
            } else {
                "disabled"
            },
            "withdraw_mode": if state.config.evm_enabled && state.config.evm_writes_enabled {
                "chain"
            } else {
                "disabled"
            }
        },
        "launch": {
            "beta": beta,
            "limitless_trading_ready": limitless_trading_ready,
            "polymarket_trading_ready": polymarket_trading_ready
        },
        "policy": {
            "mcp_method_rate_window_seconds": MCP_METHOD_WINDOW_SECONDS,
            "mcp_tool_rate_window_seconds": MCP_TOOL_WINDOW_SECONDS,
            "mcp_method_limits_per_window": {
                "default": MCP_DEFAULT_METHOD_LIMIT_PER_WINDOW,
                "query": MCP_QUERY_METHOD_LIMIT_PER_WINDOW,
                "tools_call": MCP_TOOL_CALL_METHOD_LIMIT_PER_WINDOW
            },
            "mcp_tool_limits_per_window": {
                "default": MCP_DEFAULT_TOOL_LIMIT_PER_WINDOW,
                "write_tools": MCP_WRITE_TOOL_LIMIT_PER_WINDOW,
                "swarm_tools": MCP_SWARM_TOOL_LIMIT_PER_WINDOW
            },
            "error_envelope": {
                "fields": ["code", "reason", "retryable", "quote"]
            }
        },
        "protocols": [
            {
                "id": "agents-md",
                "status": "implemented",
                "description": "Machine-readable project interface at repository root."
            },
            {
                "id": "mcp-jsonrpc-server",
                "status": "implemented",
                "description": "MCP JSON-RPC server with tools/resources/prompts over HTTP and stdio process transport.",
                "endpoint": "/v1/web4/mcp",
                "stdio_command": "npm run mcp:server"
            },
            {
                "id": "a2a-agent-card",
                "status": "implemented",
                "description": "Cross-agent discovery card for external orchestration systems.",
                "endpoint": "/v1/web4/agent-card"
            },
            {
                "id": "erc-8004-identity",
                "status": if erc8004_ready { "implemented" } else { "partial" },
                "description": if erc8004_ready {
                    "Onchain ERC-8004 identity, reputation, and validation registries integrated into runtime and write-prep flows."
                } else {
                    "ERC-8004 integration is present, but one or more registry addresses are not configured."
                }
            },
            {
                "id": "x402-agent-payments",
                "status": x402_status,
                "description": x402_description
            },
            {
                "id": "xmtp-swarm",
                "status": xmtp_status,
                "description": xmtp_description
            },
            {
                "id": "external-venues",
                "status": if state.config.external_markets_enabled { "implemented" } else { "disabled" },
                "description": "Unified market aggregation and external execution surface for Limitless and Polymarket."
            }
        ]
    }))
}

pub async fn get_mcp_manifest(req: HttpRequest, state: web::Data<Arc<AppState>>) -> impl Responder {
    let api_base = format!(
        "{}/v1",
        api_origin_from_request(&state, &req).trim_end_matches('/')
    );

    HttpResponse::Ok().json(json!({
        "name": "relay44",
        "version": "1.0.0",
        "description": "MCP JSON-RPC server for relay44 dual-chain (Base + Solana) agent market network.",
        "transport": {
            "type": "http+jsonrpc",
            "endpoint": format!("{}/web4/mcp", api_base)
        },
        "transports": [
            {
                "type": "http+jsonrpc",
                "endpoint": format!("{}/web4/mcp", api_base)
            },
            {
                "type": "stdio",
                "command": "npm",
                "args": ["run", "mcp:server"],
                "env": {
                    "RELAY44_API_BASE_URL": api_base
                }
            }
        ],
        "jsonrpc": {
            "version": "2.0",
            "supported_methods": [
                "initialize",
                "ping",
                "tools/list",
                "tools/call",
                "resources/list",
                "resources/read",
                "prompts/list",
                "prompts/get"
            ]
        },
        "tools": mcp_tools(),
        "resources": mcp_resources(api_base.as_str()),
        "prompts": mcp_prompts()
    }))
}

pub async fn get_agent_card(req: HttpRequest, state: web::Data<Arc<AppState>>) -> impl Responder {
    let api_base = format!(
        "{}/v1",
        api_origin_from_request(&state, &req).trim_end_matches('/')
    );
    let chains = configured_chains(&state);
    let auth = if state.config.chain_mode == "solana" {
        json!({
            "type": "solana-signin+jwt",
            "nonce_endpoint": format!("{}/auth/solana/nonce", api_base),
            "login_endpoint": format!("{}/auth/solana/login", api_base)
        })
    } else if state.config.chain_mode == "dual" {
        json!({
            "type": "multiwallet+jwt",
            "flows": [
                {
                    "wallet": "evm",
                    "type": "siwe+jwt",
                    "nonce_endpoint": format!("{}/auth/siwe/nonce", api_base),
                    "login_endpoint": format!("{}/auth/siwe/login", api_base)
                },
                {
                    "wallet": "solana",
                    "type": "solana-signin+jwt",
                    "nonce_endpoint": format!("{}/auth/solana/nonce", api_base),
                    "login_endpoint": format!("{}/auth/solana/login", api_base)
                }
            ]
        })
    } else {
        json!({
            "type": "siwe+jwt",
            "nonce_endpoint": format!("{}/auth/siwe/nonce", api_base),
            "login_endpoint": format!("{}/auth/siwe/login", api_base)
        })
    };

    HttpResponse::Ok().json(json!({
        "schema": "a2a-agent-card/v1",
        "name": "relay44 agent network",
        "description": "Agent-executable prediction market infrastructure on Base and Solana.",
        "network": {
            "chain_mode": state.config.chain_mode,
            "chains": chains
        },
        "auth": auth,
        "capabilities": [
            {
                "id": "market-data",
                "description": "Query markets, orderbooks and fills from configured chains."
            },
            {
                "id": "agent-runtime",
                "description": "Discover, launch and execute autonomous market agents."
            },
            {
                "id": "mcp-jsonrpc",
                "description": "MCP tools/resources/prompts via JSON-RPC."
            },
            {
                "id": "x402-payments",
                "description": "x402 payment-gated premium data routes."
            },
            {
                "id": "xmtp-swarm",
                "description": "Signed swarm coordination channels."
            }
        ],
        "actions": [
            {
                "name": "mcp_jsonrpc",
                "method": "POST",
                "url": format!("{}/web4/mcp", api_base)
            },
            {
                "name": "list_markets",
                "method": "GET",
                "url": format!("{}/evm/markets", api_base)
            },
            {
                "name": "get_solana_programs",
                "method": "GET",
                "url": format!("{}/solana/programs", api_base)
            },
            {
                "name": "list_agents",
                "method": "GET",
                "url": format!("{}/evm/agents?active=true", api_base)
            },
            {
                "name": "prepare_create_agent",
                "method": "POST",
                "url": format!("{}/evm/write/agents/create", api_base)
            },
            {
                "name": "prepare_execute_agent",
                "method": "POST",
                "url": format!("{}/evm/write/agents/execute", api_base)
            },
            {
                "name": "prepare_validation_request",
                "method": "POST",
                "url": format!("{}/evm/write/validation/request", api_base)
            },
            {
                "name": "prepare_validation_response",
                "method": "POST",
                "url": format!("{}/evm/write/validation/response", api_base)
            },
            {
                "name": "relay_solana_tx",
                "method": "POST",
                "url": format!("{}/solana/write/relay", api_base)
            },
            {
                "name": "send_swarm_message",
                "method": "POST",
                "url": format!("{}/web4/xmtp/swarm/send", api_base)
            }
        ]
    }))
}

pub async fn get_xmtp_swarm_health(state: web::Data<Arc<AppState>>) -> impl Responder {
    HttpResponse::Ok().json(xmtp_swarm::health(&state))
}

pub async fn get_web4_runtime_health(state: web::Data<Arc<AppState>>) -> impl Responder {
    let x402_receiver_valid = is_hex_address(state.config.x402_receiver_address.as_str());
    let x402_facilitator_configured = !state.config.x402_facilitator_url.trim().is_empty();
    let x402_facilitator_reachable = if state.config.x402_enabled && x402_facilitator_configured {
        let url = format!(
            "{}/health",
            state
                .config
                .x402_facilitator_url
                .trim()
                .trim_end_matches('/')
        );
        reqwest::Client::new()
            .get(url)
            .bearer_auth(state.config.x402_facilitator_token.as_str())
            .send()
            .await
            .map(|response| response.status().is_success())
            .unwrap_or(false)
    } else {
        false
    };
    let x402_ready = state.config.x402_enabled
        && x402_receiver_valid
        && x402_facilitator_configured
        && x402_facilitator_reachable;

    let mcp_ready = true;

    let xmtp_transport = state.config.xmtp_swarm_transport.as_str();
    let xmtp_transport_http = xmtp_transport == "xmtp_http";
    let xmtp_transport_redis = xmtp_transport == "redis";
    let xmtp_bridge_configured = !state.config.xmtp_swarm_bridge_url.trim().is_empty();
    let xmtp_bridge_reachable =
        if state.config.xmtp_swarm_enabled && xmtp_transport_http && xmtp_bridge_configured {
            let url = format!(
                "{}/health",
                state
                    .config
                    .xmtp_swarm_bridge_url
                    .trim()
                    .trim_end_matches('/')
            );
            reqwest::Client::new()
                .get(url)
                .send()
                .await
                .map(|response| response.status().is_success())
                .unwrap_or(false)
        } else {
            false
        };

    let xmtp_ready = state.config.xmtp_swarm_enabled
        && if xmtp_transport_http {
            xmtp_bridge_configured && xmtp_bridge_reachable
        } else if xmtp_transport_redis {
            true
        } else {
            false
        };
    let full_web4_ready = mcp_ready && x402_ready && xmtp_ready;

    let status = if full_web4_ready {
        "healthy"
    } else if mcp_ready {
        "degraded"
    } else {
        "unhealthy"
    };

    HttpResponse::Ok().json(json!({
        "status": status,
        "components": {
            "mcp": {
                "ready": mcp_ready,
                "transport": ["http+jsonrpc", "stdio"],
                "requiredForFullWeb4": true
            },
            "x402": {
                "ready": x402_ready,
                "enabled": state.config.x402_enabled,
                "requiredForFullWeb4": true,
                "config": {
                    "receiverAddressValid": x402_receiver_valid,
                    "facilitatorConfigured": x402_facilitator_configured,
                    "facilitatorReachable": x402_facilitator_reachable
                }
            },
            "xmtp": {
                "ready": xmtp_ready,
                "enabled": state.config.xmtp_swarm_enabled,
                "requiredForFullWeb4": true,
                "transport": state.config.xmtp_swarm_transport,
                "config": {
                    "transportHttp": xmtp_transport_http,
                    "transportRedis": xmtp_transport_redis,
                    "bridgeConfigured": xmtp_bridge_configured,
                    "bridgeReachable": xmtp_bridge_reachable
                }
            }
        },
        "fullWeb4Ready": full_web4_ready
    }))
}

pub async fn send_xmtp_swarm_message(
    state: web::Data<Arc<AppState>>,
    body: web::Json<SwarmSendRequest>,
) -> Result<impl Responder, ApiError> {
    let message = xmtp_swarm::send_message(&state, body.into_inner()).await?;
    Ok(HttpResponse::Created().json(message))
}

pub async fn list_xmtp_swarm_messages(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<SwarmListQuery>,
) -> Result<impl Responder, ApiError> {
    let response = xmtp_swarm::list_messages(&state, path.as_str(), query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polymarket_trading_ready_requires_live_execution_and_api_bases() {
        assert!(polymarket_trading_ready(
            true,
            true,
            true,
            ExternalExecutionMode::Live,
            "https://gamma-api.polymarket.com",
            "https://clob.polymarket.com",
        ));
        assert!(!polymarket_trading_ready(
            true,
            true,
            true,
            ExternalExecutionMode::Paper,
            "https://gamma-api.polymarket.com",
            "https://clob.polymarket.com",
        ));
        assert!(!polymarket_trading_ready(
            true,
            true,
            true,
            ExternalExecutionMode::Live,
            "",
            "https://clob.polymarket.com",
        ));
        assert!(!polymarket_trading_ready(
            true,
            false,
            true,
            ExternalExecutionMode::Live,
            "https://gamma-api.polymarket.com",
            "https://clob.polymarket.com",
        ));
    }
}
