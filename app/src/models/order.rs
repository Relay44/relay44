use super::Outcome;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub order_id: u64,
    pub market_id: String,
    pub owner: String,
    pub side: OrderSide,
    pub outcome: Outcome,
    pub order_type: OrderType,
    pub price: f64,
    pub price_bps: u16,
    pub quantity: u64,
    pub filled_quantity: u64,
    pub remaining_quantity: u64,
    pub status: OrderStatus,
    pub is_private: bool,
    pub tx_signature: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl From<u8> for OrderSide {
    fn from(value: u8) -> Self {
        match value {
            0 => OrderSide::Buy,
            1 => OrderSide::Sell,
            _ => OrderSide::Buy,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Limit,
    Market,
}

impl From<u8> for OrderType {
    fn from(value: u8) -> Self {
        match value {
            0 => OrderType::Limit,
            1 => OrderType::Market,
            _ => OrderType::Limit,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
}

impl From<u8> for OrderStatus {
    fn from(value: u8) -> Self {
        match value {
            0 => OrderStatus::Open,
            1 => OrderStatus::PartiallyFilled,
            2 => OrderStatus::Filled,
            3 => OrderStatus::Cancelled,
            4 => OrderStatus::Expired,
            _ => OrderStatus::Open,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PlaceOrderRequest {
    pub market_id: String,
    pub side: OrderSide,
    pub outcome: Outcome,
    pub order_type: OrderType,
    pub price: f64,
    pub quantity: u64,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub private: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListOrdersQuery {
    pub market_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct OrderListResponse {
    pub orders: Vec<Order>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct PlaceOrderResponse {
    pub order_id: String,
    pub market_id: String,
    pub side: OrderSide,
    pub outcome: Outcome,
    pub order_type: OrderType,
    pub price: f64,
    pub quantity: u64,
    pub filled_quantity: u64,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub tx_signature: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CancelOrderResponse {
    pub order_id: String,
    pub status: OrderStatus,
    pub cancelled_at: DateTime<Utc>,
    pub tx_signature: Option<String>,
}
