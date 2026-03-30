use log::warn;
use std::env;

fn is_valid_hex_address(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() != 42 || !trimmed.starts_with("0x") {
        return false;
    }
    trimmed[2..].chars().all(|ch| ch.is_ascii_hexdigit())
}

fn parse_csv_env(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect()
}

fn normalize_http_endpoint(raw: String) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }
    format!("http://{trimmed}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalExecutionMode {
    Live,
    Paper,
}

impl ExternalExecutionMode {
    pub fn from_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "live" => Some(Self::Live),
            "paper" => Some(Self::Paper),
            _ => None,
        }
    }

    fn from_env(raw: Option<String>) -> Self {
        raw.as_deref().and_then(Self::from_str).unwrap_or(Self::Live)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Paper => "paper",
        }
    }

    pub fn is_paper(self) -> bool {
        matches!(self, Self::Paper)
    }
}

#[derive(Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub chain_mode: String,
    pub solana_rpc_url: String,
    pub solana_ws_url: String,
    pub solana_market_program_id: String,
    pub solana_orderbook_program_id: String,
    pub solana_privacy_program_id: String,
    pub base_rpc_url: String,
    pub base_rpc_fallback_urls: Vec<String>,
    pub base_indexer_rpc_url: Option<String>,
    pub base_ws_url: String,
    pub base_chain_id: u64,
    pub siwe_domain: String,
    pub r44_token_address: String,
    pub market_core_address: String,
    pub order_book_address: String,
    pub agent_runtime_address: String,
    pub bootstrap_operator_address: String,
    pub jwt_secret: String,
    pub cors_origins: Vec<String>,
    pub is_development: bool,
    pub solana_enabled: bool,
    pub solana_reads_enabled: bool,
    pub solana_writes_enabled: bool,
    pub evm_enabled: bool,
    pub evm_reads_enabled: bool,
    pub evm_writes_enabled: bool,
    pub blindfold_webhook_secret: String,
    pub program_vault_address: String,
    pub collateral_vault_address: String,
    pub usdc_mint: String,
    pub erc8004_identity_registry_address: String,
    pub erc8004_reputation_registry_address: String,
    pub erc8004_validation_registry_address: String,
    pub x402_enabled: bool,
    pub x402_signing_key: String,
    pub x402_receiver_address: String,
    pub x402_facilitator_url: String,
    pub x402_facilitator_token: String,
    pub x402_orderbook_price_microusdc: u64,
    pub x402_trades_price_microusdc: u64,
    pub x402_mcp_price_microusdc: u64,
    pub x402_quote_ttl_seconds: u64,
    pub xmtp_swarm_enabled: bool,
    pub xmtp_swarm_signing_key: String,
    pub xmtp_swarm_transport: String,
    pub xmtp_swarm_bridge_url: String,
    pub xmtp_swarm_topic_prefix: String,
    pub xmtp_swarm_max_messages: u64,
    pub xmtp_swarm_max_message_bytes: u64,
    pub external_markets_enabled: bool,
    pub external_trading_enabled: bool,
    pub external_agents_enabled: bool,
    pub external_execution_mode: ExternalExecutionMode,
    pub limitless_enabled: bool,
    pub polymarket_enabled: bool,
    pub limitless_api_key: String,
    pub polymarket_api_key: String,
    pub polymarket_api_secret: String,
    pub polymarket_api_passphrase: String,
    pub polymarket_builder_api_key: String,
    pub polymarket_builder_api_secret: String,
    pub polymarket_builder_api_passphrase: String,
    pub polymarket_forwarder_url: String,
    pub polymarket_forwarder_shared_secret: String,
    pub external_credentials_master_key: String,
    pub external_credentials_key_id: String,
    pub paper_cohort_public_enabled: bool,
    pub paper_cohort_public_owner: String,
    pub limitless_api_base: String,
    pub polymarket_gamma_api_base: String,
    pub polymarket_clob_api_base: String,
    pub polygon_rpc_url: String,
    pub sanctions_blocked_addresses: Vec<String>,
    pub admin_wallets: Vec<String>,
    pub admin_control_key: String,
    pub paper_hold_duration_seconds: u64,
    pub paper_fee_bps: u64,
    pub paper_runner_scan_limit: u64,
    pub matcher_enabled: bool,
    pub matcher_max_fill_size: u64,
    pub matcher_rate_limit_per_market: u64,
    pub indexer_lookback_blocks: u64,
    pub indexer_confirmations: u64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let is_development = env::var("ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_string())
            .to_lowercase()
            == "development";

        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
            if is_development {
                warn!(
                    "SECURITY WARNING: Using default JWT_SECRET. Set JWT_SECRET env var in production!"
                );
                "dev-secret-key-do-not-use-in-production".to_string()
            } else {
                panic!("SECURITY ERROR: JWT_SECRET environment variable must be set in production");
            }
        });

        if !is_development && jwt_secret.len() < 32 {
            panic!("SECURITY ERROR: JWT_SECRET must be at least 32 characters in production");
        }

        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            if is_development {
                warn!(
                    "SECURITY WARNING: Using default DATABASE_URL. Set DATABASE_URL env var in production!"
                );
                "postgres://postgres:password@localhost:5432/relay44".to_string()
            } else {
                panic!("SECURITY ERROR: DATABASE_URL environment variable must be set in production");
            }
        });

        let cors_origins = env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| {
                if is_development {
                    "*".to_string()
                } else {
                    "".to_string()
                }
            })
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let evm_enabled = env::var("EVM_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase()
            == "true";
        let solana_enabled = env::var("SOLANA_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";
        let chain_mode = env::var("CHAIN_MODE")
            .unwrap_or_else(|_| {
                if evm_enabled && solana_enabled {
                    "dual".to_string()
                } else if solana_enabled {
                    "solana".to_string()
                } else {
                    "base".to_string()
                }
            })
            .trim()
            .to_ascii_lowercase();

        let x402_enabled = env::var("X402_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";
        let x402_signing_key = env::var("X402_SIGNING_KEY").unwrap_or_else(|_| {
            if is_development {
                warn!("SECURITY WARNING: Using default X402_SIGNING_KEY");
                "dev-x402-signing-key-change-me".to_string()
            } else {
                String::new()
            }
        });
        let x402_receiver_address = env::var("X402_RECEIVER_ADDRESS")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string())
            .trim()
            .to_string();
        let x402_receiver_address = if x402_receiver_address.is_empty() {
            "0x0000000000000000000000000000000000000000".to_string()
        } else {
            x402_receiver_address
        };
        if x402_enabled && !is_valid_hex_address(x402_receiver_address.as_str()) {
            panic!("SECURITY ERROR: X402_RECEIVER_ADDRESS must be a valid 0x address");
        }
        let x402_facilitator_url =
            normalize_http_endpoint(env::var("X402_FACILITATOR_URL").unwrap_or_default());
        if x402_enabled && !is_development && x402_facilitator_url.is_empty() {
            panic!("SECURITY ERROR: X402_FACILITATOR_URL must be set when X402_ENABLED=true");
        }
        let x402_facilitator_token = env::var("X402_FACILITATOR_TOKEN")
            .unwrap_or_else(|_| "".to_string())
            .trim()
            .to_string();

        let xmtp_swarm_enabled = env::var("XMTP_SWARM_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";
        let xmtp_swarm_signing_key = env::var("XMTP_SWARM_SIGNING_KEY").unwrap_or_else(|_| {
            if is_development {
                warn!("SECURITY WARNING: Using default XMTP_SWARM_SIGNING_KEY");
                "dev-xmtp-swarm-signing-key-change-me".to_string()
            } else {
                String::new()
            }
        });
        if xmtp_swarm_enabled && !is_development && xmtp_swarm_signing_key.trim().is_empty() {
            panic!(
                "SECURITY ERROR: XMTP_SWARM_SIGNING_KEY must be set when XMTP_SWARM_ENABLED=true"
            );
        }
        let xmtp_swarm_transport = env::var("XMTP_SWARM_TRANSPORT")
            .unwrap_or_else(|_| "redis".to_string())
            .trim()
            .to_ascii_lowercase();
        if xmtp_swarm_enabled
            && xmtp_swarm_transport != "redis"
            && xmtp_swarm_transport != "xmtp_http"
        {
            panic!("SECURITY ERROR: XMTP_SWARM_TRANSPORT must be one of redis|xmtp_http");
        }
        let xmtp_swarm_bridge_url =
            normalize_http_endpoint(env::var("XMTP_SWARM_BRIDGE_URL").unwrap_or_default());
        if xmtp_swarm_enabled
            && xmtp_swarm_transport == "xmtp_http"
            && xmtp_swarm_bridge_url.is_empty()
        {
            panic!(
                "SECURITY ERROR: XMTP_SWARM_BRIDGE_URL must be set when XMTP_SWARM_TRANSPORT=xmtp_http"
            );
        }

        let external_markets_enabled = env::var("EXTERNAL_MARKETS_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase()
            == "true";
        let external_trading_enabled = env::var("EXTERNAL_TRADING_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";
        let external_agents_enabled = env::var("EXTERNAL_AGENTS_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";
        let external_execution_mode =
            ExternalExecutionMode::from_env(env::var("EXTERNAL_EXECUTION_MODE").ok());
        let limitless_enabled = env::var("LIMITLESS_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase()
            == "true";
        let polymarket_enabled = env::var("POLYMARKET_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase()
            == "true";
        let limitless_api_key = env::var("LIMITLESS_API_KEY")
            .unwrap_or_default()
            .trim()
            .to_string();
        let polymarket_api_key = env::var("POLYMARKET_API_KEY")
            .unwrap_or_default()
            .trim()
            .to_string();
        let polymarket_api_secret = env::var("POLYMARKET_API_SECRET")
            .unwrap_or_default()
            .trim()
            .to_string();
        let polymarket_api_passphrase = env::var("POLYMARKET_API_PASSPHRASE")
            .unwrap_or_default()
            .trim()
            .to_string();
        let polymarket_builder_api_key = env::var("POLYMARKET_BUILDER_API_KEY")
            .unwrap_or_default()
            .trim()
            .to_string();
        let polymarket_builder_api_secret = env::var("POLYMARKET_BUILDER_API_SECRET")
            .unwrap_or_default()
            .trim()
            .to_string();
        let polymarket_builder_api_passphrase = env::var("POLYMARKET_BUILDER_API_PASSPHRASE")
            .unwrap_or_default()
            .trim()
            .to_string();
        let polymarket_forwarder_url =
            normalize_http_endpoint(env::var("POLYMARKET_FORWARDER_URL").unwrap_or_default());
        let polymarket_forwarder_shared_secret = env::var("POLYMARKET_FORWARDER_SHARED_SECRET")
            .unwrap_or_default()
            .trim()
            .to_string();

        let external_credentials_master_key =
            env::var("EXTERNAL_CREDENTIALS_MASTER_KEY").unwrap_or_else(|_| String::new());
        let external_credentials_key_id =
            env::var("EXTERNAL_CREDENTIALS_KEY_ID").unwrap_or_else(|_| "v1".to_string());
        let paper_cohort_public_owner = env::var("PAPER_COHORT_PUBLIC_OWNER")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if (external_trading_enabled || external_agents_enabled)
            && external_execution_mode == ExternalExecutionMode::Live
            && !is_development
            && external_credentials_master_key.trim().is_empty()
        {
            panic!(
                "SECURITY ERROR: EXTERNAL_CREDENTIALS_MASTER_KEY must be set when external trading or agents are enabled"
            );
        }

        let base_rpc_url = env::var("BASE_RPC_URL")
            .unwrap_or_else(|_| "https://mainnet.base.org".to_string())
            .trim()
            .to_string();
        let base_rpc_fallback_urls = env::var("BASE_RPC_FALLBACK_URLS")
            .ok()
            .map(|raw| parse_csv_env(raw.as_str()))
            .unwrap_or_default()
            .into_iter()
            .filter(|url| !url.eq_ignore_ascii_case(base_rpc_url.as_str()))
            .collect();
        let base_indexer_rpc_url = env::var("BASE_INDEXER_RPC_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let bootstrap_operator_address = env::var("BOOTSTRAP_OPERATOR_ADDRESS")
            .unwrap_or_else(|_| "".to_string())
            .trim()
            .to_ascii_lowercase();
        if !bootstrap_operator_address.is_empty()
            && !is_valid_hex_address(bootstrap_operator_address.as_str())
        {
            panic!("SECURITY ERROR: BOOTSTRAP_OPERATOR_ADDRESS must be a valid 0x address");
        }

        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a number"),
            database_url,
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            chain_mode,
            solana_rpc_url: env::var("SOLANA_RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
            solana_ws_url: env::var("SOLANA_WS_URL")
                .unwrap_or_else(|_| "wss://api.mainnet-beta.solana.com".to_string()),
            solana_market_program_id: env::var("SOLANA_MARKET_PROGRAM_ID")
                .unwrap_or_else(|_| "".to_string()),
            solana_orderbook_program_id: env::var("SOLANA_ORDERBOOK_PROGRAM_ID")
                .unwrap_or_else(|_| "".to_string()),
            solana_privacy_program_id: env::var("SOLANA_PRIVACY_PROGRAM_ID")
                .unwrap_or_else(|_| "".to_string()),
            base_rpc_url,
            base_rpc_fallback_urls,
            base_indexer_rpc_url,
            base_ws_url: env::var("BASE_WS_URL")
                .unwrap_or_else(|_| "wss://mainnet.base.org".to_string()),
            base_chain_id: env::var("BASE_CHAIN_ID")
                .unwrap_or_else(|_| "8453".to_string())
                .parse()
                .expect("BASE_CHAIN_ID must be a number"),
            siwe_domain: env::var("SIWE_DOMAIN").unwrap_or_else(|_| "localhost:3000".to_string()),
            r44_token_address: env::var("R44_TOKEN_ADDRESS").unwrap_or_else(|_| "".to_string()),
            market_core_address: env::var("MARKET_CORE_ADDRESS").unwrap_or_else(|_| "".to_string()),
            order_book_address: env::var("ORDER_BOOK_ADDRESS").unwrap_or_else(|_| "".to_string()),
            agent_runtime_address: env::var("AGENT_RUNTIME_ADDRESS")
                .unwrap_or_else(|_| "".to_string()),
            bootstrap_operator_address,
            jwt_secret,
            cors_origins,
            is_development,
            solana_enabled,
            solana_reads_enabled: env::var("SOLANA_READS_ENABLED")
                .unwrap_or_else(|_| if solana_enabled { "true" } else { "false" }.to_string())
                .to_lowercase()
                == "true",
            solana_writes_enabled: env::var("SOLANA_WRITES_ENABLED")
                .unwrap_or_else(|_| if solana_enabled { "true" } else { "false" }.to_string())
                .to_lowercase()
                == "true",
            evm_enabled,
            evm_reads_enabled: env::var("EVM_READS_ENABLED")
                .unwrap_or_else(|_| if evm_enabled { "true" } else { "false" }.to_string())
                .to_lowercase()
                == "true",
            evm_writes_enabled: env::var("EVM_WRITES_ENABLED")
                .unwrap_or_else(|_| if evm_enabled { "true" } else { "false" }.to_string())
                .to_lowercase()
                == "true",
            blindfold_webhook_secret: env::var("BLINDFOLD_WEBHOOK_SECRET").unwrap_or_else(|_| {
                if is_development {
                    warn!("SECURITY WARNING: Using default BLINDFOLD_WEBHOOK_SECRET");
                    "dev-blindfold-secret".to_string()
                } else {
                    panic!("SECURITY ERROR: BLINDFOLD_WEBHOOK_SECRET must be set in production");
                }
            }),
            program_vault_address: env::var("PROGRAM_VAULT_ADDRESS")
                .unwrap_or_else(|_| "".to_string()),
            collateral_vault_address: env::var("COLLATERAL_VAULT_ADDRESS").unwrap_or_else(|_| {
                env::var("PROGRAM_VAULT_ADDRESS").unwrap_or_else(|_| "".to_string())
            }),
            usdc_mint: env::var("USDC_MINT")
                .unwrap_or_else(|_| "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()),
            erc8004_identity_registry_address: env::var("ERC8004_IDENTITY_REGISTRY_ADDRESS")
                .unwrap_or_else(|_| "".to_string()),
            erc8004_reputation_registry_address: env::var("ERC8004_REPUTATION_REGISTRY_ADDRESS")
                .unwrap_or_else(|_| "".to_string()),
            erc8004_validation_registry_address: env::var("ERC8004_VALIDATION_REGISTRY_ADDRESS")
                .unwrap_or_else(|_| "".to_string()),
            x402_enabled,
            x402_signing_key,
            x402_receiver_address,
            x402_facilitator_url,
            x402_facilitator_token,
            x402_orderbook_price_microusdc: env::var("X402_ORDERBOOK_PRICE_MICROUSDC")
                .unwrap_or_else(|_| "2500".to_string())
                .parse()
                .expect("X402_ORDERBOOK_PRICE_MICROUSDC must be a number"),
            x402_trades_price_microusdc: env::var("X402_TRADES_PRICE_MICROUSDC")
                .unwrap_or_else(|_| "2500".to_string())
                .parse()
                .expect("X402_TRADES_PRICE_MICROUSDC must be a number"),
            x402_mcp_price_microusdc: env::var("X402_MCP_PRICE_MICROUSDC")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .expect("X402_MCP_PRICE_MICROUSDC must be a number"),
            x402_quote_ttl_seconds: env::var("X402_QUOTE_TTL_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .expect("X402_QUOTE_TTL_SECONDS must be a number"),
            xmtp_swarm_enabled,
            xmtp_swarm_signing_key,
            xmtp_swarm_transport,
            xmtp_swarm_bridge_url,
            xmtp_swarm_topic_prefix: env::var("XMTP_SWARM_TOPIC_PREFIX")
                .unwrap_or_else(|_| "relay44/base/swarm".to_string()),
            xmtp_swarm_max_messages: env::var("XMTP_SWARM_MAX_MESSAGES")
                .unwrap_or_else(|_| "500".to_string())
                .parse()
                .expect("XMTP_SWARM_MAX_MESSAGES must be a number"),
            xmtp_swarm_max_message_bytes: env::var("XMTP_SWARM_MAX_MESSAGE_BYTES")
                .unwrap_or_else(|_| "32768".to_string())
                .parse()
                .expect("XMTP_SWARM_MAX_MESSAGE_BYTES must be a number"),
            external_markets_enabled,
            external_trading_enabled,
            external_agents_enabled,
            external_execution_mode,
            limitless_enabled,
            polymarket_enabled,
            limitless_api_key,
            polymarket_api_key,
            polymarket_api_secret,
            polymarket_api_passphrase,
            polymarket_builder_api_key,
            polymarket_builder_api_secret,
            polymarket_builder_api_passphrase,
            polymarket_forwarder_url,
            polymarket_forwarder_shared_secret,
            external_credentials_master_key,
            external_credentials_key_id,
            paper_cohort_public_enabled: env::var("PAPER_COHORT_PUBLIC_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .to_ascii_lowercase()
                == "true",
            paper_cohort_public_owner,
            limitless_api_base: env::var("LIMITLESS_API_BASE")
                .unwrap_or_else(|_| "https://api.limitless.exchange".to_string()),
            polymarket_gamma_api_base: env::var("POLYMARKET_GAMMA_API_BASE")
                .unwrap_or_else(|_| "https://gamma-api.polymarket.com".to_string()),
            polymarket_clob_api_base: env::var("POLYMARKET_CLOB_API_BASE")
                .unwrap_or_else(|_| "https://clob.polymarket.com".to_string()),
            polygon_rpc_url: env::var("POLYGON_RPC_URL")
                .unwrap_or_else(|_| "https://polygon-rpc.com".to_string()),
            sanctions_blocked_addresses: env::var("SANCTIONS_BLOCKED_ADDRESSES")
                .unwrap_or_else(|_| "".to_string())
                .split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| is_valid_hex_address(s))
                .collect(),
            admin_wallets: env::var("ADMIN_WALLETS")
                .unwrap_or_else(|_| "".to_string())
                .split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| is_valid_hex_address(s))
                .collect(),
            admin_control_key: env::var("ADMIN_CONTROL_KEY").unwrap_or_else(|_| "".to_string()),
            paper_hold_duration_seconds: env::var("PAPER_HOLD_DURATION_SECONDS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .expect("PAPER_HOLD_DURATION_SECONDS must be a number"),
            paper_fee_bps: env::var("PAPER_FEE_BPS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .expect("PAPER_FEE_BPS must be a number"),
            paper_runner_scan_limit: env::var("PAPER_RUNNER_SCAN_LIMIT")
                .unwrap_or_else(|_| "200".to_string())
                .parse()
                .expect("PAPER_RUNNER_SCAN_LIMIT must be a number"),
            matcher_enabled: env::var("MATCHER_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .to_lowercase()
                == "true",
            matcher_max_fill_size: env::var("MATCHER_MAX_FILL_SIZE")
                .unwrap_or_else(|_| "1000000".to_string())
                .parse()
                .expect("MATCHER_MAX_FILL_SIZE must be a number"),
            matcher_rate_limit_per_market: env::var("MATCHER_RATE_LIMIT_PER_MARKET")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .expect("MATCHER_RATE_LIMIT_PER_MARKET must be a number"),
            indexer_lookback_blocks: env::var("INDEXER_LOOKBACK_BLOCKS")
                .unwrap_or_else(|_| "25000".to_string())
                .parse()
                .expect("INDEXER_LOOKBACK_BLOCKS must be a number"),
            indexer_confirmations: env::var("INDEXER_CONFIRMATIONS")
                .unwrap_or_else(|_| "8".to_string())
                .parse()
                .expect("INDEXER_CONFIRMATIONS must be a number"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn with_clean_env<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = TEST_MUTEX.lock().unwrap();

        let env_vars = [
            "ENVIRONMENT",
            "JWT_SECRET",
            "DATABASE_URL",
            "CORS_ORIGINS",
            "HOST",
            "PORT",
            "REDIS_URL",
            "CHAIN_MODE",
            "SOLANA_RPC_URL",
            "SOLANA_WS_URL",
            "SOLANA_MARKET_PROGRAM_ID",
            "SOLANA_ORDERBOOK_PROGRAM_ID",
            "SOLANA_PRIVACY_PROGRAM_ID",
            "BASE_RPC_URL",
            "BASE_RPC_FALLBACK_URLS",
            "BASE_INDEXER_RPC_URL",
            "BASE_WS_URL",
            "BASE_CHAIN_ID",
            "SIWE_DOMAIN",
            "SOLANA_ENABLED",
            "SOLANA_READS_ENABLED",
            "SOLANA_WRITES_ENABLED",
            "EVM_ENABLED",
            "EVM_READS_ENABLED",
            "EVM_WRITES_ENABLED",
            "ORDER_BOOK_ADDRESS",
            "COLLATERAL_VAULT_ADDRESS",
            "AGENT_RUNTIME_ADDRESS",
            "ERC8004_IDENTITY_REGISTRY_ADDRESS",
            "ERC8004_REPUTATION_REGISTRY_ADDRESS",
            "ERC8004_VALIDATION_REGISTRY_ADDRESS",
            "X402_ENABLED",
            "X402_SIGNING_KEY",
            "X402_RECEIVER_ADDRESS",
            "X402_FACILITATOR_URL",
            "X402_FACILITATOR_TOKEN",
            "X402_ORDERBOOK_PRICE_MICROUSDC",
            "X402_TRADES_PRICE_MICROUSDC",
            "X402_MCP_PRICE_MICROUSDC",
            "X402_QUOTE_TTL_SECONDS",
            "XMTP_SWARM_ENABLED",
            "XMTP_SWARM_SIGNING_KEY",
            "XMTP_SWARM_TRANSPORT",
            "XMTP_SWARM_BRIDGE_URL",
            "XMTP_SWARM_TOPIC_PREFIX",
            "XMTP_SWARM_MAX_MESSAGES",
            "XMTP_SWARM_MAX_MESSAGE_BYTES",
            "EXTERNAL_MARKETS_ENABLED",
            "EXTERNAL_TRADING_ENABLED",
            "EXTERNAL_AGENTS_ENABLED",
            "EXTERNAL_EXECUTION_MODE",
            "LIMITLESS_ENABLED",
            "POLYMARKET_ENABLED",
            "LIMITLESS_API_KEY",
            "POLYMARKET_API_KEY",
            "POLYMARKET_API_SECRET",
            "POLYMARKET_API_PASSPHRASE",
            "POLYMARKET_BUILDER_API_KEY",
            "POLYMARKET_BUILDER_API_SECRET",
            "POLYMARKET_BUILDER_API_PASSPHRASE",
            "POLYMARKET_FORWARDER_URL",
            "POLYMARKET_FORWARDER_SHARED_SECRET",
            "EXTERNAL_CREDENTIALS_MASTER_KEY",
            "EXTERNAL_CREDENTIALS_KEY_ID",
            "LIMITLESS_API_BASE",
            "POLYMARKET_GAMMA_API_BASE",
            "POLYMARKET_CLOB_API_BASE",
            "POLYGON_RPC_URL",
            "SANCTIONS_BLOCKED_ADDRESSES",
            "ADMIN_WALLETS",
            "ADMIN_CONTROL_KEY",
            "PAPER_HOLD_DURATION_SECONDS",
            "PAPER_FEE_BPS",
            "PAPER_RUNNER_SCAN_LIMIT",
            "MATCHER_ENABLED",
            "MATCHER_MAX_FILL_SIZE",
            "MATCHER_RATE_LIMIT_PER_MARKET",
            "INDEXER_LOOKBACK_BLOCKS",
            "INDEXER_CONFIRMATIONS",
        ];
        let saved: Vec<_> = env_vars
            .iter()
            .map(|k| (*k, std::env::var(*k).ok()))
            .collect();

        for k in &env_vars {
            std::env::remove_var(*k);
        }

        let result = f();

        for (k, v) in saved {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }

        result
    }

    #[test]
    fn test_development_mode_defaults() {
        with_clean_env(|| {
            std::env::set_var("ENVIRONMENT", "development");
            let config = AppConfig::from_env();

            assert!(config.is_development);
            assert_eq!(config.host, "0.0.0.0");
            assert_eq!(config.port, 8080);
            assert!(config.cors_origins.contains(&"*".to_string()));
            assert!(config.evm_enabled);
        });
    }

    #[test]
    fn test_cors_origins_parsing() {
        with_clean_env(|| {
            std::env::set_var("ENVIRONMENT", "development");
            std::env::set_var(
                "CORS_ORIGINS",
                "http://localhost:3000, https://app.example.com",
            );

            let config = AppConfig::from_env();

            assert_eq!(config.cors_origins.len(), 2);
            assert!(config
                .cors_origins
                .contains(&"http://localhost:3000".to_string()));
            assert!(config
                .cors_origins
                .contains(&"https://app.example.com".to_string()));
        });
    }

    #[test]
    fn test_custom_host_and_port() {
        with_clean_env(|| {
            std::env::set_var("ENVIRONMENT", "development");
            std::env::set_var("HOST", "127.0.0.1");
            std::env::set_var("PORT", "3000");

            let config = AppConfig::from_env();

            assert_eq!(config.host, "127.0.0.1");
            assert_eq!(config.port, 3000);
        });
    }

    #[test]
    fn test_custom_base_urls_and_chain_id() {
        with_clean_env(|| {
            std::env::set_var("ENVIRONMENT", "development");
            std::env::set_var("BASE_RPC_URL", "https://base.rpc.example");
            std::env::set_var(
                "BASE_RPC_FALLBACK_URLS",
                "https://base.rpc.backup,https://base.rpc.example, https://base.rpc.third ",
            );
            std::env::set_var("BASE_INDEXER_RPC_URL", "https://base.indexer.example");
            std::env::set_var("BASE_WS_URL", "wss://base.rpc.example");
            std::env::set_var("BASE_CHAIN_ID", "8453");
            std::env::set_var("SIWE_DOMAIN", "app.neuralminds.example");

            let config = AppConfig::from_env();

            assert_eq!(config.base_rpc_url, "https://base.rpc.example");
            assert_eq!(
                config.base_rpc_fallback_urls,
                vec![
                    "https://base.rpc.backup".to_string(),
                    "https://base.rpc.third".to_string()
                ]
            );
            assert_eq!(
                config.base_indexer_rpc_url,
                Some("https://base.indexer.example".to_string())
            );
            assert_eq!(config.base_ws_url, "wss://base.rpc.example");
            assert_eq!(config.base_chain_id, 8453);
            assert_eq!(config.siwe_domain, "app.neuralminds.example");
        });
    }

    #[test]
    fn test_evm_enabled_flag() {
        with_clean_env(|| {
            std::env::set_var("ENVIRONMENT", "development");
            std::env::set_var("EVM_ENABLED", "true");

            let config = AppConfig::from_env();
            assert!(config.evm_enabled);
        });
    }

    #[test]
    fn test_environment_case_insensitive() {
        with_clean_env(|| {
            std::env::set_var("ENVIRONMENT", "DEVELOPMENT");

            let config = AppConfig::from_env();
            assert!(config.is_development);
        });
    }

    #[test]
    fn test_paper_execution_mode_parsing() {
        with_clean_env(|| {
            std::env::set_var("ENVIRONMENT", "development");
            std::env::set_var("EXTERNAL_EXECUTION_MODE", "paper");

            let config = AppConfig::from_env();

            assert_eq!(config.external_execution_mode, ExternalExecutionMode::Paper);
            assert_eq!(config.external_execution_mode.as_str(), "paper");
        });
    }

    #[test]
    fn test_admin_wallets_parsing() {
        with_clean_env(|| {
            std::env::set_var("ENVIRONMENT", "development");
            std::env::set_var(
                "ADMIN_WALLETS",
                "0x1111111111111111111111111111111111111111,invalid,0x2222222222222222222222222222222222222222",
            );

            let config = AppConfig::from_env();

            assert_eq!(config.admin_wallets.len(), 2);
            assert!(config
                .admin_wallets
                .contains(&"0x1111111111111111111111111111111111111111".to_string()));
            assert!(config
                .admin_wallets
                .contains(&"0x2222222222222222222222222222222222222222".to_string()));
        });
    }

    #[test]
    fn test_provider_launch_credentials_parsing() {
        with_clean_env(|| {
            std::env::set_var("ENVIRONMENT", "development");
            std::env::set_var("LIMITLESS_API_KEY", "limitless-key");
            std::env::set_var("POLYMARKET_API_KEY", "poly-key");
            std::env::set_var("POLYMARKET_API_SECRET", "poly-secret");
            std::env::set_var("POLYMARKET_API_PASSPHRASE", "poly-passphrase");
            std::env::set_var("POLYMARKET_BUILDER_API_KEY", "builder-key");
            std::env::set_var("POLYMARKET_BUILDER_API_SECRET", "builder-secret");
            std::env::set_var("POLYMARKET_BUILDER_API_PASSPHRASE", "builder-passphrase");

            let config = AppConfig::from_env();

            assert_eq!(config.limitless_api_key, "limitless-key");
            assert_eq!(config.polymarket_api_key, "poly-key");
            assert_eq!(config.polymarket_api_secret, "poly-secret");
            assert_eq!(config.polymarket_api_passphrase, "poly-passphrase");
            assert_eq!(config.polymarket_builder_api_key, "builder-key");
            assert_eq!(config.polymarket_builder_api_secret, "builder-secret");
            assert_eq!(
                config.polymarket_builder_api_passphrase,
                "builder-passphrase"
            );
        });
    }

    #[test]
    fn test_polymarket_forwarder_config_parsing() {
        with_clean_env(|| {
            std::env::set_var("ENVIRONMENT", "development");
            std::env::set_var("POLYMARKET_FORWARDER_URL", "forwarder.internal:8099");
            std::env::set_var("POLYMARKET_FORWARDER_SHARED_SECRET", "shared-secret");

            let config = AppConfig::from_env();

            assert_eq!(
                config.polymarket_forwarder_url,
                "http://forwarder.internal:8099"
            );
            assert_eq!(
                config.polymarket_forwarder_shared_secret,
                "shared-secret"
            );
        });
    }
}
