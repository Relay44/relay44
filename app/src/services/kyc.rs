use anyhow::Result;
use log::{info, warn};
use serde::{Deserialize, Serialize};

const WORLD_ID_VERIFY_URL: &str = "https://developer.worldcoin.org/api/v2/verify";

#[derive(Debug, Clone)]
pub struct KycConfig {
    pub enabled: bool,
    pub world_id_app_id: String,
    pub world_id_action_id: String,
    pub unverified_max_position_usdc: f64,
    pub verified_max_position_usdc: f64,
}

impl KycConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("KYC_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true",
            world_id_app_id: std::env::var("WORLD_ID_APP_ID").unwrap_or_default(),
            world_id_action_id: std::env::var("WORLD_ID_ACTION_ID")
                .unwrap_or_else(|_| "verify-relay44".to_string()),
            unverified_max_position_usdc: std::env::var("KYC_UNVERIFIED_MAX_POSITION_USDC")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .unwrap_or(1_000.0),
            verified_max_position_usdc: std::env::var("KYC_VERIFIED_MAX_POSITION_USDC")
                .unwrap_or_else(|_| "100000".to_string())
                .parse()
                .unwrap_or(100_000.0),
        }
    }
}

pub struct KycService {
    client: reqwest::Client,
    config: KycConfig,
}

#[derive(Serialize)]
struct WorldIdVerifyRequest<'a> {
    merkle_root: &'a str,
    nullifier_hash: &'a str,
    proof: &'a str,
    action: &'a str,
    signal: &'a str,
}

#[derive(Deserialize)]
pub struct WorldIdVerifyResponse {
    pub success: bool,
    #[serde(default)]
    pub nullifier_hash: String,
}

impl KycService {
    pub fn new(config: KycConfig) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            config,
        }
    }

    pub fn config(&self) -> &KycConfig {
        &self.config
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn position_limit_usdc(&self, kyc_tier: u8) -> f64 {
        if kyc_tier >= 2 {
            self.config.verified_max_position_usdc
        } else {
            self.config.unverified_max_position_usdc
        }
    }

    /// Verify a World ID proof via the World ID Developer API.
    /// Returns Ok(nullifier_hash) on success.
    pub async fn verify_world_id(
        &self,
        merkle_root: &str,
        nullifier_hash: &str,
        proof: &str,
        signal: &str,
    ) -> Result<String> {
        if !self.config.enabled {
            anyhow::bail!("KYC verification is not enabled");
        }

        if self.config.world_id_app_id.is_empty() {
            anyhow::bail!("WORLD_ID_APP_ID is not configured");
        }

        let url = format!("{}/{}", WORLD_ID_VERIFY_URL, self.config.world_id_app_id);

        let body = WorldIdVerifyRequest {
            merkle_root,
            nullifier_hash,
            proof,
            action: &self.config.world_id_action_id,
            signal,
        };

        let resp = self.client.post(&url).json(&body).send().await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            warn!("World ID verification failed ({}): {}", status, text);
            anyhow::bail!("World ID verification failed: {}", text);
        }

        let result: WorldIdVerifyResponse = resp.json().await?;
        if !result.success {
            anyhow::bail!("World ID proof verification returned success=false");
        }

        info!(
            "World ID verification succeeded for nullifier {}",
            nullifier_hash
        );
        Ok(result.nullifier_hash)
    }
}
