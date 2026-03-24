use anchor_lang::prelude::*;

#[error_code]
pub enum PrivacyError {
    #[msg("Invalid ElGamal public key")]
    InvalidElGamalKey,

    #[msg("Invalid encrypted amount")]
    InvalidEncryptedAmount,

    #[msg("Invalid balance proof")]
    InvalidBalanceProof,

    #[msg("Invalid range proof")]
    InvalidRangeProof,

    #[msg("Invalid price commitment")]
    InvalidPriceCommitment,

    #[msg("Invalid quantity commitment")]
    InvalidQuantityCommitment,

    #[msg("Invalid MXE result")]
    InvalidMxeResult,

    #[msg("Invalid settlement proof")]
    InvalidSettlementProof,

    #[msg("Insufficient encrypted balance")]
    InsufficientEncryptedBalance,

    #[msg("Private account not initialized")]
    PrivateAccountNotInitialized,

    #[msg("Private account already exists")]
    PrivateAccountAlreadyExists,

    #[msg("Unauthorized: only admin can perform this action")]
    UnauthorizedAdmin,

    #[msg("Unauthorized: only MXE authority can settle")]
    UnauthorizedMxeAuthority,

    #[msg("MXE computation failed")]
    MxeComputationFailed,

    #[msg("Proof verification failed")]
    ProofVerificationFailed,

    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("Invalid order side")]
    InvalidOrderSide,

    #[msg("Invalid outcome type")]
    InvalidOutcomeType,

    #[msg("Invalid MXE signature")]
    InvalidMxeSignature,
}
