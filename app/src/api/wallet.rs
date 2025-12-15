use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::ApiError;
use crate::models::TransactionType;
use crate::require_auth;
use crate::services::evm_rpc::RpcLog;
use crate::AppState;

const COLLATERAL_AVAILABLE_SELECTOR: &str = "0xa0821be3";
const COLLATERAL_LOCKED_SELECTOR: &str = "0x9ae697bf";
const ORDER_BOOK_CLAIMABLE_SELECTOR: &str = "0xa0c7f71c";
const COLLATERAL_DEPOSIT_SELECTOR: &str = "0xb6b55f25";
const COLLATERAL_WITHDRAW_SELECTOR: &str = "0x2e1a7d4d";
const COLLATERAL_DEPOSITED_TOPIC: &str =
    "0x2da466a7b24304f47e87fa2e1e5a81b9831ce54fec19055ce277ca2f39ba42c4";
const COLLATERAL_WITHDRAWN_TOPIC: &str =
    "0x7084f5476618d8e60b11ef0d7d3f06914655adb8793e28ff7f018d4c76d505d5";
const WALLET_INTENT_TTL_SECONDS: u64 = 1800;
const VAULT_BALANCE_POLL_ATTEMPTS: usize = 8;
const VAULT_BALANCE_POLL_DELAY_MS: u64 = 1500;

fn ensure_wallet_read_mode(state: &web::Data<Arc<AppState>>) -> Result<(), ApiError> {
    let evm_reads = state.config.evm_enabled && state.config.evm_reads_enabled;
    let solana_reads = state.config.solana_enabled && state.config.solana_reads_enabled;
    if !evm_reads && !solana_reads {
        return Err(ApiError::bad_request(
            "CHAIN_READ_PATH_DISABLED",
            "Wallet read path is disabled for all configured chains",
        ));
    }
    Ok(())
}

fn ensure_wallet_write_mode(state: &web::Data<Arc<AppState>>) -> Result<(), ApiError> {
    let evm_writes = state.config.evm_enabled && state.config.evm_writes_enabled;
    let solana_writes = state.config.solana_enabled && state.config.solana_writes_enabled;
    if !evm_writes && !solana_writes {
        return Err(ApiError::bad_request(
            "CHAIN_WRITE_PATH_DISABLED",
            "Wallet write path is disabled for all configured chains",
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletBalance {
    pub available: u64,
    pub locked: u64,
    pub claimable: u64,
    pub total: u64,
    pub pending_deposits: u64,
    pub pending_withdrawals: u64,
    pub source_block: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositRequest {
    pub amount: u64,
    pub mode: Option<WalletWriteMode>,
    pub intent_id: Option<String>,
    pub raw_tx: Option<String>,
    pub tx_signature: Option<String>,
    pub source: Option<DepositSource>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum DepositSource {
    Wallet,
    Blindfold,
    Jupiter,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositResponse {
    pub transaction_id: String,
    pub status: String,
    pub phase: String,
    pub amount: u64,
    pub deposit_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepared_transactions: Option<Vec<PreparedWalletTx>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_signature: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawRequest {
    pub amount: u64,
    pub mode: Option<WalletWriteMode>,
    pub intent_id: Option<String>,
    pub raw_tx: Option<String>,
    pub destination: Option<String>,
    pub tx_signature: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawResponse {
    pub transaction_id: String,
    pub status: String,
    pub phase: String,
    pub amount: u64,
    pub fee: u64,
    pub net_amount: u64,
    pub estimated_completion: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepared_transactions: Option<Vec<PreparedWalletTx>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_signature: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum WalletWriteMode {
    Prepare,
    Relay,
    Confirm,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PreparedWalletTx {
    pub step: String,
    pub to: String,
    pub data: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WalletWriteIntent {
    pub id: String,
    pub wallet: String,
    pub action: String,
    pub amount: u64,
    pub source: Option<String>,
    pub created_at: String,
    pub pre_available: u64,
    pub pre_locked: u64,
    pub status: String,
}

pub async fn get_balance(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_wallet_read_mode(&state)?;

    let user = require_auth!(&req, &state);
    let wallet = &user.wallet_address;

    let source_block = state.evm_rpc.eth_block_number().await.unwrap_or(0);

    let (available, locked_balance, claimable, total) =
        if state.config.evm_enabled && state.config.evm_reads_enabled {
            get_vault_balances(&state, wallet).await?
        } else {
            let settled_balance = get_settled_balance(&state, wallet).await?;
            let locked = get_locked_balance(&state, wallet).await?;
            let available = settled_balance.saturating_sub(locked);
            (available, locked, 0, settled_balance)
        };
    let (pending_deposits, pending_withdrawals) = get_pending_amounts(&state, wallet).await?;

    Ok(HttpResponse::Ok().json(WalletBalance {
        available,
        locked: locked_balance,
        claimable,
        total,
        pending_deposits,
        pending_withdrawals,
        source_block,
    }))
}

pub async fn get_deposit_address(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    ensure_wallet_read_mode(&state)?;

    let user = require_auth!(&req, &state);

    let deposit_info = serde_json::json!({
        "address": state.config.collateral_vault_address,
        "mint": state.config.usdc_mint,
        "memo_required": false,
        "memo_format": user.wallet_address,
        "network": "base",
        "minimum_amount": 1_000_000,
    });

    Ok(HttpResponse::Ok().json(deposit_info))
}

pub async fn deposit(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<DepositRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_wallet_write_mode(&state)?;

    let user = require_auth!(&req, &state);
    let wallet = user.wallet_address.to_ascii_lowercase();

    if body.amount < 1_000_000 {
        return Err(ApiError::bad_request(
            "INVALID_AMOUNT",
            "Minimum deposit is 1 USDC",
        ));
    }

    if body.amount > 1_000_000_000_000 {
        return Err(ApiError::bad_request(
            "INVALID_AMOUNT",
            "Maximum deposit is 1M USDC",
        ));
    }

    let source = body.source.unwrap_or(DepositSource::Wallet);
    if !matches!(source, DepositSource::Wallet) {
        return Err(ApiError::bad_request(
            "DEPOSIT_SOURCE_DISABLED",
            "Only wallet source is enabled for vault-first flow",
        ));
    }

    let mode = body.mode.unwrap_or(WalletWriteMode::Prepare);
    let transaction_id = Uuid::new_v4().to_string();
    match mode {
        WalletWriteMode::Prepare => {
            let (available, locked, _, _) = get_vault_balances(&state, wallet.as_str()).await?;
            let intent = WalletWriteIntent {
                id: Uuid::new_v4().to_string(),
                wallet: wallet.clone(),
                action: "deposit".to_string(),
                amount: body.amount,
                source: Some("wallet".to_string()),
                created_at: Utc::now().to_rfc3339(),
                pre_available: available,
                pre_locked: locked,
                status: "prepared".to_string(),
            };
            store_wallet_intent(&state, &intent).await?;

            let approve_tx = PreparedWalletTx {
                step: "approve".to_string(),
                to: state.config.usdc_mint.to_ascii_lowercase(),
                data: format!(
                    "0x{}{}{}",
                    "095ea7b3",
                    encode_address_word(state.config.collateral_vault_address.as_str()),
                    encode_u256_word(body.amount),
                ),
                value: "0x0".to_string(),
            };
            let deposit_tx = PreparedWalletTx {
                step: "deposit".to_string(),
                to: state.config.collateral_vault_address.to_ascii_lowercase(),
                data: format!(
                    "0x{}{}",
                    COLLATERAL_DEPOSIT_SELECTOR.trim_start_matches("0x"),
                    encode_u256_word(body.amount)
                ),
                value: "0x0".to_string(),
            };

            Ok(HttpResponse::Ok().json(DepositResponse {
                transaction_id,
                status: "pending".into(),
                phase: "prepared".into(),
                amount: body.amount,
                deposit_address: Some(state.config.collateral_vault_address.clone()),
                intent_id: Some(intent.id),
                prepared_transactions: Some(vec![approve_tx, deposit_tx]),
                tx_signature: None,
            }))
        }
        WalletWriteMode::Relay => {
            let intent_id = body.intent_id.as_ref().ok_or_else(|| {
                ApiError::bad_request("MISSING_FIELD", "intentId is required for relay mode")
            })?;
            let raw_tx = body.raw_tx.as_ref().ok_or_else(|| {
                ApiError::bad_request("MISSING_FIELD", "rawTx is required for relay mode")
            })?;
            if !is_valid_hex_payload(raw_tx) {
                return Err(ApiError::bad_request(
                    "INVALID_RAW_TX",
                    "rawTx must be a valid 0x-prefixed hex payload",
                ));
            }
            let mut intent = load_wallet_intent(&state, intent_id).await?;
            ensure_intent_owner(&intent, wallet.as_str(), "deposit", body.amount)?;

            let tx_hash = state
                .evm_rpc
                .eth_send_raw_transaction(raw_tx)
                .await
                .map_err(|err| {
                    ApiError::internal(&format!("failed to relay transaction: {}", err))
                })?;
            intent.status = "relayed".to_string();
            store_wallet_intent(&state, &intent).await?;

            Ok(HttpResponse::Accepted().json(DepositResponse {
                transaction_id,
                status: "pending".into(),
                phase: "relayed".into(),
                amount: body.amount,
                deposit_address: Some(state.config.collateral_vault_address.clone()),
                intent_id: Some(intent.id),
                prepared_transactions: None,
                tx_signature: Some(tx_hash),
            }))
        }
        WalletWriteMode::Confirm => {
            let intent_id = body.intent_id.as_ref().ok_or_else(|| {
                ApiError::bad_request("MISSING_FIELD", "intentId is required for confirm mode")
            })?;
            let tx_sig = body.tx_signature.as_ref().ok_or_else(|| {
                ApiError::bad_request("MISSING_FIELD", "txSignature is required for confirm mode")
            })?;
            if !is_valid_tx_hash(tx_sig) {
                return Err(ApiError::bad_request(
                    "INVALID_SIGNATURE",
                    "txSignature must be a valid EVM transaction hash",
                ));
            }
            ensure_tx_signature_unused(&state, tx_sig).await?;
            let mut intent = load_wallet_intent(&state, intent_id).await?;
            ensure_intent_owner(&intent, wallet.as_str(), "deposit", body.amount)?;
            verify_vault_intent_transaction(
                &state,
                &intent,
                tx_sig,
                COLLATERAL_DEPOSIT_SELECTOR,
                COLLATERAL_DEPOSITED_TOPIC,
            )
            .await?;

            wait_for_available_balance_at_least(
                &state,
                wallet.as_str(),
                intent.pre_available.saturating_add(body.amount),
                "deposit confirmation failed: vault available balance did not increase as expected",
            )
            .await?;
            intent.status = "confirmed".to_string();
            store_wallet_intent(&state, &intent).await?;

            record_transaction(
                &state,
                &transaction_id,
                &wallet,
                TransactionType::Deposit,
                body.amount,
                None,
                0,
                Some(tx_sig.to_ascii_lowercase()),
                "confirmed",
            )
            .await?;

            Ok(HttpResponse::Ok().json(DepositResponse {
                transaction_id,
                status: "confirmed".into(),
                phase: "confirmed".into(),
                amount: body.amount,
                deposit_address: Some(state.config.collateral_vault_address.clone()),
                intent_id: Some(intent.id),
                prepared_transactions: None,
                tx_signature: Some(tx_sig.to_ascii_lowercase()),
            }))
        }
    }
}

pub async fn withdraw(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<WithdrawRequest>,
) -> Result<impl Responder, ApiError> {
    ensure_wallet_write_mode(&state)?;

    let user = require_auth!(&req, &state);
    let wallet = user.wallet_address.to_ascii_lowercase();

    if body.amount < 1_000_000 {
        return Err(ApiError::bad_request(
            "INVALID_AMOUNT",
            "Minimum withdrawal is 1 USDC",
        ));
    }

    let (available, locked, _, _total) = get_vault_balances(&state, wallet.as_str()).await?;

    if body.amount > available {
        return Err(ApiError::bad_request(
            "INSUFFICIENT_BALANCE",
            &format!(
                "Insufficient balance. Available: {} USDC",
                available as f64 / 1_000_000.0
            ),
        ));
    }

    let destination = body.destination.clone().unwrap_or_default();
    if !destination.trim().is_empty() && !is_valid_evm_address(&destination) {
        return Err(ApiError::bad_request(
            "INVALID_ADDRESS",
            "Invalid destination address",
        ));
    }
    if !destination.trim().is_empty()
        && destination.to_ascii_lowercase() != wallet.to_ascii_lowercase()
    {
        return Err(ApiError::bad_request(
            "UNSUPPORTED_DESTINATION",
            "Vault-first withdraw flow only supports withdrawing to the authenticated wallet",
        ));
    }

    let fee = 0u64;
    let net_amount = body.amount;

    let transaction_id = Uuid::new_v4().to_string();
    let mode = body.mode.unwrap_or(WalletWriteMode::Prepare);
    match mode {
        WalletWriteMode::Prepare => {
            let intent = WalletWriteIntent {
                id: Uuid::new_v4().to_string(),
                wallet: wallet.clone(),
                action: "withdraw".to_string(),
                amount: body.amount,
                source: None,
                created_at: Utc::now().to_rfc3339(),
                pre_available: available,
                pre_locked: locked,
                status: "prepared".to_string(),
            };
            store_wallet_intent(&state, &intent).await?;

            let withdraw_tx = PreparedWalletTx {
                step: "withdraw".to_string(),
                to: state.config.collateral_vault_address.to_ascii_lowercase(),
                data: format!(
                    "0x{}{}",
                    COLLATERAL_WITHDRAW_SELECTOR.trim_start_matches("0x"),
                    encode_u256_word(body.amount)
                ),
                value: "0x0".to_string(),
            };

            Ok(HttpResponse::Ok().json(WithdrawResponse {
                transaction_id,
                status: "pending".into(),
                phase: "prepared".into(),
                amount: body.amount,
                fee,
                net_amount,
                estimated_completion: "Sign and submit withdraw transaction".into(),
                intent_id: Some(intent.id),
                prepared_transactions: Some(vec![withdraw_tx]),
                tx_signature: None,
            }))
        }
        WalletWriteMode::Relay => {
            let intent_id = body.intent_id.as_ref().ok_or_else(|| {
                ApiError::bad_request("MISSING_FIELD", "intentId is required for relay mode")
            })?;
            let raw_tx = body.raw_tx.as_ref().ok_or_else(|| {
                ApiError::bad_request("MISSING_FIELD", "rawTx is required for relay mode")
            })?;
            if !is_valid_hex_payload(raw_tx) {
                return Err(ApiError::bad_request(
                    "INVALID_RAW_TX",
                    "rawTx must be a valid 0x-prefixed hex payload",
                ));
            }
            let mut intent = load_wallet_intent(&state, intent_id).await?;
            ensure_intent_owner(&intent, wallet.as_str(), "withdraw", body.amount)?;

            let tx_hash = state
                .evm_rpc
                .eth_send_raw_transaction(raw_tx)
                .await
                .map_err(|err| {
                    ApiError::internal(&format!("failed to relay transaction: {}", err))
                })?;
            intent.status = "relayed".to_string();
            store_wallet_intent(&state, &intent).await?;

            Ok(HttpResponse::Accepted().json(WithdrawResponse {
                transaction_id,
                status: "pending".into(),
                phase: "relayed".into(),
                amount: body.amount,
                fee,
                net_amount,
                estimated_completion: "Awaiting onchain confirmation".into(),
                intent_id: Some(intent.id),
                prepared_transactions: None,
                tx_signature: Some(tx_hash),
            }))
        }
        WalletWriteMode::Confirm => {
            let intent_id = body.intent_id.as_ref().ok_or_else(|| {
                ApiError::bad_request("MISSING_FIELD", "intentId is required for confirm mode")
            })?;
            let tx_sig = body.tx_signature.as_ref().ok_or_else(|| {
                ApiError::bad_request("MISSING_FIELD", "txSignature is required for confirm mode")
            })?;
            if !is_valid_tx_hash(tx_sig) {
                return Err(ApiError::bad_request(
                    "INVALID_SIGNATURE",
                    "txSignature must be a valid EVM transaction hash",
                ));
            }
            ensure_tx_signature_unused(&state, tx_sig).await?;
            let mut intent = load_wallet_intent(&state, intent_id).await?;
            ensure_intent_owner(&intent, wallet.as_str(), "withdraw", body.amount)?;
            verify_vault_intent_transaction(
                &state,
                &intent,
                tx_sig,
                COLLATERAL_WITHDRAW_SELECTOR,
                COLLATERAL_WITHDRAWN_TOPIC,
            )
            .await?;
            wait_for_available_balance_at_most(
                &state,
                wallet.as_str(),
                intent.pre_available.saturating_sub(body.amount),
                "withdraw confirmation failed: vault available balance did not decrease as expected",
            )
            .await?;
            intent.status = "confirmed".to_string();
            store_wallet_intent(&state, &intent).await?;

            record_transaction(
                &state,
                &transaction_id,
                &wallet,
                TransactionType::Withdraw,
                body.amount,
                Some(wallet.as_str()),
                fee,
                Some(tx_sig.to_ascii_lowercase()),
                "confirmed",
            )
            .await?;

            Ok(HttpResponse::Ok().json(WithdrawResponse {
                transaction_id,
                status: "confirmed".into(),
                phase: "confirmed".into(),
                amount: body.amount,
                fee,
                net_amount,
                estimated_completion: "Settled onchain".into(),
                intent_id: Some(intent.id),
                prepared_transactions: None,
                tx_signature: Some(tx_sig.to_ascii_lowercase()),
            }))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BlindpayWebhook {
    pub event: String,
    pub payment_id: String,
    pub amount: u64,
    pub wallet_address: String,
    pub signature: String,
}

pub async fn blindfold_webhook(
    state: web::Data<Arc<AppState>>,
    body: web::Json<BlindpayWebhook>,
) -> Result<impl Responder, ApiError> {
    ensure_wallet_write_mode(&state)?;

    let expected_sig = compute_blindfold_signature(&body, &state.config.blindfold_webhook_secret);
    if body.signature != expected_sig {
        return Err(ApiError::unauthorized("Invalid webhook signature"));
    }

    let wallet = body.wallet_address.to_ascii_lowercase();

    match body.event.as_str() {
        "payment.completed" => {
            let tx_id = Uuid::new_v4().to_string();
            record_transaction(
                &state,
                &tx_id,
                &wallet,
                TransactionType::Deposit,
                body.amount,
                None,
                0,
                Some(body.payment_id.clone()),
                "confirmed",
            )
            .await?;
        }
        "payment.failed" => {
            update_transaction_status(&state, &body.payment_id, "failed").await?;
        }
        _ => {}
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({"received": true})))
}

async fn get_locked_balance(state: &AppState, wallet: &str) -> Result<u64, ApiError> {
    let (orders, _) = state
        .db
        .get_orders(
            wallet,
            None,
            Some(crate::models::OrderStatus::Open),
            1000,
            0,
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let locked: u64 = orders
        .iter()
        .map(|o| {
            let price = o.price_bps as u64;
            let quantity = o.remaining_quantity;
            (price * quantity) / 10000
        })
        .sum();

    Ok(locked)
}

async fn get_settled_balance(state: &AppState, wallet: &str) -> Result<u64, ApiError> {
    let (txs, _) = state
        .db
        .get_transactions(wallet, None, 1000, 0)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let mut balance: i128 = 0;

    for tx in txs.iter().filter(|tx| tx.status == "confirmed") {
        let amount = tx.amount as i128;
        match tx.tx_type {
            TransactionType::Deposit
            | TransactionType::Mint
            | TransactionType::Claim
            | TransactionType::Sell => balance += amount,
            TransactionType::Withdraw | TransactionType::Buy | TransactionType::Redeem => {
                balance -= amount
            }
        }
    }

    if balance <= 0 {
        Ok(0)
    } else {
        Ok(balance as u64)
    }
}

async fn get_pending_amounts(state: &AppState, wallet: &str) -> Result<(u64, u64), ApiError> {
    let (txs, _) = state
        .db
        .get_transactions(wallet, None, 100, 0)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;

    let pending_deposits: u64 = txs
        .iter()
        .filter(|t| matches!(t.tx_type, TransactionType::Deposit) && t.status == "pending")
        .map(|t| t.amount)
        .sum();

    let pending_withdrawals: u64 = txs
        .iter()
        .filter(|t| matches!(t.tx_type, TransactionType::Withdraw) && t.status == "pending")
        .map(|t| t.amount)
        .sum();

    Ok((pending_deposits, pending_withdrawals))
}

async fn record_transaction(
    state: &AppState,
    id: &str,
    owner: &str,
    tx_type: TransactionType,
    amount: u64,
    market_id: Option<&str>,
    fee: u64,
    tx_signature: Option<String>,
    status: &str,
) -> Result<(), ApiError> {
    let token = if state.config.usdc_mint.trim().is_empty() {
        "USDC".to_string()
    } else {
        state.config.usdc_mint.to_ascii_lowercase()
    };

    sqlx::query(
        r#"
        INSERT INTO transactions (id, owner, market_id, tx_type, amount, token, fee, tx_signature, status, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(id)
    .bind(owner)
    .bind(market_id)
    .bind(tx_type as i16)
    .bind(amount as i64)
    .bind(token)
    .bind(fee as i64)
    .bind(tx_signature)
    .bind(status)
    .bind(Utc::now())
    .execute(state.db.pool())
    .await
    .map_err(|e| ApiError::internal(&e.to_string()))?;

