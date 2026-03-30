ALTER TABLE external_agents
    ADD COLUMN IF NOT EXISTS execution_mode VARCHAR(16) NOT NULL DEFAULT 'live';

UPDATE external_agents
SET execution_mode = 'live'
WHERE execution_mode IS NULL OR execution_mode = '';

ALTER TABLE external_agents
    DROP CONSTRAINT IF EXISTS chk_external_agents_execution_mode;

ALTER TABLE external_agents
    ADD CONSTRAINT chk_external_agents_execution_mode
    CHECK (execution_mode IN ('live', 'paper'));

CREATE INDEX IF NOT EXISTS idx_external_agents_public_cohort
    ON external_agents(owner, execution_mode, active, next_execution_at DESC);
