-- Limitless market scanner: persistent index of scanned markets and scan run log.

CREATE TABLE IF NOT EXISTS limitless_scanned_markets (
    slug              VARCHAR(256) PRIMARY KEY,
    question          TEXT NOT NULL,
    category          VARCHAR(64),
    yes_price         DOUBLE PRECISION,
    no_price          DOUBLE PRECISION,
    spread_bps        INTEGER,
    volume_usdc       DOUBLE PRECISION,
    liquidity_usdc    DOUBLE PRECISION,
    close_time        BIGINT DEFAULT 0,
    opportunity_type  VARCHAR(64),
    opportunity_score DOUBLE PRECISION DEFAULT 0,
    provider_market_ref VARCHAR(256),
    active            BOOLEAN NOT NULL DEFAULT TRUE,
    last_scanned_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_limitless_scanned_active
    ON limitless_scanned_markets(active, opportunity_score DESC);

CREATE INDEX IF NOT EXISTS idx_limitless_scanned_type
    ON limitless_scanned_markets(opportunity_type, opportunity_score DESC)
    WHERE active = TRUE;

CREATE TABLE IF NOT EXISTS limitless_scanner_runs (
    id                  SERIAL PRIMARY KEY,
    markets_scanned     INTEGER NOT NULL DEFAULT 0,
    markets_indexed     INTEGER NOT NULL DEFAULT 0,
    opportunities_found INTEGER NOT NULL DEFAULT 0,
    venue_matches_found INTEGER NOT NULL DEFAULT 0,
    completed_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    error               TEXT
);
