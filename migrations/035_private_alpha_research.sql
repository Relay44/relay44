ALTER TABLE external_agents
    ADD COLUMN IF NOT EXISTS cohort VARCHAR(32) NOT NULL DEFAULT 'public_research';

ALTER TABLE external_agents
    DROP CONSTRAINT IF EXISTS chk_external_agents_cohort;

ALTER TABLE external_agents
    ADD CONSTRAINT chk_external_agents_cohort
    CHECK (cohort IN ('public_research', 'private_alpha'));

CREATE INDEX IF NOT EXISTS idx_external_agents_cohort
    ON external_agents(owner, cohort, execution_mode, active, next_execution_at DESC);

ALTER TABLE polymarket_public_trades
    ADD COLUMN IF NOT EXISTS market_category VARCHAR(64);

CREATE INDEX IF NOT EXISTS idx_polymarket_public_trades_wallet
    ON polymarket_public_trades(taker, match_time DESC)
    WHERE taker IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_polymarket_public_trades_market_category
    ON polymarket_public_trades(market_category, match_time DESC)
    WHERE market_category IS NOT NULL;

CREATE TABLE IF NOT EXISTS polymarket_public_orderbook_snapshots (
    id VARCHAR(64) PRIMARY KEY,
    market_id VARCHAR(128) NOT NULL,
    provider_market_ref VARCHAR(256) NOT NULL,
    outcome VARCHAR(16) NOT NULL,
    depth INTEGER NOT NULL DEFAULT 20,
    best_bid DOUBLE PRECISION,
    best_ask DOUBLE PRECISION,
    mid_price DOUBLE PRECISION,
    bids JSONB NOT NULL DEFAULT '[]'::jsonb,
    asks JSONB NOT NULL DEFAULT '[]'::jsonb,
    source VARCHAR(32) NOT NULL DEFAULT 'external_polymarket',
    captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_polymarket_orderbook_snapshots_market
    ON polymarket_public_orderbook_snapshots(market_id, outcome, captured_at DESC);

CREATE TABLE IF NOT EXISTS polymarket_wallet_scores (
    wallet VARCHAR(128) PRIMARY KEY,
    market_category VARCHAR(64),
    window_hours INTEGER NOT NULL DEFAULT 168,
    trade_count BIGINT NOT NULL DEFAULT 0,
    markets_traded BIGINT NOT NULL DEFAULT 0,
    recency_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    consistency_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    specialization_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    crowding_penalty DOUBLE PRECISION NOT NULL DEFAULT 0,
    edge_persistence_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    composite_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    last_trade_at TIMESTAMPTZ,
    metrics JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_polymarket_wallet_scores_category
    ON polymarket_wallet_scores(market_category, composite_score DESC);

CREATE INDEX IF NOT EXISTS idx_polymarket_wallet_scores_composite
    ON polymarket_wallet_scores(composite_score DESC, updated_at DESC);

CREATE TABLE IF NOT EXISTS strategy_replay_runs (
    id VARCHAR(64) PRIMARY KEY,
    created_by VARCHAR(64),
    strategy VARCHAR(64) NOT NULL,
    baseline VARCHAR(64),
    status VARCHAR(32) NOT NULL DEFAULT 'completed',
    market_id VARCHAR(128),
    market_category VARCHAR(64),
    target_wallet VARCHAR(128),
    delay_ms INTEGER NOT NULL DEFAULT 0,
    window_hours INTEGER NOT NULL DEFAULT 168,
    input_params JSONB NOT NULL DEFAULT '{}'::jsonb,
    summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_strategy_replay_runs_strategy
    ON strategy_replay_runs(strategy, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_strategy_replay_runs_target_wallet
    ON strategy_replay_runs(target_wallet, created_at DESC)
    WHERE target_wallet IS NOT NULL;

CREATE TABLE IF NOT EXISTS strategy_replay_fills (
    id VARCHAR(64) PRIMARY KEY,
    replay_run_id VARCHAR(64) NOT NULL,
    event_time TIMESTAMPTZ NOT NULL,
    market_id VARCHAR(128) NOT NULL,
    outcome VARCHAR(16) NOT NULL,
    side VARCHAR(16) NOT NULL,
    target_wallet VARCHAR(128),
    followed_trade_id VARCHAR(256),
    requested_quantity DOUBLE PRECISION NOT NULL DEFAULT 0,
    filled_quantity DOUBLE PRECISION NOT NULL DEFAULT 0,
    price DOUBLE PRECISION NOT NULL DEFAULT 0,
    mark_price DOUBLE PRECISION NOT NULL DEFAULT 0,
    fee_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    slippage_ticks DOUBLE PRECISION NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_strategy_replay_fills_run
        FOREIGN KEY (replay_run_id) REFERENCES strategy_replay_runs(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_strategy_replay_fills_run
    ON strategy_replay_fills(replay_run_id, event_time DESC);

CREATE INDEX IF NOT EXISTS idx_strategy_replay_fills_market
    ON strategy_replay_fills(market_id, event_time DESC);
