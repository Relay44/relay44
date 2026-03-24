use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct PrivacyConfig {
    /// Admin authority
    pub admin: Pubkey,

    /// MXE authority (Arcium computation verifier)
    pub mxe_authority: Pubkey,

    /// Total private accounts created
    pub total_private_accounts: u64,

    /// Total private orders
    pub total_private_orders: u64,

    /// Total private settlements
    pub total_private_settlements: u64,

    /// Whether privacy features are enabled
    pub enabled: bool,

    /// Bump seed
    pub bump: u8,

    /// Creation timestamp
    pub created_at: i64,
}

impl PrivacyConfig {
    pub const SEED_PREFIX: &'static [u8] = b"privacy_config";
}
