-- Two-tier KYC system: World ID verification + market access gating
-- Supports permissionless (tier 0) and verified (tier 2) market tiers

-- ============================================================
-- User KYC fields
-- ============================================================
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS kyc_tier            SMALLINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS kyc_provider        VARCHAR(32),
    ADD COLUMN IF NOT EXISTS kyc_verified_at     TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS kyc_nullifier_hash  VARCHAR(128);

CREATE INDEX idx_users_kyc_tier ON users(kyc_tier) WHERE kyc_tier > 0;
CREATE UNIQUE INDEX idx_users_kyc_nullifier ON users(kyc_nullifier_hash) WHERE kyc_nullifier_hash IS NOT NULL;

-- ============================================================
-- KYC verifications audit log
-- ============================================================
CREATE TABLE kyc_verifications (
    id              SERIAL PRIMARY KEY,
    wallet          VARCHAR(44) NOT NULL,
    provider        VARCHAR(32) NOT NULL,           -- 'world_id'
    nullifier_hash  VARCHAR(128) NOT NULL UNIQUE,   -- anti-sybil: one proof per platform
    proof_hash      VARCHAR(128) NOT NULL,           -- sha256(proof) for audit, not the proof itself
    merkle_root     VARCHAR(128),
    action_id       VARCHAR(128),
    signal          VARCHAR(44),                     -- wallet address used as signal
    tier_granted    SMALLINT NOT NULL DEFAULT 2,
    on_chain_tx     VARCHAR(66),                     -- setTier() tx hash
    status          VARCHAR(16) NOT NULL DEFAULT 'pending',  -- pending|confirmed|revoked
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at    TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ
);

CREATE INDEX idx_kyc_verif_wallet ON kyc_verifications(wallet);
CREATE INDEX idx_kyc_verif_status ON kyc_verifications(status);

-- ============================================================
-- Market KYC tier requirement
-- ============================================================
ALTER TABLE markets
    ADD COLUMN IF NOT EXISTS required_kyc_tier SMALLINT NOT NULL DEFAULT 0;

CREATE INDEX idx_markets_kyc_tier ON markets(required_kyc_tier) WHERE required_kyc_tier > 0;
