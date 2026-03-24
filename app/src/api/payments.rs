use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use std::sync::Arc;

use crate::api::ApiError;
use crate::services::x402::{
    build_quote_for_request, ensure_payment_from_payload, X402PaymentPayload, X402Resource,
};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct X402QuoteQuery {
    pub resource: Option<String>,
}

fn parse_resource(raw: Option<&str>) -> Result<X402Resource, ApiError> {
    match raw.unwrap_or("orderbook") {
        "orderbook" => Ok(X402Resource::OrderBook),
        "trades" => Ok(X402Resource::Trades),
        "mcp_tool_call" | "mcp-tool-call" => Ok(X402Resource::McpToolCall),
        _ => Err(ApiError::bad_request(
            "INVALID_X402_RESOURCE",
            "resource must be one of: orderbook, trades, mcp_tool_call",
        )),
    }
}

pub async fn get_x402_quote(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<X402QuoteQuery>,
) -> Result<impl Responder, ApiError> {
    let resource = parse_resource(query.resource.as_deref())?;
    let quote = build_quote_for_request(&state, resource, &req);
    Ok(HttpResponse::Ok().json(quote))
}

pub async fn verify_x402_payment(
    state: web::Data<Arc<AppState>>,
    query: web::Query<X402QuoteQuery>,
    body: web::Json<X402PaymentPayload>,
) -> Result<impl Responder, ApiError> {
    let resource = parse_resource(query.resource.as_deref())?;
    ensure_payment_from_payload(&state, body.into_inner(), resource, None).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "ok": true,
        "resource": resource.as_str()
    })))
}
