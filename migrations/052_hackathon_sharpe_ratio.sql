-- 052: Add Sharpe ratio scoring to hackathon snapshots
-- Stores Sharpe ratio in basis points (e.g., 150 = 1.50 Sharpe ratio)
-- and updates the scoring_method check to allow new methods.

ALTER TABLE hackathon_snapshots
  ADD COLUMN IF NOT EXISTS sharpe_ratio_bps INTEGER NOT NULL DEFAULT 0;

-- Update the scoring_method column to accept new values.
-- The original CHECK constraint on hackathons.scoring_method was implicit (validated in app code).
-- Add a proper DB-level constraint now.
DO $$ BEGIN
  ALTER TABLE hackathons
    ADD CONSTRAINT chk_hackathons_scoring_method
    CHECK (scoring_method IN ('net_pnl', 'sharpe_ratio', 'win_rate'));
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
