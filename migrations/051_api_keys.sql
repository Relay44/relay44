-- API keys for agent/programmatic access
-- Allows long-lived authentication without wallet signing per request

CREATE TABLE api_keys (
    id              TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
    wallet_address  TEXT NOT NULL,
    key_hash        TEXT NOT NULL UNIQUE,
    key_prefix      VARCHAR(16) NOT NULL,
    label           TEXT NOT NULL DEFAULT '',
    scope           TEXT NOT NULL DEFAULT 'trade'
                    CHECK (scope IN ('read', 'trade', 'admin')),
    is_active       BOOLEAN NOT NULL DEFAULT true,
    expires_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at      TIMESTAMPTZ
);

CREATE INDEX idx_api_keys_hash ON api_keys (key_hash) WHERE is_active = true;
CREATE INDEX idx_api_keys_wallet ON api_keys (wallet_address);
