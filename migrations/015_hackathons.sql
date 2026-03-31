-- Hackathon competitions infrastructure

CREATE TABLE IF NOT EXISTS hackathons (
    id              VARCHAR(64) PRIMARY KEY,
    name            VARCHAR(256) NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    prize_pool_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    start_time      TIMESTAMPTZ NOT NULL,
    end_time        TIMESTAMPTZ NOT NULL,
    status          VARCHAR(32) NOT NULL DEFAULT 'upcoming',
    scoring_method  VARCHAR(32) NOT NULL DEFAULT 'net_pnl',
    created_by      VARCHAR(64) NOT NULL,
    rules_json      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT chk_hackathon_status CHECK (status IN ('upcoming', 'active', 'completed', 'cancelled')),
    CONSTRAINT chk_hackathon_time CHECK (end_time > start_time)
);

CREATE INDEX IF NOT EXISTS idx_hackathons_status ON hackathons(status);
CREATE INDEX IF NOT EXISTS idx_hackathons_start_time ON hackathons(start_time);

CREATE TABLE IF NOT EXISTS hackathon_registrations (
    hackathon_id    VARCHAR(64) NOT NULL REFERENCES hackathons(id) ON DELETE CASCADE,
    wallet_address  VARCHAR(64) NOT NULL,
    identity_id     VARCHAR(128),
    registered_at   TIMESTAMPTZ DEFAULT NOW(),
    status          VARCHAR(32) NOT NULL DEFAULT 'active',
    PRIMARY KEY (hackathon_id, wallet_address),
    CONSTRAINT chk_registration_status CHECK (status IN ('active', 'disqualified'))
);

CREATE INDEX IF NOT EXISTS idx_hackathon_registrations_wallet ON hackathon_registrations(wallet_address);

CREATE TABLE IF NOT EXISTS hackathon_agents (
    hackathon_id    VARCHAR(64) NOT NULL REFERENCES hackathons(id) ON DELETE CASCADE,
    agent_id        VARCHAR(128) NOT NULL,
    wallet_address  VARCHAR(64) NOT NULL,
    registered_at   TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (hackathon_id, agent_id),
    CONSTRAINT fk_hackathon_agents_registration
        FOREIGN KEY (hackathon_id, wallet_address)
        REFERENCES hackathon_registrations(hackathon_id, wallet_address) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_hackathon_agents_wallet ON hackathon_agents(wallet_address, hackathon_id);

CREATE TABLE IF NOT EXISTS hackathon_snapshots (
    id                  BIGSERIAL PRIMARY KEY,
    hackathon_id        VARCHAR(64) NOT NULL REFERENCES hackathons(id) ON DELETE CASCADE,
    wallet_address      VARCHAR(64) NOT NULL,
    snapshot_time       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    net_pnl_usdc        DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_volume_usdc   DOUBLE PRECISION NOT NULL DEFAULT 0,
    win_rate_bps        INTEGER NOT NULL DEFAULT 0,
    position_count      INTEGER NOT NULL DEFAULT 0,
    trade_count         INTEGER NOT NULL DEFAULT 0,
    rank                INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_hackathon_snapshots_lookup
    ON hackathon_snapshots(hackathon_id, snapshot_time DESC);
CREATE INDEX IF NOT EXISTS idx_hackathon_snapshots_wallet
    ON hackathon_snapshots(hackathon_id, wallet_address, snapshot_time DESC);
