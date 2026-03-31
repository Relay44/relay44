CREATE TABLE IF NOT EXISTS leaderboard_snapshots (
    id BIGSERIAL PRIMARY KEY,
    wallet_address VARCHAR(42) NOT NULL,
    period VARCHAR(16) NOT NULL,
    metric VARCHAR(16) NOT NULL,
    value DOUBLE PRECISION NOT NULL DEFAULT 0,
    rank INTEGER NOT NULL,
    previous_rank INTEGER,
    change DOUBLE PRECISION,
    snapshot_time TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_leaderboard_unique
    ON leaderboard_snapshots(wallet_address, period, metric, snapshot_time);
CREATE INDEX IF NOT EXISTS idx_leaderboard_period_metric
    ON leaderboard_snapshots(period, metric, snapshot_time DESC);
CREATE INDEX IF NOT EXISTS idx_leaderboard_wallet
    ON leaderboard_snapshots(wallet_address);
