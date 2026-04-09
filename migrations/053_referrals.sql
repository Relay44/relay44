-- Referral system: track who referred whom, referral codes, and reward status
CREATE TABLE IF NOT EXISTS referrals (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
    referrer_wallet TEXT NOT NULL,
    referee_wallet TEXT NOT NULL UNIQUE,
    referral_code TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    rewarded BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- A referrer can only refer a given referee once
CREATE UNIQUE INDEX IF NOT EXISTS idx_referrals_referrer_referee
    ON referrals(referrer_wallet, referee_wallet);

-- Fast lookup by referrer
CREATE INDEX IF NOT EXISTS idx_referrals_referrer
    ON referrals(referrer_wallet);

-- Fast lookup by referral code
CREATE INDEX IF NOT EXISTS idx_referrals_code
    ON referrals(referral_code);
