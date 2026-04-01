use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;

use super::ApiError;
use crate::api::validation::validate_pagination;
use crate::AppState;

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn validate_wallet(wallet: &str) -> Result<(), ApiError> {
    if wallet.len() != 42
        || !wallet.starts_with("0x")
        || !wallet[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "Wallet must be a valid 0x-prefixed EVM address",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// GET /v1/profiles/{wallet}
// ---------------------------------------------------------------------------

pub async fn get_public_profile(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let wallet = path.into_inner().to_lowercase();
    validate_wallet(&wallet)?;

    let pool = state.db.pool();

    // Aggregate stats from trades table
    let stats_row = sqlx::query(
        "SELECT
            COALESCE(COUNT(*), 0)::BIGINT AS total_trades,
            COALESCE(SUM(ABS(CAST(price AS DOUBLE PRECISION) * CAST(quantity AS DOUBLE PRECISION))), 0) AS total_volume,
            COALESCE(SUM(pnl), 0) AS pnl_all_time,
            COALESCE(COUNT(DISTINCT market_id), 0)::BIGINT AS markets_traded,
            MIN(created_at) AS first_trade
         FROM (
            SELECT market_id, price, quantity, created_at,
                   -1.0 * CAST(price AS DOUBLE PRECISION) * CAST(quantity AS DOUBLE PRECISION) AS pnl
            FROM trades WHERE LOWER(buyer) = $1
            UNION ALL
            SELECT market_id, price, quantity, created_at,
                   CAST(price AS DOUBLE PRECISION) * CAST(quantity AS DOUBLE PRECISION) AS pnl
            FROM trades WHERE LOWER(seller) = $1
         ) sub"
    )
    .bind(&wallet)
    .fetch_one(pool)
    .await?;

    let total_trades: i64 = stats_row.get("total_trades");
    let total_volume: f64 = stats_row.get("total_volume");
    let pnl_all_time: f64 = stats_row.get("pnl_all_time");
    let markets_traded: i64 = stats_row.get("markets_traded");
    let first_trade: Option<chrono::DateTime<chrono::Utc>> =
        stats_row.try_get("first_trade").ok().flatten();

    // 30d PnL
    let pnl_30d_row = sqlx::query(
        "SELECT COALESCE(SUM(pnl), 0) AS pnl_30d FROM (
            SELECT -1.0 * CAST(price AS DOUBLE PRECISION) * CAST(quantity AS DOUBLE PRECISION) AS pnl
            FROM trades WHERE LOWER(buyer) = $1 AND created_at >= NOW() - INTERVAL '30 days'
            UNION ALL
            SELECT CAST(price AS DOUBLE PRECISION) * CAST(quantity AS DOUBLE PRECISION) AS pnl
            FROM trades WHERE LOWER(seller) = $1 AND created_at >= NOW() - INTERVAL '30 days'
        ) sub"
    )
    .bind(&wallet)
    .fetch_one(pool)
    .await?;
    let pnl_30d: f64 = pnl_30d_row.get("pnl_30d");

    // Win rate (simplified: buy < 0.5 or sell >= 0.5)
    let win_row = sqlx::query(
        "SELECT
            COUNT(*)::DOUBLE PRECISION AS total,
            SUM(CASE WHEN is_win THEN 1 ELSE 0 END)::DOUBLE PRECISION AS wins
         FROM (
            SELECT (CAST(price AS DOUBLE PRECISION) < 0.5) AS is_win
            FROM trades WHERE LOWER(buyer) = $1
            UNION ALL
            SELECT (CAST(price AS DOUBLE PRECISION) >= 0.5) AS is_win
            FROM trades WHERE LOWER(seller) = $1
         ) sub"
    )
    .bind(&wallet)
    .fetch_one(pool)
    .await?;
    let win_total: f64 = win_row.get("total");
    let wins: f64 = win_row.get("wins");
    let win_rate = if win_total > 0.0 { wins / win_total } else { 0.0 };

    let joined_at = first_trade
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    Ok(HttpResponse::Ok().json(json!({
        "wallet": wallet,
        "username": serde_json::Value::Null,
        "bio": serde_json::Value::Null,
        "avatarUrl": serde_json::Value::Null,
        "joinedAt": joined_at,
        "stats": {
            "totalTrades": total_trades,
            "totalVolume": total_volume,
            "winRate": win_rate,
            "pnl30d": pnl_30d,
            "pnlAllTime": pnl_all_time,
            "marketsTraded": markets_traded,
            "bestTrade": 0,
            "worstTrade": 0,
            "currentStreak": 0,
            "longestStreak": 0,
        },
        "badges": [],
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/profiles/{wallet}/activity
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ActivityQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn get_profile_activity(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<ActivityQuery>,
) -> Result<impl Responder, ApiError> {
    let wallet = path.into_inner().to_lowercase();
    validate_wallet(&wallet)?;

    let (limit, offset) = validate_pagination(query.limit, query.offset)?;
    let pool = state.db.pool();

    let rows = sqlx::query(
        "SELECT t.id, t.market_id, t.outcome, t.price, t.quantity, t.created_at,
                m.question AS market_question, t.buyer, t.seller
         FROM trades t
         LEFT JOIN markets m ON m.id = t.market_id
         WHERE LOWER(t.buyer) = $1 OR LOWER(t.seller) = $1
         ORDER BY t.created_at DESC
         LIMIT $2 OFFSET $3"
    )
    .bind(&wallet)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let count_row = sqlx::query(
        "SELECT COUNT(*)::BIGINT AS total FROM trades
         WHERE LOWER(buyer) = $1 OR LOWER(seller) = $1"
    )
    .bind(&wallet)
    .fetch_one(pool)
    .await?;
    let total: i64 = count_row.get("total");

    let data: Vec<serde_json::Value> = rows.iter().map(|row| {
        let buyer: String = row.get("buyer");
        let is_buyer = buyer.to_lowercase() == wallet;
        let price: f64 = row.try_get::<String, _>("price")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(0.5);
        let quantity: f64 = row.try_get::<String, _>("quantity")
            .ok()
            .and_then(|q| q.parse().ok())
            .unwrap_or(0.0);
        let amount = price * quantity;

        json!({
            "id": row.get::<String, _>("id"),
            "type": if is_buyer { "trade" } else { "trade" },
            "marketId": row.get::<String, _>("market_id"),
            "marketQuestion": row.try_get::<String, _>("market_question").unwrap_or_default(),
            "outcome": row.try_get::<i16, _>("outcome").unwrap_or(0),
            "amount": amount,
            "pnl": if is_buyer { -amount } else { amount },
            "createdAt": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
        })
    }).collect();

    let has_more = (offset + limit) < total;

    Ok(HttpResponse::Ok().json(json!({
        "data": data,
        "total": total,
        "limit": limit,
        "offset": offset,
        "hasMore": has_more,
    })))
}

// ---------------------------------------------------------------------------
// GET /v1/profiles/{wallet}/positions
// ---------------------------------------------------------------------------

pub async fn get_profile_positions(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let wallet = path.into_inner().to_lowercase();
    validate_wallet(&wallet)?;

    let positions = state.db.get_positions(&wallet).await.map_err(ApiError::from)?;

    let data: Vec<serde_json::Value> = positions.iter().map(|p| {
        json!({
            "marketId": p.market_id,
            "marketQuestion": p.market_question,
            "owner": p.owner,
            "yesBalance": p.yes_balance,
            "noBalance": p.no_balance,
            "claimable": 0,
            "avgYesCost": p.avg_yes_cost,
            "avgNoCost": p.avg_no_cost,
            "currentYesPrice": p.current_yes_price,
            "currentNoPrice": p.current_no_price,
            "unrealizedPnl": p.unrealized_pnl,
            "realizedPnl": p.realized_pnl,
            "totalDeposited": p.total_deposited,
            "totalWithdrawn": p.total_withdrawn,
            "openOrderCount": p.open_order_count,
            "totalTrades": p.total_trades,
            "createdAt": p.created_at.to_rfc3339(),
        })
    }).collect();

    Ok(HttpResponse::Ok().json(json!({
        "data": data,
        "total": data.len(),
        "limit": data.len(),
        "offset": 0,
        "hasMore": false,
    })))
}
