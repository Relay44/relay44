-- Polymarket Alpha System: scanner state, Kelly sizing, calibration tracking

-- Market scanner state — tracks all discovered Polymarket markets with opportunity scoring
CREATE TABLE IF NOT EXISTS polymarket_scanned_markets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    condition_id TEXT NOT NULL UNIQUE,
    question TEXT NOT NULL,
    slug TEXT,
    category TEXT NOT NULL DEFAULT 'unknown',
    yes_token_id TEXT NOT NULL,
    no_token_id TEXT NOT NULL,
    end_date TIMESTAMPTZ,
    active BOOLEAN NOT NULL DEFAULT true,
    volume_usdc NUMERIC(20,2) DEFAULT 0,
    liquidity_usdc NUMERIC(20,2) DEFAULT 0,
    yes_price NUMERIC(10,6),
    no_price NUMERIC(10,6),
    best_bid NUMERIC(10,6),
    best_ask NUMERIC(10,6),
    spread_bps INTEGER,
    implied_probability NUMERIC(8,6),
    calibrated_probability NUMERIC(8,6),
    mispricing_score NUMERIC(10,4),
    opportunity_type TEXT,
    opportunity_score NUMERIC(10,4),
    duration_minutes INTEGER,
    fee_rate_bps INTEGER,
    last_scanned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pm_scan_category ON polymarket_scanned_markets(category);
CREATE INDEX IF NOT EXISTS idx_pm_scan_opportunity ON polymarket_scanned_markets(opportunity_type, opportunity_score DESC)
    WHERE opportunity_type IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_pm_scan_active ON polymarket_scanned_markets(active, last_scanned_at DESC)
    WHERE active = true;
CREATE INDEX IF NOT EXISTS idx_pm_scan_condition ON polymarket_scanned_markets(condition_id);

-- Kelly position sizing state per agent
CREATE TABLE IF NOT EXISTS kelly_position_state (
    agent_id UUID PRIMARY KEY REFERENCES external_agents(id) ON DELETE CASCADE,
    bankroll_usdc NUMERIC(20,2) NOT NULL,
    estimated_prob NUMERIC(8,6) NOT NULL,
    market_price NUMERIC(8,6) NOT NULL,
    estimated_edge_bps INTEGER NOT NULL,
    kelly_fraction NUMERIC(6,4) NOT NULL DEFAULT 0.25,
    position_size_usdc NUMERIC(20,2) NOT NULL,
    max_position_pct NUMERIC(6,4) NOT NULL DEFAULT 0.05,
    is_correlated BOOLEAN NOT NULL DEFAULT false,
    category TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Calibration tracking — per price bucket, per category
CREATE TABLE IF NOT EXISTS calibration_buckets (
    id SERIAL PRIMARY KEY,
    price_bucket_low NUMERIC(6,4) NOT NULL,
    price_bucket_high NUMERIC(6,4) NOT NULL,
    category TEXT NOT NULL DEFAULT 'all',
    total_positions INTEGER NOT NULL DEFAULT 0,
    wins INTEGER NOT NULL DEFAULT 0,
    actual_win_rate NUMERIC(8,6),
    implied_probability NUMERIC(8,6),
    mispricing_pct NUMERIC(8,4),
    last_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(price_bucket_low, price_bucket_high, category)
);

-- Seed calibration buckets from Becker research (72.1M trades)
INSERT INTO calibration_buckets (price_bucket_low, price_bucket_high, category, implied_probability, actual_win_rate, mispricing_pct)
VALUES
    (0.00, 0.02, 'all', 0.01, 0.0043, -57.00),
    (0.02, 0.05, 'all', 0.035, 0.0300, -14.29),
    (0.05, 0.10, 'all', 0.075, 0.0630, -16.00),
    (0.10, 0.15, 'all', 0.125, 0.1100, -12.00),
    (0.15, 0.20, 'all', 0.175, 0.1600, -8.57),
    (0.20, 0.30, 'all', 0.25, 0.2400, -4.00),
    (0.30, 0.40, 'all', 0.35, 0.3450, -1.43),
    (0.40, 0.50, 'all', 0.45, 0.4480, -0.44),
    (0.50, 0.60, 'all', 0.55, 0.5520, 0.36),
    (0.60, 0.70, 'all', 0.65, 0.6550, 0.77),
    (0.70, 0.80, 'all', 0.75, 0.7600, 1.33),
    (0.80, 0.85, 'all', 0.825, 0.8400, 1.82),
    (0.85, 0.90, 'all', 0.875, 0.8900, 1.71),
    (0.90, 0.95, 'all', 0.925, 0.9400, 1.62),
    (0.95, 1.00, 'all', 0.975, 0.9850, 1.03)
ON CONFLICT (price_bucket_low, price_bucket_high, category) DO NOTHING;

-- Category-specific mispricing from research
INSERT INTO calibration_buckets (price_bucket_low, price_bucket_high, category, implied_probability, mispricing_pct)
VALUES
    (0.00, 0.10, 'entertainment', 0.05, -47.90),
    (0.00, 0.10, 'crypto', 0.05, -26.90),
    (0.00, 0.10, 'sports', 0.05, -22.30),
    (0.00, 0.10, 'politics', 0.05, -10.20),
    (0.00, 0.10, 'world', 0.05, -73.20),
    (0.00, 0.10, 'finance', 0.05, -1.70)
ON CONFLICT (price_bucket_low, price_bucket_high, category) DO NOTHING;

-- Scanner run history for monitoring
CREATE TABLE IF NOT EXISTS polymarket_scanner_runs (
    id SERIAL PRIMARY KEY,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    markets_scanned INTEGER DEFAULT 0,
    opportunities_found INTEGER DEFAULT 0,
    longshots_found INTEGER DEFAULT 0,
    near_certainties_found INTEGER DEFAULT 0,
    spread_captures_found INTEGER DEFAULT 0,
    error TEXT
);
