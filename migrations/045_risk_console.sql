-- Enterprise risk console: portfolio-level analytics and export.

CREATE TABLE IF NOT EXISTS portfolio_snapshots (
    id              SERIAL PRIMARY KEY,
    owner           VARCHAR(64) NOT NULL,
    total_value_usdc DOUBLE PRECISION NOT NULL,
    unrealized_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    realized_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    open_positions  INT NOT NULL DEFAULT 0,
    gross_exposure_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    max_single_position_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
    var_95_usdc     DOUBLE PRECISION,
    var_99_usdc     DOUBLE PRECISION,
    drawdown_from_peak_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
    peak_value_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    snapshot_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_portfolio_snapshots_owner
    ON portfolio_snapshots(owner, snapshot_at DESC);

-- Regulatory event log for CTF export.
CREATE TABLE IF NOT EXISTS compliance_events (
    id              SERIAL PRIMARY KEY,
    owner           VARCHAR(64) NOT NULL,
    event_type      VARCHAR(64) NOT NULL,
    market_id       BIGINT,
    market_slug     VARCHAR(256),
    side            VARCHAR(16),
    amount_usdc     DOUBLE PRECISION,
    counterparty    VARCHAR(64),
    provider        VARCHAR(32),
    tx_hash         VARCHAR(66),
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_compliance_events_owner
    ON compliance_events(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_compliance_events_type
    ON compliance_events(event_type, created_at DESC);
