use crate::api::ApiError;
use crate::AppState;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

// ── Key Constants ───────────────────────────────────────────────────

const KEY_PREFIX: &str = "r44_";
const KEY_RANDOM_BYTES: usize = 32;
const MAX_KEYS_PER_WALLET: usize = 10;

// ── Scope ───────────────────────────────────────────────────────────

/// Hierarchical API key scope. Higher scopes include all lower permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyScope {
    Read,
    Trade,
    Admin,
}

impl ApiKeyScope {
    pub fn has_permission(&self, required: ApiKeyScope) -> bool {
        match required {
            ApiKeyScope::Read => true,
            ApiKeyScope::Trade => matches!(self, ApiKeyScope::Trade | ApiKeyScope::Admin),
            ApiKeyScope::Admin => matches!(self, ApiKeyScope::Admin),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ApiKeyScope::Read => "read",
            ApiKeyScope::Trade => "trade",
            ApiKeyScope::Admin => "admin",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "read" => Some(Self::Read),
            "trade" => Some(Self::Trade),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }
}

// ── Auth Method ─────────────────────────────────────────────────────

/// How the current request was authenticated. Carried on AuthenticatedUser
/// so downstream code (rate limiting) can select the right tier.
#[derive(Debug, Clone)]
pub enum AuthMethod {
    Jwt,
    ApiKey { scope: ApiKeyScope },
}

impl AuthMethod {
    pub fn is_api_key(&self) -> bool {
        matches!(self, AuthMethod::ApiKey { .. })
    }
}

// ── Key Generation ──────────────────────────────────────────────────

/// Generate a new API key. Returns `(full_key, sha256_hash, display_prefix)`.
pub fn generate_api_key() -> (String, String, String) {
    let random_bytes: [u8; KEY_RANDOM_BYTES] = rand::random();
    let random_hex = hex::encode(random_bytes);
    let full_key = format!("{}{}", KEY_PREFIX, random_hex);
    let prefix = format!("{}{}...", KEY_PREFIX, &random_hex[..8]);
    let hash = hash_api_key(&full_key);
    (full_key, hash, prefix)
}

/// SHA-256 hash of an API key for storage/lookup.
pub fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Check whether a raw token string looks like an API key.
pub fn is_api_key_token(token: &str) -> bool {
    token.starts_with(KEY_PREFIX)
}

// ── Request / Response Types ────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub label: String,
    #[serde(default = "default_scope")]
    pub scope: String,
    pub expires_in_days: Option<u32>,
}

fn default_scope() -> String {
    "trade".to_string()
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub key: String,
    pub prefix: String,
    pub label: String,
    pub scope: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyListItem {
    pub id: String,
    pub prefix: String,
    pub label: String,
    pub scope: String,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ── Handlers ────────────────────────────────────────────────────────

/// POST /v1/auth/api-keys — Create a new API key.
/// Requires JWT auth (wallet signing). The plaintext key is returned once.
pub async fn create_api_key_handler(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<CreateApiKeyRequest>,
) -> Result<impl Responder, ApiError> {
    let user = crate::api::auth::extract_authenticated_user(&req, &state).await?;

    // Only JWT-authenticated users can create keys (prevent key-bootstrapping-keys)
    if user.auth_method.is_api_key() {
        return Err(ApiError::forbidden(
            "API keys can only be created with wallet (JWT) authentication",
        ));
    }

    // Validate scope
    let scope = ApiKeyScope::from_str(&body.scope).ok_or_else(|| {
        ApiError::bad_request("INVALID_SCOPE", "Scope must be 'read', 'trade', or 'admin'")
    })?;

    // Admin scope requires admin role
    if scope == ApiKeyScope::Admin {
        if !state
            .config
            .admin_wallets
            .iter()
            .any(|w| w == &user.wallet_address)
        {
            return Err(ApiError::forbidden(
                "Only admin wallets can create admin-scoped API keys",
            ));
        }
    }

    // Validate and sanitize label
    let label = body.label.trim().to_string();
    if label.is_empty() {
        return Err(ApiError::bad_request("INVALID_LABEL", "Label is required"));
    }
    if label.len() > 128 {
        return Err(ApiError::bad_request(
            "INVALID_LABEL",
            "Label must be 128 characters or fewer",
        ));
    }

    // Check key count limit (only count active keys)
    let existing = state
        .db
        .list_api_keys(&user.wallet_address)
        .await
        .map_err(ApiError::from)?;
    let active_count = existing.iter().filter(|k| k.is_active).count();
    if active_count >= MAX_KEYS_PER_WALLET {
        return Err(ApiError::bad_request(
            "KEY_LIMIT_REACHED",
            &format!("Maximum {} API keys per wallet", MAX_KEYS_PER_WALLET),
        ));
    }

    let expires_at = body
        .expires_in_days
        .map(|days| Utc::now() + Duration::days(days as i64));

    let (full_key, key_hash, prefix) = generate_api_key();

    let id = state
        .db
        .create_api_key(
            &user.wallet_address,
            &key_hash,
            &prefix,
            &label,
            scope.as_str(),
            expires_at,
        )
        .await
        .map_err(ApiError::from)?;

    log::info!(
        "API key created: id={}, prefix={}, wallet={}, scope={}",
        id,
        prefix,
        user.wallet_address,
        scope.as_str()
    );

    Ok(HttpResponse::Created().json(CreateApiKeyResponse {
        id,
        key: full_key,
        prefix,
        label,
        scope: scope.as_str().to_string(),
        expires_at,
        created_at: Utc::now(),
    }))
}

/// GET /v1/auth/api-keys — List all API keys for the authenticated wallet.
pub async fn list_api_keys_handler(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> Result<impl Responder, ApiError> {
    let user = crate::api::auth::extract_authenticated_user(&req, &state).await?;

    let keys = state
        .db
        .list_api_keys(&user.wallet_address)
        .await
        .map_err(ApiError::from)?;

    Ok(HttpResponse::Ok().json(keys))
}

/// DELETE /v1/auth/api-keys/{key_id} — Revoke an API key.
pub async fn revoke_api_key_handler(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<impl Responder, ApiError> {
    let user = crate::api::auth::extract_authenticated_user(&req, &state).await?;
    let key_id = path.into_inner();

    // Get key hash before revoking (for Redis cache)
    let key_hash = state
        .db
        .get_api_key_hash_by_id(&key_id, &user.wallet_address)
        .await
        .map_err(ApiError::from)?;

    let revoked = state
        .db
        .revoke_api_key(&key_id, &user.wallet_address)
        .await
        .map_err(ApiError::from)?;

    if !revoked {
        return Err(ApiError::not_found("API key"));
    }

    // Immediate Redis revocation cache
    if let Some(hash) = key_hash {
        state.redis.revoke_api_key(&hash).await.ok();
    }

    log::info!(
        "API key revoked: id={}, wallet={}",
        key_id,
        user.wallet_address
    );

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "id": key_id,
        "revoked": true
    })))
}
