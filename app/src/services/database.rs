use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{
    postgres::{PgPoolOptions, PgRow},
    PgPool, Postgres, Row,
};
use std::env;
use std::path::PathBuf;
use std::time::Duration;

use crate::models::{
    Market, MarketStatus, Order, OrderSide, OrderStatus, OrderType, Outcome, Position, Trade,
    Transaction as ModelTransaction, TransactionType,
};

#[derive(Debug, Clone)]
pub struct LocalTradeSettlement {
    pub trade: Trade,
    pub buyer_yes_delta: i64,
    pub buyer_no_delta: i64,
    pub seller_yes_delta: i64,
    pub seller_no_delta: i64,
}

/// Database connection pool configuration
pub struct PoolConfig {
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Minimum number of connections to maintain
    pub min_connections: u32,
    /// Maximum time to wait for a connection
    pub acquire_timeout: Duration,
    /// Maximum idle time before connection is closed
    pub idle_timeout: Duration,
    /// Maximum lifetime of a connection
    pub max_lifetime: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 20,
            min_connections: 1,
            acquire_timeout: Duration::from_secs(60),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(1800),
        }
    }
}

impl PoolConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            max_connections: env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(20),
            min_connections: env::var("DB_MIN_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
            acquire_timeout: Duration::from_secs(
                env::var("DB_ACQUIRE_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(60),
            ),
            idle_timeout: Duration::from_secs(
                env::var("DB_IDLE_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(600),
            ),
            max_lifetime: Duration::from_secs(
                env::var("DB_MAX_LIFETIME_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1800),
            ),
        }
    }
}

#[derive(Clone)]
pub struct DatabaseService {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutJobRecord {
    pub market_id: u64,
    pub wallet: String,
    pub status: String,
    pub last_tx: Option<String>,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub next_retry_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutBacklogSummary {
    pub pending: u64,
    pub processing: u64,
    pub retry: u64,
    pub failed: u64,
    pub oldest_pending_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainSyncCursor {
    pub key: String,
    pub last_block: u64,
    pub meta: Value,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseMarketBootstrapConfigRecord {
    pub market_id: u64,
    pub creator: String,
    pub liquidity_mode: String,
    pub status: String,
    pub manager: Option<String>,
    pub preset: String,
    pub seed_usdc: f64,
    pub initial_yes_bps: u64,
    pub strategy: String,
    pub levels: u64,
    pub base_spread_bps: u64,
    pub step_bps: u64,
    pub cadence_seconds: u64,
    pub expiry_seconds: u64,
    pub organic_depth_window_bps: u64,
    pub target_depth_multiplier: f64,
    pub target_volume_multiplier: f64,
    pub max_age_seconds: u64,
    pub inventory_skew_bps: i32,
    pub exposure_cap_bps: u64,
    pub pause_reason: Option<String>,
    pub reserved_usdc: f64,
    pub available_usdc: f64,
    pub active_slots: u64,
    pub organic_depth_ratio: f64,
    pub consecutive_failures: u64,
    pub depth_qualified_since: Option<DateTime<Utc>>,
    pub activated_at: DateTime<Utc>,
    pub graduated_at: Option<DateTime<Utc>>,
    pub graduation_reason: Option<String>,
    pub create_tx_hash: Option<String>,
    pub launch_tx_hash: Option<String>,
    pub last_reconciled_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BaseMarketBootstrapUpsert<'a> {
    pub market_id: u64,
    pub creator: &'a str,
    pub liquidity_mode: &'a str,
    pub status: &'a str,
    pub manager: Option<&'a str>,
    pub preset: &'a str,
    pub seed_usdc: f64,
    pub initial_yes_bps: u64,
    pub strategy: &'a str,
    pub levels: u64,
    pub base_spread_bps: u64,
    pub step_bps: u64,
    pub cadence_seconds: u64,
    pub expiry_seconds: u64,
    pub organic_depth_window_bps: u64,
    pub target_depth_multiplier: f64,
    pub target_volume_multiplier: f64,
    pub max_age_seconds: u64,
    pub inventory_skew_bps: i32,
    pub exposure_cap_bps: u64,
    pub pause_reason: Option<&'a str>,
    pub reserved_usdc: f64,
    pub available_usdc: f64,
    pub active_slots: u64,
    pub organic_depth_ratio: f64,
    pub consecutive_failures: u64,
    pub activated_at: Option<DateTime<Utc>>,
    pub graduated_at: Option<DateTime<Utc>>,
    pub graduation_reason: Option<&'a str>,
    pub create_tx_hash: Option<&'a str>,
    pub launch_tx_hash: Option<&'a str>,
    pub last_reconciled_at: Option<DateTime<Utc>>,
    pub last_error: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseMarketBootstrapAgentRecord {
    pub market_id: u64,
    pub side: String,
    pub level_index: u64,
    pub agent_id: Option<u64>,
    pub desired_price_bps: u64,
    pub desired_size: u64,
    pub current_price_bps: Option<u64>,
    pub current_size: Option<u64>,
    pub active: bool,
    pub created_tx_hash: Option<String>,
    pub updated_tx_hash: Option<String>,
    pub deactivated_tx_hash: Option<String>,
    pub last_execute_tx_hash: Option<String>,
    pub last_executed_at: Option<DateTime<Utc>>,
    pub last_reconciled_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BaseMarketBootstrapAgentUpsert<'a> {
    pub market_id: u64,
    pub side: &'a str,
    pub level_index: u64,
    pub agent_id: Option<u64>,
    pub desired_price_bps: u64,
    pub desired_size: u64,
    pub current_price_bps: Option<u64>,
    pub current_size: Option<u64>,
    pub active: bool,
    pub created_tx_hash: Option<&'a str>,
    pub updated_tx_hash: Option<&'a str>,
    pub deactivated_tx_hash: Option<&'a str>,
    pub last_execute_tx_hash: Option<&'a str>,
    pub last_executed_at: Option<DateTime<Utc>>,
    pub last_reconciled_at: Option<DateTime<Utc>>,
    pub last_error: Option<&'a str>,
}

#[derive(Debug, Clone, Default)]
pub struct BaseMarketBootstrapRuntimeUpdate<'a> {
    pub inventory_skew_bps: Option<i32>,
    pub status: Option<&'a str>,
    pub manager: Option<&'a str>,
    pub preset: Option<&'a str>,
    pub strategy: Option<&'a str>,
    pub levels: Option<u64>,
    pub base_spread_bps: Option<u64>,
    pub step_bps: Option<u64>,
    pub cadence_seconds: Option<u64>,
    pub expiry_seconds: Option<u64>,
    pub exposure_cap_bps: Option<u64>,
    pub pause_reason: Option<&'a str>,
    pub clear_pause_reason: bool,
    pub launch_tx_hash: Option<&'a str>,
    pub last_reconciled_at: Option<DateTime<Utc>>,
    pub last_error: Option<&'a str>,
    pub clear_last_error: bool,
    pub reserved_usdc: Option<f64>,
    pub available_usdc: Option<f64>,
    pub active_slots: Option<u64>,
    pub organic_depth_ratio: Option<f64>,
    pub consecutive_failures: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapFillEventRecord {
    pub id: String,
    pub market_id: u64,
    pub creator: String,
    pub trade_id: String,
    pub source: String,
    pub agent_id: Option<u64>,
    pub maker_order_id: String,
    pub outcome: String,
    pub side: String,
    pub price: f64,
    pub quantity: f64,
    pub notional_usdc: f64,
    pub occurred_at: DateTime<Utc>,
    pub raw: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BootstrapFillEventUpsert<'a> {
    pub id: &'a str,
    pub market_id: u64,
    pub creator: &'a str,
    pub trade_id: &'a str,
    pub source: &'a str,
    pub agent_id: Option<u64>,
    pub maker_order_id: &'a str,
    pub outcome: &'a str,
    pub side: &'a str,
    pub price: f64,
    pub quantity: f64,
    pub notional_usdc: f64,
    pub occurred_at: DateTime<Utc>,
    pub raw: &'a Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorMarketEconomicsDailyRecord {
    pub market_id: u64,
    pub creator: String,
    pub day: NaiveDate,
    pub seed_usdc: f64,
    pub available_usdc: f64,
    pub reserved_usdc: f64,
    pub inventory_yes: f64,
    pub inventory_no: f64,
    pub inventory_mark_value_usdc: f64,
    pub cumulative_bootstrap_fills_usdc: f64,
    pub net_liquidity_pnl_usdc: f64,
    pub subsidy_burn_usdc: f64,
    pub roi_bps: f64,
    pub realized_resolution_pnl_usdc: f64,
    pub organic_depth_ratio: f64,
    pub graduated: bool,
    pub graduation_retention_24h: Option<f64>,
    pub graduation_retention_7d: Option<f64>,
    pub mirror_freshness_seconds: Option<u64>,
    pub mirror_pending_hedges: u64,
    pub mirror_error_count: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreatorMarketEconomicsDailyUpsert<'a> {
    pub market_id: u64,
    pub creator: &'a str,
    pub day: NaiveDate,
    pub seed_usdc: f64,
    pub available_usdc: f64,
    pub reserved_usdc: f64,
    pub inventory_yes: f64,
    pub inventory_no: f64,
    pub inventory_mark_value_usdc: f64,
    pub cumulative_bootstrap_fills_usdc: f64,
    pub net_liquidity_pnl_usdc: f64,
    pub subsidy_burn_usdc: f64,
    pub roi_bps: f64,
    pub realized_resolution_pnl_usdc: f64,
    pub organic_depth_ratio: f64,
    pub graduated: bool,
    pub graduation_retention_24h: Option<f64>,
    pub graduation_retention_7d: Option<f64>,
    pub mirror_freshness_seconds: Option<u64>,
    pub mirror_pending_hedges: u64,
    pub mirror_error_count: u64,
}

#[derive(Debug, Clone)]
pub struct ComplianceDecisionEntry<'a> {
    pub request_id: Option<&'a str>,
    pub wallet: Option<&'a str>,
    pub country_code: Option<&'a str>,
    pub action: &'a str,
    pub route: &'a str,
    pub method: &'a str,
    pub decision: &'a str,
    pub reason_code: &'a str,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleMarketConfigRecord {
    pub market_id: u64,
    pub feed_type: String,
    pub feed_address: Option<String>,
    pub comparison: String,
    pub target_value: String,
    pub target_currency: String,
    pub category: Option<String>,
    pub resolution_hint: Option<String>,
    pub configure_tx: Option<String>,
    pub keeper_enabled: bool,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub resolve_tx: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowerCounts {
    pub followers: u64,
    pub following: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketCommentRecord {
    pub id: String,
    pub market_id: String,
    pub wallet: String,
    pub text: String,
    pub parent_id: Option<String>,
    pub farcaster_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycVerificationRecord {
    pub id: i32,
    pub wallet: String,
    pub provider: String,
    pub nullifier_hash: String,
    pub tier_granted: u8,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

impl DatabaseService {
    fn map_oracle_market_config_row(row: &PgRow) -> OracleMarketConfigRecord {
        OracleMarketConfigRecord {
            market_id: row.get::<i64, _>("market_id") as u64,
            feed_type: row.get("feed_type"),
            feed_address: row.try_get("feed_address").ok(),
            comparison: row.get("comparison"),
            target_value: format!("{}", row.get::<f64, _>("target_value")),
            target_currency: row.get("target_currency"),
            category: row.try_get("category").ok(),
            resolution_hint: row.try_get("resolution_hint").ok(),
            configure_tx: row.try_get("configure_tx").ok(),
            keeper_enabled: row.get("keeper_enabled"),
            last_checked_at: row.try_get("last_checked_at").ok().flatten(),
            last_error: row.try_get("last_error").ok(),
            resolve_tx: row.try_get("resolve_tx").ok(),
            resolved_at: row.try_get("resolved_at").ok().flatten(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    fn map_base_market_bootstrap_row(row: &PgRow) -> BaseMarketBootstrapConfigRecord {
        BaseMarketBootstrapConfigRecord {
            market_id: row.get::<i64, _>("market_id") as u64,
            creator: row.get("creator"),
            liquidity_mode: row.get("liquidity_mode"),
            status: row.get("status"),
            manager: row.try_get("manager").ok(),
            preset: row.get("preset"),
            seed_usdc: row.get("seed_usdc"),
            initial_yes_bps: row.get::<i32, _>("initial_yes_bps") as u64,
            strategy: row.get("strategy"),
            levels: row.get::<i32, _>("levels") as u64,
            base_spread_bps: row.get::<i32, _>("base_spread_bps") as u64,
            step_bps: row.get::<i32, _>("step_bps") as u64,
            cadence_seconds: row.get::<i32, _>("cadence_seconds") as u64,
            expiry_seconds: row.get::<i32, _>("expiry_seconds") as u64,
            organic_depth_window_bps: row.get::<i32, _>("organic_depth_window_bps") as u64,
            target_depth_multiplier: row.get("target_depth_multiplier"),
            target_volume_multiplier: row.get("target_volume_multiplier"),
            max_age_seconds: row.get::<i64, _>("max_age_seconds") as u64,
            inventory_skew_bps: row.get("inventory_skew_bps"),
            exposure_cap_bps: row.get::<i32, _>("exposure_cap_bps") as u64,
            pause_reason: row.try_get("pause_reason").ok(),
            reserved_usdc: row.get("reserved_usdc"),
            available_usdc: row.get("available_usdc"),
            active_slots: row.get::<i32, _>("active_slots") as u64,
            organic_depth_ratio: row.get("organic_depth_ratio"),
            consecutive_failures: row.get::<i32, _>("consecutive_failures") as u64,
            depth_qualified_since: row.try_get("depth_qualified_since").ok(),
            activated_at: row.get("activated_at"),
            graduated_at: row.try_get("graduated_at").ok(),
            graduation_reason: row.try_get("graduation_reason").ok(),
            create_tx_hash: row.try_get("create_tx_hash").ok(),
            launch_tx_hash: row.try_get("launch_tx_hash").ok(),
            last_reconciled_at: row.try_get("last_reconciled_at").ok(),
            last_error: row.try_get("last_error").ok(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    fn map_base_market_bootstrap_agent_row(row: &PgRow) -> BaseMarketBootstrapAgentRecord {
        BaseMarketBootstrapAgentRecord {
            market_id: row.get::<i64, _>("market_id") as u64,
            side: row.get("side"),
            level_index: row.get::<i32, _>("level_index") as u64,
            agent_id: row
                .try_get::<Option<i64>, _>("agent_id")
                .ok()
                .flatten()
                .map(|value| value as u64),
            desired_price_bps: row.get::<i32, _>("desired_price_bps") as u64,
            desired_size: row.get::<i64, _>("desired_size") as u64,
            current_price_bps: row
                .try_get::<Option<i32>, _>("current_price_bps")
                .ok()
                .flatten()
                .map(|value| value as u64),
            current_size: row
                .try_get::<Option<i64>, _>("current_size")
                .ok()
                .flatten()
                .map(|value| value as u64),
            active: row.get("active"),
            created_tx_hash: row.try_get("created_tx_hash").ok(),
            updated_tx_hash: row.try_get("updated_tx_hash").ok(),
            deactivated_tx_hash: row.try_get("deactivated_tx_hash").ok(),
            last_execute_tx_hash: row.try_get("last_execute_tx_hash").ok(),
            last_executed_at: row.try_get("last_executed_at").ok(),
            last_reconciled_at: row.try_get("last_reconciled_at").ok(),
            last_error: row.try_get("last_error").ok(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    fn map_bootstrap_fill_event_row(row: &PgRow) -> BootstrapFillEventRecord {
        BootstrapFillEventRecord {
            id: row.get("id"),
            market_id: row.get::<i64, _>("market_id") as u64,
            creator: row.get("creator"),
            trade_id: row.get("trade_id"),
            source: row.get("source"),
            agent_id: row
                .try_get::<Option<i64>, _>("agent_id")
                .ok()
                .flatten()
                .map(|value| value as u64),
            maker_order_id: row.get("maker_order_id"),
            outcome: row.get("outcome"),
            side: row.get("side"),
            price: row.get("price"),
            quantity: row.get("quantity"),
            notional_usdc: row.get("notional_usdc"),
            occurred_at: row.get("occurred_at"),
            raw: row.get("raw"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    fn map_creator_market_economics_daily_row(row: &PgRow) -> CreatorMarketEconomicsDailyRecord {
        CreatorMarketEconomicsDailyRecord {
            market_id: row.get::<i64, _>("market_id") as u64,
            creator: row.get("creator"),
            day: row.get("day"),
            seed_usdc: row.get("seed_usdc"),
            available_usdc: row.get("available_usdc"),
            reserved_usdc: row.get("reserved_usdc"),
            inventory_yes: row.get("inventory_yes"),
            inventory_no: row.get("inventory_no"),
            inventory_mark_value_usdc: row.get("inventory_mark_value_usdc"),
            cumulative_bootstrap_fills_usdc: row.get("cumulative_bootstrap_fills_usdc"),
            net_liquidity_pnl_usdc: row.get("net_liquidity_pnl_usdc"),
            subsidy_burn_usdc: row.get("subsidy_burn_usdc"),
            roi_bps: row.get("roi_bps"),
            realized_resolution_pnl_usdc: row.get("realized_resolution_pnl_usdc"),
            organic_depth_ratio: row.get("organic_depth_ratio"),
            graduated: row.get("graduated"),
            graduation_retention_24h: row
                .try_get::<Option<f64>, _>("graduation_retention_24h")
                .ok()
                .flatten(),
            graduation_retention_7d: row
                .try_get::<Option<f64>, _>("graduation_retention_7d")
                .ok()
                .flatten(),
            mirror_freshness_seconds: row
                .try_get::<Option<i64>, _>("mirror_freshness_seconds")
                .ok()
                .flatten()
                .map(|value| value as u64),
            mirror_pending_hedges: row.get::<i64, _>("mirror_pending_hedges").max(0) as u64,
            mirror_error_count: row.get::<i64, _>("mirror_error_count").max(0) as u64,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    fn migrations_path() -> PathBuf {
        if let Ok(path) = env::var("MIGRATIONS_DIR") {
            return PathBuf::from(path);
        }

        let runtime_path = PathBuf::from("migrations");
        if runtime_path.exists() {
            return runtime_path;
        }

        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../migrations")
    }

    pub async fn new(database_url: &str) -> Result<Self> {
        Self::with_config(database_url, PoolConfig::from_env()).await
    }

    /// Create a stub DatabaseService for integration tests.
    /// Any actual query will fail, but the struct can be constructed.
    pub fn test_stub() -> Self {
        use sqlx::postgres::PgPoolOptions;
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .min_connections(0)
            .idle_timeout(std::time::Duration::from_secs(1))
            .connect_lazy("postgres://test@localhost:5432/relay44_test")
            .expect("test_stub pool");
        Self { pool }
    }

    pub async fn with_config(database_url: &str, config: PoolConfig) -> Result<Self> {
        info!("Connecting to database with pool config:");
        info!("  max_connections: {}", config.max_connections);
        info!("  min_connections: {}", config.min_connections);
        info!("  acquire_timeout: {:?}", config.acquire_timeout);
        info!("  idle_timeout: {:?}", config.idle_timeout);
        info!("  max_lifetime: {:?}", config.max_lifetime);

        let url = if database_url.contains("sslmode=") {
            database_url.to_string()
        } else {
            let sep = if database_url.contains('?') { "&" } else { "?" };
            format!("{}{}sslmode=require", database_url, sep)
        };

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(0)
            .acquire_timeout(config.acquire_timeout)
            .idle_timeout(config.idle_timeout)
            .max_lifetime(config.max_lifetime)
            .connect_lazy(&url)?;

        info!("Database pool created (lazy — connections open on first query)");

        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        info!("Running database migrations...");
        let migrations_path = Self::migrations_path();
        sqlx::migrate::Migrator::new(migrations_path.as_path())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to load migrations: {}", e))?
            .run(&self.pool)
            .await
            .map_err(|e| {
                log::error!("Migration failed: {}", e);
                anyhow::anyhow!("Database migration failed: {}", e)
            })?;
        info!("Database migrations completed");
        Ok(())
    }

    /// Get pool statistics for monitoring
    pub fn pool_stats(&self) -> PoolStats {
        PoolStats {
            size: self.pool.size(),
            idle_count: self.pool.num_idle(),
        }
    }

    /// Begin a new database transaction
    pub async fn begin_transaction(&self) -> Result<sqlx::Transaction<'_, Postgres>> {
        Ok(self.pool.begin().await?)
    }

    /// Get reference to the pool for advanced operations
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // Markets
    pub async fn get_markets(
        &self,
        status: Option<MarketStatus>,
        category: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Market>, i64)> {
        let mut query = String::from("SELECT * FROM markets WHERE 1=1");
        let mut count_query = String::from("SELECT COUNT(*) as total FROM markets WHERE 1=1");

        if status.is_some() {
            query.push_str(" AND status = $1");
            count_query.push_str(" AND status = $1");
        }
        if category.is_some() {
            let idx = if status.is_some() { "2" } else { "1" };
            query.push_str(&format!(" AND category = ${}", idx));
            count_query.push_str(&format!(" AND category = ${}", idx));
        }

        query.push_str(" ORDER BY created_at DESC LIMIT $3 OFFSET $4");

        // Build and execute query based on parameters
        let rows = match (status, category) {
            (Some(s), Some(c)) => {
                sqlx::query(&query)
                    .bind(s as i16)
                    .bind(c)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await?
            }
            (Some(s), None) => {
                let q = "SELECT * FROM markets WHERE status = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3";
                sqlx::query(q)
                    .bind(s as i16)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await?
            }
            (None, Some(c)) => {
                let q = "SELECT * FROM markets WHERE category = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3";
                sqlx::query(q)
                    .bind(c)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await?
            }
            (None, None) => {
                let q = "SELECT * FROM markets ORDER BY created_at DESC LIMIT $1 OFFSET $2";
                sqlx::query(q)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await?
            }
        };

        // Get total count
        let total: i64 = match (status, category) {
            (Some(s), Some(c)) => {
                let q = "SELECT COUNT(*) as total FROM markets WHERE status = $1 AND category = $2";
                sqlx::query_scalar(q)
                    .bind(s as i16)
                    .bind(c)
                    .fetch_one(&self.pool)
                    .await?
            }
            (Some(s), None) => {
                let q = "SELECT COUNT(*) as total FROM markets WHERE status = $1";
                sqlx::query_scalar(q)
                    .bind(s as i16)
                    .fetch_one(&self.pool)
                    .await?
            }
            (None, Some(c)) => {
                let q = "SELECT COUNT(*) as total FROM markets WHERE category = $1";
                sqlx::query_scalar(q).bind(c).fetch_one(&self.pool).await?
            }
            (None, None) => {
                sqlx::query_scalar("SELECT COUNT(*) as total FROM markets")
                    .fetch_one(&self.pool)
                    .await?
            }
        };

        let markets = rows.iter().map(|row| self.row_to_market(row)).collect();
        Ok((markets, total))
    }

    fn row_to_market(&self, row: &sqlx::postgres::PgRow) -> Market {
        Market {
            id: row.get("id"),
            address: row.get("address"),
            question: row.get("question"),
            description: row.get("description"),
            category: row.get("category"),
            status: MarketStatus::from(row.get::<i16, _>("status") as u8),
            yes_price: row.get("yes_price"),
            no_price: row.get("no_price"),
            yes_supply: row.get::<i64, _>("yes_supply") as u64,
            no_supply: row.get::<i64, _>("no_supply") as u64,
            volume_24h: row.get("volume_24h"),
            total_volume: row.get("total_volume"),
            total_collateral: row.get::<i64, _>("total_collateral") as u64,
            fee_bps: row.get::<i16, _>("fee_bps") as u16,
            oracle: row.get("oracle"),
            collateral_mint: row.get("collateral_mint"),
            yes_mint: row.get("yes_mint"),
            no_mint: row.get("no_mint"),
            resolution_deadline: row.get("resolution_deadline"),
            trading_end: row.get("trading_end"),
            resolved_outcome: row
                .try_get::<i16, _>("resolved_outcome")
                .ok()
                .map(|v| Outcome::from(v as u8)),
            created_at: row.get("created_at"),
            resolved_at: row.try_get("resolved_at").ok(),
        }
    }

    pub async fn get_market(&self, market_id: &str) -> Result<Option<Market>> {
        let row = sqlx::query("SELECT * FROM markets WHERE id = $1")
            .bind(market_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| self.row_to_market(&r)))
    }

    pub async fn create_market(&self, market: &Market) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO markets (
                id, address, question, description, category, status,
                yes_price, no_price, volume_24h, total_volume, total_collateral,
                fee_bps, oracle, collateral_mint, yes_mint, no_mint,
                resolution_deadline, trading_end, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
            "#,
        )
        .bind(&market.id)
        .bind(&market.address)
        .bind(&market.question)
        .bind(&market.description)
        .bind(&market.category)
        .bind(market.status as i16)
        .bind(market.yes_price)
        .bind(market.no_price)
        .bind(market.volume_24h)
        .bind(market.total_volume)
        .bind(market.total_collateral as i64)
        .bind(market.fee_bps as i16)
        .bind(&market.oracle)
        .bind(&market.collateral_mint)
        .bind(&market.yes_mint)
        .bind(&market.no_mint)
        .bind(market.resolution_deadline)
        .bind(market.trading_end)
        .bind(market.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_market_prices(
        &self,
        market_id: &str,
        yes_price: f64,
        no_price: f64,
    ) -> Result<()> {
        sqlx::query("UPDATE markets SET yes_price = $1, no_price = $2 WHERE id = $3")
            .bind(yes_price)
            .bind(no_price)
            .bind(market_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Orders
    pub async fn get_orders(
        &self,
        owner: &str,
        market_id: Option<&str>,
        status: Option<OrderStatus>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Order>, i64)> {
        let base_where = "WHERE owner = $1";

        let rows = match (market_id, status) {
            (Some(m), Some(s)) => {
                let q = format!("SELECT * FROM orders {} AND market_id = $2 AND status = $3 ORDER BY created_at DESC LIMIT $4 OFFSET $5", base_where);
                sqlx::query(&q)
                    .bind(owner)
                    .bind(m)
                    .bind(s as i16)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await?
            }
            (Some(m), None) => {
                let q = format!("SELECT * FROM orders {} AND market_id = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4", base_where);
                sqlx::query(&q)
                    .bind(owner)
                    .bind(m)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await?
            }
            (None, Some(s)) => {
                let q = format!("SELECT * FROM orders {} AND status = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4", base_where);
                sqlx::query(&q)
                    .bind(owner)
                    .bind(s as i16)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await?
            }
            (None, None) => {
                let q = format!(
                    "SELECT * FROM orders {} ORDER BY created_at DESC LIMIT $2 OFFSET $3",
                    base_where
                );
                sqlx::query(&q)
                    .bind(owner)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await?
            }
        };

        let total: i64 = match (market_id, status) {
            (Some(m), Some(s)) => sqlx::query_scalar(
                "SELECT COUNT(*) FROM orders WHERE owner = $1 AND market_id = $2 AND status = $3",
            )
            .bind(owner)
            .bind(m)
            .bind(s as i16)
            .fetch_one(&self.pool)
            .await?,
            (Some(m), None) => {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM orders WHERE owner = $1 AND market_id = $2",
                )
                .bind(owner)
                .bind(m)
                .fetch_one(&self.pool)
                .await?
            }
            (None, Some(s)) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE owner = $1 AND status = $2")
                    .bind(owner)
                    .bind(s as i16)
                    .fetch_one(&self.pool)
                    .await?
            }
            (None, None) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE owner = $1")
                    .bind(owner)
                    .fetch_one(&self.pool)
                    .await?
            }
        };

        let orders = rows.iter().map(|row| self.row_to_order(row)).collect();
        Ok((orders, total))
    }

    fn row_to_order(&self, row: &sqlx::postgres::PgRow) -> Order {
        Order {
            id: row.get("id"),
            order_id: row.get::<i64, _>("order_id") as u64,
            market_id: row.get("market_id"),
            owner: row.get("owner"),
            side: OrderSide::from(row.get::<i16, _>("side") as u8),
            outcome: Outcome::from(row.get::<i16, _>("outcome") as u8),
            order_type: OrderType::from(row.get::<i16, _>("order_type") as u8),
            price: row.get("price"),
            price_bps: row.get::<i16, _>("price_bps") as u16,
            quantity: row.get::<i64, _>("quantity") as u64,
            filled_quantity: row.get::<i64, _>("filled_quantity") as u64,
            remaining_quantity: row.get::<i64, _>("remaining_quantity") as u64,
            status: OrderStatus::from(row.get::<i16, _>("status") as u8),
            is_private: row.get("is_private"),
            tx_signature: row.try_get("tx_signature").ok(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            expires_at: row.try_get("expires_at").ok(),
        }
    }

    pub async fn get_order(&self, order_id: &str) -> Result<Option<Order>> {
        let row = sqlx::query("SELECT * FROM orders WHERE id = $1")
            .bind(order_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| self.row_to_order(&r)))
    }

    pub async fn create_order(&self, order: &Order) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO orders (
                id, order_id, market_id, owner, side, outcome, order_type,
                price, price_bps, quantity, filled_quantity, remaining_quantity,
                status, is_private, tx_signature, created_at, updated_at, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#,
        )
        .bind(&order.id)
        .bind(order.order_id as i64)
        .bind(&order.market_id)
        .bind(&order.owner)
        .bind(order.side as i16)
        .bind(order.outcome as i16)
        .bind(order.order_type as i16)
        .bind(order.price)
        .bind(order.price_bps as i16)
        .bind(order.quantity as i64)
        .bind(order.filled_quantity as i64)
        .bind(order.remaining_quantity as i64)
        .bind(order.status as i16)
        .bind(order.is_private)
        .bind(&order.tx_signature)
        .bind(order.created_at)
        .bind(order.updated_at)
        .bind(order.expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_order_status(
        &self,
        order_id: &str,
        status: OrderStatus,
        filled_quantity: u64,
        remaining_quantity: u64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE orders SET status = $1, filled_quantity = $2, remaining_quantity = $3, updated_at = $4 WHERE id = $5"
        )
        .bind(status as i16)
        .bind(filled_quantity as i64)
        .bind(remaining_quantity as i64)
        .bind(Utc::now())
        .bind(order_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn persist_local_order_flow(
        &self,
        taker: &Order,
        maker_updates: &[Order],
        settlements: &[LocalTradeSettlement],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO orders (
                id, order_id, market_id, owner, side, outcome, order_type,
                price, price_bps, quantity, filled_quantity, remaining_quantity,
                status, is_private, tx_signature, created_at, updated_at, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#,
        )
        .bind(&taker.id)
        .bind(taker.order_id as i64)
        .bind(&taker.market_id)
        .bind(&taker.owner)
        .bind(taker.side as i16)
        .bind(taker.outcome as i16)
        .bind(taker.order_type as i16)
        .bind(taker.price)
        .bind(taker.price_bps as i16)
        .bind(taker.quantity as i64)
        .bind(taker.filled_quantity as i64)
        .bind(taker.remaining_quantity as i64)
        .bind(taker.status as i16)
        .bind(taker.is_private)
        .bind(&taker.tx_signature)
        .bind(taker.created_at)
        .bind(taker.updated_at)
        .bind(taker.expires_at)
        .execute(&mut *tx)
        .await?;

        upsert_orderbook_entry_tx(&mut tx, taker).await?;

        for maker in maker_updates {
            sqlx::query(
                "UPDATE orders SET status = $1, filled_quantity = $2, remaining_quantity = $3, updated_at = $4 WHERE id = $5",
            )
            .bind(maker.status as i16)
            .bind(maker.filled_quantity as i64)
            .bind(maker.remaining_quantity as i64)
            .bind(maker.updated_at)
            .bind(&maker.id)
            .execute(&mut *tx)
            .await?;

            upsert_orderbook_entry_tx(&mut tx, maker).await?;
        }

        for settlement in settlements {
            sqlx::query(
                r#"
                INSERT INTO trades (
                    id, market_id, buy_order_id, sell_order_id, outcome,
                    price, price_bps, quantity, collateral_amount,
                    buyer, seller, tx_signature, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                "#,
            )
            .bind(&settlement.trade.id)
            .bind(&settlement.trade.market_id)
            .bind(&settlement.trade.buy_order_id)
            .bind(&settlement.trade.sell_order_id)
            .bind(settlement.trade.outcome as i16)
            .bind(settlement.trade.price)
            .bind(settlement.trade.price_bps as i16)
            .bind(settlement.trade.quantity as i64)
            .bind(settlement.trade.collateral_amount as i64)
            .bind(&settlement.trade.buyer)
            .bind(&settlement.trade.seller)
            .bind(&settlement.trade.tx_signature)
            .bind(settlement.trade.created_at)
            .execute(&mut *tx)
            .await?;

            apply_position_delta_tx(
                &mut tx,
                settlement.trade.market_id.as_str(),
                settlement.trade.buyer.as_str(),
                settlement.buyer_yes_delta,
                settlement.buyer_no_delta,
            )
            .await?;
            apply_position_delta_tx(
                &mut tx,
                settlement.trade.market_id.as_str(),
                settlement.trade.seller.as_str(),
                settlement.seller_yes_delta,
                settlement.seller_no_delta,
            )
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    // Positions
    pub async fn get_positions(&self, owner: &str) -> Result<Vec<Position>> {
        let rows =
            sqlx::query("SELECT * FROM positions WHERE LOWER(owner) = $1 ORDER BY created_at DESC")
                .bind(owner)
                .fetch_all(&self.pool)
                .await?;

        let positions = rows.iter().map(|row| self.row_to_position(row)).collect();
        Ok(positions)
    }

    fn row_to_position(&self, row: &sqlx::postgres::PgRow) -> Position {
        Position {
            market_id: row.get("market_id"),
            market_question: row.try_get("market_question").unwrap_or_default(),
            owner: row.get("owner"),
            yes_balance: row.get::<i64, _>("yes_balance") as u64,
            no_balance: row.get::<i64, _>("no_balance") as u64,
            avg_yes_cost: row.try_get("avg_yes_cost").unwrap_or(0.0),
            avg_no_cost: row.try_get("avg_no_cost").unwrap_or(0.0),
            current_yes_price: row.try_get("current_yes_price").unwrap_or(0.5),
            current_no_price: row.try_get("current_no_price").unwrap_or(0.5),
            unrealized_pnl: row.try_get("unrealized_pnl").unwrap_or(0.0),
            realized_pnl: row.try_get("realized_pnl").unwrap_or(0.0),
            total_deposited: row
                .try_get::<i64, _>("total_deposited")
                .map(|v| v as u64)
                .unwrap_or(0),
            total_withdrawn: row
                .try_get::<i64, _>("total_withdrawn")
                .map(|v| v as u64)
                .unwrap_or(0),
            open_order_count: row
                .try_get::<i32, _>("open_order_count")
                .map(|v| v as u32)
                .unwrap_or(0),
            total_trades: row
                .try_get::<i32, _>("total_trades")
                .map(|v| v as u32)
                .unwrap_or(0),
            created_at: row.get("created_at"),
        }
    }

    pub async fn get_position(&self, owner: &str, market_id: &str) -> Result<Option<Position>> {
        let row = sqlx::query(
            "SELECT * FROM positions WHERE LOWER(owner) = LOWER($1) AND market_id = $2",
        )
        .bind(owner)
        .bind(market_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| self.row_to_position(&r)))
    }

    pub async fn list_base_payout_candidates(&self, limit: i64) -> Result<Vec<(String, String)>> {
        let safe_limit = limit.clamp(1, 5000);
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT p.owner, p.market_id
            FROM positions p
            JOIN markets m ON m.id = p.market_id
            WHERE m.resolved_outcome IS NOT NULL
              AND p.market_id ~ '^[0-9]+$'
              AND p.owner ~* '^0x[0-9a-f]{40}$'
              AND (p.yes_balance > 0 OR p.no_balance > 0)
            ORDER BY p.market_id, p.owner
            LIMIT $1
            "#,
        )
        .bind(safe_limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let owner: String = row.get("owner");
                let market_id: String = row.get("market_id");
                (owner, market_id)
            })
            .collect())
    }

    pub async fn seed_payout_jobs_from_positions(&self, limit: i64) -> Result<u64> {
        let safe_limit = limit.clamp(1, 10_000);
        let rows_affected = sqlx::query(
            r#"
            INSERT INTO payout_jobs (market_id, wallet, status, attempts)
            SELECT DISTINCT p.market_id::bigint, lower(p.owner), 'pending', 0
            FROM positions p
            JOIN markets m ON m.id = p.market_id
            WHERE m.resolved_outcome IS NOT NULL
              AND p.market_id ~ '^[0-9]+$'
              AND p.owner ~* '^0x[0-9a-f]{40}$'
              AND (p.yes_balance > 0 OR p.no_balance > 0)
            ORDER BY p.market_id::bigint, lower(p.owner)
            LIMIT $1
            ON CONFLICT (market_id, wallet) DO NOTHING
            "#,
        )
        .bind(safe_limit)
        .execute(&self.pool)
        .await?
        .rows_affected();

        Ok(rows_affected)
    }

    pub async fn list_payout_jobs(
        &self,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<PayoutJobRecord>, i64)> {
        let safe_limit = limit.clamp(1, 1_000);
        let safe_offset = offset.max(0);
        let status = status.map(|value| value.trim().to_ascii_lowercase());

        let rows = if let Some(status) = status.as_ref() {
            sqlx::query(
                r#"
                SELECT market_id, wallet, status, last_tx, attempts, last_error,
                       next_retry_at, updated_at
                FROM payout_jobs
                WHERE lower(status) = $1
                ORDER BY updated_at DESC, market_id DESC, wallet ASC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(status)
            .bind(safe_limit)
            .bind(safe_offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT market_id, wallet, status, last_tx, attempts, last_error,
                       next_retry_at, updated_at
                FROM payout_jobs
                ORDER BY updated_at DESC, market_id DESC, wallet ASC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(safe_limit)
            .bind(safe_offset)
            .fetch_all(&self.pool)
            .await?
        };

        let total: i64 = if let Some(status) = status.as_ref() {
            sqlx::query_scalar("SELECT COUNT(*) FROM payout_jobs WHERE lower(status) = $1")
                .bind(status)
                .fetch_one(&self.pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM payout_jobs")
                .fetch_one(&self.pool)
                .await?
        };

        let jobs = rows
            .iter()
            .map(|row| PayoutJobRecord {
                market_id: row.get::<i64, _>("market_id") as u64,
                wallet: row.get("wallet"),
                status: row.get("status"),
                last_tx: row.try_get("last_tx").ok(),
                attempts: row.get::<i32, _>("attempts").max(0) as u32,
                last_error: row.try_get("last_error").ok(),
                next_retry_at: row
                    .try_get::<chrono::DateTime<Utc>, _>("next_retry_at")
                    .ok()
                    .map(|ts| ts.to_rfc3339()),
                updated_at: row
                    .get::<chrono::DateTime<Utc>, _>("updated_at")
                    .to_rfc3339(),
            })
            .collect();

        Ok((jobs, total))
    }

    pub async fn update_payout_job_result(
        &self,
        market_id: u64,
        wallet: &str,
        status: &str,
        last_tx: Option<&str>,
        last_error: Option<&str>,
        next_retry_after_seconds: Option<i64>,
    ) -> Result<()> {
        let normalized_wallet = wallet.trim().to_ascii_lowercase();
        let normalized_status = status.trim().to_ascii_lowercase();
        let retry_at = next_retry_after_seconds
            .map(|seconds| Utc::now() + chrono::Duration::seconds(seconds.max(0)));

        sqlx::query(
            r#"
            INSERT INTO payout_jobs (market_id, wallet, status, last_tx, attempts, last_error, next_retry_at)
            VALUES ($1, $2, $3, $4, CASE WHEN $3 = 'paid' THEN 0 ELSE 1 END, $5, $6)
            ON CONFLICT (market_id, wallet) DO UPDATE SET
                status = EXCLUDED.status,
                last_tx = EXCLUDED.last_tx,
                last_error = EXCLUDED.last_error,
                next_retry_at = EXCLUDED.next_retry_at,
                attempts = CASE
                    WHEN EXCLUDED.status = 'paid' THEN payout_jobs.attempts
                    ELSE payout_jobs.attempts + 1
                END,
                updated_at = NOW()
            "#,
        )
        .bind(market_id as i64)
        .bind(normalized_wallet)
        .bind(normalized_status)
        .bind(last_tx)
        .bind(last_error)
        .bind(retry_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn payout_backlog_summary(&self) -> Result<PayoutBacklogSummary> {
        let row = sqlx::query(
            r#"
            SELECT
              COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0) AS pending,
              COALESCE(SUM(CASE WHEN status = 'processing' THEN 1 ELSE 0 END), 0) AS processing,
              COALESCE(SUM(CASE WHEN status = 'retry' THEN 1 ELSE 0 END), 0) AS retry,
              COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0) AS failed,
              COALESCE(
                EXTRACT(
                  EPOCH FROM (
                    NOW() - MIN(CASE WHEN status IN ('pending', 'retry') THEN updated_at END)
                  )
                ),
                0
              )::bigint AS oldest_pending_seconds
            FROM payout_jobs
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(PayoutBacklogSummary {
            pending: row.get::<i64, _>("pending").max(0) as u64,
            processing: row.get::<i64, _>("processing").max(0) as u64,
            retry: row.get::<i64, _>("retry").max(0) as u64,
            failed: row.get::<i64, _>("failed").max(0) as u64,
            oldest_pending_seconds: row.get::<i64, _>("oldest_pending_seconds").max(0) as u64,
        })
    }

    pub async fn get_chain_sync_cursor(&self, key: &str) -> Result<Option<ChainSyncCursor>> {
        let row = sqlx::query(
            r#"
            SELECT key, last_block, meta, updated_at
            FROM chain_sync_cursors
            WHERE key = $1
            "#,
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| ChainSyncCursor {
            key: row.get("key"),
            last_block: row.get::<i64, _>("last_block").max(0) as u64,
            meta: row
                .try_get("meta")
                .unwrap_or_else(|_| Value::Object(Default::default())),
            updated_at: row
                .get::<chrono::DateTime<Utc>, _>("updated_at")
                .to_rfc3339(),
        }))
    }

    pub async fn upsert_chain_sync_cursor(
        &self,
        key: &str,
        last_block: u64,
        meta: Value,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO chain_sync_cursors (key, last_block, meta)
            VALUES ($1, $2, $3)
            ON CONFLICT (key) DO UPDATE SET
                last_block = EXCLUDED.last_block,
                meta = EXCLUDED.meta,
                updated_at = NOW()
            "#,
        )
        .bind(key)
        .bind(last_block as i64)
        .bind(meta)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn record_compliance_decision(
        &self,
        entry: &ComplianceDecisionEntry<'_>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO compliance_decisions (
                request_id, wallet, country_code, action, route, method,
                decision, reason_code, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(entry.request_id)
        .bind(entry.wallet)
        .bind(entry.country_code)
        .bind(entry.action)
        .bind(entry.route)
        .bind(entry.method)
        .bind(entry.decision)
        .bind(entry.reason_code)
        .bind(&entry.metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Trades

    /// Create trade with position updates in a single transaction
    /// HIGH-024: Transaction boundaries for atomicity
    pub async fn create_trade_with_positions(
        &self,
        trade: &Trade,
        buyer_yes_delta: i64,
        buyer_no_delta: i64,
        seller_yes_delta: i64,
        seller_no_delta: i64,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Insert trade
        sqlx::query(
            r#"
            INSERT INTO trades (
                id, market_id, buy_order_id, sell_order_id, outcome,
                price, price_bps, quantity, collateral_amount,
                buyer, seller, tx_signature, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(&trade.id)
        .bind(&trade.market_id)
        .bind(&trade.buy_order_id)
        .bind(&trade.sell_order_id)
        .bind(trade.outcome as i16)
        .bind(trade.price)
        .bind(trade.price_bps as i16)
        .bind(trade.quantity as i64)
        .bind(trade.collateral_amount as i64)
        .bind(&trade.buyer)
        .bind(&trade.seller)
        .bind(&trade.tx_signature)
        .bind(trade.created_at)
        .execute(&mut *tx)
        .await?;

        // Update buyer position
        sqlx::query(
            r#"
            INSERT INTO positions (market_id, owner, yes_balance, no_balance, total_trades)
            VALUES ($1, $2, $3, $4, 1)
            ON CONFLICT (market_id, owner) DO UPDATE SET
                yes_balance = positions.yes_balance + $3,
                no_balance = positions.no_balance + $4,
                total_trades = positions.total_trades + 1
            "#,
        )
        .bind(&trade.market_id)
        .bind(&trade.buyer)
        .bind(buyer_yes_delta)
        .bind(buyer_no_delta)
        .execute(&mut *tx)
        .await?;

        // Update seller position
        sqlx::query(
            r#"
            INSERT INTO positions (market_id, owner, yes_balance, no_balance, total_trades)
            VALUES ($1, $2, $3, $4, 1)
            ON CONFLICT (market_id, owner) DO UPDATE SET
                yes_balance = positions.yes_balance + $3,
                no_balance = positions.no_balance + $4,
                total_trades = positions.total_trades + 1
            "#,
        )
        .bind(&trade.market_id)
        .bind(&trade.seller)
        .bind(seller_yes_delta)
        .bind(seller_no_delta)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn create_trade(&self, trade: &Trade) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO trades (
                id, market_id, buy_order_id, sell_order_id, outcome,
                price, price_bps, quantity, collateral_amount,
                buyer, seller, tx_signature, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(&trade.id)
        .bind(&trade.market_id)
        .bind(&trade.buy_order_id)
        .bind(&trade.sell_order_id)
        .bind(trade.outcome as i16)
        .bind(trade.price)
        .bind(trade.price_bps as i16)
        .bind(trade.quantity as i64)
        .bind(trade.collateral_amount as i64)
        .bind(&trade.buyer)
        .bind(&trade.seller)
        .bind(&trade.tx_signature)
        .bind(trade.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    fn row_to_trade(&self, row: &sqlx::postgres::PgRow) -> Trade {
        Trade {
            id: row.get("id"),
            market_id: row.get("market_id"),
            buy_order_id: row.get("buy_order_id"),
            sell_order_id: row.get("sell_order_id"),
            outcome: Outcome::from(row.get::<i16, _>("outcome") as u8),
            price: row.get("price"),
            price_bps: row.get::<i16, _>("price_bps") as u16,
            quantity: row.get::<i64, _>("quantity") as u64,
            collateral_amount: row.get::<i64, _>("collateral_amount") as u64,
            buyer: row.get("buyer"),
            seller: row.get("seller"),
            tx_signature: row
                .try_get("tx_signature")
                .unwrap_or_else(|_| String::new()),
            created_at: row.get("created_at"),
        }
    }

    pub async fn get_trades(
        &self,
        market_id: &str,
        outcome: Option<Outcome>,
        limit: i64,
        before: Option<&str>,
    ) -> Result<Vec<Trade>> {
        let rows = match (outcome, before) {
            (Some(o), Some(b)) => {
                sqlx::query("SELECT * FROM trades WHERE market_id = $1 AND outcome = $2 AND id < $3 ORDER BY created_at DESC LIMIT $4")
                    .bind(market_id).bind(o as i16).bind(b).bind(limit).fetch_all(&self.pool).await?
            }
            (Some(o), None) => {
                sqlx::query("SELECT * FROM trades WHERE market_id = $1 AND outcome = $2 ORDER BY created_at DESC LIMIT $3")
                    .bind(market_id).bind(o as i16).bind(limit).fetch_all(&self.pool).await?
            }
            (None, Some(b)) => {
                sqlx::query("SELECT * FROM trades WHERE market_id = $1 AND id < $2 ORDER BY created_at DESC LIMIT $3")
                    .bind(market_id).bind(b).bind(limit).fetch_all(&self.pool).await?
            }
            (None, None) => {
                sqlx::query("SELECT * FROM trades WHERE market_id = $1 ORDER BY created_at DESC LIMIT $2")
                    .bind(market_id).bind(limit).fetch_all(&self.pool).await?
            }
        };

        let trades = rows.iter().map(|row| self.row_to_trade(row)).collect();
        Ok(trades)
    }

    // Transactions
    pub async fn get_transactions(
        &self,
        owner: &str,
        tx_type: Option<TransactionType>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<ModelTransaction>, i64)> {
        let rows = match tx_type {
            Some(t) => {
                sqlx::query("SELECT * FROM transactions WHERE owner = $1 AND tx_type = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4")
                    .bind(owner).bind(t as i16).bind(limit).bind(offset).fetch_all(&self.pool).await?
            }
            None => {
                sqlx::query("SELECT * FROM transactions WHERE owner = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3")
                    .bind(owner).bind(limit).bind(offset).fetch_all(&self.pool).await?
            }
        };

        let total: i64 = match tx_type {
            Some(t) => {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM transactions WHERE owner = $1 AND tx_type = $2",
                )
                .bind(owner)
                .bind(t as i16)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE owner = $1")
                    .bind(owner)
                    .fetch_one(&self.pool)
                    .await?
            }
        };

        let transactions = rows
            .iter()
            .map(|row| ModelTransaction {
                id: row.get("id"),
                owner: row.get("owner"),
                market_id: row.try_get("market_id").ok(),
                tx_type: TransactionType::from(row.get::<i16, _>("tx_type") as u8),
                amount: row.get::<i64, _>("amount") as u64,
                fee: row.try_get::<i64, _>("fee").map(|v| v as u64).unwrap_or(0),
                tx_signature: row.try_get::<String, _>("tx_signature").ok(),
                status: row
                    .try_get("status")
                    .unwrap_or_else(|_| "pending".to_string()),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok((transactions, total))
    }

    // Order Book Persistence
    /// Add order to persistent order book
    pub async fn add_orderbook_entry(
        &self,
        order_id: &str,
        market_id: &str,
        outcome: Outcome,
        side: OrderSide,
        price_bps: u16,
        remaining_quantity: u64,
        owner: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO orderbook_entries (market_id, order_id, outcome, side, price_bps, remaining_quantity, owner)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (order_id) DO UPDATE SET remaining_quantity = $6
            "#,
        )
        .bind(market_id)
        .bind(order_id)
        .bind(outcome as i16)
        .bind(side as i16)
        .bind(price_bps as i16)
        .bind(remaining_quantity as i64)
        .bind(owner)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove order from persistent order book
    pub async fn remove_orderbook_entry(&self, order_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM orderbook_entries WHERE order_id = $1")
            .bind(order_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update remaining quantity in persistent order book
    pub async fn update_orderbook_entry_quantity(
        &self,
        order_id: &str,
        remaining_quantity: u64,
    ) -> Result<()> {
        if remaining_quantity == 0 {
            self.remove_orderbook_entry(order_id).await
        } else {
            sqlx::query("UPDATE orderbook_entries SET remaining_quantity = $1 WHERE order_id = $2")
                .bind(remaining_quantity as i64)
                .bind(order_id)
                .execute(&self.pool)
                .await?;
            Ok(())
        }
    }

    /// Load all open order book entries for recovery
    pub async fn load_orderbook_entries(&self) -> Result<Vec<OrderBookEntry>> {
        let rows = sqlx::query(
            r#"
            SELECT o.id, o.order_id, o.market_id, o.owner, o.outcome, o.side,
                   oe.remaining_quantity, o.price_bps, o.created_at
            FROM orderbook_entries oe
            JOIN orders o ON o.id = oe.order_id
            WHERE oe.remaining_quantity > 0
              AND o.status IN (0, 1)
            ORDER BY o.market_id, o.outcome, o.side, o.price_bps, o.created_at
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let entries = rows
            .iter()
            .map(|row| OrderBookEntry {
                order_id: row.get("id"),
                on_chain_id: row.get::<i64, _>("order_id") as u64,
                market_id: row.get("market_id"),
                owner: row.get("owner"),
                outcome: Outcome::from(row.get::<i16, _>("outcome") as u8),
                side: OrderSide::from(row.get::<i16, _>("side") as u8),
                price_bps: row.get::<i16, _>("price_bps") as u16,
                remaining_quantity: row.get::<i64, _>("remaining_quantity") as u64,
            })
            .collect();

        Ok(entries)
    }

    pub async fn upsert_base_market_bootstrap(
        &self,
        input: &BaseMarketBootstrapUpsert<'_>,
    ) -> Result<BaseMarketBootstrapConfigRecord> {
        let activated_at = input.activated_at.unwrap_or_else(Utc::now);
        let row = sqlx::query(
            r#"
            INSERT INTO base_market_bootstrap_configs (
                market_id,
                creator,
                liquidity_mode,
                status,
                manager,
                preset,
                seed_usdc,
                initial_yes_bps,
                strategy,
                levels,
                base_spread_bps,
                step_bps,
                cadence_seconds,
                expiry_seconds,
                organic_depth_window_bps,
                target_depth_multiplier,
                target_volume_multiplier,
                max_age_seconds,
                inventory_skew_bps,
                exposure_cap_bps,
                pause_reason,
                reserved_usdc,
                available_usdc,
                active_slots,
                organic_depth_ratio,
                consecutive_failures,
                activated_at,
                graduated_at,
                graduation_reason,
                create_tx_hash,
                launch_tx_hash,
                last_reconciled_at,
                last_error
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25,
                $26, $27, $28, $29, $30, $31, $32, $33
            )
            ON CONFLICT (market_id) DO UPDATE
            SET creator = EXCLUDED.creator,
                liquidity_mode = EXCLUDED.liquidity_mode,
                status = EXCLUDED.status,
                manager = COALESCE(EXCLUDED.manager, base_market_bootstrap_configs.manager),
                preset = EXCLUDED.preset,
                seed_usdc = EXCLUDED.seed_usdc,
                initial_yes_bps = EXCLUDED.initial_yes_bps,
                strategy = EXCLUDED.strategy,
                levels = EXCLUDED.levels,
                base_spread_bps = EXCLUDED.base_spread_bps,
                step_bps = EXCLUDED.step_bps,
                cadence_seconds = EXCLUDED.cadence_seconds,
                expiry_seconds = EXCLUDED.expiry_seconds,
                organic_depth_window_bps = EXCLUDED.organic_depth_window_bps,
                target_depth_multiplier = EXCLUDED.target_depth_multiplier,
                target_volume_multiplier = EXCLUDED.target_volume_multiplier,
                max_age_seconds = EXCLUDED.max_age_seconds,
                inventory_skew_bps = EXCLUDED.inventory_skew_bps,
                exposure_cap_bps = EXCLUDED.exposure_cap_bps,
                pause_reason = EXCLUDED.pause_reason,
                reserved_usdc = EXCLUDED.reserved_usdc,
                available_usdc = EXCLUDED.available_usdc,
                active_slots = EXCLUDED.active_slots,
                organic_depth_ratio = EXCLUDED.organic_depth_ratio,
                consecutive_failures = EXCLUDED.consecutive_failures,
                activated_at = COALESCE(base_market_bootstrap_configs.activated_at, EXCLUDED.activated_at),
                graduated_at = EXCLUDED.graduated_at,
                graduation_reason = EXCLUDED.graduation_reason,
                create_tx_hash = COALESCE(EXCLUDED.create_tx_hash, base_market_bootstrap_configs.create_tx_hash),
                launch_tx_hash = COALESCE(EXCLUDED.launch_tx_hash, base_market_bootstrap_configs.launch_tx_hash),
                last_reconciled_at = COALESCE(EXCLUDED.last_reconciled_at, base_market_bootstrap_configs.last_reconciled_at),
                last_error = COALESCE(EXCLUDED.last_error, base_market_bootstrap_configs.last_error)
            RETURNING *
            "#,
        )
        .bind(input.market_id as i64)
        .bind(input.creator)
        .bind(input.liquidity_mode)
        .bind(input.status)
        .bind(input.manager)
        .bind(input.preset)
        .bind(input.seed_usdc)
        .bind(input.initial_yes_bps as i32)
        .bind(input.strategy)
        .bind(input.levels as i32)
        .bind(input.base_spread_bps as i32)
        .bind(input.step_bps as i32)
        .bind(input.cadence_seconds as i32)
        .bind(input.expiry_seconds as i32)
        .bind(input.organic_depth_window_bps as i32)
        .bind(input.target_depth_multiplier)
        .bind(input.target_volume_multiplier)
        .bind(input.max_age_seconds as i64)
        .bind(input.inventory_skew_bps)
        .bind(input.exposure_cap_bps as i32)
        .bind(input.pause_reason)
        .bind(input.reserved_usdc)
        .bind(input.available_usdc)
        .bind(input.active_slots as i32)
        .bind(input.organic_depth_ratio)
        .bind(input.consecutive_failures as i32)
        .bind(activated_at)
        .bind(input.graduated_at)
        .bind(input.graduation_reason)
        .bind(input.create_tx_hash)
        .bind(input.launch_tx_hash)
        .bind(input.last_reconciled_at)
        .bind(input.last_error)
        .fetch_one(&self.pool)
        .await?;

        Ok(Self::map_base_market_bootstrap_row(&row))
    }

    pub async fn get_base_market_bootstrap(
        &self,
        market_id: u64,
    ) -> Result<Option<BaseMarketBootstrapConfigRecord>> {
        let row = sqlx::query("SELECT * FROM base_market_bootstrap_configs WHERE market_id = $1")
            .bind(market_id as i64)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.as_ref().map(Self::map_base_market_bootstrap_row))
    }

    pub async fn list_base_market_bootstrap_configs(
        &self,
    ) -> Result<Vec<BaseMarketBootstrapConfigRecord>> {
        let rows =
            sqlx::query("SELECT * FROM base_market_bootstrap_configs ORDER BY market_id ASC")
                .fetch_all(&self.pool)
                .await?;

        Ok(rows
            .iter()
            .map(Self::map_base_market_bootstrap_row)
            .collect())
    }

    pub async fn list_base_market_bootstrap_configs_for_creator(
        &self,
        creator: &str,
    ) -> Result<Vec<BaseMarketBootstrapConfigRecord>> {
        let rows = sqlx::query(
            "SELECT * FROM base_market_bootstrap_configs WHERE LOWER(creator) = LOWER($1) ORDER BY market_id ASC",
        )
        .bind(creator)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(Self::map_base_market_bootstrap_row)
            .collect())
    }

    pub async fn update_base_market_bootstrap_runtime(
        &self,
        market_id: u64,
        update: &BaseMarketBootstrapRuntimeUpdate<'_>,
    ) -> Result<Option<BaseMarketBootstrapConfigRecord>> {
        let row = sqlx::query(
            r#"
            UPDATE base_market_bootstrap_configs
            SET inventory_skew_bps = COALESCE($2, inventory_skew_bps),
                status = COALESCE($3, status),
                manager = COALESCE($4, manager),
                preset = COALESCE($5, preset),
                strategy = COALESCE($6, strategy),
                levels = COALESCE($7, levels),
                base_spread_bps = COALESCE($8, base_spread_bps),
                step_bps = COALESCE($9, step_bps),
                cadence_seconds = COALESCE($10, cadence_seconds),
                expiry_seconds = COALESCE($11, expiry_seconds),
                exposure_cap_bps = COALESCE($12, exposure_cap_bps),
                pause_reason = CASE
                    WHEN $14 THEN NULL
                    ELSE COALESCE($13, pause_reason)
                END,
                launch_tx_hash = COALESCE($15, launch_tx_hash),
                last_reconciled_at = COALESCE($16, last_reconciled_at),
                last_error = CASE
                    WHEN $18 THEN NULL
                    ELSE COALESCE($17, last_error)
                END,
                reserved_usdc = COALESCE($19, reserved_usdc),
                available_usdc = COALESCE($20, available_usdc),
                active_slots = COALESCE($21, active_slots),
                organic_depth_ratio = COALESCE($22, organic_depth_ratio),
                consecutive_failures = COALESCE($23, consecutive_failures)
            WHERE market_id = $1
            RETURNING *
            "#,
        )
        .bind(market_id as i64)
        .bind(update.inventory_skew_bps)
        .bind(update.status)
        .bind(update.manager)
        .bind(update.preset)
        .bind(update.strategy)
        .bind(update.levels.map(|value| value as i32))
        .bind(update.base_spread_bps.map(|value| value as i32))
        .bind(update.step_bps.map(|value| value as i32))
        .bind(update.cadence_seconds.map(|value| value as i32))
        .bind(update.expiry_seconds.map(|value| value as i32))
        .bind(update.exposure_cap_bps.map(|value| value as i32))
        .bind(update.pause_reason)
        .bind(update.clear_pause_reason)
        .bind(update.launch_tx_hash)
        .bind(update.last_reconciled_at)
        .bind(update.last_error)
        .bind(update.clear_last_error)
        .bind(update.reserved_usdc)
        .bind(update.available_usdc)
        .bind(update.active_slots.map(|value| value as i32))
        .bind(update.organic_depth_ratio)
        .bind(update.consecutive_failures.map(|value| value as i32))
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(Self::map_base_market_bootstrap_row))
    }

    pub async fn set_base_market_bootstrap_depth_qualified_since(
        &self,
        market_id: u64,
        value: Option<DateTime<Utc>>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE base_market_bootstrap_configs
            SET depth_qualified_since = $2
            WHERE market_id = $1
            "#,
        )
        .bind(market_id as i64)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn graduate_base_market_bootstrap(
        &self,
        market_id: u64,
        reason: &str,
    ) -> Result<Option<BaseMarketBootstrapConfigRecord>> {
        let row = sqlx::query(
            r#"
            UPDATE base_market_bootstrap_configs
            SET status = 'graduated',
                graduated_at = COALESCE(graduated_at, NOW()),
                graduation_reason = $2,
                pause_reason = 'graduation_pending'
            WHERE market_id = $1
            RETURNING *
            "#,
        )
        .bind(market_id as i64)
        .bind(reason)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(Self::map_base_market_bootstrap_row))
    }

    pub async fn list_base_market_bootstrap_agents(
        &self,
        market_id: u64,
    ) -> Result<Vec<BaseMarketBootstrapAgentRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT *
            FROM base_market_bootstrap_agents
            WHERE market_id = $1
            ORDER BY side ASC, level_index ASC
            "#,
        )
        .bind(market_id as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(Self::map_base_market_bootstrap_agent_row)
            .collect())
    }

    pub async fn upsert_base_market_bootstrap_agent(
        &self,
        input: &BaseMarketBootstrapAgentUpsert<'_>,
    ) -> Result<BaseMarketBootstrapAgentRecord> {
        let row = sqlx::query(
            r#"
            INSERT INTO base_market_bootstrap_agents (
                market_id,
                side,
                level_index,
                agent_id,
                desired_price_bps,
                desired_size,
                current_price_bps,
                current_size,
                active,
                created_tx_hash,
                updated_tx_hash,
                deactivated_tx_hash,
                last_execute_tx_hash,
                last_executed_at,
                last_reconciled_at,
                last_error
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8,
                $9, $10, $11, $12, $13, $14, $15, $16
            )
            ON CONFLICT (market_id, side, level_index) DO UPDATE
            SET agent_id = COALESCE(EXCLUDED.agent_id, base_market_bootstrap_agents.agent_id),
                desired_price_bps = EXCLUDED.desired_price_bps,
                desired_size = EXCLUDED.desired_size,
                current_price_bps = COALESCE(EXCLUDED.current_price_bps, base_market_bootstrap_agents.current_price_bps),
                current_size = COALESCE(EXCLUDED.current_size, base_market_bootstrap_agents.current_size),
                active = EXCLUDED.active,
                created_tx_hash = COALESCE(EXCLUDED.created_tx_hash, base_market_bootstrap_agents.created_tx_hash),
                updated_tx_hash = COALESCE(EXCLUDED.updated_tx_hash, base_market_bootstrap_agents.updated_tx_hash),
                deactivated_tx_hash = COALESCE(EXCLUDED.deactivated_tx_hash, base_market_bootstrap_agents.deactivated_tx_hash),
                last_execute_tx_hash = COALESCE(EXCLUDED.last_execute_tx_hash, base_market_bootstrap_agents.last_execute_tx_hash),
                last_executed_at = COALESCE(EXCLUDED.last_executed_at, base_market_bootstrap_agents.last_executed_at),
                last_reconciled_at = COALESCE(EXCLUDED.last_reconciled_at, base_market_bootstrap_agents.last_reconciled_at),
                last_error = COALESCE(EXCLUDED.last_error, base_market_bootstrap_agents.last_error)
            RETURNING *
            "#,
        )
        .bind(input.market_id as i64)
        .bind(input.side)
        .bind(input.level_index as i32)
        .bind(input.agent_id.map(|value| value as i64))
        .bind(input.desired_price_bps as i32)
        .bind(input.desired_size as i64)
        .bind(input.current_price_bps.map(|value| value as i32))
        .bind(input.current_size.map(|value| value as i64))
        .bind(input.active)
        .bind(input.created_tx_hash)
        .bind(input.updated_tx_hash)
        .bind(input.deactivated_tx_hash)
        .bind(input.last_execute_tx_hash)
        .bind(input.last_executed_at)
        .bind(input.last_reconciled_at)
        .bind(input.last_error)
        .fetch_one(&self.pool)
        .await?;

        Ok(Self::map_base_market_bootstrap_agent_row(&row))
    }

    pub async fn upsert_bootstrap_fill_event(
        &self,
        input: &BootstrapFillEventUpsert<'_>,
    ) -> Result<BootstrapFillEventRecord> {
        let row = sqlx::query(
            r#"
            INSERT INTO bootstrap_fill_events (
                id, market_id, creator, trade_id, source, agent_id, maker_order_id,
                outcome, side, price, quantity, notional_usdc, occurred_at, raw
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            ON CONFLICT (trade_id, source) DO UPDATE SET
                market_id = EXCLUDED.market_id,
                creator = EXCLUDED.creator,
                agent_id = EXCLUDED.agent_id,
                maker_order_id = EXCLUDED.maker_order_id,
                outcome = EXCLUDED.outcome,
                side = EXCLUDED.side,
                price = EXCLUDED.price,
                quantity = EXCLUDED.quantity,
                notional_usdc = EXCLUDED.notional_usdc,
                occurred_at = EXCLUDED.occurred_at,
                raw = EXCLUDED.raw,
                updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(input.id)
        .bind(input.market_id as i64)
        .bind(input.creator)
        .bind(input.trade_id)
        .bind(input.source)
        .bind(input.agent_id.map(|value| value as i64))
        .bind(input.maker_order_id)
        .bind(input.outcome)
        .bind(input.side)
        .bind(input.price)
        .bind(input.quantity)
        .bind(input.notional_usdc)
        .bind(input.occurred_at)
        .bind(input.raw)
        .fetch_one(&self.pool)
        .await?;

        Ok(Self::map_bootstrap_fill_event_row(&row))
    }

    pub async fn list_bootstrap_fill_events_for_creator_market(
        &self,
        creator: &str,
        market_id: u64,
    ) -> Result<Vec<BootstrapFillEventRecord>> {
        let rows = sqlx::query(
            "SELECT * FROM bootstrap_fill_events WHERE LOWER(creator) = LOWER($1) AND market_id = $2 ORDER BY occurred_at ASC, trade_id ASC",
        )
        .bind(creator)
        .bind(market_id as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(Self::map_bootstrap_fill_event_row)
            .collect())
    }

    pub async fn upsert_creator_market_economics_daily(
        &self,
        input: &CreatorMarketEconomicsDailyUpsert<'_>,
    ) -> Result<CreatorMarketEconomicsDailyRecord> {
        let row = sqlx::query(
            r#"
            INSERT INTO creator_market_economics_daily (
                market_id, creator, day, seed_usdc, available_usdc, reserved_usdc,
                inventory_yes, inventory_no, inventory_mark_value_usdc,
                cumulative_bootstrap_fills_usdc, net_liquidity_pnl_usdc, subsidy_burn_usdc,
                roi_bps, realized_resolution_pnl_usdc, organic_depth_ratio, graduated,
                graduation_retention_24h, graduation_retention_7d, mirror_freshness_seconds,
                mirror_pending_hedges, mirror_error_count
            )
            VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21
            )
            ON CONFLICT (market_id, creator, day) DO UPDATE SET
                seed_usdc = EXCLUDED.seed_usdc,
                available_usdc = EXCLUDED.available_usdc,
                reserved_usdc = EXCLUDED.reserved_usdc,
                inventory_yes = EXCLUDED.inventory_yes,
                inventory_no = EXCLUDED.inventory_no,
                inventory_mark_value_usdc = EXCLUDED.inventory_mark_value_usdc,
                cumulative_bootstrap_fills_usdc = EXCLUDED.cumulative_bootstrap_fills_usdc,
                net_liquidity_pnl_usdc = EXCLUDED.net_liquidity_pnl_usdc,
                subsidy_burn_usdc = EXCLUDED.subsidy_burn_usdc,
                roi_bps = EXCLUDED.roi_bps,
                realized_resolution_pnl_usdc = EXCLUDED.realized_resolution_pnl_usdc,
                organic_depth_ratio = EXCLUDED.organic_depth_ratio,
                graduated = EXCLUDED.graduated,
                graduation_retention_24h = EXCLUDED.graduation_retention_24h,
                graduation_retention_7d = EXCLUDED.graduation_retention_7d,
                mirror_freshness_seconds = EXCLUDED.mirror_freshness_seconds,
                mirror_pending_hedges = EXCLUDED.mirror_pending_hedges,
                mirror_error_count = EXCLUDED.mirror_error_count
            RETURNING *
            "#,
        )
        .bind(input.market_id as i64)
        .bind(input.creator)
        .bind(input.day)
        .bind(input.seed_usdc)
        .bind(input.available_usdc)
        .bind(input.reserved_usdc)
        .bind(input.inventory_yes)
        .bind(input.inventory_no)
        .bind(input.inventory_mark_value_usdc)
        .bind(input.cumulative_bootstrap_fills_usdc)
        .bind(input.net_liquidity_pnl_usdc)
        .bind(input.subsidy_burn_usdc)
        .bind(input.roi_bps)
        .bind(input.realized_resolution_pnl_usdc)
        .bind(input.organic_depth_ratio)
        .bind(input.graduated)
        .bind(input.graduation_retention_24h)
        .bind(input.graduation_retention_7d)
        .bind(input.mirror_freshness_seconds.map(|value| value as i64))
        .bind(input.mirror_pending_hedges as i64)
        .bind(input.mirror_error_count as i64)
        .fetch_one(&self.pool)
        .await?;

        Ok(Self::map_creator_market_economics_daily_row(&row))
    }

    pub async fn list_creator_market_economics_daily_for_market(
        &self,
        creator: &str,
        market_id: u64,
        start_day: Option<NaiveDate>,
        end_day: Option<NaiveDate>,
    ) -> Result<Vec<CreatorMarketEconomicsDailyRecord>> {
        let rows = match (start_day, end_day) {
            (Some(start_day), Some(end_day)) => sqlx::query(
                "SELECT * FROM creator_market_economics_daily WHERE LOWER(creator) = LOWER($1) AND market_id = $2 AND day >= $3 AND day <= $4 ORDER BY day ASC",
            )
            .bind(creator)
            .bind(market_id as i64)
            .bind(start_day)
            .bind(end_day)
            .fetch_all(&self.pool)
            .await?,
            (Some(start_day), None) => sqlx::query(
                "SELECT * FROM creator_market_economics_daily WHERE LOWER(creator) = LOWER($1) AND market_id = $2 AND day >= $3 ORDER BY day ASC",
            )
            .bind(creator)
            .bind(market_id as i64)
            .bind(start_day)
            .fetch_all(&self.pool)
            .await?,
            (None, Some(end_day)) => sqlx::query(
                "SELECT * FROM creator_market_economics_daily WHERE LOWER(creator) = LOWER($1) AND market_id = $2 AND day <= $3 ORDER BY day ASC",
            )
            .bind(creator)
            .bind(market_id as i64)
            .bind(end_day)
            .fetch_all(&self.pool)
            .await?,
            (None, None) => sqlx::query(
                "SELECT * FROM creator_market_economics_daily WHERE LOWER(creator) = LOWER($1) AND market_id = $2 ORDER BY day ASC",
            )
            .bind(creator)
            .bind(market_id as i64)
            .fetch_all(&self.pool)
            .await?,
        };
        Ok(rows
            .iter()
            .map(Self::map_creator_market_economics_daily_row)
            .collect())
    }

    pub async fn list_creator_market_economics_daily_for_creator(
        &self,
        creator: &str,
    ) -> Result<Vec<CreatorMarketEconomicsDailyRecord>> {
        let rows = sqlx::query(
            "SELECT * FROM creator_market_economics_daily WHERE LOWER(creator) = LOWER($1) ORDER BY market_id ASC, day ASC",
        )
        .bind(creator)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(Self::map_creator_market_economics_daily_row)
            .collect())
    }

    // ── Oracle market configs ──────────────────────────────────────────

    pub async fn get_oracle_market_config(
        &self,
        market_id: u64,
    ) -> Result<Option<OracleMarketConfigRecord>> {
        let row = sqlx::query("SELECT * FROM oracle_market_configs WHERE market_id = $1")
            .bind(market_id as i64)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.as_ref().map(Self::map_oracle_market_config_row))
    }

    pub async fn list_oracle_keeper_pending(&self) -> Result<Vec<OracleMarketConfigRecord>> {
        let rows = sqlx::query(
            "SELECT * FROM oracle_market_configs WHERE keeper_enabled AND resolved_at IS NULL ORDER BY market_id ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(Self::map_oracle_market_config_row)
            .collect())
    }

    pub async fn upsert_oracle_market_config(
        &self,
        market_id: u64,
        feed_type: &str,
        feed_address: Option<&str>,
        comparison: &str,
        target_value: &str,
        target_currency: &str,
        category: Option<&str>,
        resolution_hint: Option<&str>,
        keeper_enabled: bool,
    ) -> Result<OracleMarketConfigRecord> {
        let row = sqlx::query(
            r#"
            INSERT INTO oracle_market_configs
                (market_id, feed_type, feed_address, comparison, target_value,
                 target_currency, category, resolution_hint, keeper_enabled)
            VALUES ($1, $2, $3, $4, $5::NUMERIC, $6, $7, $8, $9)
            ON CONFLICT (market_id) DO UPDATE SET
                feed_type = EXCLUDED.feed_type,
                feed_address = EXCLUDED.feed_address,
                comparison = EXCLUDED.comparison,
                target_value = EXCLUDED.target_value,
                target_currency = EXCLUDED.target_currency,
                category = EXCLUDED.category,
                resolution_hint = EXCLUDED.resolution_hint,
                keeper_enabled = EXCLUDED.keeper_enabled
            RETURNING *
            "#,
        )
        .bind(market_id as i64)
        .bind(feed_type)
        .bind(feed_address)
        .bind(comparison)
        .bind(target_value)
        .bind(target_currency)
        .bind(category)
        .bind(resolution_hint)
        .bind(keeper_enabled)
        .fetch_one(&self.pool)
        .await?;

        Ok(Self::map_oracle_market_config_row(&row))
    }

    pub async fn update_oracle_config_tx(
        &self,
        market_id: u64,
        configure_tx: &str,
    ) -> Result<Option<OracleMarketConfigRecord>> {
        let row = sqlx::query(
            "UPDATE oracle_market_configs SET configure_tx = $2 WHERE market_id = $1 RETURNING *",
        )
        .bind(market_id as i64)
        .bind(configure_tx)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(Self::map_oracle_market_config_row))
    }

    pub async fn update_oracle_keeper_check(
        &self,
        market_id: u64,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE oracle_market_configs SET last_checked_at = NOW(), last_error = $2 WHERE market_id = $1",
        )
        .bind(market_id as i64)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_oracle_resolved(
        &self,
        market_id: u64,
        resolve_tx: &str,
    ) -> Result<Option<OracleMarketConfigRecord>> {
        let row = sqlx::query(
            "UPDATE oracle_market_configs SET resolve_tx = $2, resolved_at = NOW(), last_checked_at = NOW(), last_error = NULL WHERE market_id = $1 RETURNING *",
        )
        .bind(market_id as i64)
        .bind(resolve_tx)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(Self::map_oracle_market_config_row))
    }

    // ── KYC verification ──────────────────────────────────────────────

    pub async fn get_user_kyc_tier(&self, wallet: &str) -> Result<u8> {
        let row = sqlx::query("SELECT kyc_tier FROM users WHERE LOWER(wallet) = LOWER($1)")
            .bind(wallet)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| r.get::<i16, _>("kyc_tier") as u8).unwrap_or(0))
    }

    pub async fn update_user_kyc_tier(&self, wallet: &str, tier: u8, provider: &str) -> Result<()> {
        sqlx::query(
            "UPDATE users SET kyc_tier = $2, kyc_provider = $3, kyc_verified_at = NOW() WHERE LOWER(wallet) = LOWER($1)",
        )
        .bind(wallet)
        .bind(tier as i16)
        .bind(provider)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_kyc_verification(
        &self,
        wallet: &str,
        provider: &str,
        nullifier_hash: &str,
        proof_hash: &str,
        merkle_root: Option<&str>,
        action_id: Option<&str>,
        signal: Option<&str>,
        tier_granted: u8,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO kyc_verifications
                (wallet, provider, nullifier_hash, proof_hash, merkle_root, action_id, signal, tier_granted, status, confirmed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'confirmed', NOW())
            "#,
        )
        .bind(wallet)
        .bind(provider)
        .bind(nullifier_hash)
        .bind(proof_hash)
        .bind(merkle_root)
        .bind(action_id)
        .bind(signal)
        .bind(tier_granted as i16)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_latest_kyc_verification(
        &self,
        wallet: &str,
    ) -> Result<Option<KycVerificationRecord>> {
        let row = sqlx::query(
            "SELECT * FROM kyc_verifications WHERE LOWER(wallet) = LOWER($1) ORDER BY created_at DESC LIMIT 1",
        )
        .bind(wallet)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| KycVerificationRecord {
            id: r.get("id"),
            wallet: r.get("wallet"),
            provider: r.get("provider"),
            nullifier_hash: r.get("nullifier_hash"),
            tier_granted: r.get::<i16, _>("tier_granted") as u8,
            status: r.get("status"),
            created_at: r.get("created_at"),
            confirmed_at: r.try_get("confirmed_at").ok().flatten(),
        }))
    }

    // ── Social: follows ───────────────────────────────────────────────

    pub async fn insert_follow(&self, follower: &str, following: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO trader_follows (follower, following) VALUES (LOWER($1), LOWER($2)) ON CONFLICT DO NOTHING",
        )
        .bind(follower)
        .bind(following)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_follow(&self, follower: &str, following: &str) -> Result<()> {
        sqlx::query(
            "DELETE FROM trader_follows WHERE follower = LOWER($1) AND following = LOWER($2)",
        )
        .bind(follower)
        .bind(following)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn check_follow(&self, follower: &str, following: &str) -> Result<bool> {
        let row = sqlx::query(
            "SELECT 1 FROM trader_follows WHERE follower = LOWER($1) AND following = LOWER($2)",
        )
        .bind(follower)
        .bind(following)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn list_following(
        &self,
        wallet: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT following FROM trader_follows WHERE follower = LOWER($1) ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(wallet)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| r.get::<String, _>("following"))
            .collect())
    }

    pub async fn list_followers(
        &self,
        wallet: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT follower FROM trader_follows WHERE following = LOWER($1) ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(wallet)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| r.get::<String, _>("follower"))
            .collect())
    }

    pub async fn get_follower_counts(&self, wallet: &str) -> Result<FollowerCounts> {
        let followers_row =
            sqlx::query("SELECT COUNT(*) as cnt FROM trader_follows WHERE following = LOWER($1)")
                .bind(wallet)
                .fetch_one(&self.pool)
                .await?;

        let following_row =
            sqlx::query("SELECT COUNT(*) as cnt FROM trader_follows WHERE follower = LOWER($1)")
                .bind(wallet)
                .fetch_one(&self.pool)
                .await?;

        Ok(FollowerCounts {
            followers: followers_row.get::<i64, _>("cnt") as u64,
            following: following_row.get::<i64, _>("cnt") as u64,
        })
    }

    // ── Social: profile update ────────────────────────────────────────

    pub async fn update_user_profile(
        &self,
        wallet: &str,
        bio: Option<&str>,
        avatar_url: Option<&str>,
        website_url: Option<&str>,
        twitter_handle: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE users SET
                bio = COALESCE($2, bio),
                avatar_url = COALESCE($3, avatar_url),
                website_url = COALESCE($4, website_url),
                twitter_handle = COALESCE($5, twitter_handle),
                updated_at = NOW()
            WHERE LOWER(wallet) = LOWER($1)
            "#,
        )
        .bind(wallet)
        .bind(bio)
        .bind(avatar_url)
        .bind(website_url)
        .bind(twitter_handle)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Social: market comments ───────────────────────────────────────

    pub async fn list_market_comments(
        &self,
        market_id: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<MarketCommentRecord>> {
        let rows = sqlx::query(
            "SELECT * FROM market_comments WHERE market_id = $1 ORDER BY created_at ASC LIMIT $2 OFFSET $3",
        )
        .bind(market_id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| MarketCommentRecord {
                id: r.get("id"),
                market_id: r.get("market_id"),
                wallet: r.get("wallet"),
                text: r.get("text"),
                parent_id: r.try_get("parent_id").ok(),
                farcaster_hash: r.try_get("farcaster_hash").ok(),
                created_at: r.get("created_at"),
            })
            .collect())
    }

    pub async fn insert_market_comment(
        &self,
        id: &str,
        market_id: &str,
        wallet: &str,
        text: &str,
        parent_id: Option<&str>,
    ) -> Result<MarketCommentRecord> {
        let row = sqlx::query(
            r#"
            INSERT INTO market_comments (id, market_id, wallet, text, parent_id)
            VALUES ($1, $2, LOWER($3), $4, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(market_id)
        .bind(wallet)
        .bind(text)
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(MarketCommentRecord {
            id: row.get("id"),
            market_id: row.get("market_id"),
            wallet: row.get("wallet"),
            text: row.get("text"),
            parent_id: row.try_get("parent_id").ok(),
            farcaster_hash: row.try_get("farcaster_hash").ok(),
            created_at: row.get("created_at"),
        })
    }

    // ── API Key CRUD ────────────────────────────────────────────────

    pub async fn create_api_key(
        &self,
        wallet_address: &str,
        key_hash: &str,
        key_prefix: &str,
        label: &str,
        scope: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<String> {
        let row: (String,) = sqlx::query_as(
            "INSERT INTO api_keys (wallet_address, key_hash, key_prefix, label, scope, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind(wallet_address)
        .bind(key_hash)
        .bind(key_prefix)
        .bind(label)
        .bind(scope)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn get_api_key_by_hash(
        &self,
        key_hash: &str,
    ) -> Result<Option<(String, String, String, bool, Option<DateTime<Utc>>)>> {
        let row = sqlx::query_as::<_, (String, String, String, bool, Option<DateTime<Utc>>)>(
            "SELECT id, wallet_address, scope, is_active, expires_at FROM api_keys WHERE key_hash = $1",
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_api_key_hash_by_id(
        &self,
        key_id: &str,
        wallet_address: &str,
    ) -> Result<Option<String>> {
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT key_hash FROM api_keys WHERE id = $1 AND wallet_address = $2",
        )
        .bind(key_id)
        .bind(wallet_address)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn revoke_api_key(&self, key_id: &str, wallet_address: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE api_keys SET is_active = false, revoked_at = NOW() \
             WHERE id = $1 AND wallet_address = $2 AND is_active = true",
        )
        .bind(key_id)
        .bind(wallet_address)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_api_keys(
        &self,
        wallet_address: &str,
    ) -> Result<Vec<crate::api::api_key::ApiKeyListItem>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, bool, Option<DateTime<Utc>>, Option<DateTime<Utc>>, DateTime<Utc>)>(
            "SELECT id, key_prefix, label, scope, is_active, expires_at, last_used_at, created_at \
             FROM api_keys WHERE wallet_address = $1 ORDER BY created_at DESC",
        )
        .bind(wallet_address)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, prefix, label, scope, is_active, expires_at, last_used_at, created_at)| {
                crate::api::api_key::ApiKeyListItem {
                    id,
                    prefix,
                    label,
                    scope,
                    is_active,
                    expires_at,
                    last_used_at,
                    created_at,
                }
            })
            .collect())
    }

    pub async fn touch_api_key_last_used(&self, key_id: &str) -> Result<()> {
        sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
            .bind(key_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Batch Order Persistence ─────────────────────────────────────

    pub async fn persist_batch_order_flow(
        &self,
        takers: &[Order],
        maker_updates: &[Order],
        settlements: &[LocalTradeSettlement],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for taker in takers {
            sqlx::query(
                r#"
                INSERT INTO orders (
                    id, order_id, market_id, owner, side, outcome, order_type,
                    price, price_bps, quantity, filled_quantity, remaining_quantity,
                    status, is_private, tx_signature, created_at, updated_at, expires_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                "#,
            )
            .bind(&taker.id)
            .bind(taker.order_id as i64)
            .bind(&taker.market_id)
            .bind(&taker.owner)
            .bind(taker.side as i16)
            .bind(taker.outcome as i16)
            .bind(taker.order_type as i16)
            .bind(taker.price)
            .bind(taker.price_bps as i16)
            .bind(taker.quantity as i64)
            .bind(taker.filled_quantity as i64)
            .bind(taker.remaining_quantity as i64)
            .bind(taker.status as i16)
            .bind(taker.is_private)
            .bind(&taker.tx_signature)
            .bind(taker.created_at)
            .bind(taker.updated_at)
            .bind(taker.expires_at)
            .execute(&mut *tx)
            .await?;

            upsert_orderbook_entry_tx(&mut tx, taker).await?;
        }

        for maker in maker_updates {
            sqlx::query(
                "UPDATE orders SET status = $1, filled_quantity = $2, remaining_quantity = $3, updated_at = $4 WHERE id = $5",
            )
            .bind(maker.status as i16)
            .bind(maker.filled_quantity as i64)
            .bind(maker.remaining_quantity as i64)
            .bind(maker.updated_at)
            .bind(&maker.id)
            .execute(&mut *tx)
            .await?;

            upsert_orderbook_entry_tx(&mut tx, maker).await?;
        }

        for settlement in settlements {
            sqlx::query(
                r#"
                INSERT INTO trades (
                    id, market_id, buy_order_id, sell_order_id, outcome,
                    price, price_bps, quantity, collateral_amount,
                    buyer, seller, tx_signature, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                "#,
            )
            .bind(&settlement.trade.id)
            .bind(&settlement.trade.market_id)
            .bind(&settlement.trade.buy_order_id)
            .bind(&settlement.trade.sell_order_id)
            .bind(settlement.trade.outcome as i16)
            .bind(settlement.trade.price)
            .bind(settlement.trade.price_bps as i16)
            .bind(settlement.trade.quantity as i64)
            .bind(settlement.trade.collateral_amount as i64)
            .bind(&settlement.trade.buyer)
            .bind(&settlement.trade.seller)
            .bind(&settlement.trade.tx_signature)
            .bind(settlement.trade.created_at)
            .execute(&mut *tx)
            .await?;

            apply_position_delta_tx(
                &mut tx,
                settlement.trade.market_id.as_str(),
                settlement.trade.buyer.as_str(),
                settlement.buyer_yes_delta,
                settlement.buyer_no_delta,
            )
            .await?;
            apply_position_delta_tx(
                &mut tx,
                settlement.trade.market_id.as_str(),
                settlement.trade.seller.as_str(),
                settlement.seller_yes_delta,
                settlement.seller_no_delta,
            )
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

async fn upsert_orderbook_entry_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    order: &Order,
) -> Result<()> {
    let should_rest = matches!(
        order.status,
        OrderStatus::Open | OrderStatus::PartiallyFilled
    ) && order.remaining_quantity > 0;

    if should_rest {
        sqlx::query(
            r#"
            INSERT INTO orderbook_entries (market_id, order_id, outcome, side, price_bps, remaining_quantity, owner)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (order_id) DO UPDATE
            SET remaining_quantity = EXCLUDED.remaining_quantity
            "#,
        )
        .bind(&order.market_id)
        .bind(&order.id)
        .bind(order.outcome as i16)
        .bind(order.side as i16)
        .bind(order.price_bps as i16)
        .bind(order.remaining_quantity as i64)
        .bind(&order.owner)
        .execute(&mut **tx)
        .await?;
    } else {
        sqlx::query("DELETE FROM orderbook_entries WHERE order_id = $1")
            .bind(&order.id)
            .execute(&mut **tx)
            .await?;
    }

    Ok(())
}

async fn apply_position_delta_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    market_id: &str,
    owner: &str,
    yes_delta: i64,
    no_delta: i64,
) -> Result<()> {
    if yes_delta == 0 && no_delta == 0 {
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO positions (market_id, owner, yes_balance, no_balance, total_trades)
        VALUES ($1, $2, $3, $4, 1)
        ON CONFLICT (market_id, owner) DO UPDATE SET
            yes_balance = positions.yes_balance + $3,
            no_balance = positions.no_balance + $4,
            total_trades = positions.total_trades + 1
        "#,
    )
    .bind(market_id)
    .bind(owner)
    .bind(yes_delta)
    .bind(no_delta)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Order book entry for persistence and recovery
#[derive(Debug, Clone)]
pub struct OrderBookEntry {
    pub order_id: String,
    pub on_chain_id: u64,
    pub market_id: String,
    pub owner: String,
    pub outcome: Outcome,
    pub side: OrderSide,
    pub price_bps: u16,
    pub remaining_quantity: u64,
}

/// Database pool statistics for monitoring
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Current number of connections in the pool
    pub size: u32,
    /// Number of idle connections
    pub idle_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_connections, 1);
        assert_eq!(config.acquire_timeout, Duration::from_secs(60));
        assert_eq!(config.idle_timeout, Duration::from_secs(600));
        assert_eq!(config.max_lifetime, Duration::from_secs(1800));
    }

    #[test]
    fn test_market_status_conversion() {
        assert_eq!(MarketStatus::from(0u8), MarketStatus::Active);
        assert_eq!(MarketStatus::from(1u8), MarketStatus::Paused);
        assert_eq!(MarketStatus::from(2u8), MarketStatus::Closed);
        assert_eq!(MarketStatus::from(3u8), MarketStatus::Resolved);
        assert_eq!(MarketStatus::from(4u8), MarketStatus::Cancelled);
        // Unknown values default to Active
        assert_eq!(MarketStatus::from(255u8), MarketStatus::Active);
    }

    #[test]
    fn test_order_status_conversion() {
        assert_eq!(OrderStatus::from(0u8), OrderStatus::Open);
        assert_eq!(OrderStatus::from(1u8), OrderStatus::PartiallyFilled);
        assert_eq!(OrderStatus::from(2u8), OrderStatus::Filled);
        assert_eq!(OrderStatus::from(3u8), OrderStatus::Cancelled);
        assert_eq!(OrderStatus::from(4u8), OrderStatus::Expired);
        assert_eq!(OrderStatus::from(255u8), OrderStatus::Open);
    }

    #[test]
    fn test_order_side_conversion() {
        assert_eq!(OrderSide::from(0u8), OrderSide::Buy);
        assert_eq!(OrderSide::from(1u8), OrderSide::Sell);
        assert_eq!(OrderSide::from(255u8), OrderSide::Buy);
    }

    #[test]
    fn test_outcome_conversion() {
        assert_eq!(Outcome::from(1u8), Outcome::Yes);
        assert_eq!(Outcome::from(2u8), Outcome::No);
        // Unknown values default to Yes
        assert_eq!(Outcome::from(0u8), Outcome::Yes);
        assert_eq!(Outcome::from(255u8), Outcome::Yes);
    }

    #[test]
    fn test_order_type_conversion() {
        assert_eq!(OrderType::from(0u8), OrderType::Limit);
        assert_eq!(OrderType::from(1u8), OrderType::Market);
        // Unknown values default to Limit
        assert_eq!(OrderType::from(255u8), OrderType::Limit);
    }
}
