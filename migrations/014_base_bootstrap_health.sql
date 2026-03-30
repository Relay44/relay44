ALTER TABLE base_market_bootstrap_configs
    ADD COLUMN IF NOT EXISTS preset VARCHAR(32) NOT NULL DEFAULT 'balanced',
    ADD COLUMN IF NOT EXISTS pause_reason VARCHAR(64),
    ADD COLUMN IF NOT EXISTS reserved_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS available_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS active_slots INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS organic_depth_ratio DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS consecutive_failures INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_base_market_bootstrap_pause_reason
    ON base_market_bootstrap_configs(pause_reason);
