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

fn parse_address_calldata_word(word: &str) -> Option<String> {
    if word.len() != 64 || !word.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let address = format!("0x{}", &word[24..]);
    normalize_evm_address(address.as_str()).ok()
}

fn strip_0x(value: &str) -> &str {
    value.strip_prefix("0x").unwrap_or(value)
}

fn is_zero_hex(value: &str) -> bool {
    let trimmed = strip_0x(value).trim();
    !trimmed.is_empty() && trimmed.chars().all(|c| c == '0')
}

fn topic_matches_market(topic: &str, market_id: u64) -> bool {
    let word = strip_0x(topic);
    parse_u64_calldata_word(word) == Some(market_id)
}

fn topic_matches_address(topic: &str, expected: &str) -> bool {
    let word = strip_0x(topic);
    parse_address_calldata_word(word).as_deref() == Some(expected)
}

async fn verify_claim_tx(
    state: &AppState,
    owner: &str,
    market_id: u64,
    tx_hash: &str,
) -> Result<(), ApiError> {
    let owner = normalize_evm_address(owner)?;
    let order_book =
        normalize_evm_address(state.config.order_book_address.as_str()).map_err(|_| {
            ApiError::internal("ORDER_BOOK_ADDRESS is not configured as a valid EVM address")
        })?;
    let receipt = state
        .evm_rpc
        .eth_get_transaction_receipt(tx_hash)
        .await
        .map_err(|_| {
            ApiError::bad_request(
                "INVALID_TX_SIGNATURE",
                "unable to fetch transaction receipt",
            )
        })?
        .ok_or_else(|| {
            ApiError::bad_request("INVALID_TX_SIGNATURE", "transaction receipt not found")
        })?;
    let status = receipt
        .status
        .as_deref()
        .and_then(parse_u64_hex)
        .ok_or_else(|| {
            ApiError::bad_request("INVALID_TX_SIGNATURE", "transaction status unavailable")
        })?;
    if status != 1 {
        return Err(ApiError::bad_request(
            "INVALID_TX_SIGNATURE",
            "transaction reverted onchain",
        ));
    }

    let tx = state
        .evm_rpc
        .eth_get_transaction_by_hash(tx_hash)
        .await
        .map_err(|_| ApiError::bad_request("INVALID_TX_SIGNATURE", "unable to fetch transaction"))?
        .ok_or_else(|| ApiError::bad_request("INVALID_TX_SIGNATURE", "transaction not found"))?;
    if tx.hash.trim().to_ascii_lowercase() != tx_hash.to_ascii_lowercase() {
        return Err(ApiError::bad_request(
            "INVALID_TX_SIGNATURE",
            "transaction hash mismatch",
        ));
    }
    let sender = tx
        .from
        .as_deref()
        .map(normalize_evm_address)
        .transpose()?
        .ok_or_else(|| {
            ApiError::bad_request("INVALID_TX_SIGNATURE", "transaction sender unavailable")
        })?;
    let target = tx
        .to
        .as_deref()
        .map(normalize_evm_address)
        .transpose()?
        .ok_or_else(|| {
            ApiError::bad_request("INVALID_TX_SIGNATURE", "transaction target unavailable")
        })?;
    if target != order_book {
        return Err(ApiError::bad_request(
            "INVALID_TX_SIGNATURE",
            "transaction target does not match configured order book",
        ));
    }
    if !is_zero_hex(tx.value.as_str()) {
        return Err(ApiError::bad_request(
            "INVALID_TX_SIGNATURE",
            "claim transaction must not transfer native value",
        ));
    }
    if let (Some(tx_block), Some(receipt_block)) =
        (tx.block_number.as_deref(), receipt.block_number.as_deref())
    {
        let tx_block = parse_u64_hex(tx_block).ok_or_else(|| {
            ApiError::bad_request("INVALID_TX_SIGNATURE", "transaction block is invalid")
        })?;
        let receipt_block = parse_u64_hex(receipt_block).ok_or_else(|| {
            ApiError::bad_request("INVALID_TX_SIGNATURE", "receipt block is invalid")
        })?;
        if tx_block != receipt_block {
            return Err(ApiError::bad_request(
                "INVALID_TX_SIGNATURE",
                "transaction and receipt block mismatch",
            ));
        }
    }

    let calldata = strip_0x(tx.input.as_str());
    if calldata.len() < 8 {
        return Err(ApiError::bad_request(
            "INVALID_TX_SIGNATURE",
            "transaction input is too short",
        ));
    }

    let selector = &calldata[..8];
    let args = &calldata[8..];
    match selector {
        ORDER_BOOK_CLAIM_SELECTOR => {
            if sender != owner {
                return Err(ApiError::forbidden(
                    "claim transaction sender does not match authenticated wallet",
                ));
            }
            if args.len() < 64 {
                return Err(ApiError::bad_request(
                    "INVALID_TX_SIGNATURE",
                    "claim calldata missing market id",
                ));
            }
            let tx_market_id = parse_u64_calldata_word(&args[..64]).ok_or_else(|| {
                ApiError::bad_request("INVALID_TX_SIGNATURE", "unable to decode claim market id")
            })?;
            if tx_market_id != market_id {
                return Err(ApiError::bad_request(
                    "INVALID_TX_SIGNATURE",
                    "claim transaction market id mismatch",
                ));
            }
        }
        ORDER_BOOK_CLAIM_FOR_SELECTOR => {
            if args.len() < 128 {
                return Err(ApiError::bad_request(
                    "INVALID_TX_SIGNATURE",
                    "claimFor calldata is malformed",
                ));
            }
            let tx_owner = parse_address_calldata_word(&args[..64]).ok_or_else(|| {
                ApiError::bad_request("INVALID_TX_SIGNATURE", "unable to decode claimFor owner")
            })?;
            if tx_owner != owner {
                return Err(ApiError::forbidden(
                    "claimFor owner does not match authenticated wallet",
                ));
            }
            let tx_market_id = parse_u64_calldata_word(&args[64..128]).ok_or_else(|| {
                ApiError::bad_request(
                    "INVALID_TX_SIGNATURE",
                    "unable to decode claimFor market id",
                )
            })?;
            if tx_market_id != market_id {
                return Err(ApiError::bad_request(
                    "INVALID_TX_SIGNATURE",
                    "claimFor transaction market id mismatch",
                ));
            }
        }
        _ => {
            return Err(ApiError::bad_request(
                "INVALID_TX_SIGNATURE",
                "transaction is not an orderbook claim call",
            ))
        }
    }

    let has_claim_log = receipt.logs.iter().any(|log| {
        let log_address = log
            .address
            .as_deref()
            .and_then(|value| normalize_evm_address(value).ok());
        if log_address.as_deref() != Some(order_book.as_str()) {
            return false;
        }
        if log.topics.len() < 3 {
            return false;
        }
        if log.topics[0].trim().to_ascii_lowercase() != ORDER_BOOK_CLAIMED_TOPIC {
            return false;
        }
        topic_matches_market(log.topics[1].as_str(), market_id)
            && topic_matches_address(log.topics[2].as_str(), owner.as_str())
    });
    if !has_claim_log {
        return Err(ApiError::bad_request(
            "INVALID_TX_SIGNATURE",
            "claim receipt missing expected Claimed event",
        ));
    }

    Ok(())
}
