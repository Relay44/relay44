-- Wallet transactions table for deposits and withdrawals

CREATE TABLE IF NOT EXISTS transactions (
    id VARCHAR(36) PRIMARY KEY,
    owner VARCHAR(44) NOT NULL,
    market_id VARCHAR(36),
    tx_type SMALLINT NOT NULL,
    amount BIGINT NOT NULL,
    fee BIGINT NOT NULL DEFAULT 0,
    tx_signature VARCHAR(128),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_transactions_owner ON transactions(owner);
CREATE INDEX IF NOT EXISTS idx_transactions_owner_type ON transactions(owner, tx_type);
CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);
CREATE INDEX IF NOT EXISTS idx_transactions_signature ON transactions(tx_signature);
CREATE INDEX IF NOT EXISTS idx_transactions_created ON transactions(created_at DESC);

COMMENT ON TABLE transactions IS 'Wallet deposit and withdrawal transactions';
COMMENT ON COLUMN transactions.tx_type IS '0=Deposit, 1=Withdraw, 2=OrderPlace, 3=OrderFill, 4=OrderCancel, 5=Claim';
COMMENT ON COLUMN transactions.amount IS 'Amount in USDC smallest units (6 decimals)';
COMMENT ON COLUMN transactions.status IS 'pending, confirmed, failed';
