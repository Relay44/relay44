-- Signal marketplace: third-party signal providers for agent strategies.

CREATE TABLE IF NOT EXISTS signal_providers (
    id              VARCHAR(64) PRIMARY KEY,
    owner           VARCHAR(64) NOT NULL,
    name            VARCHAR(128) NOT NULL,
    description     TEXT,
    source_url      VARCHAR(512),
    category        VARCHAR(64) NOT NULL DEFAULT 'general',
    update_frequency_secs BIGINT NOT NULL DEFAULT 3600,
    active          BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner, name)
);

CREATE INDEX IF NOT EXISTS idx_signal_providers_active
    ON signal_providers(active, category);

CREATE TABLE IF NOT EXISTS signal_emissions (
    id              SERIAL PRIMARY KEY,
    provider_id     VARCHAR(64) NOT NULL REFERENCES signal_providers(id) ON DELETE CASCADE,
    market_slug     VARCHAR(256) NOT NULL,
    outcome         VARCHAR(16) NOT NULL,
    signal_value    DOUBLE PRECISION NOT NULL,
    confidence      DOUBLE PRECISION NOT NULL DEFAULT 0.5,
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_signal_emissions_provider
    ON signal_emissions(provider_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_signal_emissions_market
    ON signal_emissions(market_slug, created_at DESC);

-- Brier scoring for provider accuracy.
CREATE TABLE IF NOT EXISTS signal_scores (
    id              SERIAL PRIMARY KEY,
    provider_id     VARCHAR(64) NOT NULL REFERENCES signal_providers(id) ON DELETE CASCADE,
    market_slug     VARCHAR(256) NOT NULL,
    outcome         VARCHAR(16) NOT NULL,
    predicted_prob  DOUBLE PRECISION NOT NULL,
    actual_outcome  BOOLEAN,
    brier_score     DOUBLE PRECISION,
    scored_at       TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider_id, market_slug, outcome)
);

CREATE INDEX IF NOT EXISTS idx_signal_scores_provider
    ON signal_scores(provider_id);

-- Aggregated provider reputation.
CREATE TABLE IF NOT EXISTS signal_provider_stats (
    provider_id     VARCHAR(64) PRIMARY KEY REFERENCES signal_providers(id) ON DELETE CASCADE,
    total_signals   BIGINT NOT NULL DEFAULT 0,
    scored_signals  BIGINT NOT NULL DEFAULT 0,
    avg_brier_score DOUBLE PRECISION,
    best_category   VARCHAR(64),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
