ALTER TABLE external_agents
    ADD COLUMN IF NOT EXISTS strategy_params JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE TABLE IF NOT EXISTS external_market_signals (
    id                  VARCHAR(64) PRIMARY KEY,
    publisher           VARCHAR(128) NOT NULL,
    provider            VARCHAR(32) NOT NULL,
    market_id           TEXT NOT NULL,
    signal_type         VARCHAR(32) NOT NULL DEFAULT 'scenario_lab',
    direction           VARCHAR(8) NOT NULL CHECK (direction IN ('yes', 'no', 'neutral')),
    confidence_bps      INTEGER NOT NULL CHECK (confidence_bps BETWEEN 0 AND 10000),
    fair_value_low      DOUBLE PRECISION NOT NULL CHECK (fair_value_low >= 0 AND fair_value_low <= 1),
    fair_value_high     DOUBLE PRECISION NOT NULL CHECK (fair_value_high >= 0 AND fair_value_high <= 1),
    midpoint_delta_bps  INTEGER NOT NULL,
    catalyst_summary    TEXT NOT NULL,
    invalidators        JSONB NOT NULL DEFAULT '[]'::jsonb,
    rationale           TEXT,
    metadata            JSONB NOT NULL DEFAULT '{}'::jsonb,
    active              BOOLEAN NOT NULL DEFAULT TRUE,
    expires_at          TIMESTAMPTZ NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    agent_id            VARCHAR(64),
    CONSTRAINT chk_external_market_signals_range
        CHECK (fair_value_low <= fair_value_high),
    CONSTRAINT fk_external_market_signals_agent
        FOREIGN KEY (agent_id) REFERENCES external_agents(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_external_market_signals_market
    ON external_market_signals(market_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_external_market_signals_active
    ON external_market_signals(active, expires_at DESC)
    WHERE active = TRUE;

CREATE INDEX IF NOT EXISTS idx_external_market_signals_publisher
    ON external_market_signals(publisher, created_at DESC);
