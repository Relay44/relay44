//! Cryptographic error types

use core::fmt;

/// Errors that can occur during cryptographic operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// Invalid point encoding (not on curve)
    InvalidPoint,
    /// Invalid scalar encoding
    InvalidScalar,
    /// Invalid public key
    InvalidPublicKey,
    /// Invalid ciphertext format
    InvalidCiphertext,
    /// Invalid commitment format
    InvalidCommitment,
    /// Invalid proof format
    InvalidProof,
    /// Proof verification failed
    ProofVerificationFailed,
    /// Amount exceeds maximum (discrete log not feasible)
    AmountTooLarge,
    /// Insufficient balance for operation
    InsufficientBalance,
    /// Commitment mismatch in proof
    CommitmentMismatch,
    /// Randomness generation failed
    RandomnessError,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CryptoError::InvalidPoint => write!(f, "Invalid point encoding"),
            CryptoError::InvalidScalar => write!(f, "Invalid scalar encoding"),
            CryptoError::InvalidPublicKey => write!(f, "Invalid public key"),
            CryptoError::InvalidCiphertext => write!(f, "Invalid ciphertext format"),
            CryptoError::InvalidCommitment => write!(f, "Invalid commitment format"),
            CryptoError::InvalidProof => write!(f, "Invalid proof format"),
            CryptoError::ProofVerificationFailed => write!(f, "Proof verification failed"),
            CryptoError::AmountTooLarge => write!(f, "Amount exceeds maximum for decryption"),
            CryptoError::InsufficientBalance => write!(f, "Insufficient balance"),
            CryptoError::CommitmentMismatch => write!(f, "Commitment mismatch"),
            CryptoError::RandomnessError => write!(f, "Randomness generation failed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CryptoError {}
