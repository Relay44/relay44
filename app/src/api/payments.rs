use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use std::sync::Arc;

use crate::api::ApiError;
use crate::services::x402::{
    append_payment_response_header, build_quote_for_request, ensure_payment_from_payload,
    X402PaymentPayload, X402Resource, X402SettleResponse,
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

fn verify_payment_response(
    resource: X402Resource,
    settlement: Option<&X402SettleResponse>,
) -> Result<HttpResponse, ApiError> {
    let mut response = HttpResponse::Ok();
    append_payment_response_header(&mut response, settlement)?;
    Ok(response.json(serde_json::json!({
        "ok": true,
        "resource": resource.as_str()
    })))
}

pub async fn get_x402_quote(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<X402QuoteQuery>,
) -> Result<impl Responder, ApiError> {
    let resource = parse_resource(query.resource.as_deref())?;
    let quote = build_quote_for_request(&state.config, resource, &req);
    Ok(HttpResponse::Ok().json(quote))
}

pub async fn verify_x402_payment(
    state: web::Data<Arc<AppState>>,
    query: web::Query<X402QuoteQuery>,
    body: web::Json<X402PaymentPayload>,
) -> Result<impl Responder, ApiError> {
    let resource = parse_resource(query.resource.as_deref())?;
    let settlement =
        ensure_payment_from_payload(&state.config, body.into_inner(), resource, None).await?;
    verify_payment_response(resource, settlement.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::body::to_bytes;
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    #[actix_rt::test]
    async fn verify_payment_response_includes_payment_response_header() {
        let settlement = X402SettleResponse {
            success: true,
            error_reason: None,
            error_message: None,
            payer: Some("0x1111111111111111111111111111111111111111".to_string()),
            transaction: "0xabc".to_string(),
            network: "eip155:8453".to_string(),
            extensions: None,
        };

        let response = verify_payment_response(X402Resource::OrderBook, Some(&settlement)).unwrap();
        let header = response
            .headers()
            .get("PAYMENT-RESPONSE")
            .and_then(|value| value.to_str().ok())
            .expect("missing payment response header");
        let decoded: X402SettleResponse =
            serde_json::from_slice(&BASE64.decode(header).unwrap()).unwrap();
        let body = to_bytes(response.into_body()).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(decoded.transaction, "0xabc");
        assert_eq!(payload["ok"], serde_json::json!(true));
        assert_eq!(payload["resource"], serde_json::json!("orderbook"));
    }

    #[actix_rt::test]
    async fn verify_payment_response_omits_header_without_settlement() {
        let response = verify_payment_response(X402Resource::Trades, None).unwrap();
        let header = response.headers().get("PAYMENT-RESPONSE").cloned();
        let body = to_bytes(response.into_body()).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(header.is_none());
        assert_eq!(payload["ok"], serde_json::json!(true));
        assert_eq!(payload["resource"], serde_json::json!("trades"));
    }
}
