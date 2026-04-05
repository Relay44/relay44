-- Creator tier system: tiered seed limits and platform revenue share.

CREATE TABLE IF NOT EXISTS creator_tiers (
    id              VARCHAR(32) PRIMARY KEY,
    name            VARCHAR(64) NOT NULL,
    max_seed_usdc   DOUBLE PRECISION NOT NULL,
    platform_take_bps INT NOT NULL DEFAULT 0,
    max_markets     INT NOT NULL DEFAULT 5,
    priority_placement BOOLEAN NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO creator_tiers (id, name, max_seed_usdc, platform_take_bps, max_markets, priority_placement)
VALUES
    ('starter', 'Starter', 5000, 2000, 5, false),
    ('pro', 'Pro', 50000, 1500, 25, false),
    ('institutional', 'Institutional', 1000000, 1000, 100, true)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS creator_profiles (
    owner           VARCHAR(64) PRIMARY KEY,
    tier_id         VARCHAR(32) NOT NULL DEFAULT 'starter' REFERENCES creator_tiers(id),
    total_seed_deployed DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_pnl_usdc  DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_platform_fees_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    markets_created INT NOT NULL DEFAULT 0,
    markets_graduated INT NOT NULL DEFAULT 0,
    staking_amount_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS platform_fee_ledger (
    id              SERIAL PRIMARY KEY,
    creator_owner   VARCHAR(64) NOT NULL,
    market_id       BIGINT,
    fee_type        VARCHAR(32) NOT NULL,
    amount_usdc     DOUBLE PRECISION NOT NULL,
    tier_id         VARCHAR(32) NOT NULL,
    take_bps        INT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_platform_fee_ledger_creator
    ON platform_fee_ledger(creator_owner, created_at DESC);
