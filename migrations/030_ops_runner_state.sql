CREATE TABLE IF NOT EXISTS ops_runner_state (
    runner_name TEXT PRIMARY KEY,
    last_started_at TIMESTAMPTZ,
    last_succeeded_at TIMESTAMPTZ,
    last_failed_at TIMESTAMPTZ,
    last_status TEXT NOT NULL DEFAULT 'unknown',
    last_error_code TEXT,
    last_error_message TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
