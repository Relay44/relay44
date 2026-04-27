use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;

use crate::services::staking;
use crate::{api::ApiError, AppState};

const ERC20_TOTAL_SUPPLY_SELECTOR: &str = "0x18160ddd";
const ERC20_DECIMALS_SELECTOR: &str = "0x313ce567";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolMetricsResponse {
    pub markets: ProtocolMarketMetrics,
    pub volume: ProtocolVolumeMetrics,
    pub agents: ProtocolAgentMetrics,
    pub collateral: ProtocolCollateralMetrics,
    pub source: String,
    pub updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolMarketMetrics {
    pub total: i64,
    pub active: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolVolumeMetrics {
    pub settlement_usdc: f64,
    pub table_reported_usdc: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolAgentMetrics {
    pub connected: i64,
    pub active: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolCollateralMetrics {
    pub usdc: f64,
}

async fn query_i64(state: &AppState, sql: &str) -> Result<i64, ApiError> {
    sqlx::query_scalar::<_, i64>(sql)
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| {
            log::error!("protocol metrics integer query failed: {}", err);
            ApiError::internal("Failed to load protocol metrics")
        })
}

async fn query_f64(state: &AppState, sql: &str) -> Result<f64, ApiError> {
    sqlx::query_scalar::<_, f64>(sql)
        .fetch_one(state.db.pool())
        .await
        .map_err(|err| {
            log::error!("protocol metrics decimal query failed: {}", err);
            ApiError::internal("Failed to load protocol metrics")
        })
}

pub async fn get_protocol_metrics(
    state: web::Data<Arc<AppState>>,
) -> Result<HttpResponse, ApiError> {
    let total_markets = query_i64(
        &state,
        "SELECT \
            (SELECT COUNT(*) FROM markets)::BIGINT + \
            (SELECT COUNT(*) FROM distribution_markets)::BIGINT",
    )
    .await?;

    let active_markets = query_i64(
        &state,
        "SELECT \
            (SELECT COUNT(*) FROM markets WHERE status = 0)::BIGINT + \
            (SELECT COUNT(*) FROM distribution_markets WHERE status = 0)::BIGINT",
    )
    .await?;

    let settlement_usdc = query_f64(
        &state,
        "SELECT \
            COALESCE((SELECT SUM(collateral_amount)::DOUBLE PRECISION / 1000000.0 FROM trades), 0) + \
            COALESCE((SELECT SUM(cost)::DOUBLE PRECISION FROM distribution_trades), 0)",
    )
    .await?;

    let table_reported_usdc = query_f64(
        &state,
        "SELECT \
            COALESCE((SELECT SUM(total_volume)::DOUBLE PRECISION FROM markets), 0) + \
            COALESCE((SELECT SUM(total_volume)::DOUBLE PRECISION / 1000000.0 FROM distribution_markets), 0)",
    )
    .await?;

    let collateral_usdc = query_f64(
        &state,
        "SELECT \
            COALESCE((SELECT SUM(total_collateral)::DOUBLE PRECISION / 1000000.0 FROM markets), 0) + \
            COALESCE((SELECT SUM(total_collateral)::DOUBLE PRECISION / 1000000.0 FROM distribution_markets), 0)",
    )
    .await?;

    let connected_agents = query_i64(
        &state,
        "SELECT \
            (SELECT COUNT(*) FROM external_agents)::BIGINT + \
            (SELECT COUNT(*) FROM managed_agents)::BIGINT + \
            (SELECT COUNT(DISTINCT agent_id) FROM base_market_bootstrap_agents WHERE agent_id IS NOT NULL)::BIGINT",
    )
    .await?;

    let active_agents = query_i64(
        &state,
        "SELECT \
            (SELECT COUNT(*) FROM external_agents WHERE active = true)::BIGINT + \
            (SELECT COUNT(*) FROM managed_agents WHERE status = 'active')::BIGINT + \
            (SELECT COUNT(DISTINCT agent_id) FROM base_market_bootstrap_agents WHERE active = true AND agent_id IS NOT NULL)::BIGINT",
    )
    .await?;

    Ok(HttpResponse::Ok().json(ProtocolMetricsResponse {
        markets: ProtocolMarketMetrics {
            total: total_markets,
            active: active_markets,
        },
        volume: ProtocolVolumeMetrics {
            settlement_usdc,
            table_reported_usdc,
        },
        agents: ProtocolAgentMetrics {
            connected: connected_agents,
            active: active_agents,
        },
        collateral: ProtocolCollateralMetrics {
            usdc: collateral_usdc,
        },
        source: "relay44-api".to_string(),
        updated_at: Utc::now().to_rfc3339(),
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayUtilityResponse {
    pub chain_id: u64,
    pub token: TokenSummary,
    pub staking: StakingSummary,
    pub reward_distributor: RewardDistributorSummary,
    pub flags: UtilityFlags,
    pub source: String,
    pub updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenSummary {
    pub address: String,
    pub total_supply_hex: String,
    pub decimals: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StakingSummary {
    pub address: String,
    pub total_staked_hex: String,
    pub tiers: Vec<StakingTierSummary>,
    pub x402_bypass_tier: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StakingTierSummary {
    pub tier: u64,
    pub name: String,
    pub min_relay_wei: String,
    pub fee_discount_bps: u64,
    pub x402_bypass: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RewardDistributorSummary {
    pub address: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UtilityFlags {
    pub fee_discount: bool,
    pub x402_discount: bool,
    pub staking_rewards: bool,
    pub agent_rewards: bool,
    pub creator_rewards: bool,
    pub governance: bool,
}

fn is_valid_evm_address(addr: &str) -> bool {
    let trimmed = addr.trim();
    trimmed.len() == 42
        && trimmed.starts_with("0x")
        && trimmed[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_u8_hex(value: &str) -> Result<u8, ApiError> {
    let trimmed = value.trim_start_matches("0x");
    let normalized = trimmed.trim_start_matches('0');
    if normalized.is_empty() {
        return Ok(0);
    }
    u8::from_str_radix(normalized, 16)
        .map_err(|_| ApiError::internal("Failed to parse hex value"))
}

pub async fn get_relay_utility(
    state: web::Data<Arc<AppState>>,
) -> Result<HttpResponse, ApiError> {
    if !state.config.evm_enabled || !state.config.evm_reads_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM services are disabled",
        ));
    }

    let token_address = state.config.relay_token_address.trim();
    if token_address.is_empty() || !is_valid_evm_address(token_address) {
        return Err(ApiError::bad_request(
            "INVALID_TOKEN_ADDRESS",
            "RELAY_TOKEN_ADDRESS must be configured as a valid 0x EVM address",
        ));
    }

    let staking_address = state.config.relay_staking_address.trim();
    if staking_address.is_empty() || !is_valid_evm_address(staking_address) {
        return Err(ApiError::bad_request(
            "INVALID_STAKING_ADDRESS",
            "RELAY_STAKING_ADDRESS must be configured as a valid 0x EVM address",
        ));
    }

    let reward_distributor_address = state.config.reward_distributor_address.trim();
    if reward_distributor_address.is_empty() || !is_valid_evm_address(reward_distributor_address) {
        return Err(ApiError::bad_request(
            "INVALID_REWARD_DISTRIBUTOR_ADDRESS",
            "REWARD_DISTRIBUTOR_ADDRESS must be configured as a valid 0x EVM address",
        ));
    }

    let total_supply_hex = state
        .evm_rpc
        .eth_call(token_address, ERC20_TOTAL_SUPPLY_SELECTOR)
        .await
        .map_err(|err| {
            log::error!("relay-utility: totalSupply call failed: {}", err);
            ApiError::internal("Failed to read RELAY total supply")
        })?;
    let decimals_hex = state
        .evm_rpc
        .eth_call(token_address, ERC20_DECIMALS_SELECTOR)
        .await
        .map_err(|err| {
            log::error!("relay-utility: decimals call failed: {}", err);
            ApiError::internal("Failed to read RELAY decimals")
        })?;
    let decimals = parse_u8_hex(&decimals_hex)?;

    let total_staked_hex = staking::get_total_staked_hex(&state.evm_rpc, staking_address)
        .await
        .map_err(|err| {
            log::error!("relay-utility: totalStaked call failed: {}", err);
            ApiError::internal("Failed to read RELAY totalStaked")
        })?;

    let tiers = staking::TIERS
        .iter()
        .map(|t| StakingTierSummary {
            tier: t.tier,
            name: t.name.to_string(),
            min_relay_wei: t.min_relay_wei.to_string(),
            fee_discount_bps: t.fee_discount_bps,
            x402_bypass: t.x402_bypass,
        })
        .collect();

    let response = RelayUtilityResponse {
        chain_id: state.config.base_chain_id,
        token: TokenSummary {
            address: token_address.to_ascii_lowercase(),
            total_supply_hex,
            decimals,
        },
        staking: StakingSummary {
            address: staking_address.to_ascii_lowercase(),
            total_staked_hex,
            tiers,
            x402_bypass_tier: staking::X402_BYPASS_TIER,
        },
        reward_distributor: RewardDistributorSummary {
            address: reward_distributor_address.to_ascii_lowercase(),
        },
        flags: UtilityFlags {
            fee_discount: true,
            x402_discount: state.config.x402_enabled,
            staking_rewards: true,
            agent_rewards: true,
            creator_rewards: true,
            governance: false,
        },
        source: "relay44-api".to_string(),
        updated_at: Utc::now().to_rfc3339(),
    };

    Ok(HttpResponse::Ok().json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_evm_addresses() {
        assert!(is_valid_evm_address("0x580fF5Ae64eC792A949c6123386A8A936c7EBB07"));
        assert!(is_valid_evm_address(
            "0x0000000000000000000000000000000000000000"
        ));
        assert!(!is_valid_evm_address(""));
        assert!(!is_valid_evm_address("0x"));
        assert!(!is_valid_evm_address("0x123"));
        assert!(!is_valid_evm_address(
            "580fF5Ae64eC792A949c6123386A8A936c7EBB07"
        )); // missing 0x
        assert!(!is_valid_evm_address(
            "0x580fF5Ae64eC792A949c6123386A8A936c7EBBZZ"
        )); // non-hex
    }

    #[test]
    fn parses_u8_hex_values() {
        assert_eq!(parse_u8_hex("0x00").unwrap(), 0);
        assert_eq!(
            parse_u8_hex("0x0000000000000000000000000000000000000000000000000000000000000012")
                .unwrap(),
            18
        );
        assert_eq!(parse_u8_hex("0x12").unwrap(), 18);
        assert_eq!(parse_u8_hex("0xff").unwrap(), 255);
        assert!(parse_u8_hex("0xnotahex").is_err());
    }

    #[test]
    fn utility_response_includes_all_four_tiers() {
        let tiers: Vec<StakingTierSummary> = staking::TIERS
            .iter()
            .map(|t| StakingTierSummary {
                tier: t.tier,
                name: t.name.to_string(),
                min_relay_wei: t.min_relay_wei.to_string(),
                fee_discount_bps: t.fee_discount_bps,
                x402_bypass: t.x402_bypass,
            })
            .collect();
        assert_eq!(tiers.len(), 4);
        assert_eq!(tiers[0].name, "Bronze");
        assert_eq!(tiers[3].name, "Diamond");
        assert_eq!(tiers[3].fee_discount_bps, 7_500);
    }
}
