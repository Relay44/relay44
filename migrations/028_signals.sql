-- Signal marketplace: trading signals and subscriptions

-- ============================================================
-- Trading signals
-- ============================================================
CREATE TABLE trading_signals (
    id               VARCHAR(64) PRIMARY KEY,
    publisher        VARCHAR(42) NOT NULL,
    market_id        VARCHAR(64) NOT NULL REFERENCES markets(id),
    direction        VARCHAR(8) NOT NULL CHECK (direction IN ('yes', 'no', 'neutral')),
    confidence_bps   INTEGER NOT NULL CHECK (confidence_bps BETWEEN 0 AND 10000),
    rationale        TEXT,
    valid_until      TIMESTAMPTZ NOT NULL,
    is_agent         BOOLEAN NOT NULL DEFAULT FALSE,
    agent_id         VARCHAR(64),
    subscriber_count INTEGER NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at      TIMESTAMPTZ,
    outcome_correct  BOOLEAN
);

CREATE INDEX idx_signals_market ON trading_signals(market_id, valid_until DESC);
CREATE INDEX idx_signals_publisher ON trading_signals(publisher, created_at DESC);
CREATE INDEX idx_signals_active ON trading_signals(valid_until) WHERE resolved_at IS NULL;

-- ============================================================
-- Signal subscriptions
-- ============================================================
CREATE TABLE signal_subscriptions (
    subscriber  VARCHAR(42) NOT NULL,
    publisher   VARCHAR(42) NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (subscriber, publisher)
);

CREATE INDEX idx_signal_subs_subscriber ON signal_subscriptions(subscriber);
CREATE INDEX idx_signal_subs_publisher ON signal_subscriptions(publisher);
