-- Agent-as-a-Service: template strategies and managed execution.

CREATE TABLE IF NOT EXISTS agent_templates (
    id              VARCHAR(64) PRIMARY KEY,
    name            VARCHAR(128) NOT NULL UNIQUE,
    description     TEXT,
    strategy        TEXT NOT NULL,
    default_params  JSONB NOT NULL DEFAULT '{}'::jsonb,
    category        VARCHAR(64) NOT NULL DEFAULT 'general',
    risk_tier       VARCHAR(16) NOT NULL DEFAULT 'moderate',
    min_seed_usdc   DOUBLE PRECISION NOT NULL DEFAULT 10.0,
    active          BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS managed_agents (
    id              VARCHAR(64) PRIMARY KEY,
    owner           VARCHAR(64) NOT NULL,
    template_id     VARCHAR(64) NOT NULL REFERENCES agent_templates(id),
    name            VARCHAR(128) NOT NULL,
    params          JSONB NOT NULL DEFAULT '{}'::jsonb,
    seed_usdc       DOUBLE PRECISION NOT NULL,
    status          VARCHAR(32) NOT NULL DEFAULT 'active',
    execution_mode  VARCHAR(32) NOT NULL DEFAULT 'managed',
    pnl_usdc        DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_trades    BIGINT NOT NULL DEFAULT 0,
    max_drawdown_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
    high_watermark_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    last_executed_at TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_managed_agents_owner
    ON managed_agents(owner, status);
CREATE INDEX IF NOT EXISTS idx_managed_agents_template
    ON managed_agents(template_id);

CREATE TABLE IF NOT EXISTS managed_agent_trades (
    id              SERIAL PRIMARY KEY,
    agent_id        VARCHAR(64) NOT NULL REFERENCES managed_agents(id) ON DELETE CASCADE,
    market_slug     VARCHAR(256) NOT NULL,
    outcome         VARCHAR(16) NOT NULL,
    side            VARCHAR(16) NOT NULL,
    price           DOUBLE PRECISION NOT NULL,
    quantity        DOUBLE PRECISION NOT NULL,
    pnl_usdc        DOUBLE PRECISION,
    provider        VARCHAR(32),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_managed_agent_trades_agent
    ON managed_agent_trades(agent_id, created_at DESC);
