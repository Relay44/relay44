-- Copy trading subscriptions: agent-based strategy mirroring

-- ============================================================
-- Copy trading subscriptions
-- ============================================================
CREATE TABLE copy_trading_subscriptions (
    id                VARCHAR(64) PRIMARY KEY,
    subscriber        VARCHAR(42) NOT NULL,
    target_wallet     VARCHAR(42) NOT NULL,
    agent_id          VARCHAR(64),                                 -- linked external_agent
    allocation_usdc   DOUBLE PRECISION NOT NULL DEFAULT 50.0,
    max_position_usdc DOUBLE PRECISION NOT NULL DEFAULT 20.0,
    active            BOOLEAN NOT NULL DEFAULT TRUE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(subscriber, target_wallet)
);

CREATE INDEX idx_copy_subs_subscriber ON copy_trading_subscriptions(subscriber);
CREATE INDEX idx_copy_subs_target ON copy_trading_subscriptions(target_wallet, active) WHERE active;

DROP TRIGGER IF EXISTS update_copy_trading_subs_updated_at ON copy_trading_subscriptions;
CREATE TRIGGER update_copy_trading_subs_updated_at
    BEFORE UPDATE ON copy_trading_subscriptions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
