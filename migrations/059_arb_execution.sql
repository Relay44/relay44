-- Cross-venue arbitrage execution tracking.

ALTER TABLE arbitrage_opportunities
    ADD COLUMN IF NOT EXISTS execution_mode VARCHAR(16) NOT NULL DEFAULT 'alert',
    ADD COLUMN IF NOT EXISTS buy_order_id VARCHAR(256),
    ADD COLUMN IF NOT EXISTS sell_order_id VARCHAR(256),
    ADD COLUMN IF NOT EXISTS buy_leg_status VARCHAR(32),
    ADD COLUMN IF NOT EXISTS sell_leg_status VARCHAR(32),
    ADD COLUMN IF NOT EXISTS error_message TEXT;

CREATE TABLE IF NOT EXISTS arb_execution_legs (
    id                SERIAL PRIMARY KEY,
    arb_id            INT NOT NULL REFERENCES arbitrage_opportunities(id),
    leg               VARCHAR(8)   NOT NULL,
    provider          VARCHAR(32)  NOT NULL,
    provider_market_id VARCHAR(256) NOT NULL,
    token_id          VARCHAR(256),
    side              SMALLINT     NOT NULL,
    price             NUMERIC(18,6) NOT NULL,
    quantity_usdc     NUMERIC(18,6) NOT NULL,
    provider_order_id VARCHAR(256),
    status            VARCHAR(32)  NOT NULL DEFAULT 'pending',
    error_message     TEXT,
    submitted_at      TIMESTAMPTZ,
    created_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_arb_legs_arb_id ON arb_execution_legs(arb_id);
