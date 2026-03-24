use actix_web::{web, HttpResponse, Responder};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use super::ApiError;
use crate::AppState;

fn is_base58_address(value: &str) -> bool {
    let trimmed = value.trim();
    let len = trimmed.len();
    if !(32..=44).contains(&len) {
        return false;
    }
    trimmed
        .chars()
        .all(|c| matches!(c, '1'..='9' | 'A'..='H' | 'J'..='N' | 'P'..='Z' | 'a'..='k' | 'm'..='z'))
}

fn require_solana_program_id(value: &str, env_key: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if !is_base58_address(trimmed) {
        return Err(ApiError::bad_request(
            "SOLANA_PROGRAM_ID_NOT_CONFIGURED",
            &format!("{env_key} must be set to a valid base58 address"),
        ));
    }
    Ok(trimmed.to_string())
}

fn ensure_solana_reads_enabled(state: &web::Data<Arc<AppState>>) -> Result<(), ApiError> {
    if !state.config.solana_enabled || !state.config.solana_reads_enabled {
        return Err(ApiError::bad_request(
            "SOLANA_DISABLED",
            "Solana read APIs are disabled",
        ));
    }
    Ok(())
}

fn ensure_solana_writes_enabled(state: &web::Data<Arc<AppState>>) -> Result<(), ApiError> {
    if !state.config.solana_enabled || !state.config.solana_writes_enabled {
        return Err(ApiError::bad_request(
            "SOLANA_WRITES_DISABLED",
            "Solana write APIs are disabled",
        ));
    }
    Ok(())
}

#[derive(Serialize)]
pub struct SolanaProgramsResponse {
    pub chain: &'static str,
    pub rpc_url: String,
    pub ws_url: String,
    pub market_program_id: String,
    pub orderbook_program_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy_program_id: Option<String>,
    pub reads_enabled: bool,
    pub writes_enabled: bool,
}

pub async fn get_solana_programs(
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_solana_reads_enabled(&state)?;

    let market_program_id = require_solana_program_id(
        state.config.solana_market_program_id.as_str(),
        "SOLANA_MARKET_PROGRAM_ID",
    )?;
    let orderbook_program_id = require_solana_program_id(
        state.config.solana_orderbook_program_id.as_str(),
        "SOLANA_ORDERBOOK_PROGRAM_ID",
    )?;
    let privacy_program_id = if state.config.solana_privacy_program_id.trim().is_empty() {
        None
    } else {
        Some(require_solana_program_id(
            state.config.solana_privacy_program_id.as_str(),
            "SOLANA_PRIVACY_PROGRAM_ID",
        )?)
    };

    Ok(HttpResponse::Ok().json(SolanaProgramsResponse {
        chain: "solana",
        rpc_url: state.config.solana_rpc_url.clone(),
        ws_url: state.config.solana_ws_url.clone(),
        market_program_id,
        orderbook_program_id,
        privacy_program_id,
        reads_enabled: state.config.solana_reads_enabled,
        writes_enabled: state.config.solana_writes_enabled,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaySolanaTransactionRequest {
    pub raw_tx: String,
    pub skip_preflight: Option<bool>,
    pub max_retries: Option<u64>,
}

#[derive(Serialize)]
pub struct RelaySolanaTransactionResponse {
    pub chain: &'static str,
    pub signature: String,
}

#[derive(Deserialize)]
struct SolanaSendTxResponse {
    result: Option<String>,
    error: Option<Value>,
}

pub async fn relay_raw_transaction(
    state: web::Data<Arc<AppState>>,
    body: web::Json<RelaySolanaTransactionRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_solana_writes_enabled(&state)?;

    let raw_tx = body.raw_tx.trim();
    if raw_tx.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_RAW_TX",
            "raw_tx must not be empty",
        ));
    }

    base64::engine::general_purpose::STANDARD
        .decode(raw_tx)
        .map_err(|_| ApiError::bad_request("INVALID_RAW_TX", "raw_tx must be base64 encoded"))?;

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sendTransaction",
        "params": [
            raw_tx,
            {
                "encoding": "base64",
                "skipPreflight": body.skip_preflight.unwrap_or(false),
                "maxRetries": body.max_retries.unwrap_or(3)
            }
        ]
    });

    let response = reqwest::Client::new()
        .post(&state.config.solana_rpc_url)
        .json(&payload)
        .send()
        .await
        .map_err(|_| ApiError::internal("Failed to relay Solana transaction"))?;

    if !response.status().is_success() {
        return Err(ApiError::bad_request(
            "SOLANA_RELAY_FAILED",
            "Solana RPC relay returned non-success status",
        ));
    }

    let rpc = response
        .json::<SolanaSendTxResponse>()
        .await
        .map_err(|_| ApiError::internal("Failed to decode Solana relay response"))?;

    if let Some(error) = rpc.error {
        return Err(ApiError::bad_request(
            "SOLANA_RELAY_FAILED",
            &format!("RPC error: {}", error),
        ));
    }

    let signature = rpc.result.ok_or_else(|| {
        ApiError::bad_request(
            "SOLANA_RELAY_FAILED",
            "Solana relay response missing transaction signature",
        )
    })?;

    Ok(HttpResponse::Ok().json(RelaySolanaTransactionResponse {
        chain: "solana",
        signature,
    }))
}

#[cfg(test)]
mod tests {
    use super::is_base58_address;

    #[test]
    fn validates_base58_program_id() {
        assert!(is_base58_address(
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        ));
        assert!(!is_base58_address("0xdeadbeef"));
        assert!(!is_base58_address("invalid@@@"));
    }
}
