use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use sha2::{Digest, Sha256};

use crate::api::ApiError;

const NONCE_BYTES: usize = 12;

fn derive_key(master_key: &str, key_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(master_key.as_bytes());
    hasher.update(b":");
    hasher.update(key_id.as_bytes());
    let digest = hasher.finalize();

    let mut key = [0_u8; 32];
    key.copy_from_slice(&digest[..32]);
    key
}

pub fn encrypt_json(
    master_key: &str,
    key_id: &str,
    payload: &serde_json::Value,
) -> Result<String, ApiError> {
    if master_key.trim().is_empty() {
        return Err(ApiError::internal(
            "EXTERNAL_CREDENTIALS_MASTER_KEY is not configured",
        ));
    }

    let plaintext = serde_json::to_vec(payload).map_err(|err| {
        ApiError::internal(&format!("failed to serialize credential payload: {}", err))
    })?;

    let key = derive_key(master_key, key_id);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|err| ApiError::internal(&format!("credential cipher init failed: {}", err)))?;

    let mut nonce_bytes = [0_u8; NONCE_BYTES];
    let mut rng = OsRng;
    aes_gcm::aead::rand_core::RngCore::fill_bytes(&mut rng, &mut nonce_bytes);

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext.as_ref()).map_err(|err| {
        ApiError::internal(&format!("failed to encrypt credential payload: {}", err))
    })?;

    let mut combined = Vec::with_capacity(NONCE_BYTES + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(combined))
}

pub fn decrypt_json(
    master_key: &str,
    key_id: &str,
    encrypted: &str,
) -> Result<serde_json::Value, ApiError> {
    if master_key.trim().is_empty() {
        return Err(ApiError::internal(
            "EXTERNAL_CREDENTIALS_MASTER_KEY is not configured",
        ));
    }

    let raw = BASE64.decode(encrypted).map_err(|_| {
        ApiError::bad_request(
            "INVALID_CREDENTIAL_PAYLOAD",
            "credential payload encoding is invalid",
        )
    })?;

    if raw.len() <= NONCE_BYTES {
        return Err(ApiError::bad_request(
            "INVALID_CREDENTIAL_PAYLOAD",
            "credential payload length is invalid",
        ));
    }

    let (nonce_raw, ciphertext) = raw.split_at(NONCE_BYTES);
    let key = derive_key(master_key, key_id);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|err| ApiError::internal(&format!("credential cipher init failed: {}", err)))?;

    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_raw), ciphertext)
        .map_err(|_| ApiError::unauthorized("unable to decrypt credential payload"))?;

    serde_json::from_slice(&plaintext).map_err(|err| {
        ApiError::bad_request(
            "INVALID_CREDENTIAL_PAYLOAD",
            &format!("credential payload JSON is invalid: {}", err),
        )
    })
}

pub fn mask_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 8 {
        return "****".to_string();
    }

    let prefix = &trimmed[..4];
    let suffix = &trimmed[trimmed.len() - 4..];
    format!("{}****{}", prefix, suffix)
}
