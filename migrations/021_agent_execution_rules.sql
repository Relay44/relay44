-- Per-agent execution guardrails.
-- These columns enforce safety limits before every submit_to_provider call.
ALTER TABLE external_agents
    ADD COLUMN IF NOT EXISTS max_notional_per_execution DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS max_daily_spend_usdc DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS max_slippage_bps INTEGER;

-- Track daily spend per agent (rolling window via agent_runs metadata).
-- No new table needed — we query external_agent_runs for the last 24h.
