use crate::api::ApiError;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// JWT token claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (wallet address)
    pub sub: String,
    /// Issued at timestamp (Unix seconds)
    pub iat: i64,
    /// Expiration timestamp (Unix seconds)
    pub exp: i64,
    /// User role
    pub role: UserRole,
    /// Token ID for revocation tracking
    pub jti: String,
    /// Audience - the intended recipient of the token
    pub aud: String,
    /// Issuer - who created the token
    pub iss: String,
}

/// Expected audience for newly-issued tokens.
pub const TOKEN_AUDIENCE: &str = "relay44-api";
/// Issuer for newly-issued tokens.
pub const TOKEN_ISSUER: &str = "relay44";
const LEGACY_TOKEN_AUDIENCE: &str = "polyguard-api";
const LEGACY_TOKEN_ISSUER: &str = "polyguard";

/// User roles for RBAC
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    /// Regular user - can trade and manage their own orders/positions
    #[default]
    User,
    /// Keeper - can settle trades and manage order book
    Keeper,
    /// Admin - full access to all operations
    Admin,
}

/// Token expiration times
pub const ACCESS_TOKEN_EXPIRATION_HOURS: i64 = 1;
pub const REFRESH_TOKEN_EXPIRATION_DAYS: i64 = 7;

/// Token type for different purposes
#[derive(Debug, Clone, Copy)]
pub enum TokenType {
    Access,
    Refresh,
}

/// A signing key with its ID for rotation support
struct SigningKey {
    #[allow(dead_code)]
    kid: String,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

/// JWT service for token management with key rotation support
///
/// Supports multiple active keys for graceful rotation:
/// 1. Add new key via `add_key()`
/// 2. Set it as primary via `set_primary_key()`
/// 3. Old tokens continue to validate during grace period
/// 4. Remove old key via `remove_key()` after grace period
pub struct JwtService {
    /// Current primary key ID (used for signing new tokens)
    primary_kid: RwLock<String>,
    /// All valid keys (can validate tokens signed with any of these)
    keys: RwLock<HashMap<String, SigningKey>>,
}

impl JwtService {
    /// Create a new JWT service with the given secret
    pub fn new(secret: &str) -> Self {
        let kid = Self::generate_kid();
        let signing_key = SigningKey {
            kid: kid.clone(),
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
        };

        let mut keys = HashMap::new();
        keys.insert(kid.clone(), signing_key);

        Self {
            primary_kid: RwLock::new(kid),
            keys: RwLock::new(keys),
        }
    }

    /// Generate a unique key ID
    fn generate_kid() -> String {
        format!("k{}_{:08x}", Utc::now().timestamp(), rand::random::<u32>())
    }

    /// Add a new key for rotation. Returns the key ID.
    /// The key is not used for signing until set as primary.
    pub fn add_key(&self, secret: &str) -> String {
        let kid = Self::generate_kid();
        let signing_key = SigningKey {
            kid: kid.clone(),
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
        };

        let mut keys = self.keys.write().unwrap();
        keys.insert(kid.clone(), signing_key);
        log::info!("Added new JWT signing key: {}", kid);
        kid
    }

    /// Set the primary key (used for signing new tokens)
    pub fn set_primary_key(&self, kid: &str) -> Result<(), ApiError> {
        let keys = self.keys.read().unwrap();
        if !keys.contains_key(kid) {
            return Err(ApiError::bad_request("INVALID_KEY_ID", "Key not found"));
        }
        drop(keys);

        let mut primary = self.primary_kid.write().unwrap();
        log::info!("Rotating primary JWT key from {} to {}", *primary, kid);
        *primary = kid.to_string();
        Ok(())
    }

    /// Remove an old key after grace period
    pub fn remove_key(&self, kid: &str) -> Result<(), ApiError> {
        let primary = self.primary_kid.read().unwrap();
        if *primary == kid {
            return Err(ApiError::bad_request(
                "CANNOT_REMOVE_PRIMARY",
                "Cannot remove the primary signing key",
            ));
        }
        drop(primary);

        let mut keys = self.keys.write().unwrap();
        if keys.remove(kid).is_some() {
            log::info!("Removed old JWT signing key: {}", kid);
            Ok(())
        } else {
            Err(ApiError::bad_request("INVALID_KEY_ID", "Key not found"))
        }
    }

    /// Get list of active key IDs
    pub fn list_key_ids(&self) -> Vec<String> {
        let keys = self.keys.read().unwrap();
        keys.keys().cloned().collect()
    }

    /// Get the current primary key ID
    pub fn primary_key_id(&self) -> String {
        self.primary_kid.read().unwrap().clone()
    }

    /// Generate an access token for a user
    pub fn generate_access_token(
        &self,
        wallet_address: &str,
        role: UserRole,
    ) -> Result<String, ApiError> {
        self.generate_token(wallet_address, role, TokenType::Access)
    }

    /// Generate a refresh token for a user
    pub fn generate_refresh_token(
        &self,
        wallet_address: &str,
        role: UserRole,
    ) -> Result<String, ApiError> {
        self.generate_token(wallet_address, role, TokenType::Refresh)
    }

    /// Generate a token of the specified type
    fn generate_token(
        &self,
        wallet_address: &str,
        role: UserRole,
        token_type: TokenType,
    ) -> Result<String, ApiError> {
        let now = Utc::now();

        let expiration = match token_type {
            TokenType::Access => now + Duration::hours(ACCESS_TOKEN_EXPIRATION_HOURS),
            TokenType::Refresh => now + Duration::days(REFRESH_TOKEN_EXPIRATION_DAYS),
        };

        // Generate unique token ID
        let jti = format!(
            "{:x}{:016x}",
            now.timestamp_nanos_opt().unwrap_or(0),
            rand::random::<u64>()
        );

        let claims = Claims {
            sub: wallet_address.to_string(),
            iat: now.timestamp(),
            exp: expiration.timestamp(),
            role,
            jti,
            aud: TOKEN_AUDIENCE.to_string(),
            iss: TOKEN_ISSUER.to_string(),
        };

        // Get primary key for signing
        let primary_kid = self.primary_kid.read().unwrap().clone();
        let keys = self.keys.read().unwrap();
        let signing_key = keys.get(&primary_kid).ok_or_else(|| {
            log::error!("Primary key {} not found", primary_kid);
            ApiError::internal("Signing key not available")
        })?;

        // Include kid in header for key identification during validation
        let header = Header {
            kid: Some(primary_kid),
            ..Header::default()
        };

        encode(&header, &claims, &signing_key.encoding_key).map_err(|e| {
            log::error!("Failed to generate JWT: {}", e);
            ApiError::internal("Failed to generate authentication token")
        })
    }

    /// Validate and decode a token
    pub fn validate_token(&self, token: &str) -> Result<Claims, ApiError> {
        // First, decode header to get kid
        let header = jsonwebtoken::decode_header(token)
            .map_err(|_| ApiError::unauthorized("Invalid token format"))?;

        let keys = self.keys.read().unwrap();

        // If kid specified, use that key; otherwise try all keys
        let decoding_key = if let Some(kid) = &header.kid {
            if let Some(key) = keys.get(kid) {
                &key.decoding_key
            } else {
                let primary_kid = self.primary_kid.read().unwrap().clone();
                log::debug!(
                    "Token signed with unknown key: {}. Falling back to primary key {}",
                    kid, primary_kid
                );
                keys.get(&primary_kid)
                    .map(|key| &key.decoding_key)
                    .ok_or_else(|| ApiError::unauthorized("No valid signing key"))?
            }
        } else {
            // Legacy token without kid - try primary key
            let primary_kid = self.primary_kid.read().unwrap().clone();
            keys.get(&primary_kid)
                .map(|k| &k.decoding_key)
                .ok_or_else(|| ApiError::unauthorized("No valid signing key"))?
        };

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.set_audience(&[TOKEN_AUDIENCE, LEGACY_TOKEN_AUDIENCE]);
        validation.set_issuer(&[TOKEN_ISSUER, LEGACY_TOKEN_ISSUER]);

        decode::<Claims>(token, decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| {
                log::debug!("JWT validation failed: {}", e);
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        ApiError::unauthorized("Token expired")
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidToken => {
                        ApiError::unauthorized("Invalid token format")
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                        ApiError::unauthorized("Invalid token signature")
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                        ApiError::unauthorized("Invalid token audience")
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidIssuer => {
                        ApiError::unauthorized("Invalid token issuer")
                    }
                    _ => ApiError::unauthorized("Invalid token"),
                }
            })
    }
}

/// Token pair response for authentication endpoints
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

impl TokenPair {
    pub fn new(access_token: String, refresh_token: String) -> Self {
        Self {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: ACCESS_TOKEN_EXPIRATION_HOURS * 3600,
        }
    }
}

/// Check if user has required role
#[allow(dead_code)]
pub fn check_role(user_role: UserRole, required_role: UserRole) -> Result<(), ApiError> {
    let has_access = match required_role {
        UserRole::User => true, // Everyone can access user-level resources
        UserRole::Keeper => matches!(user_role, UserRole::Keeper | UserRole::Admin),
        UserRole::Admin => matches!(user_role, UserRole::Admin),
    };

    if has_access {
        Ok(())
    } else {
        Err(ApiError::forbidden("Insufficient permissions"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_jwt_service() -> JwtService {
        JwtService::new("test-secret-key-for-unit-tests-only-12345678")
    }

    #[test]
    fn test_generate_and_validate_access_token() {
        let service = test_jwt_service();
        let wallet = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

        let token = service
            .generate_access_token(wallet, UserRole::User)
            .unwrap();
        let claims = service.validate_token(&token).unwrap();

        assert_eq!(claims.sub, wallet);
        assert_eq!(claims.role, UserRole::User);
    }

    #[test]
    fn test_generate_and_validate_refresh_token() {
        let service = test_jwt_service();
        let wallet = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

        let token = service
            .generate_refresh_token(wallet, UserRole::Admin)
            .unwrap();
        let claims = service.validate_token(&token).unwrap();

        assert_eq!(claims.sub, wallet);
        assert_eq!(claims.role, UserRole::Admin);
    }

    #[test]
    fn test_validate_legacy_audience_and_issuer() {
        let service = test_jwt_service();
        let wallet = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
        let now = Utc::now();
        let claims = Claims {
            sub: wallet.to_string(),
            iat: now.timestamp(),
            exp: (now + Duration::hours(1)).timestamp(),
            role: UserRole::User,
            jti: "legacy-token".to_string(),
            aud: LEGACY_TOKEN_AUDIENCE.to_string(),
            iss: LEGACY_TOKEN_ISSUER.to_string(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret("test-secret-key-for-unit-tests-only-12345678".as_bytes()),
        )
        .unwrap();

        let validated = service.validate_token(&token).unwrap();
        assert_eq!(validated.sub, wallet);
        assert_eq!(validated.aud, LEGACY_TOKEN_AUDIENCE);
        assert_eq!(validated.iss, LEGACY_TOKEN_ISSUER);
    }

    #[test]
    fn test_invalid_token() {
        let service = test_jwt_service();

        let result = service.validate_token("invalid.token.here");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_token_across_restart_with_same_secret() {
        let issuer = JwtService::new("test-secret-key-for-unit-tests-only-12345678");
        let validator = JwtService::new("test-secret-key-for-unit-tests-only-12345678");

        let token = issuer
            .generate_access_token("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU", UserRole::User)
            .unwrap();

        let claims = validator.validate_token(&token).unwrap();
        assert_eq!(claims.role, UserRole::User);
    }

    #[test]
    fn test_tampered_token() {
        let service = test_jwt_service();
        let wallet = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

        let token = service
            .generate_access_token(wallet, UserRole::User)
            .unwrap();

        // Tamper with the token
        let tampered = format!("{}x", token);

        let result = service.validate_token(&tampered);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_role_user() {
        // Users can access user resources
        assert!(check_role(UserRole::User, UserRole::User).is_ok());
        // Keepers can access user resources
        assert!(check_role(UserRole::Keeper, UserRole::User).is_ok());
        // Admins can access user resources
        assert!(check_role(UserRole::Admin, UserRole::User).is_ok());
    }

    #[test]
    fn test_check_role_keeper() {
        // Users cannot access keeper resources
        assert!(check_role(UserRole::User, UserRole::Keeper).is_err());
        // Keepers can access keeper resources
        assert!(check_role(UserRole::Keeper, UserRole::Keeper).is_ok());
        // Admins can access keeper resources
        assert!(check_role(UserRole::Admin, UserRole::Keeper).is_ok());
    }

    #[test]
    fn test_check_role_admin() {
        // Users cannot access admin resources
        assert!(check_role(UserRole::User, UserRole::Admin).is_err());
        // Keepers cannot access admin resources
        assert!(check_role(UserRole::Keeper, UserRole::Admin).is_err());
        // Only admins can access admin resources
        assert!(check_role(UserRole::Admin, UserRole::Admin).is_ok());
    }

    #[test]
    fn test_unique_token_ids() {
        let service = test_jwt_service();
        let wallet = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

        let token1 = service
            .generate_access_token(wallet, UserRole::User)
            .unwrap();
        let token2 = service
            .generate_access_token(wallet, UserRole::User)
            .unwrap();

        let claims1 = service.validate_token(&token1).unwrap();
        let claims2 = service.validate_token(&token2).unwrap();

        // Token IDs should be unique
        assert_ne!(claims1.jti, claims2.jti);
    }

    #[test]
    fn test_key_rotation() {
        let service = test_jwt_service();
        let wallet = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

        // Generate token with original key
        let token1 = service
            .generate_access_token(wallet, UserRole::User)
            .unwrap();
        let original_kid = service.primary_key_id();

        // Add new key and rotate
        let new_kid = service.add_key("new-secret-key-for-rotation-test-12345");
        service.set_primary_key(&new_kid).unwrap();

        // Generate token with new key
        let token2 = service
            .generate_access_token(wallet, UserRole::User)
            .unwrap();

        // Both tokens should validate (old key still present)
        assert!(service.validate_token(&token1).is_ok());
        assert!(service.validate_token(&token2).is_ok());

        // Remove old key
        service.remove_key(&original_kid).unwrap();

        // Old token should still work (until expiry) if kid matches existing key
        // But since old key is removed, it should fail
        assert!(service.validate_token(&token1).is_err());
        // New token still works
        assert!(service.validate_token(&token2).is_ok());
    }

    #[test]
    fn test_cannot_remove_primary_key() {
        let service = test_jwt_service();
        let primary_kid = service.primary_key_id();

        let result = service.remove_key(&primary_kid);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_keys() {
        let service = test_jwt_service();

        assert_eq!(service.list_key_ids().len(), 1);

        service.add_key("another-secret-key");
        assert_eq!(service.list_key_ids().len(), 2);
    }
}
