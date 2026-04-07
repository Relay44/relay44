-- Aerodrome pool scanner: persistent index of scanned pools and scan run log.

CREATE TABLE IF NOT EXISTS aerodrome_scanned_pools (
    pool_address        VARCHAR(42) PRIMARY KEY,
    token0              VARCHAR(42) NOT NULL,
    token1              VARCHAR(42) NOT NULL,
    token0_symbol       VARCHAR(32),
    token1_symbol       VARCHAR(32),
    tick_spacing        INTEGER,
    liquidity           NUMERIC(40, 0) DEFAULT 0,
    sqrt_price_x96      NUMERIC(50, 0) DEFAULT 0,
    tick                INTEGER,
    price               DOUBLE PRECISION,
    spread_bps          INTEGER DEFAULT 0,
    opportunity_type    VARCHAR(64),
    opportunity_score   DOUBLE PRECISION DEFAULT 0,
    active              BOOLEAN NOT NULL DEFAULT TRUE,
    last_scanned_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_aerodrome_scanned_active
    ON aerodrome_scanned_pools(active, opportunity_score DESC);

CREATE INDEX IF NOT EXISTS idx_aerodrome_scanned_type
    ON aerodrome_scanned_pools(opportunity_type, opportunity_score DESC)
    WHERE active = TRUE;

CREATE TABLE IF NOT EXISTS aerodrome_scanner_runs (
    id                  SERIAL PRIMARY KEY,
    pools_scanned       INTEGER NOT NULL DEFAULT 0,
    pools_indexed       INTEGER NOT NULL DEFAULT 0,
    opportunities_found INTEGER NOT NULL DEFAULT 0,
    completed_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    error               TEXT
);
