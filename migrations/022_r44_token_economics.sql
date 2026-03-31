-- R44 Token Economics: staking, rewards, and fee tracking
-- Supports the on-chain R44Staking, RewardDistributor, and OrderBook fee system

-- ============================================================
-- Staking positions (mirrors on-chain R44Staking state)
-- ============================================================
CREATE TABLE r44_stakes (
    id              SERIAL PRIMARY KEY,
    wallet          VARCHAR(44) NOT NULL,
    amount          NUMERIC(78, 0) NOT NULL DEFAULT 0,
    locked_at       TIMESTAMPTZ NOT NULL,
    unlock_at       TIMESTAMPTZ NOT NULL,
    tier            SMALLINT NOT NULL DEFAULT 0,  -- 0=Bronze, 1=Silver, 2=Gold, 3=Diamond
    tx_hash         VARCHAR(66),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    unstaked_at     TIMESTAMPTZ,
    UNIQUE(wallet)  -- one active stake per wallet
);

CREATE INDEX idx_r44_stakes_wallet ON r44_stakes(wallet);
CREATE INDEX idx_r44_stakes_tier ON r44_stakes(tier);

-- ============================================================
-- Reward distribution epochs
-- ============================================================
CREATE TABLE r44_reward_epochs (
    epoch           INTEGER PRIMARY KEY,
    distributed_at  TIMESTAMPTZ NOT NULL,
    total_amount    NUMERIC(78, 0) NOT NULL,
    staking_amount  NUMERIC(78, 0) NOT NULL,
    agent_amount    NUMERIC(78, 0) NOT NULL,
    creator_amount  NUMERIC(78, 0) NOT NULL,
    treasury_amount NUMERIC(78, 0) NOT NULL,
    tx_hash         VARCHAR(66),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- Per-user reward allocations within an epoch
-- ============================================================
CREATE TABLE r44_reward_allocations (
    id              SERIAL PRIMARY KEY,
    epoch           INTEGER NOT NULL REFERENCES r44_reward_epochs(epoch),
    wallet          VARCHAR(44) NOT NULL,
    reward_type     VARCHAR(16) NOT NULL,  -- 'agent', 'creator', 'staking'
    amount          NUMERIC(78, 0) NOT NULL,
    claimed         BOOLEAN NOT NULL DEFAULT FALSE,
    claim_tx_hash   VARCHAR(66),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    claimed_at      TIMESTAMPTZ,
    UNIQUE(epoch, wallet, reward_type)
);

CREATE INDEX idx_r44_reward_alloc_wallet ON r44_reward_allocations(wallet);
CREATE INDEX idx_r44_reward_alloc_epoch ON r44_reward_allocations(epoch);
CREATE INDEX idx_r44_reward_alloc_unclaimed ON r44_reward_allocations(wallet, claimed) WHERE NOT claimed;

-- ============================================================
-- Agent execution burn log
-- ============================================================
CREATE TABLE r44_execution_burns (
    id              SERIAL PRIMARY KEY,
    agent_id        INTEGER NOT NULL,
    wallet          VARCHAR(44) NOT NULL,
    amount          NUMERIC(78, 0) NOT NULL,
    tx_hash         VARCHAR(66),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_r44_burns_wallet ON r44_execution_burns(wallet);
CREATE INDEX idx_r44_burns_agent ON r44_execution_burns(agent_id);

-- ============================================================
-- Market creation deposits
-- ============================================================
CREATE TABLE r44_market_deposits (
    id              SERIAL PRIMARY KEY,
    market_id       VARCHAR(64) NOT NULL REFERENCES markets(id),
    wallet          VARCHAR(44) NOT NULL,
    amount          NUMERIC(78, 0) NOT NULL,
    status          VARCHAR(16) NOT NULL DEFAULT 'locked',  -- 'locked', 'refunded', 'slashed'
    tx_hash         VARCHAR(66),
    resolution_tx   VARCHAR(66),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at     TIMESTAMPTZ,
    UNIQUE(market_id)
);

CREATE INDEX idx_r44_deposits_wallet ON r44_market_deposits(wallet);
CREATE INDEX idx_r44_deposits_status ON r44_market_deposits(status);

-- ============================================================
-- Fee collection tracking (complements existing fee_ledger)
-- ============================================================
CREATE TABLE r44_fee_collections (
    id              SERIAL PRIMARY KEY,
    market_id       VARCHAR(64) NOT NULL,
    claimer_wallet  VARCHAR(44) NOT NULL,
    gross_payout    NUMERIC(78, 0) NOT NULL,
    fee_amount      NUMERIC(78, 0) NOT NULL,
    net_payout      NUMERIC(78, 0) NOT NULL,
    discount_bps    INTEGER NOT NULL DEFAULT 0,
    r44_tier        SMALLINT NOT NULL DEFAULT 0,
    tx_hash         VARCHAR(66),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_r44_fees_market ON r44_fee_collections(market_id);
CREATE INDEX idx_r44_fees_wallet ON r44_fee_collections(claimer_wallet);

-- ============================================================
-- Protocol fee withdrawals
-- ============================================================
CREATE TABLE r44_fee_withdrawals (
    id              SERIAL PRIMARY KEY,
    amount          NUMERIC(78, 0) NOT NULL,
    recipient       VARCHAR(44) NOT NULL,
    tx_hash         VARCHAR(66),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
