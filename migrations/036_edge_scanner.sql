-- Edge scanner: calibration curve + time decay signal infrastructure

-- ============================================================
-- Calibration curve snapshots
-- ============================================================
CREATE TABLE IF NOT EXISTS calibration_curve_snapshots (
    id              VARCHAR(64) PRIMARY KEY,
    provider        VARCHAR(32) NOT NULL,
    bucket_low_bps  INTEGER NOT NULL,
    bucket_high_bps INTEGER NOT NULL,
    sample_count    INTEGER NOT NULL,
    resolved_yes    INTEGER NOT NULL,
    actual_rate_bps INTEGER NOT NULL,
    expected_midpoint_bps INTEGER NOT NULL,
    edge_bps        INTEGER NOT NULL,
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_calibration_curve_provider_time
    ON calibration_curve_snapshots(provider, computed_at DESC);

CREATE INDEX IF NOT EXISTS idx_calibration_curve_edge
    ON calibration_curve_snapshots(provider, edge_bps)
    WHERE edge_bps != 0;

-- ============================================================
-- Edge scanner signals (unified output for calibration + decay)
-- ============================================================
CREATE TABLE IF NOT EXISTS edge_scanner_signals (
    id              VARCHAR(64) PRIMARY KEY,
    strategy        VARCHAR(32) NOT NULL CHECK (strategy IN ('calibration_arb', 'time_decay')),
    provider        VARCHAR(32) NOT NULL,
    market_id       VARCHAR(128) NOT NULL,
    direction       VARCHAR(8) NOT NULL CHECK (direction IN ('yes', 'no')),
    edge_bps        INTEGER NOT NULL,
    confidence_bps  INTEGER NOT NULL CHECK (confidence_bps BETWEEN 0 AND 10000),
    kelly_fraction  DOUBLE PRECISION NOT NULL DEFAULT 0,
    market_price    DOUBLE PRECISION NOT NULL,
    fair_value      DOUBLE PRECISION NOT NULL,
    deadline        TIMESTAMPTZ,
    days_remaining  INTEGER,
    rationale       TEXT NOT NULL,
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    active          BOOLEAN NOT NULL DEFAULT TRUE,
    expires_at      TIMESTAMPTZ NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acted_on        BOOLEAN NOT NULL DEFAULT FALSE,
    acted_at        TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_edge_scanner_active
    ON edge_scanner_signals(strategy, active, edge_bps DESC)
    WHERE active = TRUE;

CREATE INDEX IF NOT EXISTS idx_edge_scanner_market
    ON edge_scanner_signals(market_id, strategy, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_edge_scanner_expiry
    ON edge_scanner_signals(expires_at)
    WHERE active = TRUE;
