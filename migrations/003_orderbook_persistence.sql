-- Order book persistence for recovery on restart
-- Migration: 003_orderbook_persistence

-- Order book snapshots for recovery
CREATE TABLE IF NOT EXISTS orderbook_entries (
    id SERIAL PRIMARY KEY,
    market_id VARCHAR(64) NOT NULL REFERENCES markets(id),
    order_id VARCHAR(64) NOT NULL REFERENCES orders(id),
    outcome SMALLINT NOT NULL,
    side SMALLINT NOT NULL,
    price_bps SMALLINT NOT NULL,
    remaining_quantity BIGINT NOT NULL,
    owner VARCHAR(44) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(order_id)
);

CREATE INDEX idx_orderbook_market_outcome ON orderbook_entries(market_id, outcome, side);
CREATE INDEX idx_orderbook_price ON orderbook_entries(market_id, outcome, side, price_bps);

-- Cleanup trigger: remove entry when order is cancelled or filled
CREATE OR REPLACE FUNCTION cleanup_orderbook_entry()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status IN (2, 3) THEN  -- Filled or Cancelled
        DELETE FROM orderbook_entries WHERE order_id = NEW.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_cleanup_orderbook
AFTER UPDATE ON orders
FOR EACH ROW
WHEN (OLD.status != NEW.status)
EXECUTE FUNCTION cleanup_orderbook_entry();

COMMENT ON TABLE orderbook_entries IS 'Live order book entries for persistence and recovery';
