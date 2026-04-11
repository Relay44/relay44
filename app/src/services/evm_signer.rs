use k256::ecdsa::{RecoveryId, Signature, SigningKey, VerifyingKey};
use rlp::RlpStream;
use sha3::{Digest, Keccak256};

use crate::api::ApiError;

/// Parameters for an EIP-1559 (type 2) transaction.
pub struct Eip1559TxParams {
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee_per_gas: u128,
    pub max_fee_per_gas: u128,
    pub gas_limit: u64,
    pub to: String,
    pub value: u128,
    pub data: String,
    pub private_key: String,
}

/// Derive a checksumless 0x-prefixed lowercase Ethereum address from a hex private key.
pub fn address_from_private_key(private_key_hex: &str) -> Result<String, ApiError> {
    let key_bytes = hex::decode(private_key_hex.trim_start_matches("0x"))
        .map_err(|e| ApiError::bad_request("INVALID_KEY", &format!("Invalid hex: {}", e)))?;

    if key_bytes.len() != 32 {
        return Err(ApiError::bad_request(
            "INVALID_KEY",
            "Private key must be 32 bytes",
        ));
    }

    let signing_key = SigningKey::from_bytes(key_bytes.as_slice().into()).map_err(|e| {
        ApiError::bad_request("INVALID_KEY", &format!("Invalid private key: {}", e))
    })?;

    Ok(eth_address_from_verifying_key(signing_key.verifying_key()))
}

/// Sign an EIP-1559 transaction and return the 0x-prefixed raw signed transaction hex.
pub fn sign_eip1559_transaction(params: &Eip1559TxParams) -> Result<String, ApiError> {
    let key_bytes = hex::decode(params.private_key.trim_start_matches("0x")).map_err(|e| {
        ApiError::bad_request("INVALID_KEY", &format!("Invalid private key hex: {}", e))
    })?;

    if key_bytes.len() != 32 {
        return Err(ApiError::bad_request(
            "INVALID_KEY",
            "Private key must be 32 bytes",
        ));
    }

    let signing_key = SigningKey::from_bytes(key_bytes.as_slice().into()).map_err(|e| {
        ApiError::bad_request("INVALID_KEY", &format!("Invalid private key: {}", e))
    })?;

    let to_bytes = decode_address(&params.to)?;
    let data_bytes = hex::decode(params.data.trim_start_matches("0x")).map_err(|e| {
        ApiError::bad_request("INVALID_DATA", &format!("Invalid calldata hex: {}", e))
    })?;

    // RLP encode the unsigned transaction fields (9 items)
    let unsigned_rlp = encode_unsigned_tx(params, &to_bytes, &data_bytes);

    // Create the typed envelope: 0x02 || RLP(unsigned fields)
    let mut typed_data = vec![0x02u8];
    typed_data.extend_from_slice(&unsigned_rlp);

    // Keccak256 hash the typed envelope
    let digest = Keccak256::new_with_prefix(&typed_data);

    // Sign the hash with secp256k1 using RFC 6979 deterministic k
    let (signature, recovery_id): (Signature, RecoveryId) = signing_key
        .sign_digest_recoverable(digest)
        .map_err(|e| ApiError::internal(&format!("Signing failed: {}", e)))?;

    let sig_bytes = signature.to_bytes();
    let r_bytes = &sig_bytes[..32];
    let s_bytes = &sig_bytes[32..];
    let v = recovery_id.to_byte();

    // RLP encode the signed transaction (12 items)
    let signed_rlp = {
        let mut stream = RlpStream::new_list(12);
        encode_u64(&mut stream, params.chain_id);
        encode_u64(&mut stream, params.nonce);
        encode_u128(&mut stream, params.max_priority_fee_per_gas);
        encode_u128(&mut stream, params.max_fee_per_gas);
        encode_u64(&mut stream, params.gas_limit);
        stream.append(&to_bytes.as_slice());
        encode_u128(&mut stream, params.value);
        stream.append(&data_bytes.as_slice());
        // Empty access list
        stream.begin_list(0);
        // v (recovery id: 0 or 1)
        encode_u64(&mut stream, v as u64);
        // r — 32-byte big-endian, trim leading zeros for RLP
        stream.append(&trim_leading_zeros(r_bytes));
        // s — 32-byte big-endian, trim leading zeros for RLP
        stream.append(&trim_leading_zeros(s_bytes));
        stream.out()
    };

    // Prepend the EIP-1559 type byte
    let mut raw_tx = vec![0x02u8];
    raw_tx.extend_from_slice(&signed_rlp);

    Ok(format!("0x{}", hex::encode(&raw_tx)))
}

fn eth_address_from_verifying_key(key: &VerifyingKey) -> String {
    let public_key = key.to_encoded_point(false);
    // Skip the 0x04 prefix byte, hash the 64-byte uncompressed public key
    let pubkey_bytes = &public_key.as_bytes()[1..];
    let hash = Keccak256::digest(pubkey_bytes);
    format!("0x{}", hex::encode(&hash[12..]))
}

fn encode_unsigned_tx(params: &Eip1559TxParams, to_bytes: &[u8], data_bytes: &[u8]) -> Vec<u8> {
    let mut stream = RlpStream::new_list(9);
    encode_u64(&mut stream, params.chain_id);
    encode_u64(&mut stream, params.nonce);
    encode_u128(&mut stream, params.max_priority_fee_per_gas);
    encode_u128(&mut stream, params.max_fee_per_gas);
    encode_u64(&mut stream, params.gas_limit);
    stream.append(&to_bytes);
    encode_u128(&mut stream, params.value);
    stream.append(&data_bytes);
    // Empty access list
    stream.begin_list(0);
    stream.out().to_vec()
}

fn decode_address(addr: &str) -> Result<Vec<u8>, ApiError> {
    let bytes = hex::decode(addr.trim_start_matches("0x")).map_err(|e| {
        ApiError::bad_request("INVALID_ADDRESS", &format!("Invalid address hex: {}", e))
    })?;
    if bytes.len() != 20 {
        return Err(ApiError::bad_request(
            "INVALID_ADDRESS",
            "Address must be 20 bytes",
        ));
    }
    Ok(bytes)
}

fn encode_u64(stream: &mut RlpStream, value: u64) {
    if value == 0 {
        stream.append_empty_data();
    } else {
        let bytes = value.to_be_bytes();
        let trimmed = trim_leading_zeros(&bytes);
        stream.append(&trimmed);
    }
}

fn encode_u128(stream: &mut RlpStream, value: u128) {
    if value == 0 {
        stream.append_empty_data();
    } else {
        let bytes = value.to_be_bytes();
        let trimmed = trim_leading_zeros(&bytes);
        stream.append(&trimmed);
    }
}

fn trim_leading_zeros(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
    if start == bytes.len() {
        &bytes[..0]
    } else {
        &bytes[start..]
    }
}

/// Sign an EIP-712 typed-data hash with a raw private key.
/// Returns a 65-byte signature hex string: `0x{r}{s}{v}` where v ∈ {27, 28}.
pub fn sign_eip712_hash(hash: &[u8; 32], private_key_hex: &str) -> Result<String, ApiError> {
    let key_bytes = hex::decode(private_key_hex.trim_start_matches("0x"))
        .map_err(|e| ApiError::bad_request("INVALID_KEY", &format!("Invalid hex: {}", e)))?;
    if key_bytes.len() != 32 {
        return Err(ApiError::bad_request(
            "INVALID_KEY",
            "Private key must be 32 bytes",
        ));
    }
    let signing_key = SigningKey::from_bytes(key_bytes.as_slice().into()).map_err(|e| {
        ApiError::bad_request("INVALID_KEY", &format!("Invalid private key: {}", e))
    })?;

    use k256::ecdsa::signature::hazmat::PrehashSigner;
    let (signature, recovery_id): (Signature, RecoveryId) = signing_key
        .sign_prehash_recoverable(hash)
        .map_err(|e| ApiError::internal(&format!("EIP-712 signing failed: {}", e)))?;

    let sig_bytes = signature.to_bytes();
    let r = &sig_bytes[..32];
    let s = &sig_bytes[32..];
    let v = recovery_id.to_byte() + 27;
    Ok(format!("0x{}{}{:02x}", hex::encode(r), hex::encode(s), v))
}

/// Compute the EIP-712 struct hash for a given primary type and values.
/// `encode_data` should already be the ABI-encoded struct fields (no selector).
pub fn eip712_struct_hash(type_hash: &[u8; 32], encode_data: &[u8]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(32 + encode_data.len());
    buf.extend_from_slice(type_hash);
    buf.extend_from_slice(encode_data);
    Keccak256::digest(&buf).into()
}

/// Compute the EIP-712 domain separator hash.
pub fn eip712_domain_separator(
    name: &str,
    version: &str,
    chain_id: u64,
    verifying_contract: &str,
) -> [u8; 32] {
    let type_hash = Keccak256::digest(
        b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
    );
    let name_hash = Keccak256::digest(name.as_bytes());
    let version_hash = Keccak256::digest(version.as_bytes());

    let contract_bytes =
        hex::decode(verifying_contract.trim_start_matches("0x")).unwrap_or_default();
    let mut contract_word = [0u8; 32];
    if contract_bytes.len() == 20 {
        contract_word[12..].copy_from_slice(&contract_bytes);
    }

    let mut chain_word = [0u8; 32];
    chain_word[24..].copy_from_slice(&chain_id.to_be_bytes());

    let mut buf = Vec::with_capacity(5 * 32);
    buf.extend_from_slice(&type_hash);
    buf.extend_from_slice(&name_hash);
    buf.extend_from_slice(&version_hash);
    buf.extend_from_slice(&chain_word);
    buf.extend_from_slice(&contract_word);
    Keccak256::digest(&buf).into()
}

/// Final EIP-712 hash: `keccak256("\x19\x01" || domainSeparator || structHash)`.
pub fn eip712_signing_hash(domain_separator: &[u8; 32], struct_hash: &[u8; 32]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(2 + 32 + 32);
    buf.push(0x19);
    buf.push(0x01);
    buf.extend_from_slice(domain_separator);
    buf.extend_from_slice(struct_hash);
    Keccak256::digest(&buf).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Well-known test private key (DO NOT use for real funds)
    const TEST_PRIVATE_KEY: &str =
        "4c0883a69102937d6231471b5dbb6204fe512961708279f23efb02b21c12b964";
    const TEST_ADDRESS: &str = "0x425c5a2b0755bfabbcdc7c47a5c2f626e0d62b88";

    #[test]
    fn address_from_private_key_returns_correct_address() {
        let address = address_from_private_key(TEST_PRIVATE_KEY).unwrap();
        assert_eq!(address, TEST_ADDRESS);
    }

    #[test]
    fn address_from_private_key_with_0x_prefix() {
        let address = address_from_private_key(&format!("0x{}", TEST_PRIVATE_KEY)).unwrap();
        assert_eq!(address, TEST_ADDRESS);
    }

    #[test]
    fn address_from_private_key_invalid_hex_returns_error() {
        let result = address_from_private_key("not_valid_hex_zzzz");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "INVALID_KEY");
    }

    #[test]
    fn address_from_private_key_wrong_length_returns_error() {
        let result = address_from_private_key("aabbccdd");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, "INVALID_KEY");
    }

    #[test]
    fn sign_eip1559_transaction_produces_valid_signed_tx() {
        let params = Eip1559TxParams {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: 1_500_000_000,
            max_fee_per_gas: 30_000_000_000,
            gas_limit: 21_000,
            to: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
            value: 1_000_000_000_000_000_000, // 1 ETH
            data: "0x".to_string(),
            private_key: TEST_PRIVATE_KEY.to_string(),
        };

        let signed_tx = sign_eip1559_transaction(&params).unwrap();

        // Basic structure checks
        assert!(signed_tx.starts_with("0x02"));
        assert!(
            signed_tx.len() > 100,
            "Signed tx too short: {}",
            signed_tx.len()
        );

        // Decode and verify internal structure
        let raw_bytes = hex::decode(signed_tx.trim_start_matches("0x")).unwrap();
        assert_eq!(
            raw_bytes[0], 0x02,
            "First byte must be EIP-1559 type prefix"
        );

        // The rest should be valid RLP with 12 items
        let rlp_data = &raw_bytes[1..];
        let rlp = rlp::Rlp::new(rlp_data);
        assert!(rlp.is_list());
        assert_eq!(
            rlp.item_count().unwrap(),
            12,
            "Signed tx must have 12 RLP items"
        );
    }

    #[test]
    fn sign_eip1559_transaction_is_deterministic() {
        let params = Eip1559TxParams {
            chain_id: 8453,
            nonce: 42,
            max_priority_fee_per_gas: 100_000_000,
            max_fee_per_gas: 1_000_000_000,
            gas_limit: 21_000,
            to: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
            value: 0,
            data: "0x".to_string(),
            private_key: TEST_PRIVATE_KEY.to_string(),
        };

        let signed_tx_1 = sign_eip1559_transaction(&params).unwrap();
        let signed_tx_2 = sign_eip1559_transaction(&params).unwrap();
        assert_eq!(
            signed_tx_1, signed_tx_2,
            "Signing must be deterministic (RFC 6979)"
        );
    }

    #[test]
    fn sign_eip1559_transaction_with_calldata() {
        // ERC-20 transfer(address,uint256) selector + params
        let calldata = "0xa9059cbb000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa960450000000000000000000000000000000000000000000000000de0b6b3a7640000";
        let params = Eip1559TxParams {
            chain_id: 8453,
            nonce: 1,
            max_priority_fee_per_gas: 1_000_000,
            max_fee_per_gas: 5_000_000_000,
            gas_limit: 65_000,
            to: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            value: 0,
            data: calldata.to_string(),
            private_key: TEST_PRIVATE_KEY.to_string(),
        };

        let signed_tx = sign_eip1559_transaction(&params).unwrap();
        assert!(signed_tx.starts_with("0x02"));

        let raw_bytes = hex::decode(signed_tx.trim_start_matches("0x")).unwrap();
        let rlp_data = &raw_bytes[1..];
        let rlp = rlp::Rlp::new(rlp_data);
        assert_eq!(rlp.item_count().unwrap(), 12);
    }

    #[test]
    fn sign_eip1559_transaction_invalid_key_returns_error() {
        let params = Eip1559TxParams {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: 0,
            max_fee_per_gas: 0,
            gas_limit: 21_000,
            to: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
            value: 0,
            data: "0x".to_string(),
            private_key: "deadbeef".to_string(),
        };

        let result = sign_eip1559_transaction(&params);
        assert!(result.is_err());
    }

    #[test]
    fn sign_eip1559_transaction_invalid_address_returns_error() {
        let params = Eip1559TxParams {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: 0,
            max_fee_per_gas: 0,
            gas_limit: 21_000,
            to: "0xINVALID".to_string(),
            value: 0,
            data: "0x".to_string(),
            private_key: TEST_PRIVATE_KEY.to_string(),
        };

        let result = sign_eip1559_transaction(&params);
        assert!(result.is_err());
    }

    #[test]
    fn sign_eip1559_transaction_recovers_to_correct_sender() {
        let params = Eip1559TxParams {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: 1_500_000_000,
            max_fee_per_gas: 30_000_000_000,
            gas_limit: 21_000,
            to: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
            value: 1_000_000_000_000_000_000,
            data: "0x".to_string(),
            private_key: TEST_PRIVATE_KEY.to_string(),
        };

        let signed_tx = sign_eip1559_transaction(&params).unwrap();
        let raw_bytes = hex::decode(signed_tx.trim_start_matches("0x")).unwrap();
        let rlp_data = &raw_bytes[1..];
        let rlp = rlp::Rlp::new(rlp_data);

        // Extract v, r, s from the signed tx
        let v_bytes: Vec<u8> = rlp.at(9).unwrap().data().unwrap().to_vec();
        let v = if v_bytes.is_empty() { 0u8 } else { v_bytes[0] };
        let r: Vec<u8> = rlp.at(10).unwrap().data().unwrap().to_vec();
        let s: Vec<u8> = rlp.at(11).unwrap().data().unwrap().to_vec();

        // Reconstruct the unsigned tx hash
        let to_bytes = decode_address(&params.to).unwrap();
        let unsigned_rlp = encode_unsigned_tx(&params, &to_bytes, &[]);
        let mut typed_data = vec![0x02u8];
        typed_data.extend_from_slice(&unsigned_rlp);
        let digest = Keccak256::new_with_prefix(&typed_data);

        // Recover the public key from signature — pad r and s to 32 bytes each
        let mut sig_bytes = [0u8; 64];
        let r_offset = 32 - r.len();
        sig_bytes[r_offset..32].copy_from_slice(&r);
        let s_offset = 32 - s.len();
        sig_bytes[32 + s_offset..64].copy_from_slice(&s);

        let recovery_id = RecoveryId::from_byte(v).unwrap();
        let signature = Signature::from_bytes(sig_bytes.as_slice().into()).unwrap();
        let recovered_key =
            VerifyingKey::recover_from_digest(digest, &signature, recovery_id).unwrap();

        let recovered_address = eth_address_from_verifying_key(&recovered_key);
        assert_eq!(recovered_address, TEST_ADDRESS);
    }

    #[test]
    fn sign_eip1559_transaction_zero_value_and_zero_fees() {
        let params = Eip1559TxParams {
            chain_id: 1,
            nonce: 0,
            max_priority_fee_per_gas: 0,
            max_fee_per_gas: 0,
            gas_limit: 21_000,
            to: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
            value: 0,
            data: "0x".to_string(),
            private_key: TEST_PRIVATE_KEY.to_string(),
        };

        let signed_tx = sign_eip1559_transaction(&params).unwrap();
        assert!(signed_tx.starts_with("0x02"));

        let raw_bytes = hex::decode(signed_tx.trim_start_matches("0x")).unwrap();
        let rlp_data = &raw_bytes[1..];
        let rlp = rlp::Rlp::new(rlp_data);
        assert_eq!(rlp.item_count().unwrap(), 12);
    }
}
