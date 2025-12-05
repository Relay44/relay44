use anyhow::Result;
use log::info;
use redis::{AsyncCommands, Client};
use serde::{de::DeserializeOwned, Serialize};

pub struct RedisService {
    client: Client,
}

impl RedisService {
    pub async fn new(redis_url: &str) -> Result<Self> {
        info!("Connecting to Redis...");
        let client = Client::open(redis_url)?;

        // Test connection
        let mut conn = client.get_multiplexed_async_connection().await?;
        let _: () = redis::cmd("PING").query_async(&mut conn).await?;

        info!("Redis connected successfully");
        Ok(Self { client })
    }

    /// Get a value from cache
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let value: Option<String> = conn.get(key).await?;

        match value {
            Some(v) => Ok(Some(serde_json::from_str(&v)?)),
            None => Ok(None),
        }
    }

    /// Set a value in cache with optional TTL
    pub async fn set<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl_seconds: Option<u64>,
    ) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let serialized = serde_json::to_string(value)?;

        match ttl_seconds {
            Some(ttl) => {
                let _: () = conn.set_ex(key, serialized, ttl).await?;
            }
            None => {
                let _: () = conn.set(key, serialized).await?;
            }
        }

        Ok(())
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: () = conn.del(key).await?;
        Ok(())
    }

    /// Publish a message to a channel
    pub async fn publish(&self, channel: &str, message: &str) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: () = conn.publish(channel, message).await?;
        Ok(())
    }

    /// Push an item into a list and trim to max length.
    /// Uses LPUSH so newest entries are returned first by LRANGE.
    pub async fn list_push_with_trim(&self, key: &str, value: &str, max_len: u64) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: i64 = conn.lpush(key, value).await?;
        let trim_end = max_len.saturating_sub(1) as isize;
        let _: () = conn.ltrim(key, 0, trim_end).await?;
        Ok(())
    }

    /// Read a list range as raw strings.
    pub async fn list_range_raw(&self, key: &str, start: u64, end: u64) -> Result<Vec<String>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let start_idx = start as isize;
        let end_idx = end as isize;
        let values: Vec<String> = conn.lrange(key, start_idx, end_idx).await?;
        Ok(values)
    }

    /// Cache market data
    pub async fn cache_market_prices(
        &self,
        market_id: &str,
        yes_price: f64,
        no_price: f64,
    ) -> Result<()> {
        let key = format!("market:{}:prices", market_id);
        let value = serde_json::json!({
            "yes_price": yes_price,
            "no_price": no_price,
            "updated_at": chrono::Utc::now().to_rfc3339()
        });
        self.set(&key, &value, Some(60)).await
    }

    /// Get cached market prices
    pub async fn get_market_prices(&self, market_id: &str) -> Result<Option<(f64, f64)>> {
        let key = format!("market:{}:prices", market_id);
        let value: Option<serde_json::Value> = self.get(&key).await?;

        match value {
            Some(v) => {
                let yes = v["yes_price"].as_f64().unwrap_or(0.5);
                let no = v["no_price"].as_f64().unwrap_or(0.5);
                Ok(Some((yes, no)))
            }
            None => Ok(None),
        }
    }

    /// Publish price update to subscribers
    pub async fn publish_price_update(
        &self,
        market_id: &str,
        yes_price: f64,
        no_price: f64,
    ) -> Result<()> {
        let message = serde_json::json!({
            "type": "price",
            "market_id": market_id,
            "yes_price": yes_price,
            "no_price": no_price,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        self.publish(&format!("market:{}", market_id), &message.to_string())
            .await
    }

    /// Publish order book update
    pub async fn publish_orderbook_update(
        &self,
        market_id: &str,
        outcome: &str,
        side: &str,
        price: f64,
        quantity: u64,
    ) -> Result<()> {
        let message = serde_json::json!({
            "type": "orderbook",
            "market_id": market_id,
            "outcome": outcome,
            "side": side,
            "price": price,
            "quantity": quantity,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        self.publish(
            &format!("orderbook:{}:{}", market_id, outcome),
            &message.to_string(),
        )
        .await
    }

    /// Publish trade execution
    pub async fn publish_trade(
        &self,
        market_id: &str,
        outcome: &str,
        price: f64,
        quantity: u64,
    ) -> Result<()> {
        let message = serde_json::json!({
            "type": "trade",
            "market_id": market_id,
            "outcome": outcome,
            "price": price,
            "quantity": quantity,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        self.publish(&format!("trades:{}", market_id), &message.to_string())
            .await
    }

