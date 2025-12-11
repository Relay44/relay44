-- Add dispute resolution and auth tables

-- Auth nonces (for wallet signature verification)
CREATE TABLE IF NOT EXISTS auth_nonces (
    wallet_address VARCHAR(64) PRIMARY KEY,
    nonce VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_nonces_expires_at ON auth_nonces(expires_at);

-- Disputes table
CREATE TABLE IF NOT EXISTS disputes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id VARCHAR(64) NOT NULL REFERENCES markets(id),
    disputer VARCHAR(64) NOT NULL,
    original_outcome SMALLINT NOT NULL,
    status SMALLINT NOT NULL DEFAULT 0,
    bond_amount BIGINT NOT NULL,
    reason_hash VARCHAR(128) NOT NULL,
    consensus_outcome SMALLINT,
    consensus_score SMALLINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    UNIQUE(market_id)
);

CREATE INDEX IF NOT EXISTS idx_disputes_status ON disputes(status);
CREATE INDEX IF NOT EXISTS idx_disputes_disputer ON disputes(disputer);

-- Dispute votes table
CREATE TABLE IF NOT EXISTS dispute_votes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dispute_id UUID NOT NULL REFERENCES disputes(id),
    oracle VARCHAR(64) NOT NULL,
    outcome_vote SMALLINT NOT NULL,
    confidence_score SMALLINT NOT NULL,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(dispute_id, oracle)
);

CREATE INDEX IF NOT EXISTS idx_dispute_votes_dispute_id ON dispute_votes(dispute_id);

-- Add updated_at trigger function if not exists
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Add updated_at column to markets if not exists
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'markets' AND column_name = 'updated_at') THEN
        ALTER TABLE markets ADD COLUMN updated_at TIMESTAMPTZ DEFAULT NOW();
    END IF;
END $$;

-- Create trigger for markets updated_at
DROP TRIGGER IF EXISTS update_markets_updated_at ON markets;
CREATE TRIGGER update_markets_updated_at BEFORE UPDATE ON markets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
