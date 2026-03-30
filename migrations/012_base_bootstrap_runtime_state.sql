ALTER TABLE base_market_bootstrap_configs
    ADD COLUMN IF NOT EXISTS manager VARCHAR(42),
    ADD COLUMN IF NOT EXISTS launch_tx_hash VARCHAR(66),
    ADD COLUMN IF NOT EXISTS last_reconciled_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_error TEXT;

CREATE TABLE IF NOT EXISTS base_market_bootstrap_agents (
    market_id BIGINT NOT NULL,
    side VARCHAR(3) NOT NULL,
    level_index INTEGER NOT NULL,
    agent_id BIGINT,
    desired_price_bps INTEGER NOT NULL,
    desired_size BIGINT NOT NULL,
    current_price_bps INTEGER,
    current_size BIGINT,
    active BOOLEAN NOT NULL DEFAULT false,
    created_tx_hash VARCHAR(66),
    updated_tx_hash VARCHAR(66),
    deactivated_tx_hash VARCHAR(66),
    last_execute_tx_hash VARCHAR(66),
    last_executed_at TIMESTAMPTZ,
    last_reconciled_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (market_id, side, level_index),
    CONSTRAINT chk_base_market_bootstrap_agents_side
        CHECK (side IN ('yes', 'no'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_base_market_bootstrap_agents_agent_id
    ON base_market_bootstrap_agents(agent_id)
    WHERE agent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_base_market_bootstrap_agents_market
    ON base_market_bootstrap_agents(market_id, side, level_index);

DROP TRIGGER IF EXISTS update_base_market_bootstrap_agents_updated_at ON base_market_bootstrap_agents;
CREATE TRIGGER update_base_market_bootstrap_agents_updated_at
    BEFORE UPDATE ON base_market_bootstrap_agents
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
