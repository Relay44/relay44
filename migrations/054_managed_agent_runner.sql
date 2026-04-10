-- Managed agent paper-trade execution engine: open positions table.
--
-- The managed agent runner evaluates strategies against external markets on a
-- fixed interval, opens paper positions, and closes them based on hold time,
-- take-profit, stop-loss, or market resolution. Closed fills are appended to
-- managed_agent_trades; running positions live here until closed.

CREATE TABLE IF NOT EXISTS managed_agent_positions (
    id              BIGSERIAL PRIMARY KEY,
    agent_id        VARCHAR(64) NOT NULL REFERENCES managed_agents(id) ON DELETE CASCADE,
    market_slug     VARCHAR(256) NOT NULL,
    provider        VARCHAR(32) NOT NULL,
    outcome         VARCHAR(16) NOT NULL,
    side            VARCHAR(16) NOT NULL,
    entry_price     DOUBLE PRECISION NOT NULL,
    quantity        DOUBLE PRECISION NOT NULL,
    notional_usdc   DOUBLE PRECISION NOT NULL,
    fees_usdc       DOUBLE PRECISION NOT NULL DEFAULT 0,
    mark_price      DOUBLE PRECISION NOT NULL,
    unrealized_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    opened_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    hold_until      TIMESTAMPTZ NOT NULL,
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_managed_agent_positions_agent
    ON managed_agent_positions(agent_id);

CREATE INDEX IF NOT EXISTS idx_managed_agent_positions_market
    ON managed_agent_positions(agent_id, market_slug, outcome);

CREATE INDEX IF NOT EXISTS idx_managed_agent_positions_hold_until
    ON managed_agent_positions(hold_until);

-- Prevent the same agent from opening duplicate positions on the same
-- market/outcome/side while one is still open.
CREATE UNIQUE INDEX IF NOT EXISTS uq_managed_agent_positions_open
    ON managed_agent_positions(agent_id, market_slug, outcome, side);

-- Add a runner-facing column to managed_agents so we can backoff failing
-- agents without touching `status` (which is user-facing).
ALTER TABLE managed_agents
    ADD COLUMN IF NOT EXISTS consecutive_failures INTEGER NOT NULL DEFAULT 0;

ALTER TABLE managed_agents
    ADD COLUMN IF NOT EXISTS last_error TEXT;

ALTER TABLE managed_agents
    ADD COLUMN IF NOT EXISTS next_execution_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

CREATE INDEX IF NOT EXISTS idx_managed_agents_next_execution
    ON managed_agents(status, next_execution_at)
    WHERE status = 'active';
