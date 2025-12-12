CREATE TABLE IF NOT EXISTS notification_preferences (
    owner VARCHAR(64) PRIMARY KEY,
    order_fills BOOLEAN NOT NULL DEFAULT TRUE,
    market_resolutions BOOLEAN NOT NULL DEFAULT TRUE,
    price_alerts BOOLEAN NOT NULL DEFAULT TRUE,
    system_announcements BOOLEAN NOT NULL DEFAULT TRUE,
    decision_alerts BOOLEAN NOT NULL DEFAULT TRUE,
    email_notifications BOOLEAN NOT NULL DEFAULT FALSE,
    push_notifications BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS notifications (
    id VARCHAR(64) PRIMARY KEY,
    owner VARCHAR(64) NOT NULL,
    type VARCHAR(48) NOT NULL,
    title VARCHAR(160) NOT NULL,
    message TEXT NOT NULL,
    market_id VARCHAR(128),
    order_id VARCHAR(128),
    decision_cell_id VARCHAR(64),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_notifications_owner_created
    ON notifications(owner, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_notifications_owner_unread
    ON notifications(owner, created_at DESC)
    WHERE read_at IS NULL;

CREATE TABLE IF NOT EXISTS decision_cells (
    id VARCHAR(64) PRIMARY KEY,
    owner VARCHAR(64) NOT NULL,
    title VARCHAR(160) NOT NULL,
    statement TEXT NOT NULL,
    decision_type VARCHAR(24) NOT NULL,
    horizon_at TIMESTAMPTZ,
    status VARCHAR(24) NOT NULL DEFAULT 'active',
    automation_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    current_recommendation VARCHAR(160) NOT NULL DEFAULT 'insufficient_signal',
    confidence_bps INTEGER NOT NULL DEFAULT 0,
    summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_decision_cells_owner_updated
    ON decision_cells(owner, updated_at DESC);

CREATE TABLE IF NOT EXISTS decision_cell_actions (
    id VARCHAR(64) PRIMARY KEY,
    cell_id VARCHAR(64) NOT NULL REFERENCES decision_cells(id) ON DELETE CASCADE,
    label VARCHAR(120) NOT NULL,
    rank INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cell_id, rank),
    UNIQUE(cell_id, label)
);

CREATE TABLE IF NOT EXISTS decision_nodes (
    id VARCHAR(64) PRIMARY KEY,
    cell_id VARCHAR(64) NOT NULL REFERENCES decision_cells(id) ON DELETE CASCADE,
    label VARCHAR(160) NOT NULL,
    description TEXT,
    weight_bps INTEGER NOT NULL DEFAULT 3333,
    source_type VARCHAR(24) NOT NULL DEFAULT 'draft_market',
    source_ref VARCHAR(160),
    status VARCHAR(24) NOT NULL DEFAULT 'draft',
    last_probability_bps INTEGER,
    last_market_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    action_effects JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_decision_nodes_cell_updated
    ON decision_nodes(cell_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS decision_alerts (
    id VARCHAR(64) PRIMARY KEY,
    cell_id VARCHAR(64) NOT NULL REFERENCES decision_cells(id) ON DELETE CASCADE,
    kind VARCHAR(48) NOT NULL,
    threshold JSONB NOT NULL DEFAULT '{}'::jsonb,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    last_triggered_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(cell_id, kind)
);

CREATE TABLE IF NOT EXISTS decision_events (
    id VARCHAR(64) PRIMARY KEY,
    cell_id VARCHAR(64) NOT NULL REFERENCES decision_cells(id) ON DELETE CASCADE,
    node_id VARCHAR(64) REFERENCES decision_nodes(id) ON DELETE SET NULL,
    kind VARCHAR(64) NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_decision_events_cell_created
    ON decision_events(cell_id, created_at DESC);

CREATE TABLE IF NOT EXISTS decision_automation_policies (
    cell_id VARCHAR(64) PRIMARY KEY REFERENCES decision_cells(id) ON DELETE CASCADE,
    max_agent_notional_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
    max_triggers_per_day INTEGER NOT NULL DEFAULT 0,
    min_trigger_interval_seconds BIGINT NOT NULL DEFAULT 0,
    allowed_provider VARCHAR(32),
    require_confidence_bps INTEGER NOT NULL DEFAULT 0,
    active BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
