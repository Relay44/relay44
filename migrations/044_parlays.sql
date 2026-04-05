-- Lightweight parlay product: multi-leg prediction market bets.

CREATE TABLE IF NOT EXISTS parlays (
    id              VARCHAR(64) PRIMARY KEY,
    owner           VARCHAR(64) NOT NULL,
    stake_usdc      DOUBLE PRECISION NOT NULL,
    leg_count       INT NOT NULL,
    resolved_count  INT NOT NULL DEFAULT 0,
    all_won         BOOLEAN NOT NULL DEFAULT true,
    payout_usdc     DOUBLE PRECISION,
    status          VARCHAR(32) NOT NULL DEFAULT 'active',
    chain_parlay_id BIGINT,
    tx_hash         VARCHAR(66),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    settled_at      TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_parlays_owner
    ON parlays(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_parlays_status
    ON parlays(status) WHERE status = 'active';

CREATE TABLE IF NOT EXISTS parlay_legs (
    id              SERIAL PRIMARY KEY,
    parlay_id       VARCHAR(64) NOT NULL REFERENCES parlays(id) ON DELETE CASCADE,
    leg_index       INT NOT NULL,
    market_slug     VARCHAR(256) NOT NULL,
    market_id       BIGINT,
    outcome_yes     BOOLEAN NOT NULL,
    odds_bps        INT NOT NULL,
    resolved        BOOLEAN NOT NULL DEFAULT false,
    won             BOOLEAN,
    resolved_at     TIMESTAMPTZ,
    UNIQUE(parlay_id, leg_index)
);

CREATE INDEX IF NOT EXISTS idx_parlay_legs_parlay
    ON parlay_legs(parlay_id);
CREATE INDEX IF NOT EXISTS idx_parlay_legs_market
    ON parlay_legs(market_slug);
