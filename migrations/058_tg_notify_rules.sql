-- Per-chat one-shot price-cross alerts. /notify <slug> <pct> creates a row;
-- the scheduler ticks, detects a crossing, sends a DM, marks fired_at, and
-- the row is no longer evaluated. Users who want a recurring trigger can
-- re-issue /notify after it fires.

CREATE TABLE IF NOT EXISTS tg_notify_rules (
    id BIGSERIAL PRIMARY KEY,
    chat_id BIGINT NOT NULL,
    venue TEXT NOT NULL CHECK (venue IN ('polymarket', 'limitless')),
    slug TEXT NOT NULL,
    -- Threshold in YES-price space, 0.0..1.0. A "60%" arg becomes 0.60.
    threshold DOUBLE PRECISION NOT NULL CHECK (threshold > 0.0 AND threshold < 1.0),
    -- Baseline price at rule creation. The scheduler fires when the current
    -- price crosses `threshold` from the side `baseline_price` was on, so a
    -- rule created when YES=0.45 with threshold 0.60 fires on the upside,
    -- and a rule created when YES=0.70 with threshold 0.60 fires on the
    -- downside.
    baseline_price DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    fired_at TIMESTAMPTZ,
    fired_price DOUBLE PRECISION
);

-- Active rules, by chat (for /notify list).
CREATE INDEX IF NOT EXISTS idx_tg_notify_rules_active_by_chat
    ON tg_notify_rules (chat_id, created_at DESC)
    WHERE fired_at IS NULL;

-- Active rules, for the scheduler's per-tick scan grouped by market.
CREATE INDEX IF NOT EXISTS idx_tg_notify_rules_active_by_market
    ON tg_notify_rules (venue, slug)
    WHERE fired_at IS NULL;
