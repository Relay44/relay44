-- External venue integrations (Limitless + Polymarket)

CREATE TABLE IF NOT EXISTS external_credentials (
    id VARCHAR(64) PRIMARY KEY,
    owner VARCHAR(64) NOT NULL,
    provider VARCHAR(32) NOT NULL,
    label VARCHAR(64) NOT NULL DEFAULT 'default',
    encrypted_payload TEXT NOT NULL,
    key_id VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    revoked_at TIMESTAMPTZ,
    UNIQUE(owner, provider, label)
);

CREATE INDEX IF NOT EXISTS idx_external_credentials_owner ON external_credentials(owner, provider);

CREATE TABLE IF NOT EXISTS external_order_intents (
    id VARCHAR(64) PRIMARY KEY,
    owner VARCHAR(64) NOT NULL,
    provider VARCHAR(32) NOT NULL,
    market_id VARCHAR(128) NOT NULL,
    provider_market_ref VARCHAR(256),
    outcome VARCHAR(16) NOT NULL,
    side VARCHAR(16) NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    quantity DOUBLE PRECISION NOT NULL,
    preflight JSONB NOT NULL DEFAULT '{}'::jsonb,
    typed_data JSONB NOT NULL DEFAULT '{}'::jsonb,
    status VARCHAR(32) NOT NULL DEFAULT 'prepared',
    credential_id VARCHAR(64),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT fk_external_order_intents_credential
        FOREIGN KEY (credential_id) REFERENCES external_credentials(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_external_order_intents_owner ON external_order_intents(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_external_order_intents_status ON external_order_intents(status);

CREATE TABLE IF NOT EXISTS external_orders (
    id VARCHAR(64) PRIMARY KEY,
    owner VARCHAR(64) NOT NULL,
    provider VARCHAR(32) NOT NULL,
    intent_id VARCHAR(64),
    market_id VARCHAR(128) NOT NULL,
    provider_order_id VARCHAR(256),
    status VARCHAR(32) NOT NULL,
    request_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    response_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT fk_external_orders_intent
        FOREIGN KEY (intent_id) REFERENCES external_order_intents(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_external_orders_owner ON external_orders(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_external_orders_provider_order_id ON external_orders(provider, provider_order_id);

CREATE TABLE IF NOT EXISTS external_agents (
    id VARCHAR(64) PRIMARY KEY,
    owner VARCHAR(64) NOT NULL,
    name VARCHAR(120) NOT NULL,
    provider VARCHAR(32) NOT NULL,
    market_id VARCHAR(128) NOT NULL,
    provider_market_ref VARCHAR(256),
    outcome VARCHAR(16) NOT NULL,
    side VARCHAR(16) NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    quantity DOUBLE PRECISION NOT NULL,
    cadence_seconds BIGINT NOT NULL,
    strategy TEXT NOT NULL,
    credential_id VARCHAR(64),
    active BOOLEAN NOT NULL DEFAULT TRUE,
    last_executed_at TIMESTAMPTZ,
    next_execution_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT fk_external_agents_credential
        FOREIGN KEY (credential_id) REFERENCES external_credentials(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_external_agents_owner ON external_agents(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_external_agents_due ON external_agents(active, next_execution_at);

CREATE TABLE IF NOT EXISTS external_agent_runs (
    id VARCHAR(64) PRIMARY KEY,
    agent_id VARCHAR(64) NOT NULL,
    owner VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL,
    intent_id VARCHAR(64),
    external_order_id VARCHAR(64),
    error_message TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT fk_external_agent_runs_agent
        FOREIGN KEY (agent_id) REFERENCES external_agents(id) ON DELETE CASCADE,
    CONSTRAINT fk_external_agent_runs_intent
        FOREIGN KEY (intent_id) REFERENCES external_order_intents(id) ON DELETE SET NULL,
    CONSTRAINT fk_external_agent_runs_order
        FOREIGN KEY (external_order_id) REFERENCES external_orders(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_external_agent_runs_agent ON external_agent_runs(agent_id, created_at DESC);

CREATE TABLE IF NOT EXISTS external_market_cache (
    id VARCHAR(96) PRIMARY KEY,
    provider VARCHAR(32) NOT NULL,
    market_id VARCHAR(128) NOT NULL,
    payload JSONB NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    UNIQUE(provider, market_id)
);

CREATE INDEX IF NOT EXISTS idx_external_market_cache_expires ON external_market_cache(expires_at);
