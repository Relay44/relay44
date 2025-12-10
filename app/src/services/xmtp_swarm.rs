use crate::api::ApiError;
use crate::AppState;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct SwarmSendRequest {
    pub swarm_id: String,
    pub sender: String,
    pub message: String,
    pub signature: String,
    pub nonce: Option<String>,
    pub expires_at: Option<u64>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct SwarmListQuery {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmMessage {
    pub id: String,
    pub swarm_id: String,
    pub topic: String,
    pub sender: String,
    pub message: String,
    pub signature: String,
    pub metadata: Option<Value>,
    pub created_at: String,
    pub unix_ms: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwarmMessagesResponse {
    pub data: Vec<SwarmMessage>,
    pub total_returned: usize,
    pub limit: u64,
    pub offset: u64,
    pub topic: String,
}

fn sign_payload(signing_key: &str, payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(signing_key.as_bytes());
    hasher.update(b":");
    hasher.update(payload.as_bytes());
    hex::encode(hasher.finalize())
}

fn build_payload_legacy(request: &SwarmSendRequest) -> String {
    format!(
        "swarm_id={};sender={};message={}",
        request.swarm_id.trim(),
        request.sender.trim().to_ascii_lowercase(),
        request.message
    )
}

fn build_payload_v2(request: &SwarmSendRequest, nonce: &str, expires_at: u64) -> String {
    format!(
        "swarm_id={};sender={};message={};nonce={};expires_at={}",
        request.swarm_id.trim(),
        request.sender.trim().to_ascii_lowercase(),
        request.message,
        nonce,
        expires_at
    )
}

fn validate_swarm_id(value: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.len() < 3 || trimmed.len() > 128 {
        return Err(ApiError::bad_request(
            "INVALID_SWARM_ID",
            "swarm_id length must be between 3 and 128 characters",
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(ApiError::bad_request(
            "INVALID_SWARM_ID",
            "swarm_id contains invalid characters",
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_sender(value: &str) -> Result<String, ApiError> {
    let trimmed = value.trim().to_ascii_lowercase();
    let is_valid_hex = trimmed.starts_with("0x")
        && trimmed.len() == 42
        && trimmed[2..].chars().all(|c| c.is_ascii_hexdigit());
    if !is_valid_hex {
        return Err(ApiError::bad_request(
            "INVALID_SENDER",
            "sender must be a valid 0x EVM wallet address",
        ));
    }
    Ok(trimmed)
}

fn validate_nonce(value: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.len() < 8 || trimmed.len() > 128 {
        return Err(ApiError::bad_request(
            "INVALID_SWARM_NONCE",
            "nonce length must be between 8 and 128 characters",
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':')
    {
        return Err(ApiError::bad_request(
            "INVALID_SWARM_NONCE",
            "nonce contains invalid characters",
        ));
    }
    Ok(trimmed.to_string())
}

fn message_key(swarm_id: &str) -> String {
    format!("xmtp:swarm:{swarm_id}:messages")
}

fn topic(state: &AppState, swarm_id: &str) -> String {
    format!(
        "{}/{}",
        state.config.xmtp_swarm_topic_prefix.trim_end_matches('/'),
        swarm_id
    )
}

fn bridge_base_url(state: &AppState) -> Result<String, ApiError> {
    let base = state
        .config
        .xmtp_swarm_bridge_url
        .trim()
        .trim_end_matches('/');
    if base.is_empty() {
        return Err(ApiError::internal(
            "XMTP_SWARM_BRIDGE_URL is required for xmtp_http transport",
        ));
    }
    Ok(base.to_string())
}

async fn send_message_via_bridge(
    state: &AppState,
    request: &SwarmSendRequest,
) -> Result<SwarmMessage, ApiError> {
    let base = bridge_base_url(state)?;
    let url = format!("{base}/swarm/send");
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .json(request)
        .send()
        .await
        .map_err(|_| ApiError::internal("Failed to send message to XMTP bridge"))?;
    let status = response.status().as_u16();
    if status >= 400 {
        let payload = response.text().await.unwrap_or_default();
        return Err(ApiError::internal(&format!(
            "XMTP bridge rejected send request (status {status}): {payload}"
        )));
    }
    response
        .json::<SwarmMessage>()
        .await
        .map_err(|_| ApiError::internal("Invalid XMTP bridge send response"))
}

async fn list_messages_via_bridge(
    state: &AppState,
    swarm_id: &str,
    limit: u64,
    offset: u64,
) -> Result<SwarmMessagesResponse, ApiError> {
    let base = bridge_base_url(state)?;
    let url = format!(
        "{base}/swarm/{}/messages?limit={limit}&offset={offset}",
        swarm_id
    );
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| ApiError::internal("Failed to list messages from XMTP bridge"))?;
    let status = response.status().as_u16();
    if status >= 400 {
        let payload = response.text().await.unwrap_or_default();
        return Err(ApiError::internal(&format!(
            "XMTP bridge rejected list request (status {status}): {payload}"
        )));
    }
    response
        .json::<SwarmMessagesResponse>()
        .await
        .map_err(|_| ApiError::internal("Invalid XMTP bridge list response"))
}

pub async fn send_message(
    state: &AppState,
    request: SwarmSendRequest,
) -> Result<SwarmMessage, ApiError> {
    if !state.config.xmtp_swarm_enabled {
        return Err(ApiError::bad_request(
            "XMTP_SWARM_DISABLED",
            "XMTP swarm messaging is disabled",
        ));
    }

    let swarm_id = validate_swarm_id(request.swarm_id.as_str())?;
    let sender = validate_sender(request.sender.as_str())?;
    let message = request.message.trim().to_string();
    if message.is_empty() {
        return Err(ApiError::bad_request(
            "INVALID_MESSAGE",
            "message must not be empty",
        ));
    }
