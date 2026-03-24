use anchor_lang::prelude::*;
use crate::crypto::{ElGamalCiphertext, ElGamalPubkey, CryptoError};

/// Type alias for crypto results that can convert to Anchor errors
type CryptoResult<T> = core::result::Result<T, CryptoError>;

/// Private account for confidential balance management
///
/// This account stores:
/// - Owner's ElGamal public key for encryption
/// - Encrypted balance (homomorphic)
/// - Activity tracking (no balance info)
///
/// SECURITY: The plaintext balance has been REMOVED.
/// All balance operations use homomorphic encryption.
#[account]
#[derive(InitSpace)]
pub struct PrivateAccount {
    /// Account owner (Solana wallet)
    pub owner: Pubkey,

    /// ElGamal public key for encryption (32 bytes compressed)
    /// Client derives this from wallet signature for key recovery
    pub elgamal_pubkey: [u8; 32],

    /// Encrypted balance using twisted ElGamal (64 bytes)
    /// Format: (C1, C2) where C1 = r*G, C2 = m*G + r*P
    /// Supports homomorphic operations: add/subtract ciphertexts
    pub encrypted_balance: [u8; 64],

    /// Pending encrypted balance for async operations (64 bytes)
    /// Used during multi-step transactions (e.g., order settlement)
    pub pending_balance: [u8; 64],

    /// Total deposited (encrypted, for auditing without revealing balance)
    pub total_deposited_encrypted: [u8; 64],

    /// Total withdrawn (encrypted, for auditing without revealing balance)
    pub total_withdrawn_encrypted: [u8; 64],

    /// Number of private orders placed (public counter)
    pub private_order_count: u64,

    /// Number of private settlements (public counter)
    pub private_settlement_count: u64,

    /// Account version (for migration support)
    /// 0 = legacy placeholder crypto (invalid)
    /// 1 = real ElGamal crypto
    pub version: u8,

    /// Whether account is active for trading
    pub is_active: bool,

    /// Bump seed for PDA derivation
    pub bump: u8,

    /// Creation timestamp
    pub created_at: i64,

    /// Last activity timestamp
    pub last_activity: i64,
}

impl PrivateAccount {
    pub const SEED_PREFIX: &'static [u8] = b"private_account";

    /// Current version for new accounts
    pub const CURRENT_VERSION: u8 = 1;

    /// Validate ElGamal public key is a valid curve point
    pub fn validate_pubkey(pubkey: &[u8; 32]) -> CryptoResult<()> {
        ElGamalPubkey::from_bytes(pubkey)?;
        Ok(())
    }

    /// Get the ElGamal public key as a typed struct
    pub fn get_elgamal_pubkey(&self) -> CryptoResult<ElGamalPubkey> {
        ElGamalPubkey::from_bytes(&self.elgamal_pubkey)
    }

    /// Get the encrypted balance as a typed struct
    pub fn get_encrypted_balance(&self) -> CryptoResult<ElGamalCiphertext> {
        ElGamalCiphertext::from_bytes(&self.encrypted_balance)
    }

    /// Set the encrypted balance from a typed struct
    pub fn set_encrypted_balance(&mut self, ciphertext: &ElGamalCiphertext) {
        self.encrypted_balance = ciphertext.to_bytes();
    }

    /// Add to encrypted balance (homomorphic)
    pub fn add_to_balance(&mut self, amount_ciphertext: &ElGamalCiphertext) -> CryptoResult<()> {
        let current = self.get_encrypted_balance()?;
        let new_balance = current.add(amount_ciphertext)?;
        self.set_encrypted_balance(&new_balance);
        Ok(())
    }

    /// Subtract from encrypted balance (homomorphic)
    pub fn subtract_from_balance(&mut self, amount_ciphertext: &ElGamalCiphertext) -> CryptoResult<()> {
        let current = self.get_encrypted_balance()?;
        let new_balance = current.subtract(amount_ciphertext)?;
        self.set_encrypted_balance(&new_balance);
        Ok(())
    }

    /// Initialize balance to encryption of zero
    pub fn initialize_zero_balance(&mut self) -> CryptoResult<()> {
        let pubkey = self.get_elgamal_pubkey()?;
        let zero_ciphertext = pubkey.encrypt_zero();
        self.encrypted_balance = zero_ciphertext.to_bytes();
        self.pending_balance = zero_ciphertext.to_bytes();
        self.total_deposited_encrypted = zero_ciphertext.to_bytes();
        self.total_withdrawn_encrypted = zero_ciphertext.to_bytes();
        Ok(())
    }

    /// Check if account uses real cryptography (version >= 1)
    pub fn is_crypto_enabled(&self) -> bool {
        self.version >= Self::CURRENT_VERSION
    }

    /// Update last activity timestamp
    pub fn touch(&mut self, clock: &Clock) {
        self.last_activity = clock.unix_timestamp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::ElGamalKeypair;

    #[test]
    fn test_account_initialization() {
        let seed = [42u8; 32];
        let keypair = ElGamalKeypair::from_seed(&seed);

        let mut account = PrivateAccount {
            owner: Pubkey::default(),
            elgamal_pubkey: keypair.public.to_bytes(),
            encrypted_balance: [0u8; 64],
            pending_balance: [0u8; 64],
            total_deposited_encrypted: [0u8; 64],
            total_withdrawn_encrypted: [0u8; 64],
            private_order_count: 0,
            private_settlement_count: 0,
            version: PrivateAccount::CURRENT_VERSION,
            is_active: true,
            bump: 255,
            created_at: 0,
            last_activity: 0,
        };

        // Initialize with zero balance
        account.initialize_zero_balance().unwrap();

        // Verify the balance is encryption of zero
        let balance = account.get_encrypted_balance().unwrap();
        let decrypted = keypair.secret.decrypt(&balance).unwrap();
        assert_eq!(decrypted, 0);
    }

    #[test]
    fn test_homomorphic_balance_operations() {
        let seed = [42u8; 32];
        let keypair = ElGamalKeypair::from_seed(&seed);

        let mut account = PrivateAccount {
            owner: Pubkey::default(),
            elgamal_pubkey: keypair.public.to_bytes(),
            encrypted_balance: [0u8; 64],
            pending_balance: [0u8; 64],
            total_deposited_encrypted: [0u8; 64],
            total_withdrawn_encrypted: [0u8; 64],
            private_order_count: 0,
            private_settlement_count: 0,
            version: PrivateAccount::CURRENT_VERSION,
            is_active: true,
            bump: 255,
            created_at: 0,
            last_activity: 0,
        };

        account.initialize_zero_balance().unwrap();

        // Deposit 1000
        use curve25519_dalek::scalar::Scalar;
        let deposit = keypair.public.encrypt_with_randomness(1000, &Scalar::from(111u64)).unwrap();
        account.add_to_balance(&deposit).unwrap();

        let balance = account.get_encrypted_balance().unwrap();
        assert_eq!(keypair.secret.decrypt(&balance).unwrap(), 1000);

        // Withdraw 300
        let withdraw = keypair.public.encrypt_with_randomness(300, &Scalar::from(222u64)).unwrap();
        account.subtract_from_balance(&withdraw).unwrap();

        let balance = account.get_encrypted_balance().unwrap();
        assert_eq!(keypair.secret.decrypt(&balance).unwrap(), 700);
    }
}
