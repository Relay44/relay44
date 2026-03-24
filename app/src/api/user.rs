use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use std::sync::Arc;

use super::ApiError;
use crate::models::{
    ListTransactionsQuery, TransactionListResponse, TransactionType, User, UserSettings, UserStats,
};
use crate::require_auth;
use crate::AppState;

/// Get user profile
pub async fn get_profile(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    // SECURITY: Extract authenticated user from request
    let user = require_auth!(&req, &state);
    let wallet = user.wallet_address;

    // In production, aggregate stats from database
    let profile = User {
        wallet: wallet.clone(),
        username: None,
        created_at: Utc::now(),
        stats: UserStats {
            total_trades: 0,
            total_volume: 0.0,
            win_rate: 0.0,
            pnl_30d: 0.0,
            pnl_all_time: 0.0,
        },
        settings: UserSettings {
            default_privacy_mode: "public".to_string(),
            notifications_enabled: true,
        },
    };

    Ok(HttpResponse::Ok().json(profile))
}

/// Get transaction history
pub async fn get_transactions(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListTransactionsQuery>,
) -> Result<impl Responder, ApiError> {
    // SECURITY: Extract authenticated user from request
    let user = require_auth!(&req, &state);
    let owner = &user.wallet_address;

    let tx_type = query.tx_type.as_ref().map(|t| match t.as_str() {
        "deposit" => TransactionType::Deposit,
        "withdraw" => TransactionType::Withdraw,
        "buy" => TransactionType::Buy,
        "sell" => TransactionType::Sell,
        "claim" => TransactionType::Claim,
        "mint" => TransactionType::Mint,
        "redeem" => TransactionType::Redeem,
        _ => TransactionType::Buy,
    });

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let (transactions, total) = state
        .db
        .get_transactions(owner, tx_type, limit, offset)
        .await
        .map_err(ApiError::from)?;

    Ok(HttpResponse::Ok().json(TransactionListResponse {
        transactions,
        total,
    }))
}
