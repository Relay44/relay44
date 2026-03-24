//! Pedersen Commitments
//!
//! C = v*G + r*H
//!
//! Properties: perfect hiding, computational binding, homomorphic.

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

/// Pedersen commitment (32 bytes compressed)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PedersenCommitment(pub CompressedRistretto);

unsafe impl Zeroable for PedersenCommitment {}
unsafe impl Pod for PedersenCommitment {}

/// Opening for a Pedersen commitment (value + blinding factor)
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct PedersenOpening {
    pub value: u64,
    pub blinding: Scalar,
}

/// Get the secondary generator H via hash-to-curve
/// This is deterministically derived and the discrete log relative to G is unknown
pub fn get_h_generator() -> RistrettoPoint {
    // Domain separation for Polyguard Pedersen commitments
    let mut hasher = Sha512::new();
    hasher.update(b"polyguard_pedersen_generator_h_v1");
    let hash = hasher.finalize();

    // Use hash_from_bytes which is a valid hash-to-curve method
    RistrettoPoint::from_uniform_bytes(&hash.into())
}

impl PedersenCommitment {
    /// Create a commitment with random blinding factor
    #[cfg(feature = "std")]
    pub fn commit(value: u64) -> Result<(Self, PedersenOpening), CryptoError> {
        use rand::RngCore;
        let mut r_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut r_bytes);

        let blinding = Scalar::from_bytes_mod_order(r_bytes);
        let commitment = Self::commit_with_blinding(value, &blinding);
        let opening = PedersenOpening { value, blinding };

        Ok((commitment, opening))
    }

    /// Create a commitment with random blinding factor (no_std version - requires external randomness)
    #[cfg(not(feature = "std"))]
    pub fn commit(_value: u64) -> Result<(Self, PedersenOpening), CryptoError> {
        // For no_std, caller must use commit_with_blinding and provide randomness
        Err(CryptoError::RandomnessError)
    }

    /// Create a commitment with specific blinding factor
    pub fn commit_with_blinding(value: u64, blinding: &Scalar) -> Self {
        let v = Scalar::from(value);
        let h = get_h_generator();

        // C = v*G + r*H
        let point = v * RISTRETTO_BASEPOINT_POINT + blinding * h;

        Self(point.compress())
    }

    /// Create a commitment to zero with zero blinding (identity)
    pub fn zero() -> Self {
        Self(CompressedRistretto::identity())
    }

    /// Create a commitment to a value with zero blinding (publicly verifiable)
    /// WARNING: This reveals the value! Use only for known public values.
    pub fn commit_public(value: u64) -> Self {
        Self::commit_with_blinding(value, &Scalar::ZERO)
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        let compressed = CompressedRistretto::from_slice(bytes)
            .map_err(|_| CryptoError::InvalidCommitment)?;
        // Verify it's a valid point
        compressed.decompress().ok_or(CryptoError::InvalidCommitment)?;
        Ok(Self(compressed))
    }

    /// Convert to raw bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        *self.0.as_bytes()
    }

    /// Verify that an opening matches this commitment
    pub fn verify(&self, opening: &PedersenOpening) -> bool {
        let expected = Self::commit_with_blinding(opening.value, &opening.blinding);
        // Use constant-time comparison
        constant_time_eq::constant_time_eq(self.0.as_bytes(), expected.0.as_bytes())
    }

    /// Homomorphic addition of commitments
    /// C(a, r1) + C(b, r2) = C(a+b, r1+r2)
    pub fn add(&self, other: &PedersenCommitment) -> Result<Self, CryptoError> {
        let p1 = self.0.decompress().ok_or(CryptoError::InvalidCommitment)?;
        let p2 = other.0.decompress().ok_or(CryptoError::InvalidCommitment)?;

        Ok(Self((p1 + p2).compress()))
    }

    /// Homomorphic subtraction of commitments
    /// C(a, r1) - C(b, r2) = C(a-b, r1-r2)
    pub fn subtract(&self, other: &PedersenCommitment) -> Result<Self, CryptoError> {
        let p1 = self.0.decompress().ok_or(CryptoError::InvalidCommitment)?;
        let p2 = other.0.decompress().ok_or(CryptoError::InvalidCommitment)?;

        Ok(Self((p1 - p2).compress()))
    }

    /// Scalar multiplication
    /// k * C(v, r) = C(k*v, k*r)
    pub fn scalar_mult(&self, scalar: u64) -> Result<Self, CryptoError> {
        let p = self.0.decompress().ok_or(CryptoError::InvalidCommitment)?;
        let k = Scalar::from(scalar);

        Ok(Self((k * p).compress()))
    }

    /// Subtract a public value from a commitment
    /// C(v, r) - v' = C(v - v', r)
    /// Used to validate that a proof's difference commitment matches expected
    pub fn subtract_value(&self, value: u64) -> Result<Self, CryptoError> {
        let p = self.0.decompress().ok_or(CryptoError::InvalidCommitment)?;
        let v_point = Scalar::from(value) * RISTRETTO_BASEPOINT_POINT;

        Ok(Self((p - v_point).compress()))
    }

    /// Check if this is a commitment to zero
    /// Note: Only works if you know the blinding factor is zero
    pub fn is_identity(&self) -> bool {
        self.0 == CompressedRistretto::identity()
    }

    /// Extract the point (for use in proofs)
    pub fn decompress(&self) -> Option<RistrettoPoint> {
        self.0.decompress()
    }
}

impl PedersenOpening {
    /// Create a new opening
    pub fn new(value: u64, blinding: Scalar) -> Self {
        Self { value, blinding }
    }

    /// Create opening with random blinding
    #[cfg(feature = "std")]
    pub fn random(value: u64) -> Result<Self, CryptoError> {
        use rand::RngCore;
        let mut r_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut r_bytes);

        Ok(Self {
            value,
            blinding: Scalar::from_bytes_mod_order(r_bytes),
        })
    }

    /// Create opening with random blinding (no_std version)
    #[cfg(not(feature = "std"))]
    pub fn random(_value: u64) -> Result<Self, CryptoError> {
        Err(CryptoError::RandomnessError)
    }

    /// Create opening with zero blinding (for public values)
    pub fn public(value: u64) -> Self {
        Self {
            value,
            blinding: Scalar::ZERO,
        }
    }

    /// Add two openings (corresponds to adding commitments)
    /// Uses wrapping arithmetic - for cryptographic operations only
    pub fn add(&self, other: &PedersenOpening) -> Self {
        Self {
            value: self.value.wrapping_add(other.value),
            blinding: self.blinding + other.blinding,
        }
    }

    /// Add two openings with overflow checking
    /// Returns None if the value would overflow
    pub fn checked_add(&self, other: &PedersenOpening) -> Option<Self> {
        self.value.checked_add(other.value).map(|value| Self {
            value,
            blinding: self.blinding + other.blinding,
        })
    }

    /// Subtract openings (corresponds to subtracting commitments)
    /// Uses wrapping arithmetic - for cryptographic operations only
    pub fn subtract(&self, other: &PedersenOpening) -> Self {
        Self {
            value: self.value.wrapping_sub(other.value),
            blinding: self.blinding - other.blinding,
        }
    }

    /// Subtract openings with underflow checking
    /// Returns None if the value would underflow
    pub fn checked_subtract(&self, other: &PedersenOpening) -> Option<Self> {
        self.value.checked_sub(other.value).map(|value| Self {
            value,
            blinding: self.blinding - other.blinding,
        })
    }

    /// Get the commitment for this opening
    pub fn to_commitment(&self) -> PedersenCommitment {
        PedersenCommitment::commit_with_blinding(self.value, &self.blinding)
    }
}

/// Verify that a sum of commitments equals expected value
/// Useful for verifying transaction balance (inputs = outputs + fee)
pub fn verify_balance(
    inputs: &[PedersenCommitment],
    outputs: &[PedersenCommitment],
    excess: &PedersenCommitment,
) -> Result<bool, CryptoError> {
    // Sum inputs
    let mut input_sum = RistrettoPoint::identity();
    for c in inputs {
        input_sum += c.0.decompress().ok_or(CryptoError::InvalidCommitment)?;
    }

    // Sum outputs + excess
    let mut output_sum = RistrettoPoint::identity();
    for c in outputs {
        output_sum += c.0.decompress().ok_or(CryptoError::InvalidCommitment)?;
    }
    output_sum += excess.0.decompress().ok_or(CryptoError::InvalidCommitment)?;

    // Input sum should equal output sum
    Ok(input_sum == output_sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create opening with deterministic blinding for tests
    fn test_commit(value: u64, blinding_seed: u64) -> (PedersenCommitment, PedersenOpening) {
        let blinding = Scalar::from(blinding_seed);
        let commitment = PedersenCommitment::commit_with_blinding(value, &blinding);
        let opening = PedersenOpening::new(value, blinding);
        (commitment, opening)
    }

    #[test]
    fn test_commit_verify() {
        let value = 12345u64;
        let (commitment, opening) = test_commit(value, 42);

        assert!(commitment.verify(&opening));

        // Wrong value should not verify
        let wrong_opening = PedersenOpening::new(value + 1, opening.blinding.clone());
        assert!(!commitment.verify(&wrong_opening));
    }

    #[test]
    fn test_deterministic_commitment() {
        let value = 1000u64;
        let blinding = Scalar::from(42u64);

        let c1 = PedersenCommitment::commit_with_blinding(value, &blinding);
        let c2 = PedersenCommitment::commit_with_blinding(value, &blinding);

        assert_eq!(c1, c2);
    }

    #[test]
    fn test_homomorphic_addition() {
        let a = 1000u64;
        let b = 2000u64;

        let (c_a, o_a) = test_commit(a, 111);
        let (c_b, o_b) = test_commit(b, 222);

        let c_sum = c_a.add(&c_b).unwrap();
        let o_sum = o_a.add(&o_b);

        assert!(c_sum.verify(&o_sum));
        assert_eq!(o_sum.value, a + b);
    }

    #[test]
    fn test_homomorphic_subtraction() {
        let a = 3000u64;
        let b = 1000u64;

        let (c_a, o_a) = test_commit(a, 333);
        let (c_b, o_b) = test_commit(b, 444);

        let c_diff = c_a.subtract(&c_b).unwrap();
        let o_diff = o_a.subtract(&o_b);

        assert!(c_diff.verify(&o_diff));
        assert_eq!(o_diff.value, a - b);
    }

    #[test]
    fn test_serialization() {
        let value = 42u64;
        let (commitment, _) = test_commit(value, 555);

        let bytes = commitment.to_bytes();
        let restored = PedersenCommitment::from_bytes(&bytes).unwrap();

        assert_eq!(commitment, restored);
    }

    #[test]
    fn test_public_commitment() {
        let value = 100u64;
        let c = PedersenCommitment::commit_public(value);
        let opening = PedersenOpening::public(value);

        assert!(c.verify(&opening));
    }

    #[test]
    fn test_balance_verification() {
        // Simulate: 1000 + 2000 = 2500 + 500
        let (c_in1, o_in1) = test_commit(1000, 11);
        let (c_in2, o_in2) = test_commit(2000, 22);
        let (c_out1, o_out1) = test_commit(2500, 33);
        let (c_out2, o_out2) = test_commit(500, 44);

        // Excess commitment = sum(input blindings) - sum(output blindings)
        let excess_blinding = o_in1.blinding + o_in2.blinding - o_out1.blinding - o_out2.blinding;
        let excess = PedersenCommitment::commit_with_blinding(0, &excess_blinding);

        assert!(verify_balance(
            &[c_in1, c_in2],
            &[c_out1, c_out2],
            &excess,
        ).unwrap());
    }
}
