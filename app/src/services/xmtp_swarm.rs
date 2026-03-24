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
    if message.as_bytes().len() > state.config.xmtp_swarm_max_message_bytes as usize {
        return Err(ApiError::bad_request(
            "MESSAGE_TOO_LARGE",
            "message exceeds XMTP swarm max payload size",
        ));
    }

    let canonical_legacy = build_payload_legacy(&SwarmSendRequest {
        swarm_id: swarm_id.clone(),
        sender: sender.clone(),
        message: message.clone(),
        signature: request.signature.clone(),
        nonce: request.nonce.clone(),
        expires_at: request.expires_at,
        metadata: request.metadata.clone(),
    });
    let expected_legacy_signature = sign_payload(
        state.config.xmtp_swarm_signing_key.as_str(),
        canonical_legacy.as_str(),
    );
    let mut v2_verified = false;
    let mut validated_nonce = None::<String>;
    let mut validated_expires_at = None::<u64>;

    if let (Some(nonce), Some(expires_at)) = (request.nonce.as_ref(), request.expires_at) {
        let nonce = validate_nonce(nonce)?;
        let canonical_v2 = build_payload_v2(
            &SwarmSendRequest {
                swarm_id: swarm_id.clone(),
                sender: sender.clone(),
                message: message.clone(),
                signature: request.signature.clone(),
                nonce: Some(nonce.clone()),
                expires_at: Some(expires_at),
                metadata: request.metadata.clone(),
            },
            nonce.as_str(),
            expires_at,
        );
        let expected_v2_signature = sign_payload(
            state.config.xmtp_swarm_signing_key.as_str(),
            canonical_v2.as_str(),
        );
        if expected_v2_signature.eq_ignore_ascii_case(request.signature.trim()) {
            let now = Utc::now().timestamp().max(0) as u64;
            if now > expires_at {
                return Err(ApiError::bad_request(
                    "SWARM_MESSAGE_EXPIRED",
                    "xmtp swarm message signature has expired",
                ));
            }

            let ttl = expires_at.saturating_sub(now).max(1);
            let nonce_key = format!("xmtp:swarm:{}:nonce:{}", swarm_id, nonce);
            let newly_recorded = state
                .redis
                .check_and_record_nonce(nonce_key.as_str(), ttl)
                .await
                .map_err(|_| ApiError::internal("Failed to validate XMTP swarm nonce"))?;
            if !newly_recorded {
                return Err(ApiError::conflict(
                    "SWARM_NONCE_REPLAYED",
                    "xmtp swarm nonce has already been used",
                ));
            }

            v2_verified = true;
            validated_nonce = Some(nonce);
            validated_expires_at = Some(expires_at);
        }
    }

    if !v2_verified && !expected_legacy_signature.eq_ignore_ascii_case(request.signature.trim()) {
        return Err(ApiError::unauthorized("Invalid XMTP swarm signature"));
    }

    if state.config.xmtp_swarm_transport == "xmtp_http" {
        let bridged = SwarmSendRequest {
            swarm_id,
            sender,
            message,
            signature: request.signature,
            nonce: validated_nonce,
            expires_at: validated_expires_at,
            metadata: request.metadata,
        };
        return send_message_via_bridge(state, &bridged).await;
    }

    let unix_ms = Utc::now().timestamp_millis();
    let created_at = Utc::now().to_rfc3339();
    let envelope = SwarmMessage {
        id: format!("xmtp_{}", Uuid::new_v4()),
        swarm_id: swarm_id.clone(),
        topic: topic(state, swarm_id.as_str()),
        sender,
        message,
        signature: request.signature,
        metadata: request.metadata,
        created_at,
        unix_ms,
    };

    let raw = serde_json::to_string(&envelope)
        .map_err(|_| ApiError::internal("Failed to serialize XMTP envelope"))?;
    state
        .redis
        .list_push_with_trim(
            message_key(swarm_id.as_str()).as_str(),
            raw.as_str(),
            state.config.xmtp_swarm_max_messages.max(10),
        )
        .await
        .map_err(|_| ApiError::internal("Failed to persist XMTP swarm message"))?;
    state
        .redis
        .publish(envelope.topic.as_str(), raw.as_str())
        .await
        .map_err(|_| ApiError::internal("Failed to publish XMTP swarm message"))?;

    Ok(envelope)
}

pub async fn list_messages(
    state: &AppState,
    swarm_id: &str,
    query: SwarmListQuery,
) -> Result<SwarmMessagesResponse, ApiError> {
    if !state.config.xmtp_swarm_enabled {
        return Err(ApiError::bad_request(
            "XMTP_SWARM_DISABLED",
            "XMTP swarm messaging is disabled",
        ));
    }

    let validated_swarm_id = validate_swarm_id(swarm_id)?;
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);
    if state.config.xmtp_swarm_transport == "xmtp_http" {
        return list_messages_via_bridge(state, validated_swarm_id.as_str(), limit, offset).await;
    }
    let end = offset.saturating_add(limit).saturating_sub(1);
    let raw_values = state
        .redis
        .list_range_raw(
            message_key(validated_swarm_id.as_str()).as_str(),
            offset,
            end,
        )
        .await
        .map_err(|_| ApiError::internal("Failed to load XMTP swarm messages"))?;

    let mut data = Vec::with_capacity(raw_values.len());
    for raw in raw_values {
        if let Ok(entry) = serde_json::from_str::<SwarmMessage>(raw.as_str()) {
            data.push(entry);
        }
    }

    Ok(SwarmMessagesResponse {
        total_returned: data.len(),
        limit,
        offset,
        topic: topic(state, validated_swarm_id.as_str()),
        data,
    })
}

pub fn health(state: &AppState) -> Value {
    serde_json::json!({
        "enabled": state.config.xmtp_swarm_enabled,
        "transport": state.config.xmtp_swarm_transport,
        "bridge_url_configured": !state.config.xmtp_swarm_bridge_url.trim().is_empty(),
        "topic_prefix": state.config.xmtp_swarm_topic_prefix,
        "max_messages": state.config.xmtp_swarm_max_messages,
        "max_message_bytes": state.config.xmtp_swarm_max_message_bytes
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_nonce_accepts_safe_charset() {
        let nonce = validate_nonce("alpha-01:beta_02").expect("nonce should be valid");
        assert_eq!(nonce, "alpha-01:beta_02");
    }

    #[test]
    fn test_validate_nonce_rejects_invalid_chars() {
        let err = validate_nonce("nonce with spaces").expect_err("nonce should be rejected");
        assert_eq!(err.code, "INVALID_SWARM_NONCE");
    }

    #[test]
    fn test_payload_v2_is_stable() {
        let request = SwarmSendRequest {
            swarm_id: "alpha".to_string(),
            sender: "0x1111111111111111111111111111111111111111".to_string(),
            message: "rebalance".to_string(),
            signature: String::new(),
            nonce: Some("alpha-1".to_string()),
            expires_at: Some(1_900_000_000),
            metadata: None,
        };
        let payload = build_payload_v2(&request, "alpha-1", 1_900_000_000);
        assert_eq!(
            payload,
            "swarm_id=alpha;sender=0x1111111111111111111111111111111111111111;message=rebalance;nonce=alpha-1;expires_at=1900000000"
        );
    }
}
