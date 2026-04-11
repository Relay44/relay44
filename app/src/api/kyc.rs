use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::api::auth::extract_jwt_user;
use crate::api::ApiError;
use crate::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyKycRequest {
    pub merkle_root: String,
    pub nullifier_hash: String,
    pub proof: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyKycResponse {
    pub wallet: String,
    pub tier: u8,
    pub provider: String,
    pub verified: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KycStatusResponse {
    pub wallet: String,
    pub tier: u8,
    pub provider: Option<String>,
    pub verified_at: Option<String>,
    pub enabled: bool,
}

pub async fn verify_kyc(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<VerifyKycRequest>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;

    if !state.kyc.is_enabled() {
        return Err(ApiError::bad_request(
            "KYC_DISABLED",
            "KYC verification is not enabled on this instance",
        ));
    }

    let wallet = user.wallet_address.to_ascii_lowercase();

    // Check if already verified
    let existing = state
        .db
        .get_user_kyc_tier(&wallet)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;
    if existing >= 2 {
        return Ok(HttpResponse::Ok().json(VerifyKycResponse {
            wallet,
            tier: existing,
            provider: "world_id".to_string(),
            verified: true,
        }));
    }

    // Verify with World ID
    let verified_nullifier = state
        .kyc
        .verify_world_id(
            body.merkle_root.trim(),
            body.nullifier_hash.trim(),
            body.proof.trim(),
            &wallet,
        )
        .await
        .map_err(|err| ApiError::bad_request("VERIFICATION_FAILED", &err.to_string()))?;

    // Hash the proof for audit storage (don't store raw ZK proofs)
    let proof_hash = {
        let mut hasher = Sha256::new();
        hasher.update(body.proof.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    // Record verification in DB (nullifier uniqueness enforced at DB level)
    let result = state
        .db
        .insert_kyc_verification(
            &wallet,
            "world_id",
            &verified_nullifier,
            &proof_hash,
            Some(body.merkle_root.trim()),
            Some(&state.kyc.config().world_id_action_id),
            Some(&wallet),
            2,
        )
        .await;

    match result {
        Ok(_) => {}
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("idx_kyc_verif_nullifier") || msg.contains("duplicate key") {
                return Err(ApiError::bad_request(
                    "NULLIFIER_ALREADY_USED",
                    "This World ID has already been used to verify another wallet",
                ));
            }
            return Err(ApiError::internal(&msg));
        }
    }

    // Update user's KYC tier
    state
        .db
        .update_user_kyc_tier(&wallet, 2, "world_id")
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(VerifyKycResponse {
        wallet,
        tier: 2,
        provider: "world_id".to_string(),
        verified: true,
    }))
}

pub async fn get_kyc_status(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = extract_jwt_user(&req, &state)?;
    let wallet = user.wallet_address.to_ascii_lowercase();

    let tier = state
        .db
        .get_user_kyc_tier(&wallet)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    let verification = state
        .db
        .get_latest_kyc_verification(&wallet)
        .await
        .map_err(|err| ApiError::internal(&err.to_string()))?;

    Ok(HttpResponse::Ok().json(KycStatusResponse {
        wallet,
        tier,
        provider: verification.as_ref().map(|v| v.provider.clone()),
        verified_at: verification
            .as_ref()
            .and_then(|v| v.confirmed_at.as_ref())
            .map(|dt| dt.to_rfc3339()),
        enabled: state.kyc.is_enabled(),
    }))
}
