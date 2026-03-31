-- 016: Hackathon hardening — constraints, indexes, data integrity
-- Adds check constraints and unique indexes to hackathon tables.

-- Prevent duplicate snapshots for the same wallet at the same time
ALTER TABLE hackathon_snapshots
  ADD CONSTRAINT uq_hackathon_snapshots_wallet_time
  UNIQUE (hackathon_id, wallet_address, snapshot_time);

-- Composite index for per-wallet snapshot time-series queries
CREATE INDEX IF NOT EXISTS idx_hackathon_snapshots_wallet_time
  ON hackathon_snapshots (hackathon_id, wallet_address, snapshot_time DESC);

-- Enforce valid hackathon status values
ALTER TABLE hackathons
  ADD CONSTRAINT chk_hackathons_status
  CHECK (status IN ('upcoming', 'active', 'completed', 'cancelled'));

-- Prize pool must be non-negative
ALTER TABLE hackathons
  ADD CONSTRAINT chk_hackathons_prize
  CHECK (prize_pool_usdc >= 0);

-- Registration status must be valid
ALTER TABLE hackathon_registrations
  ADD CONSTRAINT chk_hackathon_registrations_status
  CHECK (status IN ('active', 'disqualified'));
