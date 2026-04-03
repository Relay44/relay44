CREATE TABLE IF NOT EXISTS bootstrap_fill_events (
    id VARCHAR(64) PRIMARY KEY,
    market_id BIGINT NOT NULL,
    creator VARCHAR(42) NOT NULL,
    trade_id VARCHAR(64) NOT NULL,
    source VARCHAR(32) NOT NULL,
    agent_id BIGINT,
    maker_order_id VARCHAR(64) NOT NULL,
    outcome VARCHAR(16) NOT NULL,
    side VARCHAR(16) NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    quantity DOUBLE PRECISION NOT NULL,
    notional_usdc DOUBLE PRECISION NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    raw JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_bootstrap_fill_events_side
        CHECK (side IN ('buy', 'sell'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_bootstrap_fill_events_trade_source
    ON bootstrap_fill_events(trade_id, source);

CREATE INDEX IF NOT EXISTS idx_bootstrap_fill_events_creator_market
    ON bootstrap_fill_events(creator, market_id, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_bootstrap_fill_events_market_day
    ON bootstrap_fill_events(market_id, occurred_at DESC);

CREATE TABLE IF NOT EXISTS creator_market_economics_daily (
    market_id BIGINT NOT NULL,
    creator VARCHAR(42) NOT NULL,
    day DATE NOT NULL,
    seed_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    available_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    reserved_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    inventory_yes DOUBLE PRECISION NOT NULL DEFAULT 0,
    inventory_no DOUBLE PRECISION NOT NULL DEFAULT 0,
    inventory_mark_value_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    cumulative_bootstrap_fills_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    net_liquidity_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    subsidy_burn_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    roi_bps DOUBLE PRECISION NOT NULL DEFAULT 0,
    realized_resolution_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    organic_depth_ratio DOUBLE PRECISION NOT NULL DEFAULT 0,
    graduated BOOLEAN NOT NULL DEFAULT FALSE,
    graduation_retention_24h DOUBLE PRECISION,
    graduation_retention_7d DOUBLE PRECISION,
    mirror_freshness_seconds BIGINT,
    mirror_pending_hedges BIGINT NOT NULL DEFAULT 0,
    mirror_error_count BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (market_id, creator, day)
);

CREATE INDEX IF NOT EXISTS idx_creator_market_economics_daily_creator_day
    ON creator_market_economics_daily(creator, day DESC);

CREATE INDEX IF NOT EXISTS idx_creator_market_economics_daily_market_day
    ON creator_market_economics_daily(market_id, day DESC);

DROP TRIGGER IF EXISTS update_creator_market_economics_daily_updated_at ON creator_market_economics_daily;
CREATE TRIGGER update_creator_market_economics_daily_updated_at
    BEFORE UPDATE ON creator_market_economics_daily
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
