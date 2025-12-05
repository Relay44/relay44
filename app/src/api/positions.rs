use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use std::sync::Arc;

use super::rate_limit::check_claim_rate_limit;
use super::ApiError;
use crate::models::{ClaimWinningsResponse, Outcome, PositionListResponse};
use crate::require_auth;
use crate::AppState;

const ORDER_BOOK_CLAIM_SELECTOR: &str = "379607f5";
const ORDER_BOOK_CLAIM_FOR_SELECTOR: &str = "0de05659";
const ORDER_BOOK_CLAIMED_TOPIC: &str =
    "0x93c1c30a0fa404e7a08a9f6a9d68323786a7e120f3adc0c16eb8855922e35dfa";

fn ensure_position_read_mode(state: &web::Data<Arc<AppState>>) -> Result<(), ApiError> {
    let evm_reads = state.config.evm_enabled && state.config.evm_reads_enabled;
    let solana_reads = state.config.solana_enabled && state.config.solana_reads_enabled;
    if !evm_reads && !solana_reads {
        return Err(ApiError::bad_request(
            "CHAIN_READ_PATH_DISABLED",
            "Position read path is disabled for all configured chains",
        ));
    }
    Ok(())
}

fn ensure_position_write_mode(state: &web::Data<Arc<AppState>>) -> Result<(), ApiError> {
    let evm_writes = state.config.evm_enabled && state.config.evm_writes_enabled;
    let solana_writes = state.config.solana_enabled && state.config.solana_writes_enabled;
    if !evm_writes && !solana_writes {
        return Err(ApiError::bad_request(
            "CHAIN_WRITE_PATH_DISABLED",
            "Position write path is disabled for all configured chains",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimWinningsRequest {
    pub tx_signature: String,
}

/// List all positions for authenticated user
pub async fn list_positions(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_position_read_mode(&state)?;

    // SECURITY: Extract authenticated user from request
    let user = require_auth!(&req, &state);
    let owner = &user.wallet_address;

    let positions = state
        .db
        .get_positions(owner)
        .await
        .map_err(ApiError::from)?;

    Ok(HttpResponse::Ok().json(PositionListResponse { positions }))
}

/// Get position for a specific market
pub async fn get_position(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    ensure_position_read_mode(&state)?;

    // SECURITY: Extract authenticated user from request
    let user = require_auth!(&req, &state);
    let owner = &user.wallet_address;

    let market_id = path.into_inner();

    let position = state
        .db
        .get_position(owner, &market_id)
        .await
        .map_err(ApiError::from)?;

    match position {
        Some(p) => Ok(HttpResponse::Ok().json(p)),
        None => Err(ApiError::not_found("Position")),
    }
}

/// Claim winnings for a resolved market
pub async fn claim_winnings(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<ClaimWinningsRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_position_write_mode(&state)?;

    // SECURITY: Extract authenticated user from request
    let user = require_auth!(&req, &state);
    let owner = &user.wallet_address;

    // SECURITY: Per-user rate limit (5 claims/min)
    check_claim_rate_limit(owner, &state.redis).await?;

    let market_id = path.into_inner();

    // Get market to verify it's resolved
    let market = state
        .db
        .get_market(&market_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Market"))?;

    if market.resolved_outcome.is_none() {
        return Err(ApiError::bad_request(
            "MARKET_NOT_RESOLVED",
            "Market has not been resolved yet",
        ));
    }

    // Get position - this will only return the authenticated user's position
    let position = state
        .db
        .get_position(owner, &market_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Position"))?;

    // SECURITY: Double-check ownership (defense in depth)
    if position.owner != *owner {
        return Err(ApiError::forbidden("You can only claim your own winnings"));
    }

    // Safe: already verified resolved_outcome.is_some() above
    let winning_outcome = market.resolved_outcome.expect("checked is_some above");
    let winning_tokens = match winning_outcome {
        Outcome::Yes => position.yes_balance,
        Outcome::No => position.no_balance,
    };

    if winning_tokens == 0 {
        return Err(ApiError::bad_request(
            "NO_WINNINGS",
            "No winning tokens to claim",
        ));
    }

    let claimed_amount = winning_tokens; // 1:1 redemption
    let tx_signature = body.tx_signature.trim().to_ascii_lowercase();
    if !is_valid_tx_hash(tx_signature.as_str()) {
        return Err(ApiError::bad_request(
            "INVALID_TX_SIGNATURE",
            "tx_signature must be a valid EVM transaction hash",
        ));
    }
    let market_id_num = market_id.parse::<u64>().map_err(|_| {
        ApiError::bad_request("INVALID_MARKET_ID", "market_id must be a positive integer")
    })?;
    verify_claim_tx(&state, owner.as_str(), market_id_num, tx_signature.as_str()).await?;

    // SECURITY: Log claim for audit trail
    log::info!(
        "Claim processed: market={}, user={}, amount={}, outcome={:?}",
        market_id,
        owner,
        claimed_amount,
        winning_outcome
    );

    Ok(HttpResponse::Ok().json(ClaimWinningsResponse {
        market_id,
        claimed_amount,
        winning_outcome,
        winning_tokens_burned: winning_tokens,
        tx_signature,
    }))
}

fn is_valid_tx_hash(tx: &str) -> bool {
    let hash = tx.strip_prefix("0x").unwrap_or(tx);
    hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

fn normalize_evm_address(address: &str) -> Result<String, ApiError> {
    let normalized = address.trim().to_ascii_lowercase();
    if normalized.len() != 42
        || !normalized.starts_with("0x")
        || !normalized[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "wallet must be a valid 0x EVM address",
        ));
    }
    Ok(normalized)
}

fn parse_u64_hex(value: &str) -> Option<u64> {
    let trimmed = value.trim().trim_start_matches("0x");
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.trim_start_matches('0');
    if normalized.is_empty() {
        return Some(0);
    }
    if normalized.len() > 16 {
        return None;
    }
    u64::from_str_radix(normalized, 16).ok()
}

fn parse_u64_calldata_word(word: &str) -> Option<u64> {
    if word.len() != 64 || !word.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    parse_u64_hex(word)
}
