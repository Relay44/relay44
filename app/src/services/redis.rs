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

    // =========================================================================
    // Token Revocation List
    // =========================================================================

    /// Revoke a JWT token by its JTI (token ID)
    /// TTL is set to match the token's remaining lifetime
    pub async fn revoke_token(&self, jti: &str, expires_at: i64) -> Result<()> {
        let key = format!("revoked_token:{}", jti);
        let now = chrono::Utc::now().timestamp();
        let ttl = (expires_at - now).max(1) as u64;

        // Store the revocation with TTL matching token expiration
        // After token expires, we don't need to track it anymore
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: () = conn.set_ex(&key, "1", ttl).await?;

        info!("Token {} revoked, TTL: {}s", jti, ttl);
        Ok(())
    }

    /// Check if a token has been revoked
    pub async fn is_token_revoked(&self, jti: &str) -> Result<bool> {
        let key = format!("revoked_token:{}", jti);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let exists: bool = conn.exists(&key).await?;
        Ok(exists)
    }

    /// Revoke all tokens for a specific user (logout from all devices)
    /// This uses a user-specific generation counter
    pub async fn revoke_all_user_tokens(&self, wallet_address: &str) -> Result<()> {
        let key = format!("user_token_gen:{}", wallet_address);
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        // Increment the generation counter
        let _: i64 = conn.incr(&key, 1i64).await?;

        info!("All tokens revoked for user {}", wallet_address);
        Ok(())
    }

    /// Get the current token generation for a user
    pub async fn get_user_token_generation(&self, wallet_address: &str) -> Result<i64> {
        let key = format!("user_token_gen:{}", wallet_address);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let gen: Option<i64> = conn.get(&key).await?;
        Ok(gen.unwrap_or(0))
    }

    /// Store user token generation in the token claims for validation
    pub async fn set_user_token_generation(
        &self,
        wallet_address: &str,
        generation: i64,
    ) -> Result<()> {
        let key = format!("user_token_gen:{}", wallet_address);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        // 30 days TTL for generation counter
        let _: () = conn.set_ex(&key, generation, 30 * 24 * 3600).await?;
        Ok(())
    }

    // =========================================================================
    // Rate Limiting Support
    // =========================================================================

    /// Increment a counter with TTL, returns new count
    /// Used for simple rate limiting (e.g., WebSocket connections by IP)
    pub async fn increment_with_ttl(&self, key: &str, ttl_secs: u64) -> Result<i64> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        // Increment counter
        let count: i64 = conn.incr(key, 1i64).await?;

        // Set expiry if this is the first request in the window
        if count == 1 {
            let _: () = conn.expire(key, ttl_secs as i64).await?;
        }

        Ok(count)
    }

    /// Increment rate limit counter for an IP/user
    /// Returns the current count and remaining TTL
    pub async fn increment_rate_limit(&self, key: &str, window_secs: u64) -> Result<(i64, i64)> {
        let rate_key = format!("rate_limit:{}", key);
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        // Increment counter
        let count: i64 = conn.incr(&rate_key, 1i64).await?;

        // Set expiry if this is the first request in the window
        if count == 1 {
            let _: () = conn.expire(&rate_key, window_secs as i64).await?;
        }

        // Get TTL
        let ttl: i64 = conn.ttl(&rate_key).await?;

        Ok((count, ttl))
    }

    /// Check rate limit without incrementing
    pub async fn get_rate_limit_count(&self, key: &str) -> Result<i64> {
        let rate_key = format!("rate_limit:{}", key);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let count: Option<i64> = conn.get(&rate_key).await?;
        Ok(count.unwrap_or(0))
    }

    // =========================================================================
    // Nonce Storage for Replay Protection
    // =========================================================================

    /// Check if a nonce has been used and record it
    /// Returns Ok(false) if nonce was already used, Ok(true) if newly recorded
    pub async fn check_and_record_nonce(&self, nonce: &str, ttl_secs: u64) -> Result<bool> {
        let key = format!("auth_nonce:{}", nonce);
        let mut conn = self.client.get_multiplexed_async_connection().await?;

        // Use SETNX (SET if Not eXists) for atomic check-and-set
        let was_set: bool = conn.set_nx(&key, "1").await?;

        if was_set {
            // Set expiration for automatic cleanup
            let _: () = conn.expire(&key, ttl_secs as i64).await?;
        }

        Ok(was_set)
    }

    /// Check if a nonce has been used without recording
    pub async fn is_nonce_used(&self, nonce: &str) -> Result<bool> {
        let key = format!("auth_nonce:{}", nonce);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let exists: bool = conn.exists(&key).await?;
        Ok(exists)
    }

    // =========================================================================
    // Idempotency Keys
    // =========================================================================

    /// Check idempotency key and return cached response if exists.
    /// Returns None if key is new, Some(response) if duplicate request.
    pub async fn check_idempotency_key(&self, key: &str) -> Result<Option<String>> {
        let idem_key = format!("idempotency:{}", key);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let value: Option<String> = conn.get(&idem_key).await?;
        Ok(value)
    }

    /// Store idempotency key with response. TTL: 24 hours.
    pub async fn store_idempotency_key(&self, key: &str, response: &str) -> Result<()> {
        let idem_key = format!("idempotency:{}", key);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        // 24 hour TTL
        let _: () = conn.set_ex(&idem_key, response, 86400).await?;
        Ok(())
    }

    /// Acquire lock for idempotency key (to handle concurrent requests).
    /// Returns true if lock acquired, false if already processing.
    pub async fn acquire_idempotency_lock(&self, key: &str) -> Result<bool> {
        let lock_key = format!("idempotency_lock:{}", key);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        // Lock with 30 second TTL to handle crashes
        let acquired: bool = conn.set_nx(&lock_key, "1").await?;
        if acquired {
            let _: () = conn.expire(&lock_key, 30).await?;
        }
        Ok(acquired)
    }

    /// Release idempotency lock
    pub async fn release_idempotency_lock(&self, key: &str) -> Result<()> {
        let lock_key = format!("idempotency_lock:{}", key);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: () = conn.del(&lock_key).await?;
        Ok(())
    }
}
