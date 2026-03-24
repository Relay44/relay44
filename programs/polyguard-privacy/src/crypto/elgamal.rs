//! Twisted ElGamal on Ristretto255
//!
//! Additively homomorphic encryption for confidential balances.
//! Encrypts as (r*G, m*G + r*P) where m is the amount.
//!
//! - Homomorphic ciphertext addition
//! - Decryption via discrete log (small amounts only)
//! - ZK proofs of plaintext properties

use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
    traits::Identity,
};
use sha2::{Digest, Sha512};
use zeroize::{Zeroize, ZeroizeOnDrop};
use bytemuck::{Pod, Zeroable};

use super::CryptoError;

/// Maximum amount that can be decrypted via discrete log lookup
/// 2^32 allows up to ~4 billion base units (sufficient for most tokens)
pub const MAX_DECRYPTABLE_AMOUNT: u64 = 1u64 << 32;

/// ElGamal public key (32 bytes compressed Ristretto point)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct ElGamalPubkey(pub CompressedRistretto);

/// ElGamal secret key (32-byte scalar)
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct ElGamalSecretKey(pub Scalar);

/// ElGamal keypair
pub struct ElGamalKeypair {
    pub public: ElGamalPubkey,
    pub secret: ElGamalSecretKey,
}

/// ElGamal ciphertext (64 bytes: two compressed Ristretto points)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct ElGamalCiphertext {
    /// Randomness component: r * G
    pub c1: CompressedRistretto,
    /// Message component: m * G + r * P
    pub c2: CompressedRistretto,
}

// Implement Pod/Zeroable for on-chain storage compatibility
unsafe impl Zeroable for ElGamalCiphertext {}
unsafe impl Pod for ElGamalCiphertext {}

unsafe impl Zeroable for ElGamalPubkey {}
unsafe impl Pod for ElGamalPubkey {}

impl ElGamalKeypair {
    /// Generate a new random keypair (off-chain only)
    #[cfg(feature = "std")]
    pub fn new_rand() -> Self {
        use rand::RngCore;
        let mut seed = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut seed);
        Self::from_seed(&seed)
    }

    /// Generate keypair from 32-byte seed (deterministic)
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let secret = Scalar::from_bytes_mod_order(*seed);
        let public_point = secret * RISTRETTO_BASEPOINT_POINT;
        Self {
            public: ElGamalPubkey(public_point.compress()),
            secret: ElGamalSecretKey(secret),
        }
    }

    /// Derive keypair from a signature (for wallet-based key derivation)
    pub fn from_signature(signature: &[u8; 64]) -> Self {
        let mut hasher = Sha512::new();
        hasher.update(b"polyguard_elgamal_key_derivation");
        hasher.update(signature);
        let hash = hasher.finalize();

        let mut seed = [0u8; 32];
        seed.copy_from_slice(&hash[..32]);
        Self::from_seed(&seed)
    }
}

impl ElGamalPubkey {
    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        let compressed = CompressedRistretto::from_slice(bytes)
            .map_err(|_| CryptoError::InvalidPoint)?;
        // Verify it's a valid point
        compressed.decompress().ok_or(CryptoError::InvalidPublicKey)?;
        Ok(Self(compressed))
    }

    /// Convert to raw bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        *self.0.as_bytes()
    }

    /// Encrypt an amount using system randomness (off-chain only)
    #[cfg(feature = "std")]
    pub fn encrypt(&self, amount: u64) -> Result<ElGamalCiphertext, CryptoError> {
        use rand::RngCore;
        let mut r_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut r_bytes);
        let r = Scalar::from_bytes_mod_order(r_bytes);
        self.encrypt_with_randomness(amount, &r)
    }

    /// Encrypt an amount using provided entropy (for on-chain use)
    /// The entropy should come from a combination of:
    /// - Recent blockhash / slot hash
    /// - Transaction signature
    /// - User-provided randomness
    pub fn encrypt_with_entropy(
        &self,
        amount: u64,
        entropy: &[u8; 32],
    ) -> Result<ElGamalCiphertext, CryptoError> {
        let r = Scalar::from_bytes_mod_order(*entropy);
        self.encrypt_with_randomness(amount, &r)
    }

    /// Encrypt with specific randomness (for deterministic encryption or proofs)
    pub fn encrypt_with_randomness(
        &self,
        amount: u64,
        randomness: &Scalar,
    ) -> Result<ElGamalCiphertext, CryptoError> {
        let pubkey_point = self.0.decompress()
            .ok_or(CryptoError::InvalidPublicKey)?;

        let amount_scalar = Scalar::from(amount);

        // c1 = r * G
        let c1 = randomness * RISTRETTO_BASEPOINT_POINT;

        // c2 = m * G + r * P
        let c2 = amount_scalar * RISTRETTO_BASEPOINT_POINT + randomness * pubkey_point;

        Ok(ElGamalCiphertext {
            c1: c1.compress(),
            c2: c2.compress(),
        })
    }

    /// Encrypt zero with zero randomness (for initialization)
    pub fn encrypt_zero(&self) -> ElGamalCiphertext {
        ElGamalCiphertext {
            c1: CompressedRistretto::identity(),
            c2: CompressedRistretto::identity(),
        }
    }
}

impl ElGamalSecretKey {
    /// Decrypt a ciphertext
    /// Returns None if the amount exceeds MAX_DECRYPTABLE_AMOUNT
    pub fn decrypt(&self, ciphertext: &ElGamalCiphertext) -> Result<u64, CryptoError> {
        let c1 = ciphertext.c1.decompress()
            .ok_or(CryptoError::InvalidCiphertext)?;
        let c2 = ciphertext.c2.decompress()
            .ok_or(CryptoError::InvalidCiphertext)?;

        // m * G = c2 - s * c1
        let m_point = c2 - &self.0 * c1;

        // Solve discrete log for small amounts using baby-step giant-step
        discrete_log(&m_point).ok_or(CryptoError::AmountTooLarge)
    }

    /// Verify a ciphertext decrypts to expected amount (constant-time)
    pub fn verify_amount(
        &self,
        ciphertext: &ElGamalCiphertext,
        expected_amount: u64,
    ) -> Result<bool, CryptoError> {
        let c1 = ciphertext.c1.decompress()
            .ok_or(CryptoError::InvalidCiphertext)?;
        let c2 = ciphertext.c2.decompress()
            .ok_or(CryptoError::InvalidCiphertext)?;

        // m * G = c2 - s * c1
        let m_point = c2 - self.0 * c1;

        // Expected: expected_amount * G
        let expected_point = Scalar::from(expected_amount) * RISTRETTO_BASEPOINT_POINT;

        Ok(m_point == expected_point)
    }
}

impl ElGamalCiphertext {
    /// Create from raw bytes (64 bytes)
    pub fn from_bytes(bytes: &[u8; 64]) -> Result<Self, CryptoError> {
        let c1 = CompressedRistretto::from_slice(&bytes[0..32])
            .map_err(|_| CryptoError::InvalidCiphertext)?;
        let c2 = CompressedRistretto::from_slice(&bytes[32..64])
            .map_err(|_| CryptoError::InvalidCiphertext)?;

        // Validate both points are valid
        c1.decompress().ok_or(CryptoError::InvalidCiphertext)?;
        c2.decompress().ok_or(CryptoError::InvalidCiphertext)?;

        Ok(Self { c1, c2 })
    }

    /// Convert to raw bytes (64 bytes)
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut bytes = [0u8; 64];
        bytes[0..32].copy_from_slice(self.c1.as_bytes());
        bytes[32..64].copy_from_slice(self.c2.as_bytes());
        bytes
    }

    /// Encryption of zero with zero randomness
    pub fn zero() -> Self {
        Self {
            c1: CompressedRistretto::identity(),
            c2: CompressedRistretto::identity(),
        }
    }

    /// Homomorphic addition of ciphertexts
    /// Enc(a) + Enc(b) = Enc(a + b)
    pub fn add(&self, other: &ElGamalCiphertext) -> Result<Self, CryptoError> {
        let c1_a = self.c1.decompress().ok_or(CryptoError::InvalidCiphertext)?;
        let c1_b = other.c1.decompress().ok_or(CryptoError::InvalidCiphertext)?;
        let c2_a = self.c2.decompress().ok_or(CryptoError::InvalidCiphertext)?;
        let c2_b = other.c2.decompress().ok_or(CryptoError::InvalidCiphertext)?;

        Ok(Self {
            c1: (c1_a + c1_b).compress(),
            c2: (c2_a + c2_b).compress(),
        })
    }

    /// Homomorphic subtraction of ciphertexts
    /// Enc(a) - Enc(b) = Enc(a - b)
    pub fn subtract(&self, other: &ElGamalCiphertext) -> Result<Self, CryptoError> {
        let c1_a = self.c1.decompress().ok_or(CryptoError::InvalidCiphertext)?;
        let c1_b = other.c1.decompress().ok_or(CryptoError::InvalidCiphertext)?;
        let c2_a = self.c2.decompress().ok_or(CryptoError::InvalidCiphertext)?;
        let c2_b = other.c2.decompress().ok_or(CryptoError::InvalidCiphertext)?;

        Ok(Self {
            c1: (c1_a - c1_b).compress(),
            c2: (c2_a - c2_b).compress(),
        })
    }

    /// Scalar multiplication of ciphertext
    /// k * Enc(a) = Enc(k * a)
    pub fn scalar_mult(&self, scalar: u64) -> Result<Self, CryptoError> {
        let c1 = self.c1.decompress().ok_or(CryptoError::InvalidCiphertext)?;
        let c2 = self.c2.decompress().ok_or(CryptoError::InvalidCiphertext)?;
        let k = Scalar::from(scalar);

        Ok(Self {
            c1: (k * c1).compress(),
            c2: (k * c2).compress(),
        })
    }
}

/// Cached baby-step lookup table for discrete log
/// HIGH-011 FIX: Use lazy static initialization to build table once
#[cfg(feature = "std")]
mod dlog_cache {
    use super::*;
    use std::sync::OnceLock;

    /// Baby step size: sqrt(MAX_DECRYPTABLE_AMOUNT) = 2^16
    pub const BABY_STEP_SIZE: u64 = 65536;

    /// Cached baby steps table
    static BABY_STEPS: OnceLock<Vec<(CompressedRistretto, u64)>> = OnceLock::new();

    /// Get or initialize the baby steps table
    pub fn get_baby_steps() -> &'static [(CompressedRistretto, u64)] {
        BABY_STEPS.get_or_init(|| {
            let mut table = Vec::with_capacity(BABY_STEP_SIZE as usize);
            let mut current = RistrettoPoint::identity();
            for i in 0..BABY_STEP_SIZE {
                table.push((current.compress(), i));
                current += RISTRETTO_BASEPOINT_POINT;
            }
            table.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            table
        })
    }
}

/// Baby-step giant-step discrete log solver for small amounts
/// HIGH-011 FIX: Uses cached lookup table for performance
fn discrete_log(point: &RistrettoPoint) -> Option<u64> {
    // Check for zero first
    if *point == RistrettoPoint::identity() {
        return Some(0);
    }

    // Baby step size: sqrt(MAX_DECRYPTABLE_AMOUNT)
    let baby_step_size = 65536u64; // 2^16
    let giant_step_size = (MAX_DECRYPTABLE_AMOUNT / baby_step_size) + 1;

    // Base point G
    let g = RISTRETTO_BASEPOINT_POINT;

    // Get baby steps table (cached in std, built fresh in no_std)
    #[cfg(feature = "std")]
    let baby_steps = dlog_cache::get_baby_steps();

    #[cfg(not(feature = "std"))]
    let baby_steps = {
        let mut table = alloc::vec::Vec::with_capacity(baby_step_size as usize);
        let mut current = RistrettoPoint::identity();
        for i in 0..baby_step_size {
            table.push((current.compress(), i));
            current += g;
        }
        table.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        table
    };

    // Giant step: -baby_step_size * G
    let giant_step = -Scalar::from(baby_step_size) * g;

    // Search
    let mut gamma = *point;
    for j in 0..giant_step_size {
        // Binary search in baby steps
        if let Ok(idx) = baby_steps.binary_search_by(|probe| {
            probe.0.as_bytes().cmp(gamma.compress().as_bytes())
        }) {
            let i = baby_steps[idx].1;
            let result = j * baby_step_size + i;
            if result <= MAX_DECRYPTABLE_AMOUNT {
                return Some(result);
            }
        }
        gamma += giant_step;
    }

    None
}

// Require alloc for the discrete log lookup table
extern crate alloc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let seed = [42u8; 32];
        let keypair = ElGamalKeypair::from_seed(&seed);

        // Verify public key is valid
        assert!(keypair.public.0.decompress().is_some());
    }

    #[test]
    fn test_encrypt_decrypt_zero() {
        let seed = [42u8; 32];
        let keypair = ElGamalKeypair::from_seed(&seed);

        let ciphertext = keypair.public.encrypt_with_randomness(0, &Scalar::from(123u64)).unwrap();
        let decrypted = keypair.secret.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, 0);
    }

    #[test]
    fn test_encrypt_decrypt_small() {
        let seed = [42u8; 32];
        let keypair = ElGamalKeypair::from_seed(&seed);

        for amount in [1, 10, 100, 1000, 10000] {
            let ciphertext = keypair.public.encrypt_with_randomness(amount, &Scalar::from(456u64)).unwrap();
            let decrypted = keypair.secret.decrypt(&ciphertext).unwrap();
            assert_eq!(decrypted, amount);
        }
    }

    #[test]
    fn test_homomorphic_addition() {
        let seed = [42u8; 32];
        let keypair = ElGamalKeypair::from_seed(&seed);

        let a = 1000u64;
        let b = 2000u64;

        let c1 = keypair.public.encrypt_with_randomness(a, &Scalar::from(111u64)).unwrap();
        let c2 = keypair.public.encrypt_with_randomness(b, &Scalar::from(222u64)).unwrap();
        let sum = c1.add(&c2).unwrap();

        let decrypted = keypair.secret.decrypt(&sum).unwrap();
        assert_eq!(decrypted, a + b);
    }

    #[test]
    fn test_homomorphic_subtraction() {
        let seed = [42u8; 32];
        let keypair = ElGamalKeypair::from_seed(&seed);

        let a = 3000u64;
        let b = 1000u64;

        let c1 = keypair.public.encrypt_with_randomness(a, &Scalar::from(111u64)).unwrap();
        let c2 = keypair.public.encrypt_with_randomness(b, &Scalar::from(222u64)).unwrap();
        let diff = c1.subtract(&c2).unwrap();

        let decrypted = keypair.secret.decrypt(&diff).unwrap();
        assert_eq!(decrypted, a - b);
    }

    #[test]
    fn test_serialization() {
        let seed = [42u8; 32];
        let keypair = ElGamalKeypair::from_seed(&seed);

        let ciphertext = keypair.public.encrypt_with_randomness(42, &Scalar::from(789u64)).unwrap();
        let bytes = ciphertext.to_bytes();
        let restored = ElGamalCiphertext::from_bytes(&bytes).unwrap();

        assert_eq!(ciphertext.c1, restored.c1);
        assert_eq!(ciphertext.c2, restored.c2);
    }

    #[test]
    fn test_verify_amount() {
        let seed = [42u8; 32];
        let keypair = ElGamalKeypair::from_seed(&seed);

        let amount = 12345u64;
        let ciphertext = keypair.public.encrypt_with_randomness(amount, &Scalar::from(999u64)).unwrap();

        assert!(keypair.secret.verify_amount(&ciphertext, amount).unwrap());
        assert!(!keypair.secret.verify_amount(&ciphertext, amount + 1).unwrap());
    }
}
