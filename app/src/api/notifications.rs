use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder, Row};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::auth::extract_authenticated_user;
use crate::api::ApiError;
use crate::AppState;

const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    OrderFilled,
    OrderCancelled,
    MarketResolved,
    PositionLiquidated,
    DepositConfirmed,
    WithdrawalCompleted,
    PriceAlert,
    System,
    DecisionRecommendationChanged,
    DecisionThresholdCrossed,
    DecisionConfidenceDropped,
}

impl NotificationType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::OrderFilled => "order_filled",
            Self::OrderCancelled => "order_cancelled",
            Self::MarketResolved => "market_resolved",
            Self::PositionLiquidated => "position_liquidated",
            Self::DepositConfirmed => "deposit_confirmed",
            Self::WithdrawalCompleted => "withdrawal_completed",
            Self::PriceAlert => "price_alert",
            Self::System => "system",
            Self::DecisionRecommendationChanged => "decision_recommendation_changed",
            Self::DecisionThresholdCrossed => "decision_threshold_crossed",
            Self::DecisionConfidenceDropped => "decision_confidence_dropped",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NewNotification {
    pub owner: String,
    pub kind: NotificationType,
    pub title: String,
    pub message: String,
    pub market_id: Option<String>,
    pub order_id: Option<String>,
    pub decision_cell_id: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRecord {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub message: String,
    pub read: bool,
    pub market_id: Option<String>,
    pub order_id: Option<String>,
    pub decision_cell_id: Option<String>,
    pub metadata: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPreferences {
    pub order_fills: bool,
    pub market_resolutions: bool,
    pub price_alerts: bool,
    pub system_announcements: bool,
    pub decision_alerts: bool,
    pub email_notifications: bool,
    pub push_notifications: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNotificationsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub unread_only: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNotificationPreferencesRequest {
    pub order_fills: Option<bool>,
    pub market_resolutions: Option<bool>,
    pub price_alerts: Option<bool>,
    pub system_announcements: Option<bool>,
    pub decision_alerts: Option<bool>,
    pub email_notifications: Option<bool>,
    pub push_notifications: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsListResponse {
    pub data: Vec<NotificationRecord>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
    pub has_more: bool,
}

#[cfg(test)]
fn default_preferences() -> NotificationPreferences {
    NotificationPreferences {
        order_fills: true,
        market_resolutions: true,
        price_alerts: true,
        system_announcements: true,
        decision_alerts: true,
        email_notifications: false,
        push_notifications: false,
    }
}

fn notification_allowed_by_preferences(kind: &NotificationType, prefs: &NotificationPreferences) -> bool {
    match kind {
        NotificationType::OrderFilled | NotificationType::OrderCancelled => prefs.order_fills,
        NotificationType::MarketResolved => prefs.market_resolutions,
        NotificationType::PriceAlert => prefs.price_alerts,
        NotificationType::DecisionRecommendationChanged
        | NotificationType::DecisionThresholdCrossed
        | NotificationType::DecisionConfidenceDropped => prefs.decision_alerts,
        NotificationType::PositionLiquidated
        | NotificationType::DepositConfirmed
        | NotificationType::WithdrawalCompleted
        | NotificationType::System => prefs.system_announcements,
    }
}

fn parse_preferences_row(row: sqlx::postgres::PgRow) -> Result<NotificationPreferences, ApiError> {
    Ok(NotificationPreferences {
        order_fills: row.try_get("order_fills").map_err(|err| ApiError::internal(&err.to_string()))?,
        market_resolutions: row
            .try_get("market_resolutions")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        price_alerts: row.try_get("price_alerts").map_err(|err| ApiError::internal(&err.to_string()))?,
        system_announcements: row
            .try_get("system_announcements")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        decision_alerts: row
            .try_get("decision_alerts")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        email_notifications: row
            .try_get("email_notifications")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        push_notifications: row
            .try_get("push_notifications")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
    })
}

pub(crate) async fn load_notification_preferences(
    state: &AppState,
    owner: &str,
) -> Result<NotificationPreferences, ApiError> {
    let row = sqlx::query(
        "INSERT INTO notification_preferences (
            owner, order_fills, market_resolutions, price_alerts,
            system_announcements, decision_alerts, email_notifications, push_notifications
         ) VALUES ($1, TRUE, TRUE, TRUE, TRUE, TRUE, FALSE, FALSE)
         ON CONFLICT (owner) DO UPDATE SET owner = notification_preferences.owner
         RETURNING order_fills, market_resolutions, price_alerts, system_announcements,
                   decision_alerts, email_notifications, push_notifications",
    )
    .bind(owner)
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    parse_preferences_row(row)
}

pub(crate) async fn create_notification(
    state: &AppState,
    notification: NewNotification,
) -> Result<Option<String>, ApiError> {
    let owner = notification.owner.trim().to_ascii_lowercase();
    if owner.is_empty() {
        return Err(ApiError::bad_request("INVALID_OWNER", "owner is required"));
    }

    let preferences = load_notification_preferences(state, owner.as_str()).await?;
    if !notification_allowed_by_preferences(&notification.kind, &preferences) {
        return Ok(None);
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO notifications (
            id, owner, type, title, message, market_id, order_id, decision_cell_id, metadata
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id.as_str())
    .bind(owner.as_str())
    .bind(notification.kind.as_str())
    .bind(notification.title.trim())
    .bind(notification.message.trim())
    .bind(notification.market_id.as_deref())
    .bind(notification.order_id.as_deref())
    .bind(notification.decision_cell_id.as_deref())
    .bind(notification.metadata)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(Some(id))
}

fn parse_notification_row(row: sqlx::postgres::PgRow) -> Result<NotificationRecord, ApiError> {
    let created_at: DateTime<Utc> = row
        .try_get("created_at")
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(NotificationRecord {
        id: row.try_get("id").map_err(|err| ApiError::internal(&err.to_string()))?,
        kind: row.try_get("type").map_err(|err| ApiError::internal(&err.to_string()))?,
        title: row.try_get("title").map_err(|err| ApiError::internal(&err.to_string()))?,
        message: row.try_get("message").map_err(|err| ApiError::internal(&err.to_string()))?,
        read: row
            .try_get::<Option<DateTime<Utc>>, _>("read_at")
            .map_err(|err| ApiError::internal(&err.to_string()))?
            .is_some(),
        market_id: row.try_get("market_id").ok(),
        order_id: row.try_get("order_id").ok(),
        decision_cell_id: row.try_get("decision_cell_id").ok(),
        metadata: row
            .try_get("metadata")
            .map_err(|err| ApiError::internal(&err.to_string()))?,
        created_at: created_at.to_rfc3339(),
    })
}

pub async fn list_notifications(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<ListNotificationsQuery>,
) -> Result<impl Responder, ApiError> {
    let user = extract_authenticated_user(&req, &state).await?;
    let owner = user.wallet_address;
    let limit = query.limit.unwrap_or(50).clamp(1, MAX_PAGE_SIZE);
    let offset = query.offset.unwrap_or(0).max(0);

    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT id, type, title, message, market_id, order_id, decision_cell_id, metadata, read_at, created_at
         FROM notifications WHERE owner = ",
    );
    builder.push_bind(owner.as_str());
    if query.unread_only.unwrap_or(false) {
        builder.push(" AND read_at IS NULL");
    }
    builder.push(" ORDER BY created_at DESC LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    let rows = builder
        .build()
        .fetch_all(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let mut count_builder = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*)::BIGINT AS total FROM notifications WHERE owner = ",
    );
    count_builder.push_bind(owner.as_str());
    if query.unread_only.unwrap_or(false) {
        count_builder.push(" AND read_at IS NULL");
    }

    let total = count_builder
        .build()
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?
        .try_get::<i64, _>("total")
        .map_err(|err| ApiError::internal(&err.to_string()))?
        .max(0) as u64;

    let notifications = rows
        .into_iter()
        .map(parse_notification_row)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(HttpResponse::Ok().json(NotificationsListResponse {
        data: notifications,
        total,
        limit: limit as u64,
        offset: offset as u64,
        has_more: (offset as u64 + limit as u64) < total,
    }))
}

pub async fn get_unread_count(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = extract_authenticated_user(&req, &state).await?;
    let count = sqlx::query(
        "SELECT COUNT(*)::BIGINT AS total FROM notifications WHERE owner = $1 AND read_at IS NULL",
    )
    .bind(user.wallet_address.as_str())
    .fetch_one(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .try_get::<i64, _>("total")
    .map_err(|err| ApiError::internal(&err.to_string()))?
    .max(0);

    Ok(HttpResponse::Ok().json(json!({ "count": count })))
}

pub async fn mark_notification_read(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = extract_authenticated_user(&req, &state).await?;
    let notification_id = path.into_inner();
    let result = sqlx::query(
        "UPDATE notifications SET read_at = COALESCE(read_at, NOW()) WHERE id = $1 AND owner = $2",
    )
    .bind(notification_id.as_str())
    .bind(user.wallet_address.as_str())
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("Notification"));
    }

    Ok(HttpResponse::NoContent().finish())
}

pub async fn mark_all_notifications_read(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = extract_authenticated_user(&req, &state).await?;
    sqlx::query("UPDATE notifications SET read_at = COALESCE(read_at, NOW()) WHERE owner = $1")
        .bind(user.wallet_address.as_str())
        .execute(state.db.pool())
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::NoContent().finish())
}

pub async fn get_notification_preferences(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = extract_authenticated_user(&req, &state).await?;
    let preferences = load_notification_preferences(&state, user.wallet_address.as_str()).await?;
    Ok(HttpResponse::Ok().json(preferences))
}

pub async fn update_notification_preferences(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<UpdateNotificationPreferencesRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_authenticated_user(&req, &state).await?;
    let current = load_notification_preferences(&state, user.wallet_address.as_str()).await?;
    let next = NotificationPreferences {
        order_fills: body.order_fills.unwrap_or(current.order_fills),
        market_resolutions: body.market_resolutions.unwrap_or(current.market_resolutions),
        price_alerts: body.price_alerts.unwrap_or(current.price_alerts),
        system_announcements: body
            .system_announcements
            .unwrap_or(current.system_announcements),
        decision_alerts: body.decision_alerts.unwrap_or(current.decision_alerts),
        email_notifications: body.email_notifications.unwrap_or(current.email_notifications),
        push_notifications: body.push_notifications.unwrap_or(current.push_notifications),
    };

    sqlx::query(
        "UPDATE notification_preferences
         SET order_fills = $2,
             market_resolutions = $3,
             price_alerts = $4,
             system_announcements = $5,
             decision_alerts = $6,
             email_notifications = $7,
             push_notifications = $8
         WHERE owner = $1",
    )
    .bind(user.wallet_address.as_str())
    .bind(next.order_fills)
    .bind(next.market_resolutions)
    .bind(next.price_alerts)
    .bind(next.system_announcements)
    .bind(next.decision_alerts)
    .bind(next.email_notifications)
    .bind(next.push_notifications)
    .execute(state.db.pool())
    .await
    .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_notifications_obey_decision_preference() {
        let mut prefs = default_preferences();
        prefs.decision_alerts = false;
        assert!(!notification_allowed_by_preferences(
            &NotificationType::DecisionThresholdCrossed,
            &prefs,
        ));
        assert!(notification_allowed_by_preferences(&NotificationType::OrderFilled, &prefs));
    }
}
