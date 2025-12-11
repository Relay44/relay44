-- Normalize transactions schema for wallet accounting and EVM references.

ALTER TABLE transactions
    ADD COLUMN IF NOT EXISTS market_id VARCHAR(64);

ALTER TABLE transactions
    ADD COLUMN IF NOT EXISTS fee BIGINT NOT NULL DEFAULT 0;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'transactions'
          AND column_name = 'status'
          AND data_type IN ('smallint', 'integer', 'bigint')
    ) THEN
        ALTER TABLE transactions ADD COLUMN IF NOT EXISTS status_new VARCHAR(20);
        UPDATE transactions
        SET status_new = CASE status
            WHEN 0 THEN 'pending'
            WHEN 1 THEN 'confirmed'
            WHEN 2 THEN 'failed'
            ELSE 'pending'
        END;
        ALTER TABLE transactions DROP COLUMN status;
        ALTER TABLE transactions RENAME COLUMN status_new TO status;
    END IF;
END $$;

ALTER TABLE transactions
    ALTER COLUMN owner TYPE VARCHAR(64),
    ALTER COLUMN market_id TYPE VARCHAR(64),
    ALTER COLUMN tx_signature TYPE VARCHAR(128),
    ALTER COLUMN tx_signature DROP NOT NULL,
    ALTER COLUMN status TYPE VARCHAR(20),
    ALTER COLUMN status SET DEFAULT 'pending';

CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);
CREATE INDEX IF NOT EXISTS idx_transactions_signature ON transactions(tx_signature);
