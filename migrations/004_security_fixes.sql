-- Security and integrity fixes
-- Migration: 004_security_fixes

-- HIGH-023: Add explicit ON DELETE behavior to prevent accidental cascade deletes
-- Drop existing foreign keys and recreate with ON DELETE RESTRICT

-- Orders table
ALTER TABLE orders DROP CONSTRAINT IF EXISTS orders_market_id_fkey;
ALTER TABLE orders ADD CONSTRAINT orders_market_id_fkey
    FOREIGN KEY (market_id) REFERENCES markets(id) ON DELETE RESTRICT;

-- Trades table
ALTER TABLE trades DROP CONSTRAINT IF EXISTS trades_market_id_fkey;
ALTER TABLE trades ADD CONSTRAINT trades_market_id_fkey
    FOREIGN KEY (market_id) REFERENCES markets(id) ON DELETE RESTRICT;

ALTER TABLE trades DROP CONSTRAINT IF EXISTS trades_buy_order_id_fkey;
ALTER TABLE trades ADD CONSTRAINT trades_buy_order_id_fkey
    FOREIGN KEY (buy_order_id) REFERENCES orders(id) ON DELETE RESTRICT;

ALTER TABLE trades DROP CONSTRAINT IF EXISTS trades_sell_order_id_fkey;
ALTER TABLE trades ADD CONSTRAINT trades_sell_order_id_fkey
    FOREIGN KEY (sell_order_id) REFERENCES orders(id) ON DELETE RESTRICT;

-- Positions table
ALTER TABLE positions DROP CONSTRAINT IF EXISTS positions_market_id_fkey;
ALTER TABLE positions ADD CONSTRAINT positions_market_id_fkey
    FOREIGN KEY (market_id) REFERENCES markets(id) ON DELETE RESTRICT;

-- Transactions table
ALTER TABLE transactions DROP CONSTRAINT IF EXISTS transactions_market_id_fkey;
ALTER TABLE transactions ADD CONSTRAINT transactions_market_id_fkey
    FOREIGN KEY (market_id) REFERENCES markets(id) ON DELETE RESTRICT;

-- Orderbook entries
ALTER TABLE orderbook_entries DROP CONSTRAINT IF EXISTS orderbook_entries_market_id_fkey;
ALTER TABLE orderbook_entries ADD CONSTRAINT orderbook_entries_market_id_fkey
    FOREIGN KEY (market_id) REFERENCES markets(id) ON DELETE RESTRICT;

ALTER TABLE orderbook_entries DROP CONSTRAINT IF EXISTS orderbook_entries_order_id_fkey;
ALTER TABLE orderbook_entries ADD CONSTRAINT orderbook_entries_order_id_fkey
    FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE CASCADE;


-- HIGH-025: Fix trigger race condition by using BEFORE trigger
-- Drop old AFTER trigger and create BEFORE trigger
DROP TRIGGER IF EXISTS trg_cleanup_orderbook ON orders;

CREATE OR REPLACE FUNCTION cleanup_orderbook_entry_before()
RETURNS TRIGGER AS $$
BEGIN
    -- Check if status is changing to Filled (2) or Cancelled (3)
    IF NEW.status IN (2, 3) AND OLD.status != NEW.status THEN
        DELETE FROM orderbook_entries WHERE order_id = NEW.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_cleanup_orderbook_before
BEFORE UPDATE ON orders
FOR EACH ROW
EXECUTE FUNCTION cleanup_orderbook_entry_before();


-- HIGH-026: Add fee tracking table
CREATE TABLE IF NOT EXISTS fee_ledger (
    id SERIAL PRIMARY KEY,
    market_id VARCHAR(64) NOT NULL REFERENCES markets(id) ON DELETE RESTRICT,
    tx_type VARCHAR(16) NOT NULL, -- 'collection', 'protocol_withdrawal', 'creator_withdrawal'
    amount BIGINT NOT NULL,
    recipient VARCHAR(44),
    tx_signature VARCHAR(88),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_fee_ledger_market ON fee_ledger(market_id);
CREATE INDEX idx_fee_ledger_type ON fee_ledger(tx_type);

COMMENT ON TABLE fee_ledger IS 'Tracks fee collection and withdrawals per market';


-- Additional index for reconciliation queries
CREATE INDEX IF NOT EXISTS idx_markets_address ON markets(address);
CREATE INDEX IF NOT EXISTS idx_orders_tx_signature ON orders(tx_signature) WHERE tx_signature IS NOT NULL;
