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

    let eoa_result = message.verify(&signature, &opts).await;

    if eoa_result.is_err() && is_erc6492_signature(&signature) {
        verify_erc6492_signature(
            &state.config.base_rpc_url,
            &wallet,
            &req.message,
            &signature,
        )
        .await
        .map_err(|e| ApiError::unauthorized(&format!("ERC-6492 verification failed: {}", e)))?;
    } else {
        eoa_result
            .map_err(|e| ApiError::unauthorized(&format!("SIWE verification failed: {}", e)))?;
    }

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
    if now - issued_at > time::Duration::seconds(MESSAGE_EXPIRATION_SECS as i64) {
        return Err(ApiError::unauthorized("Solana signin message expired"));
    }

    Ok(nonce.to_string())
}

fn validate_evm_address(address: &str) -> Result<(), ApiError> {
    if address.len() != EVM_ADDRESS_LEN || !address.starts_with("0x") {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "Invalid EVM wallet address format",
        ));
    }

    if !address[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "Invalid EVM wallet address format",
        ));
    }

    if !is_eip55_checksum(address) {
        return Err(ApiError::bad_request(
            "INVALID_WALLET",
            "EVM wallet address must use EIP-55 checksum casing",
        ));
    }

    Ok(())
}

fn normalize_evm_address(address: &str) -> String {
    address.to_ascii_lowercase()
}

fn is_eip55_checksum(address: &str) -> bool {
    if address.len() != EVM_ADDRESS_LEN || !address.starts_with("0x") {
        return false;
    }

    let hex_part = &address[2..];
    let lower = hex_part.to_ascii_lowercase();

    if hex_part == lower || hex_part == lower.to_ascii_uppercase() {
        return false;
    }

    let mut hasher = Keccak256::new();
    hasher.update(lower.as_bytes());
    let hash = hasher.finalize();

    for (idx, ch) in hex_part.chars().enumerate() {
        if ch.is_ascii_digit() {
            continue;
        }

        let hash_byte = hash[idx / 2];
        let nibble = if idx % 2 == 0 {
            hash_byte >> 4
        } else {
            hash_byte & 0x0f
        };

        if nibble >= 8 && !ch.is_ascii_uppercase() {
            return false;
        }

        if nibble < 8 && !ch.is_ascii_lowercase() {
            return false;
        }
    }

    true
}

fn decode_hex_signature(signature: &str) -> Result<Vec<u8>, ApiError> {
    let sig = signature.strip_prefix("0x").unwrap_or(signature);

    hex::decode(sig)
        .map_err(|_| ApiError::bad_request("INVALID_SIGNATURE", "Signature must be valid hex"))
}

const ERC6492_MAGIC_SUFFIX: [u8; 32] = [
    0x64, 0x92, 0x64, 0x92, 0x64, 0x92, 0x64, 0x92, 0x64, 0x92, 0x64, 0x92,
    0x64, 0x92, 0x64, 0x92, 0x64, 0x92, 0x64, 0x92, 0x64, 0x92, 0x64, 0x92,
    0x64, 0x92, 0x64, 0x92, 0x64, 0x92, 0x64, 0x92,
];

fn is_erc6492_signature(signature: &[u8]) -> bool {
    signature.len() > 32 && signature[signature.len() - 32..] == ERC6492_MAGIC_SUFFIX
}

/// Verify an ERC-6492 wrapped signature via eth_call to the UniversalSigValidator.
/// Reference: https://eips.ethereum.org/EIPS/eip-6492
async fn verify_erc6492_signature(
    rpc_url: &str,
    signer: &str,
    message: &str,
    signature: &[u8],
) -> Result<(), String> {
    // UniversalSigValidator deployed on Base
    let validator = "0x82b554b0eEb9BC7A4dE1a9C01890015e14cB398e";

    let message_hash = {
        let mut hasher = Keccak256::new();
        let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
        hasher.update(prefix.as_bytes());
        hasher.update(message.as_bytes());
        hasher.finalize()
    };

    // ABI encode: isValidSig(address,bytes32,bytes)
    // selector: 0x6ccea652
    let selector = hex::decode("6ccea652").unwrap();
    let signer_addr = hex::decode(signer.strip_prefix("0x").unwrap_or(signer))
        .map_err(|_| "invalid signer address")?;

    let mut calldata = selector;
    // address param (padded to 32 bytes)
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(&signer_addr);
    // bytes32 hash
    calldata.extend_from_slice(&message_hash);
    // offset to bytes (dynamic) = 96
    calldata.extend_from_slice(&[0u8; 31]);
    calldata.push(96);
    // length of signature bytes
    let sig_len = signature.len();
    let mut len_bytes = [0u8; 32];
    len_bytes[28..32].copy_from_slice(&(sig_len as u32).to_be_bytes());
    calldata.extend_from_slice(&len_bytes);
    // signature data (padded to 32-byte boundary)
    calldata.extend_from_slice(signature);
    let padding = (32 - (sig_len % 32)) % 32;
    calldata.extend_from_slice(&vec![0u8; padding]);

    let calldata_hex = format!("0x{}", hex::encode(&calldata));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client build failed: {}", e))?;
    let resp = client
        .post(rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": validator,
                "data": calldata_hex
            }, "latest"],
            "id": 1
        }))
        .send()
        .await
        .map_err(|e| format!("RPC request failed: {}", e))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("RPC response parse failed: {}", e))?;

    if let Some(error) = body.get("error") {
        return Err(format!("RPC error: {}", error));
    }

    let result = body["result"]
        .as_str()
        .ok_or("missing result in RPC response")?;

    let result_bytes =
        hex::decode(result.strip_prefix("0x").unwrap_or(result)).map_err(|_| "invalid result hex")?;

    // isValidSig returns a bool — expect 32 bytes with the last byte == 1
    if result_bytes.len() < 32 || result_bytes[31] != 1 {
        return Err("smart wallet signature verification failed".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_solana_signin_message(
        domain: &str,
        wallet: &str,
        nonce: &str,
        issued_at: OffsetDateTime,
    ) -> String {
        let issued_at = issued_at
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        format!(
            "{domain} wants you to sign in with your Solana account:\n{wallet}\n\nSign in to relay44\n\nURI: https://{domain}\nVersion: 1\nChain: solana\nNonce: {nonce}\nIssued At: {issued_at}"
        )
    }

    #[test]
    fn test_validate_evm_address_valid() {
        let valid = "0x71C7656EC7ab88b098defB751B7401B5f6d8976F";
        assert!(validate_evm_address(valid).is_ok());
    }

    #[test]
    fn test_validate_evm_address_invalid() {
        assert!(validate_evm_address("0x123").is_err());
        assert!(validate_evm_address("71C7656EC7ab88b098defB751B7401B5f6d8976F").is_err());
        assert!(validate_evm_address("0xZZC7656EC7ab88b098defB751B7401B5f6d8976F").is_err());
        assert!(validate_evm_address("0x71c7656ec7ab88b098defb751b7401b5f6d8976f").is_err());
    }

    #[test]
    fn test_decode_hex_signature() {
        let sig = format!("0x{}", "11".repeat(65));
        let decoded = decode_hex_signature(&sig).unwrap();
        assert_eq!(decoded.len(), 65);
    }

    #[test]
    fn test_generate_nonce_uniqueness() {
        let nonce1 = generate_nonce();
        let nonce2 = generate_nonce();

        assert_ne!(nonce1, nonce2);
        assert!(nonce1.len() >= 32);
    }

    #[test]
    fn test_generate_nonce_format() {
        let nonce = generate_nonce();
        assert!(nonce.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_authenticated_user_struct() {
        let user = AuthenticatedUser {
            wallet_address: "0xabc".to_string(),
        };
        assert_eq!(user.wallet_address, "0xabc");
    }

    #[test]
    fn test_message_expiration_constant() {
        assert_eq!(MESSAGE_EXPIRATION_SECS, 300);
    }

    #[test]
    fn test_nonce_cleanup_age_constant() {
        assert_eq!(NONCE_CLEANUP_AGE_SECS, 600);
    }

    #[test]
    fn test_generate_multiple_nonces_all_unique() {
        use std::collections::HashSet;
        let nonces: HashSet<String> = (0..100).map(|_| generate_nonce()).collect();
        assert_eq!(nonces.len(), 100, "All generated nonces should be unique");
    }

    #[test]
    fn test_validate_solana_signin_message_valid() {
        let domain = "localhost:3000";
        let wallet = "11111111111111111111111111111111";
        let nonce = "abc123ef45";
        let message = build_solana_signin_message(domain, wallet, nonce, OffsetDateTime::now_utc());
        let parsed =
            validate_solana_signin_message(message.as_str(), wallet, domain).expect("valid signin");
        assert_eq!(parsed, nonce);
    }

    #[test]
    fn test_validate_solana_signin_message_rejects_domain_mismatch() {
        let wallet = "11111111111111111111111111111111";
        let message = build_solana_signin_message(
            "localhost:3000",
            wallet,
            "abc123ef45",
            OffsetDateTime::now_utc(),
        );
        assert!(validate_solana_signin_message(message.as_str(), wallet, "example.com").is_err());
    }

    #[test]
    fn test_validate_solana_signin_message_rejects_expired_timestamp() {
        let domain = "localhost:3000";
        let wallet = "11111111111111111111111111111111";
        let message = build_solana_signin_message(
            domain,
            wallet,
            "abc123ef45",
            OffsetDateTime::now_utc() - time::Duration::seconds(601),
        );
        assert!(validate_solana_signin_message(message.as_str(), wallet, domain).is_err());
    }

    #[test]
    fn test_determine_user_role_from_allowlists_grants_admin() {
        let role = determine_user_role_from_allowlists(
            "0x1111111111111111111111111111111111111111",
            &["0x1111111111111111111111111111111111111111".to_string()],
        );

        assert_eq!(role, UserRole::Admin);
    }

    #[test]
    fn test_determine_user_role_from_allowlists_defaults_to_user() {
        let role = determine_user_role_from_allowlists(
            "0x2222222222222222222222222222222222222222",
            &["0x1111111111111111111111111111111111111111".to_string()],
        );

        assert_eq!(role, UserRole::User);
    }
}
