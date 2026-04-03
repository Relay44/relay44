CREATE TABLE IF NOT EXISTS polymarket_index_state (
    lane VARCHAR(32) NOT NULL,
    market_id VARCHAR(128) NOT NULL,
    provider_market_ref VARCHAR(128) NOT NULL,
    index_status VARCHAR(32) NOT NULL DEFAULT 'pending',
    indexed_from TIMESTAMPTZ,
    indexed_through TIMESTAMPTZ,
    is_partial_backfill BOOLEAN NOT NULL DEFAULT TRUE,
    last_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (lane, market_id)
);

CREATE INDEX IF NOT EXISTS idx_polymarket_index_state_status
    ON polymarket_index_state(lane, index_status, updated_at DESC);

CREATE TABLE IF NOT EXISTS polymarket_public_trades (
    id VARCHAR(64) PRIMARY KEY,
    provider_trade_id VARCHAR(256) NOT NULL,
    market_id VARCHAR(128) NOT NULL,
    provider_market_ref VARCHAR(128) NOT NULL,
    outcome VARCHAR(16) NOT NULL,
    side VARCHAR(16),
    price DOUBLE PRECISION NOT NULL,
    price_bps BIGINT NOT NULL,
    quantity DOUBLE PRECISION NOT NULL,
    tx_hash VARCHAR(128),
    block_number BIGINT,
    token_id VARCHAR(256),
    maker VARCHAR(128),
    taker VARCHAR(128),
    match_time TIMESTAMPTZ NOT NULL,
    raw_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_polymarket_public_trades_provider_trade_id
    ON polymarket_public_trades(provider_trade_id);

CREATE INDEX IF NOT EXISTS idx_polymarket_public_trades_market_time
    ON polymarket_public_trades(market_id, match_time DESC);

CREATE INDEX IF NOT EXISTS idx_polymarket_public_trades_market_outcome_time
    ON polymarket_public_trades(market_id, outcome, match_time DESC);

CREATE INDEX IF NOT EXISTS idx_polymarket_public_trades_tx_hash
    ON polymarket_public_trades(tx_hash)
    WHERE tx_hash IS NOT NULL;

CREATE TABLE IF NOT EXISTS polymarket_user_trade_events (
    id VARCHAR(64) PRIMARY KEY,
    agent_id VARCHAR(64),
    run_id VARCHAR(64),
    external_order_id VARCHAR(64),
    owner VARCHAR(64),
    market_id VARCHAR(128) NOT NULL,
    provider_market_ref VARCHAR(128),
    provider_order_id VARCHAR(256),
    builder_trade_id VARCHAR(256),
    taker_hash VARCHAR(256),
    tx_hash VARCHAR(128),
    block_number BIGINT,
    outcome VARCHAR(16),
    side VARCHAR(16),
    price DOUBLE PRECISION,
    price_bps BIGINT,
    requested_quantity DOUBLE PRECISION,
    filled_quantity DOUBLE PRECISION,
    fee_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    lifecycle_status VARCHAR(16) NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    raw_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    matched_at TIMESTAMPTZ,
    mined_at TIMESTAMPTZ,
    confirmed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_polymarket_user_trade_events_agent
        FOREIGN KEY (agent_id) REFERENCES external_agents(id) ON DELETE SET NULL,
    CONSTRAINT fk_polymarket_user_trade_events_run
        FOREIGN KEY (run_id) REFERENCES external_agent_runs(id) ON DELETE SET NULL,
    CONSTRAINT fk_polymarket_user_trade_events_order
        FOREIGN KEY (external_order_id) REFERENCES external_orders(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_polymarket_user_trade_events_market_status
    ON polymarket_user_trade_events(market_id, lifecycle_status, confirmed_at DESC);

CREATE INDEX IF NOT EXISTS idx_polymarket_user_trade_events_run
    ON polymarket_user_trade_events(run_id)
    WHERE run_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_polymarket_user_trade_events_order
    ON polymarket_user_trade_events(external_order_id)
    WHERE external_order_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_polymarket_user_trade_events_provider_order
    ON polymarket_user_trade_events(provider_order_id)
    WHERE provider_order_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_polymarket_user_trade_events_taker_hash
    ON polymarket_user_trade_events(taker_hash)
    WHERE taker_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_polymarket_user_trade_events_tx_hash
    ON polymarket_user_trade_events(tx_hash)
    WHERE tx_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_polymarket_user_trade_events_builder_trade
    ON polymarket_user_trade_events(builder_trade_id)
    WHERE builder_trade_id IS NOT NULL;
