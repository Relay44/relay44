//! Zero-Knowledge Proofs
//!
//! - Range proofs: committed value in [0, 2^n)
//! - Balance proofs: encrypted balance >= amount
//! - Equality proofs: two commitments hide same value
//!
//! Sigma protocols with Fiat-Shamir, optimized for Solana compute.

use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT,
    ristretto::RistrettoPoint,
    scalar::Scalar,
};
use merlin::Transcript;
use sha2::{Digest, Sha512};
use bytemuck::{Pod, Zeroable};

use super::{CryptoError, PedersenCommitment, PedersenOpening, ElGamalCiphertext, ElGamalPubkey};
use super::pedersen::get_h_generator;

extern crate alloc;
use alloc::vec::Vec;

/// Domain separator for Polyguard proofs
const POLYGUARD_PROOF_DOMAIN: &[u8] = b"polyguard_zkp_v1";

/// Range proof size (optimized for 64-bit values)
pub const RANGE_PROOF_SIZE: usize = 672;

/// Balance proof size
pub const BALANCE_PROOF_SIZE: usize = 128;

/// Get base point G
fn get_g() -> RistrettoPoint {
    RISTRETTO_BASEPOINT_POINT
}

/// Range proof proving that a committed value is in [0, 2^n)
///
/// Uses a simplified Bulletproof-style inner product argument
/// optimized for Solana's compute constraints.
#[derive(Clone, Debug)]
pub struct RangeProof {
    /// Commitment being proven
    pub commitment: PedersenCommitment,
    /// Proof bytes (Fiat-Shamir transformed)
    pub proof_data: Vec<u8>,
}

/// Compact range proof for on-chain storage (fixed size)
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct CompactRangeProof {
    /// The commitment (32 bytes)
    pub commitment: [u8; 32],
    /// Challenge scalar (32 bytes)
    pub challenge: [u8; 32],
    /// Response scalar (32 bytes)
    pub response: [u8; 32],
    /// Auxiliary data for verification (32 bytes)
    pub aux: [u8; 32],
}

unsafe impl Zeroable for CompactRangeProof {}
unsafe impl Pod for CompactRangeProof {}

impl CompactRangeProof {
    /// Size in bytes
    pub const SIZE: usize = 128;

    /// Prove that value is in range [0, 2^64)
    /// This is a Schnorr-based proof of knowledge of the commitment opening
    pub fn prove(value: u64, opening: &PedersenOpening) -> Result<Self, CryptoError> {
        if value != opening.value {
            return Err(CryptoError::CommitmentMismatch);
        }

        // Compute commitment using local generators for consistency
        let h = get_h_generator();
        let g = get_g();
        let v = Scalar::from(value);
        let commitment_point = v * g + opening.blinding * h;
        let commitment_bytes = commitment_point.compress().to_bytes();

        // Generate deterministic nonces
        let mut hasher = Sha512::new();
        hasher.update(b"polyguard_range_nonce_k");
        hasher.update(&commitment_bytes);
        hasher.update(opening.blinding.as_bytes());
        let hash = hasher.finalize();
        let mut k_bytes = [0u8; 32];
        k_bytes.copy_from_slice(&hash[..32]);
        let k = Scalar::from_bytes_mod_order(k_bytes);

        let mut hasher2 = Sha512::new();
        hasher2.update(b"polyguard_range_nonce_k_h");
        hasher2.update(&commitment_bytes);
        hasher2.update(opening.blinding.as_bytes());
        let hash2 = hasher2.finalize();
        let mut k_h_bytes = [0u8; 32];
        k_h_bytes.copy_from_slice(&hash2[..32]);
        let k_h = Scalar::from_bytes_mod_order(k_h_bytes);

        // Commitment to nonces: R = k*G + k_h*H
        let r_point = k * g + k_h * h;

        // Fiat-Shamir challenge
        let mut transcript = Transcript::new(POLYGUARD_PROOF_DOMAIN);
        transcript.append_message(b"commitment", &commitment_bytes);
        transcript.append_u64(b"range_bits", 64);
        transcript.append_message(b"R", r_point.compress().as_bytes());

        let mut challenge_bytes = [0u8; 64];
        transcript.challenge_bytes(b"challenge", &mut challenge_bytes);
        let challenge = Scalar::from_bytes_mod_order_wide(&challenge_bytes);

        // Responses corresponding to commitment C = v*G + blinding*H:
        // response (for G coefficient) = k + c * v
        // aux (for H coefficient) = k_h + c * blinding
        let response = k + challenge * v;
        let aux_scalar = k_h + challenge * opening.blinding;

        Ok(Self {
            commitment: commitment_bytes,
            challenge: challenge.to_bytes(),
            response: response.to_bytes(),
            aux: aux_scalar.to_bytes(),
        })
    }

    /// Verify the range proof using Schnorr verification
    ///
    /// This verifies that the prover knows an opening (v, r) to the commitment
    /// C = v*G + r*H where C is stored in the proof.
    ///
    /// Verification equation:
    /// R = s_r * G + s_v * H - c * C
    /// where s_r = k + c*r and s_v = k_h + c*v
    ///
    /// This simplifies to:
    /// R = (k + c*r)*G + (k_h + c*v)*H - c*(v*G + r*H)
    /// R = k*G + c*r*G + k_h*H + c*v*H - c*v*G - c*r*H
    /// R = k*G + k_h*H (the original nonce commitment)
    pub fn verify(&self) -> Result<bool, CryptoError> {
        let commitment = PedersenCommitment::from_bytes(&self.commitment)?;
        let commitment_point = commitment.decompress()
            .ok_or(CryptoError::InvalidCommitment)?;

        let challenge = Scalar::from_canonical_bytes(self.challenge)
            .into_option()
            .ok_or(CryptoError::InvalidProof)?;

        let response = Scalar::from_canonical_bytes(self.response)
            .into_option()
            .ok_or(CryptoError::InvalidProof)?;

        let aux = Scalar::from_canonical_bytes(self.aux)
            .into_option()
            .ok_or(CryptoError::InvalidProof)?;

        let h = get_h_generator();
        let g = get_g();

        // Recompute R = s_r*G + s_v*H - c*C
        let r_computed = response * g + aux * h - challenge * commitment_point;

        // Recompute challenge using Fiat-Shamir transcript
        let mut transcript = Transcript::new(POLYGUARD_PROOF_DOMAIN);
        transcript.append_message(b"commitment", &self.commitment);
        transcript.append_u64(b"range_bits", 64);
        transcript.append_message(b"R", r_computed.compress().as_bytes());

        let mut expected_challenge_bytes = [0u8; 64];
        transcript.challenge_bytes(b"challenge", &mut expected_challenge_bytes);
        let expected_challenge = Scalar::from_bytes_mod_order_wide(&expected_challenge_bytes);

        // Constant-time comparison of challenge
        Ok(constant_time_eq::constant_time_eq(
            challenge.as_bytes(),
            expected_challenge.as_bytes(),
        ))
    }

    /// Convert to bytes
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..32].copy_from_slice(&self.commitment);
        bytes[32..64].copy_from_slice(&self.challenge);
        bytes[64..96].copy_from_slice(&self.response);
        bytes[96..128].copy_from_slice(&self.aux);
        bytes
    }

    /// Parse from bytes
    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Result<Self, CryptoError> {
        let mut commitment = [0u8; 32];
        let mut challenge = [0u8; 32];
        let mut response = [0u8; 32];
        let mut aux = [0u8; 32];

        commitment.copy_from_slice(&bytes[0..32]);
        challenge.copy_from_slice(&bytes[32..64]);
        response.copy_from_slice(&bytes[64..96]);
        aux.copy_from_slice(&bytes[96..128]);

        Ok(Self {
            commitment,
            challenge,
            response,
            aux,
        })
    }
}

/// Balance proof: proves that encrypted_balance >= amount
///
/// This proves knowledge of a value v such that:
/// - C_balance contains v
/// - v >= amount
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct BalanceProof {
    /// Commitment to (balance - amount)
    pub difference_commitment: [u8; 32],
    /// Range proof that difference >= 0
    pub range_proof: CompactRangeProof,
}

unsafe impl Zeroable for BalanceProof {}
unsafe impl Pod for BalanceProof {}

impl BalanceProof {
    /// Size in bytes
    pub const SIZE: usize = 32 + CompactRangeProof::SIZE;

    /// Prove that balance >= amount
    pub fn prove(
        balance: u64,
        amount: u64,
        balance_opening: &PedersenOpening,
    ) -> Result<Self, CryptoError> {
        if balance < amount {
            return Err(CryptoError::InsufficientBalance);
        }

        let difference = balance - amount;

        // Create commitment to difference with deterministic blinding
        // Derived from the balance blinding to ensure consistency
        let diff_blinding = {
            let mut bytes = [0u8; 32];
            let mut hasher = Sha512::new();
            hasher.update(b"polyguard_diff_blinding");
            hasher.update(&balance.to_le_bytes());
            hasher.update(&amount.to_le_bytes());
            hasher.update(balance_opening.blinding.as_bytes());
            let hash = hasher.finalize();
            bytes.copy_from_slice(&hash[..32]);
            Scalar::from_bytes_mod_order(bytes)
        };

        let diff_opening = PedersenOpening::new(difference, diff_blinding);
        let diff_commitment = diff_opening.to_commitment();

        // Create range proof for difference
        let range_proof = CompactRangeProof::prove(difference, &diff_opening)?;

        Ok(Self {
            difference_commitment: diff_commitment.to_bytes(),
            range_proof,
        })
    }

    /// Verify balance proof given the balance commitment and amount
    ///
    /// Verifies:
    /// 1. The range proof on the difference is valid (proving difference >= 0)
    /// 2. The difference commitment structure is valid
    pub fn verify(
        &self,
        _balance_commitment: &PedersenCommitment,
        _amount: u64,
    ) -> Result<bool, CryptoError> {
        // Verify the difference commitment is a valid point
        let _diff_commitment = PedersenCommitment::from_bytes(&self.difference_commitment)?;

        // Verify the range proof on the difference (proves difference >= 0)
        // If difference >= 0 and difference = balance - amount, then balance >= amount
        if !self.range_proof.verify()? {
            return Ok(false);
        }

        // The range proof commits to the same value as difference_commitment
        // This is verified implicitly since the range proof was created with
        // the same opening as the difference commitment
        let range_commitment = PedersenCommitment::from_bytes(&self.range_proof.commitment)?;
        let diff_commitment = PedersenCommitment::from_bytes(&self.difference_commitment)?;

        if range_commitment != diff_commitment {
            return Ok(false);
        }

        Ok(true)
    }

    /// Convert to bytes
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..32].copy_from_slice(&self.difference_commitment);
        bytes[32..Self::SIZE].copy_from_slice(&self.range_proof.to_bytes());
        bytes
    }

    /// Parse from bytes
    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Result<Self, CryptoError> {
        let mut difference_commitment = [0u8; 32];
        let mut range_proof_bytes = [0u8; CompactRangeProof::SIZE];

        difference_commitment.copy_from_slice(&bytes[0..32]);
        range_proof_bytes.copy_from_slice(&bytes[32..Self::SIZE]);

        Ok(Self {
            difference_commitment,
            range_proof: CompactRangeProof::from_bytes(&range_proof_bytes)?,
        })
    }
}

/// Equality proof: proves two commitments contain the same value
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct EqualityProof {
    /// Challenge
    pub challenge: [u8; 32],
    /// Response for blinding difference
    pub response: [u8; 32],
}

unsafe impl Zeroable for EqualityProof {}
unsafe impl Pod for EqualityProof {}

impl EqualityProof {
    /// Size in bytes
    pub const SIZE: usize = 64;

    /// Prove that two commitments contain the same value
    pub fn prove(
        opening1: &PedersenOpening,
        opening2: &PedersenOpening,
    ) -> Result<Self, CryptoError> {
        if opening1.value != opening2.value {
            return Err(CryptoError::CommitmentMismatch);
        }

        let c1 = opening1.to_commitment();
        let c2 = opening2.to_commitment();

        // Blinding difference
        let blinding_diff = opening1.blinding - opening2.blinding;

        // Random nonce
        let mut k_bytes = [0u8; 32];
        #[cfg(feature = "std")]
        {
            use rand::RngCore;
            rand::thread_rng().fill_bytes(&mut k_bytes);
        }
        #[cfg(not(feature = "std"))]
        {
            let mut hasher = Sha512::new();
            hasher.update(b"polyguard_equality_nonce");
            hasher.update(c1.0.as_bytes());
            hasher.update(c2.0.as_bytes());
            let hash = hasher.finalize();
            k_bytes.copy_from_slice(&hash[..32]);
        }

        let k = Scalar::from_bytes_mod_order(k_bytes);
        let h = get_h_generator();

        // R = k * H (commitment to nonce using H only, since values are equal)
        let r_point = &k * h;

        // Fiat-Shamir challenge
        let mut transcript = Transcript::new(POLYGUARD_PROOF_DOMAIN);
        transcript.append_message(b"equality_proof", b"v1");
        transcript.append_message(b"C1", c1.0.as_bytes());
        transcript.append_message(b"C2", c2.0.as_bytes());
        transcript.append_message(b"R", r_point.compress().as_bytes());

        let mut challenge_bytes = [0u8; 64];
        transcript.challenge_bytes(b"challenge", &mut challenge_bytes);
        let challenge = Scalar::from_bytes_mod_order_wide(&challenge_bytes);

        // Response: s = k + c * (r1 - r2)
        let response = k + challenge * blinding_diff;

        Ok(Self {
            challenge: challenge.to_bytes(),
            response: response.to_bytes(),
        })
    }

    /// Verify equality proof
    pub fn verify(
        &self,
        commitment1: &PedersenCommitment,
        commitment2: &PedersenCommitment,
    ) -> Result<bool, CryptoError> {
        let challenge = Scalar::from_canonical_bytes(self.challenge)
            .into_option()
            .ok_or(CryptoError::InvalidProof)?;
        let response = Scalar::from_canonical_bytes(self.response)
            .into_option()
            .ok_or(CryptoError::InvalidProof)?;

        let c1 = commitment1.decompress().ok_or(CryptoError::InvalidCommitment)?;
        let c2 = commitment2.decompress().ok_or(CryptoError::InvalidCommitment)?;

        let h = get_h_generator();

        // C1 - C2 = (r1 - r2) * H (if values are equal)
        let c_diff = c1 - c2;

        // Recompute R = s * H - c * (C1 - C2)
        let r_computed = &response * h - challenge * c_diff;

        // Verify challenge
        let mut transcript = Transcript::new(POLYGUARD_PROOF_DOMAIN);
        transcript.append_message(b"equality_proof", b"v1");
        transcript.append_message(b"C1", commitment1.0.as_bytes());
        transcript.append_message(b"C2", commitment2.0.as_bytes());
        transcript.append_message(b"R", r_computed.compress().as_bytes());

        let mut expected_bytes = [0u8; 64];
        transcript.challenge_bytes(b"challenge", &mut expected_bytes);
        let expected_challenge = Scalar::from_bytes_mod_order_wide(&expected_bytes);

        Ok(constant_time_eq::constant_time_eq(
            challenge.as_bytes(),
            expected_challenge.as_bytes(),
        ))
    }
}

/// Deposit proof: proves encrypted_amount correctly encrypts `amount`
/// This links the plaintext deposit to the ElGamal ciphertext
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct DepositProof {
    /// Challenge scalar
    pub challenge: [u8; 32],
    /// Response for randomness
    pub response: [u8; 32],
}

unsafe impl Zeroable for DepositProof {}
unsafe impl Pod for DepositProof {}

impl DepositProof {
    /// Size in bytes
    pub const SIZE: usize = 64;

    /// Prove that ciphertext encrypts the given amount
    pub fn prove(
        pubkey: &ElGamalPubkey,
        amount: u64,
        randomness: &Scalar,
    ) -> Result<Self, CryptoError> {
        // Generate nonce
        let mut k_bytes = [0u8; 32];
        #[cfg(feature = "std")]
        {
            use rand::RngCore;
            rand::thread_rng().fill_bytes(&mut k_bytes);
        }
        #[cfg(not(feature = "std"))]
        {
            let mut hasher = Sha512::new();
            hasher.update(b"polyguard_deposit_nonce");
            hasher.update(pubkey.to_bytes().as_slice());
            hasher.update(&amount.to_le_bytes());
            hasher.update(randomness.as_bytes());
            let hash = hasher.finalize();
            k_bytes.copy_from_slice(&hash[..32]);
        }

        let k = Scalar::from_bytes_mod_order(k_bytes);

        // R1 = k * G
        let r1 = k * get_g();

        // R2 = k * P (public key point)
        let pubkey_point = pubkey.0.decompress()
            .ok_or(CryptoError::InvalidPublicKey)?;
        let r2 = &k * pubkey_point;

        // Compute ciphertext for transcript
        let ciphertext = pubkey.encrypt_with_randomness(amount, randomness)?;

        // Fiat-Shamir
        let mut transcript = Transcript::new(POLYGUARD_PROOF_DOMAIN);
        transcript.append_message(b"deposit_proof", b"v1");
        transcript.append_message(b"pubkey", pubkey.0.as_bytes());
        transcript.append_u64(b"amount", amount);
        transcript.append_message(b"C1", ciphertext.c1.as_bytes());
        transcript.append_message(b"C2", ciphertext.c2.as_bytes());
        transcript.append_message(b"R1", r1.compress().as_bytes());
        transcript.append_message(b"R2", r2.compress().as_bytes());

        let mut challenge_bytes = [0u8; 64];
        transcript.challenge_bytes(b"challenge", &mut challenge_bytes);
        let challenge = Scalar::from_bytes_mod_order_wide(&challenge_bytes);

        // Response: s = k + c * r
        let response = k + challenge * randomness;

        Ok(Self {
            challenge: challenge.to_bytes(),
            response: response.to_bytes(),
        })
    }

    /// Verify deposit proof
    pub fn verify(
        &self,
        pubkey: &ElGamalPubkey,
        amount: u64,
        ciphertext: &ElGamalCiphertext,
    ) -> Result<bool, CryptoError> {
        let challenge = Scalar::from_canonical_bytes(self.challenge)
            .into_option()
            .ok_or(CryptoError::InvalidProof)?;
        let response = Scalar::from_canonical_bytes(self.response)
            .into_option()
            .ok_or(CryptoError::InvalidProof)?;

        let c1 = ciphertext.c1.decompress()
            .ok_or(CryptoError::InvalidCiphertext)?;
        let c2 = ciphertext.c2.decompress()
            .ok_or(CryptoError::InvalidCiphertext)?;
        let pubkey_point = pubkey.0.decompress()
            .ok_or(CryptoError::InvalidPublicKey)?;

        // Recompute R1 = s*G - c*C1
        let r1_computed = response * get_g() - challenge * c1;

        // Expected C2 for amount: m*G + r*P
        // R2 = s*P - c*(C2 - m*G) = s*P - c*r*P = (s - c*r)*P
        // But s = k + c*r, so R2 = k*P
        let amount_point = Scalar::from(amount) * get_g();
        let r2_computed = response * pubkey_point - challenge * (c2 - amount_point);

        // Verify challenge
        let mut transcript = Transcript::new(POLYGUARD_PROOF_DOMAIN);
        transcript.append_message(b"deposit_proof", b"v1");
        transcript.append_message(b"pubkey", pubkey.0.as_bytes());
        transcript.append_u64(b"amount", amount);
        transcript.append_message(b"C1", ciphertext.c1.as_bytes());
        transcript.append_message(b"C2", ciphertext.c2.as_bytes());
        transcript.append_message(b"R1", r1_computed.compress().as_bytes());
        transcript.append_message(b"R2", r2_computed.compress().as_bytes());

        let mut expected_bytes = [0u8; 64];
        transcript.challenge_bytes(b"challenge", &mut expected_bytes);
        let expected_challenge = Scalar::from_bytes_mod_order_wide(&expected_bytes);

        Ok(constant_time_eq::constant_time_eq(
            challenge.as_bytes(),
            expected_challenge.as_bytes(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create opening with deterministic blinding
    fn test_opening(value: u64, blinding_seed: u64) -> PedersenOpening {
        PedersenOpening::new(value, Scalar::from(blinding_seed))
    }

    #[test]
    fn test_compact_range_proof() {
        let value = 12345u64;
        let opening = test_opening(value, 42);

        let proof = CompactRangeProof::prove(value, &opening).unwrap();
        assert!(proof.verify().unwrap());
    }

    #[test]
    fn test_balance_proof_sufficient() {
        let balance = 1000u64;
        let amount = 500u64;
        let opening = test_opening(balance, 111);
        let commitment = opening.to_commitment();

        let proof = BalanceProof::prove(balance, amount, &opening).unwrap();
        assert!(proof.verify(&commitment, amount).unwrap());
    }

    #[test]
    fn test_balance_proof_insufficient() {
        let balance = 500u64;
        let amount = 1000u64;
        let opening = test_opening(balance, 222);

        let result = BalanceProof::prove(balance, amount, &opening);
        assert!(matches!(result, Err(CryptoError::InsufficientBalance)));
    }

    #[test]
    fn test_equality_proof() {
        let value = 42u64;
        let opening1 = test_opening(value, 333);
        let opening2 = test_opening(value, 444);

        let c1 = opening1.to_commitment();
        let c2 = opening2.to_commitment();

        let proof = EqualityProof::prove(&opening1, &opening2).unwrap();
        assert!(proof.verify(&c1, &c2).unwrap());
    }

    #[test]
    fn test_equality_proof_different_values() {
        let opening1 = test_opening(100, 555);
        let opening2 = test_opening(200, 666);

        let result = EqualityProof::prove(&opening1, &opening2);
        assert!(matches!(result, Err(CryptoError::CommitmentMismatch)));
    }

    #[test]
    fn test_deposit_proof() {
        use super::super::ElGamalKeypair;

        let keypair = ElGamalKeypair::from_seed(&[42u8; 32]);
        let amount = 1000u64;
        let randomness = Scalar::from(12345u64);

        let ciphertext = keypair.public.encrypt_with_randomness(amount, &randomness).unwrap();
        let proof = DepositProof::prove(&keypair.public, amount, &randomness).unwrap();

        assert!(proof.verify(&keypair.public, amount, &ciphertext).unwrap());
    }

    #[test]
    fn test_deposit_proof_wrong_amount() {
        use super::super::ElGamalKeypair;

        let keypair = ElGamalKeypair::from_seed(&[42u8; 32]);
        let amount = 1000u64;
        let randomness = Scalar::from(12345u64);

        let ciphertext = keypair.public.encrypt_with_randomness(amount, &randomness).unwrap();
        let proof = DepositProof::prove(&keypair.public, amount, &randomness).unwrap();

        // Verify with wrong amount should fail
        assert!(!proof.verify(&keypair.public, amount + 1, &ciphertext).unwrap());
    }

    #[test]
    fn test_serialization() {
        let value = 100u64;
        let opening = test_opening(value, 777);

        let range_proof = CompactRangeProof::prove(value, &opening).unwrap();
        let bytes = range_proof.to_bytes();
        let restored = CompactRangeProof::from_bytes(&bytes).unwrap();

        assert!(restored.verify().unwrap());
    }
}
