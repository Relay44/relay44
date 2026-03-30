CREATE TABLE IF NOT EXISTS base_market_bootstrap_configs (
    market_id BIGINT PRIMARY KEY,
    creator VARCHAR(42) NOT NULL,
    liquidity_mode VARCHAR(32) NOT NULL DEFAULT 'bootstrap_hybrid',
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    seed_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    initial_yes_bps INTEGER NOT NULL DEFAULT 5000,
    strategy VARCHAR(64) NOT NULL DEFAULT 'ladder_v1',
    levels INTEGER NOT NULL DEFAULT 5,
    base_spread_bps INTEGER NOT NULL DEFAULT 150,
    step_bps INTEGER NOT NULL DEFAULT 100,
    cadence_seconds INTEGER NOT NULL DEFAULT 300,
    expiry_seconds INTEGER NOT NULL DEFAULT 900,
    organic_depth_window_bps INTEGER NOT NULL DEFAULT 500,
    target_depth_multiplier DOUBLE PRECISION NOT NULL DEFAULT 2,
    target_volume_multiplier DOUBLE PRECISION NOT NULL DEFAULT 10,
    max_age_seconds BIGINT NOT NULL DEFAULT 604800,
    inventory_skew_bps INTEGER NOT NULL DEFAULT 0,
    exposure_cap_bps INTEGER NOT NULL DEFAULT 6500,
    depth_qualified_since TIMESTAMPTZ,
    activated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    graduated_at TIMESTAMPTZ,
    graduation_reason VARCHAR(64),
    create_tx_hash VARCHAR(66),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_base_market_bootstrap_status
    ON base_market_bootstrap_configs(status);

CREATE INDEX IF NOT EXISTS idx_base_market_bootstrap_activated_at
    ON base_market_bootstrap_configs(activated_at DESC);

DROP TRIGGER IF EXISTS update_base_market_bootstrap_configs_updated_at ON base_market_bootstrap_configs;
CREATE TRIGGER update_base_market_bootstrap_configs_updated_at
    BEFORE UPDATE ON base_market_bootstrap_configs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
