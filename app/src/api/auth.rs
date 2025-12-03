use crate::api::{
    check_auth_rate_limit,
    jwt::{TokenPair, UserRole},
    ApiError,
};
use crate::services::database::ComplianceDecisionEntry;
use crate::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use base64::Engine;
use ed25519_dalek::{Signature as Ed25519Signature, Verifier, VerifyingKey};
use http::uri::Authority;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use siwe::VerificationOpts;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;

const MESSAGE_EXPIRATION_SECS: u64 = 300;
const NONCE_CLEANUP_AGE_SECS: u64 = 600;
const EVM_ADDRESS_LEN: usize = 42;
const SOLANA_PUBKEY_BYTES_LEN: usize = 32;
const SOLANA_SIGNATURE_BYTES_LEN: usize = 64;

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub wallet_address: String,
}

pub async fn extract_authenticated_user(
    req: &HttpRequest,
    state: &web::Data<Arc<AppState>>,
) -> Result<AuthenticatedUser, ApiError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("Missing Authorization header"))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(ApiError::unauthorized(
            "Invalid Authorization header format",
        ));
    }

    let token = &auth_header[7..];
    let claims = state.jwt.validate_token(token)?;

    let revoked = state
        .redis
        .is_token_revoked(&claims.jti)
        .await
        .map_err(|e| {
            log::error!("Token revocation check failed: {}", e);
            ApiError::internal("Authentication validation failed")
        })?;

    if revoked {
        return Err(ApiError::unauthorized("Token revoked"));
    }

    let wallet = claims.sub.to_ascii_lowercase();
    if is_write_method(req.method())
        && state
            .config
            .sanctions_blocked_addresses
            .iter()
            .any(|blocked| blocked == &wallet)
    {
        let request_id = req
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok());
        let route = req.path().to_string();
        let method = req.method().as_str().to_string();
        let metadata = serde_json::json!({
            "source": "sanctions_blocked_addresses",
        });
        let decision = ComplianceDecisionEntry {
            request_id,
            wallet: Some(wallet.as_str()),
            country_code: None,
            action: "write",
            route: route.as_str(),
            method: method.as_str(),
            decision: "deny",
            reason_code: "SANCTIONS_BLOCKED",
            metadata,
        };
        let _ = state.db.record_compliance_decision(&decision).await;

        return Err(ApiError::forbidden(
            "wallet is restricted under sanctions policy",
        ));
    }

    Ok(AuthenticatedUser {
        wallet_address: wallet,
    })
}

fn is_write_method(method: &actix_web::http::Method) -> bool {
    matches!(
        *method,
        actix_web::http::Method::POST
            | actix_web::http::Method::PUT
            | actix_web::http::Method::PATCH
            | actix_web::http::Method::DELETE
    )
}

async fn check_and_record_nonce_redis(
    redis: &crate::services::RedisService,
    prefix: &str,
    nonce: &str,
) -> Result<(), ApiError> {
    let scoped_nonce = format!("{}:{}", prefix, nonce);
    let was_new = redis
        .check_and_record_nonce(&scoped_nonce, NONCE_CLEANUP_AGE_SECS)
        .await
        .map_err(|e| {
            log::error!("Redis nonce check failed: {}", e);
            ApiError::internal("Nonce verification failed")
        })?;

    if !was_new {
        log::warn!("Replay attack detected: nonce {} already used", nonce);
        return Err(ApiError::unauthorized(
            "Nonce already used (possible replay attack)",
        ));
    }

    Ok(())
}

async fn check_and_record_siwe_nonce_redis(
    redis: &crate::services::RedisService,
    nonce: &str,
) -> Result<(), ApiError> {
    check_and_record_nonce_redis(redis, "siwe", nonce).await
}

async fn check_and_record_solana_nonce_redis(
    redis: &crate::services::RedisService,
    nonce: &str,
) -> Result<(), ApiError> {
    check_and_record_nonce_redis(redis, "solana", nonce).await
}

async fn check_and_record_farcaster_nonce_redis(
    redis: &crate::services::RedisService,
    nonce: &str,
) -> Result<(), ApiError> {
    check_and_record_nonce_redis(redis, "farcaster", nonce).await
}

pub fn generate_nonce() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let random_part: u64 = rand::random();

    format!("{:x}{:016x}", timestamp, random_part)
}

#[macro_export]
macro_rules! require_auth {
    ($req:expr, $state:expr) => {
        $crate::api::auth::extract_authenticated_user($req, $state).await?
    };
}

#[derive(Serialize)]
pub struct NonceResponse {
    pub nonce: String,
    pub expires_at: u64,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Deserialize)]
pub struct SiweLoginRequest {
    pub wallet: String,
    pub message: String,
    pub signature: String,
}

#[derive(Deserialize)]
pub struct SolanaLoginRequest {
    pub wallet: String,
    pub message: String,
    pub signature: String,
}

#[derive(Deserialize)]
pub struct FarcasterLoginRequest {
    pub message: String,
    pub signature: String,
    pub nonce: String,
}

pub async fn get_nonce() -> Result<HttpResponse, ApiError> {
    let nonce = generate_nonce();
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApiError::internal("System time error"))?
        .as_secs()
        + MESSAGE_EXPIRATION_SECS;

    Ok(HttpResponse::Ok().json(NonceResponse { nonce, expires_at }))
}

pub async fn get_siwe_nonce() -> Result<HttpResponse, ApiError> {
    get_nonce().await
}

pub async fn get_solana_nonce() -> Result<HttpResponse, ApiError> {
    get_nonce().await
}

pub async fn login(
    _http_req: HttpRequest,
    _state: web::Data<Arc<AppState>>,
    _body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, ApiError> {
    Err(ApiError::bad_request(
        "LEGACY_AUTH_REMOVED",
        "Use /v1/auth/siwe/login",
    ))
}

pub async fn siwe_login(
    http_req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<SiweLoginRequest>,
) -> Result<HttpResponse, ApiError> {
    if !state.config.evm_enabled {
        return Err(ApiError::bad_request(
            "EVM_DISABLED",
            "EVM auth is disabled",
        ));
    }

    check_auth_rate_limit(&http_req, &state.redis).await?;

    let req = body.into_inner();
    validate_evm_address(&req.wallet)?;
    let wallet = normalize_evm_address(&req.wallet);

    let message: siwe::Message = req.message.parse().map_err(|e| {
        ApiError::bad_request(
            "INVALID_SIWE_MESSAGE",
            &format!("Invalid SIWE message: {}", e),
        )
    })?;

    let message_address = format!("0x{}", hex::encode(message.address));
    if wallet != message_address {
        return Err(ApiError::unauthorized(
            "Wallet address mismatch in SIWE message",
        ));
    }

    if message.chain_id != state.config.base_chain_id {
        return Err(ApiError::bad_request(
            "INVALID_CHAIN_ID",
            "SIWE message chain ID does not match configured Base chain",
        ));
    }

    let expected_domain: Authority = state
        .config
        .siwe_domain
        .parse()
        .map_err(|_| ApiError::internal("Invalid SIWE domain configuration"))?;
    if message.domain != expected_domain {
        return Err(ApiError::unauthorized("SIWE domain mismatch"));
    }

    check_and_record_siwe_nonce_redis(&state.redis, &message.nonce).await?;

    let signature = decode_hex_signature(&req.signature)?;

    let opts = VerificationOpts {
        domain: Some(expected_domain.clone()),
        nonce: Some(message.nonce.clone()),
        timestamp: Some(OffsetDateTime::now_utc()),
    };

    message
        .verify(&signature, &opts)
        .await
        .map_err(|e| ApiError::unauthorized(&format!("SIWE verification failed: {}", e)))?;

    let role = determine_user_role(&wallet, &state).await;
    let access_token = state.jwt.generate_access_token(&wallet, role)?;
    let refresh_token = state.jwt.generate_refresh_token(&wallet, role)?;

    log::info!("SIWE user logged in: {} with role {:?}", wallet, role);

    Ok(HttpResponse::Ok().json(TokenPair::new(access_token, refresh_token)))
}

pub async fn solana_login(
    http_req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<SolanaLoginRequest>,
) -> Result<HttpResponse, ApiError> {
    if !state.config.solana_enabled {
        return Err(ApiError::bad_request(
            "SOLANA_DISABLED",
            "Solana auth is disabled",
        ));
    }

    check_auth_rate_limit(&http_req, &state.redis).await?;

    let req = body.into_inner();
    let wallet = normalize_solana_address(req.wallet.as_str())?;
    let message_nonce = validate_solana_signin_message(
        req.message.as_str(),
        wallet.as_str(),
        state.config.siwe_domain.as_str(),
    )?;
    let signature = decode_solana_signature(req.signature.as_str())?;
    let verifying_key = decode_solana_public_key(wallet.as_str())?;

    verifying_key
        .verify(req.message.as_bytes(), &signature)
        .map_err(|_| ApiError::unauthorized("Solana signature verification failed"))?;

    check_and_record_solana_nonce_redis(&state.redis, message_nonce.as_str()).await?;

    let role = determine_user_role(wallet.as_str(), &state).await;
    let access_token = state.jwt.generate_access_token(wallet.as_str(), role)?;
    let refresh_token = state.jwt.generate_refresh_token(wallet.as_str(), role)?;

    log::info!("Solana user logged in: {} with role {:?}", wallet, role);

    Ok(HttpResponse::Ok().json(TokenPair::new(access_token, refresh_token)))
}

pub async fn get_farcaster_nonce() -> Result<HttpResponse, ApiError> {
    get_nonce().await
}

/// Sign In With Farcaster (SIWF) login.
///
/// The Farcaster Mini App SDK returns a SIWE-compatible message signed by the
/// user's custody or auth address. We parse and verify it using the `siwe`
/// crate, then issue a JWT keyed to the recovered wallet address.
pub async fn farcaster_login(
    http_req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<FarcasterLoginRequest>,
) -> Result<HttpResponse, ApiError> {
    check_auth_rate_limit(&http_req, &state.redis).await?;

    let req = body.into_inner();

    // The SIWF message is EIP-4361 (SIWE) formatted
    let message: siwe::Message = req.message.parse().map_err(|e| {
        ApiError::bad_request(
            "INVALID_FARCASTER_MESSAGE",
            &format!("Invalid SIWF message: {}", e),
        )
    })?;

    // Extract the signer's wallet address from the parsed SIWE message
    let wallet_address = format!("0x{}", hex::encode(message.address));

    // Verify the nonce matches what we issued (replay protection)
    if message.nonce != req.nonce {
        return Err(ApiError::unauthorized("Farcaster nonce mismatch"));
    }
    check_and_record_farcaster_nonce_redis(&state.redis, &message.nonce).await?;

    // Verify the signature against the message
    let signature = decode_hex_signature(&req.signature)?;
    let opts = VerificationOpts {
        domain: None, // SIWF messages use the Farcaster client domain, not ours
        nonce: Some(message.nonce.clone()),
        timestamp: Some(OffsetDateTime::now_utc()),
    };

    message
        .verify(&signature, &opts)
        .await
        .map_err(|e| ApiError::unauthorized(&format!("SIWF verification failed: {}", e)))?;

    let wallet = wallet_address.to_ascii_lowercase();
    let role = determine_user_role(&wallet, &state).await;
    let access_token = state.jwt.generate_access_token(&wallet, role)?;
    let refresh_token = state.jwt.generate_refresh_token(&wallet, role)?;

    log::info!("Farcaster user logged in: {} with role {:?}", wallet, role);

    Ok(HttpResponse::Ok().json(TokenPair::new(access_token, refresh_token)))
}

pub async fn refresh_token(
    http_req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<RefreshRequest>,
) -> Result<HttpResponse, ApiError> {
    check_auth_rate_limit(&http_req, &state.redis).await?;

    let req = body.into_inner();
    let claims = state.jwt.validate_token(&req.refresh_token)?;

    let access_token = state.jwt.generate_access_token(&claims.sub, claims.role)?;
    let refresh_token = state.jwt.generate_refresh_token(&claims.sub, claims.role)?;

    log::info!("Tokens refreshed for user: {}", claims.sub);

    Ok(HttpResponse::Ok().json(TokenPair::new(access_token, refresh_token)))
}

pub async fn logout(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<HttpResponse, ApiError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    if let Some(header) = auth_header {
        if let Some(token) = header.strip_prefix("Bearer ") {
            if let Ok(claims) = state.jwt.validate_token(token) {
                log::info!("User logged out: {}", claims.sub);

                if let Err(e) = state.redis.revoke_token(&claims.jti, claims.exp).await {
                    log::warn!("Failed to add token to revocation list: {}", e);
                }
            }
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Logged out successfully"
    })))
}

#[allow(dead_code)]
pub async fn logout_all(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<HttpResponse, ApiError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    if let Some(header) = auth_header {
        if let Some(token) = header.strip_prefix("Bearer ") {
            if let Ok(claims) = state.jwt.validate_token(token) {
                log::info!("User {} logging out from all devices", claims.sub);

                if let Err(e) = state.redis.revoke_all_user_tokens(&claims.sub).await {
                    log::error!("Failed to revoke all tokens: {}", e);
                    return Err(ApiError::internal("Failed to logout from all devices"));
                }

                return Ok(HttpResponse::Ok().json(serde_json::json!({
                    "message": "Logged out from all devices successfully"
                })));
            }
        }
    }

    Err(ApiError::unauthorized("Invalid or missing token"))
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AuthenticatedUserWithRole {
    pub wallet_address: String,
    pub role: UserRole,
}

pub fn extract_jwt_user(
    req: &HttpRequest,
    state: &web::Data<Arc<AppState>>,
) -> Result<AuthenticatedUserWithRole, ApiError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("Missing Authorization header"))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(ApiError::unauthorized(
            "Invalid Authorization header format",
        ));
    }

    let token = &auth_header[7..];
    let claims = state.jwt.validate_token(token)?;

    Ok(AuthenticatedUserWithRole {
        wallet_address: claims.sub,
        role: claims.role,
    })
}

async fn determine_user_role(wallet: &str, _state: &web::Data<Arc<AppState>>) -> UserRole {
    determine_user_role_from_allowlists(wallet, &_state.config.admin_wallets)
}

fn determine_user_role_from_allowlists(wallet: &str, admin_wallets: &[String]) -> UserRole {
    let normalized = wallet.trim().to_ascii_lowercase();
    if admin_wallets.iter().any(|entry| entry == &normalized) {
        UserRole::Admin
    } else {
        UserRole::User
    }
}

fn normalize_solana_address(wallet: &str) -> Result<String, ApiError> {
    let trimmed = wallet.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "Wallet is required",
        ));
    }

    let bytes = bs58::decode(trimmed)
        .into_vec()
        .map_err(|_| ApiError::bad_request("INVALID_WALLET", "Invalid Solana wallet format"))?;
    if bytes.len() != SOLANA_PUBKEY_BYTES_LEN {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "Invalid Solana wallet length",
        ));
    }

    Ok(trimmed.to_string())
}

fn decode_solana_public_key(wallet: &str) -> Result<VerifyingKey, ApiError> {
    let bytes = bs58::decode(wallet)
        .into_vec()
        .map_err(|_| ApiError::bad_request("INVALID_WALLET", "Invalid Solana wallet format"))?;
    let key_bytes: [u8; SOLANA_PUBKEY_BYTES_LEN] = bytes.try_into().map_err(|_| {
        ApiError::bad_request("INVALID_WALLET", "Invalid Solana wallet public key bytes")
    })?;

    VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| ApiError::bad_request("INVALID_WALLET", "Invalid Solana wallet public key"))
}

fn decode_solana_signature(signature: &str) -> Result<Ed25519Signature, ApiError> {
    let trimmed = signature.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_SIGNATURE",
            "signature is required",
        ));
    }

    let decoded = if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(trimmed) {
        bytes
    } else {
        bs58::decode(trimmed).into_vec().map_err(|_| {
            ApiError::bad_request("INVALID_SIGNATURE", "signature must be base64 or base58")
        })?
    };

    if decoded.len() != SOLANA_SIGNATURE_BYTES_LEN {
        return Err(ApiError::bad_request(
            "INVALID_SIGNATURE",
            "signature must be 64-byte Ed25519 signature",
        ));
    }

    Ed25519Signature::from_slice(&decoded)
        .map_err(|_| ApiError::bad_request("INVALID_SIGNATURE", "invalid signature bytes"))
}

fn message_line_value<'a>(message: &'a str, label: &str) -> Option<&'a str> {
    message
        .lines()
        .find_map(|line| line.strip_prefix(label).map(str::trim))
}

fn validate_solana_signin_message(
    message: &str,
    wallet: &str,
    expected_domain: &str,
) -> Result<String, ApiError> {
    if message.len() > 4096 {
        return Err(ApiError::bad_request(
            "INVALID_MESSAGE",
            "Solana signin message too large",
        ));
    }

    let lines = message.lines().collect::<Vec<_>>();
    if lines.len() < 8 {
        return Err(ApiError::bad_request(
            "INVALID_MESSAGE",
            "Malformed Solana signin message",
        ));
    }

    let expected_prefix = format!(
        "{} wants you to sign in with your Solana account:",
        expected_domain
    );
    if lines[0].trim() != expected_prefix {
        return Err(ApiError::unauthorized("Solana signin domain mismatch"));
    }
    if lines[1].trim() != wallet {
        return Err(ApiError::unauthorized(
            "Wallet address mismatch in Solana signin message",
        ));
    }
    if !message.contains("Sign in to relay44") {
        return Err(ApiError::bad_request(
            "INVALID_MESSAGE",
            "Missing Solana signin statement",
        ));
    }

    if let Some(chain) = message_line_value(message, "Chain:") {
        if chain.to_ascii_lowercase() != "solana" {
            return Err(ApiError::bad_request(
                "INVALID_MESSAGE",
                "Chain must be solana in Solana signin message",
            ));
        }
    }

    let nonce = message_line_value(message, "Nonce:").ok_or_else(|| {
        ApiError::bad_request("INVALID_MESSAGE", "Nonce missing in signin message")
    })?;
    if nonce.is_empty() || nonce.len() > 128 || !nonce.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request(
            "INVALID_MESSAGE",
            "Invalid nonce in Solana signin message",
        ));
    }

    let issued_at = message_line_value(message, "Issued At:").ok_or_else(|| {
        ApiError::bad_request("INVALID_MESSAGE", "Issued At missing in signin message")
    })?;
    let issued_at =
        OffsetDateTime::parse(issued_at, &time::format_description::well_known::Rfc3339)
            .map_err(|_| ApiError::bad_request("INVALID_MESSAGE", "Invalid Issued At timestamp"))?;
    let now = OffsetDateTime::now_utc();
    if issued_at > now + time::Duration::seconds(30) {
        return Err(ApiError::bad_request(
            "INVALID_MESSAGE",
            "Issued At cannot be in the future",
        ));
    }
