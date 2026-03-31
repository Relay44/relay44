-- Agent failure tracking: exponential backoff for repeatedly failing agents
ALTER TABLE external_agents
    ADD COLUMN IF NOT EXISTS consecutive_failures INTEGER NOT NULL DEFAULT 0;

ALTER TABLE external_agents
    ADD COLUMN IF NOT EXISTS last_error_code TEXT;
