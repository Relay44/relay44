-- Oracle resolver configuration for on-chain markets.
-- Stores oracle feed config per market for automated resolution via OracleResolver contract.

-- ============================================================
-- Oracle market configurations
-- ============================================================
CREATE TABLE oracle_market_configs (
    market_id        BIGINT PRIMARY KEY,
    feed_type        VARCHAR(16) NOT NULL DEFAULT 'manual',   -- 'chainlink', 'manual'
    feed_address     VARCHAR(42),                              -- Chainlink feed contract address
    comparison       VARCHAR(4) NOT NULL DEFAULT 'gt',         -- 'gt', 'gte', 'lt', 'lte', 'eq'
    target_value     NUMERIC(38, 8) NOT NULL DEFAULT 0,        -- threshold scaled to feed decimals
    target_currency  VARCHAR(8) NOT NULL DEFAULT 'usd',
    category         VARCHAR(32),                              -- 'crypto', 'energy', 'finance'
    resolution_hint  TEXT,                                     -- human-readable summary
    configure_tx     VARCHAR(66),                              -- on-chain configureOracle tx hash
    keeper_enabled   BOOLEAN NOT NULL DEFAULT TRUE,
    last_checked_at  TIMESTAMPTZ,
    last_error       TEXT,
    resolve_tx       VARCHAR(66),                              -- on-chain resolve tx hash
    resolved_at      TIMESTAMPTZ,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_oracle_configs_feed_type ON oracle_market_configs(feed_type);
CREATE INDEX idx_oracle_configs_keeper ON oracle_market_configs(keeper_enabled) WHERE keeper_enabled AND resolved_at IS NULL;

DROP TRIGGER IF EXISTS update_oracle_market_configs_updated_at ON oracle_market_configs;
CREATE TRIGGER update_oracle_market_configs_updated_at
    BEFORE UPDATE ON oracle_market_configs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
