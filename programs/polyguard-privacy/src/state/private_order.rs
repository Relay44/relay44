use anchor_lang::prelude::*;
use crate::crypto::PedersenCommitment;

#[account]
#[derive(InitSpace)]
pub struct PrivateOrder {
    /// Order owner
    pub owner: Pubkey,

    /// Associated market
    pub market: Pubkey,

    /// Order ID
    pub order_id: u64,

    /// Order side (0 = Buy, 1 = Sell)
    pub side: u8,

    /// Outcome (0 = Yes, 1 = No)
    pub outcome: u8,

    /// Pedersen commitment to price
    pub price_commitment: [u8; 32],

    /// Pedersen commitment to quantity
    pub quantity_commitment: [u8; 32],

    /// Range proof for valid price/quantity
    pub range_proof: [u8; 128],

    /// Order status (0 = Open, 1 = Filled, 2 = Cancelled)
    pub status: u8,

    /// Plaintext hints (for MVP matching - removed in production)
    /// These help the backend match orders without full MPC
    pub price_hint_bps: u16,
    pub quantity_hint: u64,

    /// Bump seed
    pub bump: u8,

    /// Creation timestamp
    pub created_at: i64,

    /// Settlement timestamp
    pub settled_at: i64,
}

impl PrivateOrder {
    pub const SEED_PREFIX: &'static [u8] = b"private_order";

    pub const STATUS_OPEN: u8 = 0;
    pub const STATUS_FILLED: u8 = 1;
    pub const STATUS_CANCELLED: u8 = 2;

    pub const SIDE_BUY: u8 = 0;
    pub const SIDE_SELL: u8 = 1;

    pub const OUTCOME_YES: u8 = 0;
    pub const OUTCOME_NO: u8 = 1;

    /// Create a Pedersen commitment: commitment = value * G + blinding * H
    pub fn create_commitment(value: u64, blinding: &[u8; 32]) -> [u8; 32] {
        use curve25519_dalek::scalar::Scalar;
        let blinding_scalar = Scalar::from_bytes_mod_order(*blinding);
        let commitment = PedersenCommitment::commit_with_blinding(value, &blinding_scalar);
        commitment.to_bytes()
    }

    /// Verify a commitment matches the value and blinding factor
    pub fn verify_commitment(
        commitment: &[u8; 32],
        value: u64,
        blinding: &[u8; 32],
    ) -> bool {
        use curve25519_dalek::scalar::Scalar;
        let blinding_scalar = Scalar::from_bytes_mod_order(*blinding);
        let expected = PedersenCommitment::commit_with_blinding(value, &blinding_scalar);
        match PedersenCommitment::from_bytes(commitment) {
            Ok(stored) => expected.0 == stored.0,
            Err(_) => false,
        }
    }
}

#[account]
#[derive(InitSpace)]
pub struct PrivateSettlement {
    /// Buy order
    pub buy_order: Pubkey,

    /// Sell order
    pub sell_order: Pubkey,

    /// Market
    pub market: Pubkey,

    /// MXE computation result
    pub mxe_result: [u8; 256],

    /// Settlement proof
    pub settlement_proof: [u8; 128],

    /// Encrypted fill quantity
    pub encrypted_fill_quantity: [u8; 64],

    /// Encrypted fill price
    pub encrypted_fill_price: [u8; 64],

    /// Settlement status
    pub status: u8,

    /// Bump seed
    pub bump: u8,

    /// Settlement timestamp
    pub settled_at: i64,
}

impl PrivateSettlement {
    pub const SEED_PREFIX: &'static [u8] = b"private_settlement";

    pub const STATUS_PENDING: u8 = 0;
    pub const STATUS_COMPLETED: u8 = 1;
    pub const STATUS_FAILED: u8 = 2;
}
