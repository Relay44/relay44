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

