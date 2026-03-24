ALTER TABLE external_agent_runs
    ADD COLUMN IF NOT EXISTS metadata JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE TABLE IF NOT EXISTS paper_positions (
    id VARCHAR(64) PRIMARY KEY,
    agent_id VARCHAR(64) NOT NULL,
    owner VARCHAR(64) NOT NULL,
    provider VARCHAR(32) NOT NULL,
    market_id VARCHAR(128) NOT NULL,
    outcome VARCHAR(16) NOT NULL,
    side VARCHAR(16) NOT NULL,
    strategy TEXT NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'open',
    entry_price DOUBLE PRECISION NOT NULL,
    mark_price DOUBLE PRECISION NOT NULL,
    requested_quantity DOUBLE PRECISION NOT NULL,
    filled_quantity DOUBLE PRECISION NOT NULL,
    notional_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    fees_paid_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    realized_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    unrealized_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    hold_until TIMESTAMPTZ NOT NULL,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    last_marked_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT fk_paper_positions_agent
        FOREIGN KEY (agent_id) REFERENCES external_agents(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_paper_positions_open_agent
    ON paper_positions(agent_id)
    WHERE status = 'open';

CREATE INDEX IF NOT EXISTS idx_paper_positions_owner
    ON paper_positions(owner, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_paper_positions_market
    ON paper_positions(market_id, status, updated_at DESC);

CREATE TABLE IF NOT EXISTS paper_fills (
    id VARCHAR(64) PRIMARY KEY,
    run_id VARCHAR(64),
    position_id VARCHAR(64),
    agent_id VARCHAR(64) NOT NULL,
    owner VARCHAR(64) NOT NULL,
    provider VARCHAR(32) NOT NULL,
    market_id VARCHAR(128) NOT NULL,
    outcome VARCHAR(16) NOT NULL,
    side VARCHAR(16) NOT NULL,
    fill_type VARCHAR(16) NOT NULL,
    requested_quantity DOUBLE PRECISION NOT NULL,
    filled_quantity DOUBLE PRECISION NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    mark_price DOUBLE PRECISION NOT NULL,
    notional_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    fee_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT fk_paper_fills_agent
        FOREIGN KEY (agent_id) REFERENCES external_agents(id) ON DELETE CASCADE,
    CONSTRAINT fk_paper_fills_run
        FOREIGN KEY (run_id) REFERENCES external_agent_runs(id) ON DELETE SET NULL,
    CONSTRAINT fk_paper_fills_position
        FOREIGN KEY (position_id) REFERENCES paper_positions(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_paper_fills_agent
    ON paper_fills(agent_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_paper_fills_owner
    ON paper_fills(owner, created_at DESC);

CREATE TABLE IF NOT EXISTS paper_marks (
    id VARCHAR(64) PRIMARY KEY,
    position_id VARCHAR(64) NOT NULL,
    agent_id VARCHAR(64) NOT NULL,
    owner VARCHAR(64) NOT NULL,
    market_id VARCHAR(128) NOT NULL,
    outcome VARCHAR(16) NOT NULL,
    mark_price DOUBLE PRECISION NOT NULL,
    unrealized_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    notional_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT fk_paper_marks_position
        FOREIGN KEY (position_id) REFERENCES paper_positions(id) ON DELETE CASCADE,
    CONSTRAINT fk_paper_marks_agent
        FOREIGN KEY (agent_id) REFERENCES external_agents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_paper_marks_position
    ON paper_marks(position_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_paper_marks_owner
    ON paper_marks(owner, created_at DESC);

CREATE TABLE IF NOT EXISTS paper_outcomes (
    id VARCHAR(64) PRIMARY KEY,
    position_id VARCHAR(64) NOT NULL,
    agent_id VARCHAR(64) NOT NULL,
    owner VARCHAR(64) NOT NULL,
    provider VARCHAR(32) NOT NULL,
    market_id VARCHAR(128) NOT NULL,
    outcome VARCHAR(16) NOT NULL,
    side VARCHAR(16) NOT NULL,
    strategy TEXT NOT NULL,
    entry_price DOUBLE PRECISION NOT NULL,
    exit_price DOUBLE PRECISION NOT NULL,
    quantity DOUBLE PRECISION NOT NULL,
    gross_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    fee_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    realized_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    hold_seconds BIGINT NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    closed_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT fk_paper_outcomes_position
        FOREIGN KEY (position_id) REFERENCES paper_positions(id) ON DELETE CASCADE,
    CONSTRAINT fk_paper_outcomes_agent
        FOREIGN KEY (agent_id) REFERENCES external_agents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_paper_outcomes_owner
    ON paper_outcomes(owner, closed_at DESC);

CREATE INDEX IF NOT EXISTS idx_paper_outcomes_strategy
    ON paper_outcomes(strategy, closed_at DESC);
