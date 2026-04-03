-- Portfolio-level risk governor state per owner.
-- Tracks aggregate exposure, daily PnL, and kill switch state.

CREATE TABLE IF NOT EXISTS risk_governor_state (
    owner              TEXT PRIMARY KEY,
    bankroll_usdc      DOUBLE PRECISION NOT NULL DEFAULT 5000,
    daily_pnl_usdc     DOUBLE PRECISION NOT NULL DEFAULT 0,
    daily_reset_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    weekly_high_usdc   DOUBLE PRECISION NOT NULL DEFAULT 5000,
    gross_open_usdc    DOUBLE PRECISION NOT NULL DEFAULT 0,
    kill_switch_active BOOLEAN NOT NULL DEFAULT false,
    kill_switch_reason TEXT,
    kill_switch_at     TIMESTAMPTZ,
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
