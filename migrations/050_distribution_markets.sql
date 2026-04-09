-- Distribution markets (continuous outcome markets)
CREATE TABLE distribution_markets (
    id              VARCHAR(64) PRIMARY KEY,
    address         VARCHAR(42),
    question        TEXT NOT NULL,
    description     TEXT,
    category        VARCHAR(64),
    status          SMALLINT NOT NULL DEFAULT 0,  -- 0=Active,1=Paused,2=Closed,3=Resolved,4=Cancelled
    outcome_min     DOUBLE PRECISION NOT NULL,
    outcome_max     DOUBLE PRECISION NOT NULL,
    outcome_unit    VARCHAR(32),                   -- e.g. "ECV", "%", "USD"
    liquidity_param DOUBLE PRECISION NOT NULL,     -- LMSR "b"
    market_mu       DOUBLE PRECISION,
    market_sigma    DOUBLE PRECISION,
    stiffness       DOUBLE PRECISION,
    peak_density    DOUBLE PRECISION,
    headroom_pct    DOUBLE PRECISION,
    lambda          DOUBLE PRECISION,
    collateral_token VARCHAR(42) NOT NULL,
    total_collateral BIGINT DEFAULT 0,
    total_volume     BIGINT DEFAULT 0,
    volume_24h       BIGINT DEFAULT 0,
    fee_bps          SMALLINT DEFAULT 100,
    resolver         VARCHAR(42),
    use_oracle       BOOLEAN DEFAULT FALSE,
    oracle_feed_id   VARCHAR(128),
    resolved_value   DOUBLE PRECISION,
    trading_end          TIMESTAMPTZ,
    resolution_deadline  TIMESTAMPTZ,
    created_at           TIMESTAMPTZ DEFAULT NOW(),
    resolved_at          TIMESTAMPTZ
);

CREATE INDEX idx_dist_markets_status ON distribution_markets(status);
CREATE INDEX idx_dist_markets_category ON distribution_markets(category);

CREATE TABLE distribution_positions (
    id              SERIAL PRIMARY KEY,
    position_id     BIGINT NOT NULL,
    market_id       VARCHAR(64) REFERENCES distribution_markets(id),
    owner           VARCHAR(42) NOT NULL,
    mu              DOUBLE PRECISION NOT NULL,
    sigma           DOUBLE PRECISION NOT NULL,
    size            BIGINT NOT NULL,
    collateral      BIGINT NOT NULL,
    cost_basis      DOUBLE PRECISION,
    status          SMALLINT DEFAULT 0,  -- 0=Open, 1=Closed, 2=Resolved(payout set), 3=Claimed
    payout          BIGINT,
    pnl             DOUBLE PRECISION,
    tx_signature    VARCHAR(128),
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    closed_at       TIMESTAMPTZ,
    UNIQUE(market_id, position_id)
);

CREATE INDEX idx_dist_positions_owner ON distribution_positions(owner);
CREATE INDEX idx_dist_positions_market ON distribution_positions(market_id);

CREATE TABLE distribution_trades (
    id              VARCHAR(64) PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id       VARCHAR(64) REFERENCES distribution_markets(id),
    position_id     BIGINT,
    owner           VARCHAR(42) NOT NULL,
    trade_type      VARCHAR(16) NOT NULL,  -- 'open', 'close', 'claim'
    mu              DOUBLE PRECISION NOT NULL,
    sigma           DOUBLE PRECISION NOT NULL,
    size            BIGINT NOT NULL,
    cost            DOUBLE PRECISION NOT NULL DEFAULT 0,
    fees            DOUBLE PRECISION NOT NULL DEFAULT 0,
    new_market_mu   DOUBLE PRECISION,
    new_market_sigma DOUBLE PRECISION,
    tx_signature    VARCHAR(128),
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_dist_trades_market ON distribution_trades(market_id);
CREATE INDEX idx_dist_trades_owner ON distribution_trades(owner);

-- Curve snapshots for time-lapse visualization
CREATE TABLE distribution_curve_snapshots (
    id              SERIAL PRIMARY KEY,
    market_id       VARCHAR(64) REFERENCES distribution_markets(id),
    market_mu       DOUBLE PRECISION NOT NULL,
    market_sigma    DOUBLE PRECISION NOT NULL,
    total_collateral BIGINT DEFAULT 0,
    position_count  INT DEFAULT 0,
    captured_at     TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_dist_curve_snapshots_market ON distribution_curve_snapshots(market_id, captured_at);
