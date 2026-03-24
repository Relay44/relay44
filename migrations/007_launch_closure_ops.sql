-- Launch closure operational durability tables (matcher/payout/indexer/compliance)

CREATE TABLE IF NOT EXISTS payout_jobs (
    id BIGSERIAL PRIMARY KEY,
    market_id BIGINT NOT NULL,
    wallet VARCHAR(64) NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'pending',
    last_tx VARCHAR(128),
    attempts INT NOT NULL DEFAULT 0,
    last_error TEXT,
    next_retry_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (market_id, wallet)
);

CREATE INDEX IF NOT EXISTS idx_payout_jobs_status_next_retry
    ON payout_jobs(status, next_retry_at);
CREATE INDEX IF NOT EXISTS idx_payout_jobs_market_wallet
    ON payout_jobs(market_id, wallet);

CREATE TABLE IF NOT EXISTS chain_sync_cursors (
    key VARCHAR(128) PRIMARY KEY,
    last_block BIGINT NOT NULL DEFAULT 0,
    meta JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS compliance_decisions (
    id BIGSERIAL PRIMARY KEY,
    request_id VARCHAR(128),
    wallet VARCHAR(64),
    country_code VARCHAR(2),
    action VARCHAR(16) NOT NULL,
    route TEXT NOT NULL,
    method VARCHAR(10) NOT NULL,
    decision VARCHAR(16) NOT NULL,
    reason_code VARCHAR(64) NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    decided_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_compliance_decisions_decided_at
    ON compliance_decisions(decided_at DESC);
CREATE INDEX IF NOT EXISTS idx_compliance_decisions_reason
    ON compliance_decisions(reason_code);
CREATE INDEX IF NOT EXISTS idx_compliance_decisions_wallet
    ON compliance_decisions(wallet);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.routines
        WHERE routine_schema = 'public'
          AND routine_name = 'update_updated_at_column'
    ) THEN
        DROP TRIGGER IF EXISTS update_payout_jobs_updated_at ON payout_jobs;
        CREATE TRIGGER update_payout_jobs_updated_at
            BEFORE UPDATE ON payout_jobs
            FOR EACH ROW
            EXECUTE FUNCTION update_updated_at_column();

        DROP TRIGGER IF EXISTS update_chain_sync_cursors_updated_at ON chain_sync_cursors;
        CREATE TRIGGER update_chain_sync_cursors_updated_at
            BEFORE UPDATE ON chain_sync_cursors
            FOR EACH ROW
            EXECUTE FUNCTION update_updated_at_column();
    END IF;
END $$;
