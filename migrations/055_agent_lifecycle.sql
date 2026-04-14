-- Managed agent lifecycle states + durable intent log.
--
-- Turns a managed agent from a cron row into a recoverable protocol object:
--   * `lifecycle_state` tracks runtime execution (initializing → active ⇄ paused,
--     → liquidating → settled, or → failed). Distinct from user-facing `status`
--     so the runner can pause on failures without overriding user intent.
--   * `managed_agent_intents` is an append-only log written before every
--     external side-effect (open_position, close_position, post_order,
--     cancel_order). On process restart the runner reconciles any `pending`
--     intents against observed state and marks them confirmed or abandoned
--     instead of silently re-executing.

ALTER TABLE managed_agents
    ADD COLUMN IF NOT EXISTS lifecycle_state VARCHAR(32) NOT NULL DEFAULT 'initializing';

ALTER TABLE managed_agents
    ADD COLUMN IF NOT EXISTS lifecycle_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

ALTER TABLE managed_agents
    ADD COLUMN IF NOT EXISTS last_checkpoint_at TIMESTAMPTZ;

ALTER TABLE managed_agents
    ADD COLUMN IF NOT EXISTS last_checkpoint_seq BIGINT NOT NULL DEFAULT 0;

ALTER TABLE managed_agents
    ADD COLUMN IF NOT EXISTS failure_reason TEXT;

UPDATE managed_agents
SET lifecycle_state = CASE
    WHEN status = 'active'  THEN 'active'
    WHEN status = 'stopped' THEN 'settled'
    WHEN status = 'paused'  THEN 'paused'
    ELSE 'active'
END,
    lifecycle_updated_at = NOW()
WHERE lifecycle_state = 'initializing';

CREATE INDEX IF NOT EXISTS idx_managed_agents_lifecycle
    ON managed_agents(lifecycle_state, next_execution_at)
    WHERE lifecycle_state IN ('active', 'liquidating');

CREATE TABLE IF NOT EXISTS managed_agent_intents (
    id              BIGSERIAL PRIMARY KEY,
    agent_id        VARCHAR(64) NOT NULL REFERENCES managed_agents(id) ON DELETE CASCADE,
    seq             BIGINT NOT NULL,
    kind            VARCHAR(32) NOT NULL,
    payload         JSONB NOT NULL DEFAULT '{}'::jsonb,
    state           VARCHAR(16) NOT NULL DEFAULT 'pending',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at     TIMESTAMPTZ,
    resolution_note TEXT,
    UNIQUE (agent_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_agent_intents_pending
    ON managed_agent_intents(agent_id)
    WHERE state = 'pending';

CREATE INDEX IF NOT EXISTS idx_agent_intents_created
    ON managed_agent_intents(created_at DESC);
