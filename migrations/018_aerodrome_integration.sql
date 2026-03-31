-- Aerodrome pool registry for DEX-based agent execution and bootstrap LP
CREATE TABLE IF NOT EXISTS aerodrome_pools (
    id VARCHAR(64) PRIMARY KEY,
    pool_address VARCHAR(42) NOT NULL UNIQUE,
    token0 VARCHAR(42) NOT NULL,
    token1 VARCHAR(42) NOT NULL,
    fee INTEGER NOT NULL DEFAULT 0,
    tick_spacing INTEGER NOT NULL DEFAULT 200,
    token0_symbol VARCHAR(16),
    token1_symbol VARCHAR(16),
    token0_decimals INTEGER NOT NULL DEFAULT 18,
    token1_decimals INTEGER NOT NULL DEFAULT 6,
    is_slipstream BOOLEAN NOT NULL DEFAULT TRUE,
    market_id VARCHAR(128),
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_aerodrome_pools_market ON aerodrome_pools(market_id);
CREATE INDEX IF NOT EXISTS idx_aerodrome_pools_active ON aerodrome_pools(active);
CREATE INDEX IF NOT EXISTS idx_aerodrome_pools_address ON aerodrome_pools(pool_address);
