-- Polyguard Database Schema
-- Initial migration

-- Markets table
CREATE TABLE IF NOT EXISTS markets (
    id VARCHAR(64) PRIMARY KEY,
    address VARCHAR(44) NOT NULL UNIQUE,
    question TEXT NOT NULL,
    description TEXT,
    category VARCHAR(32),
    status SMALLINT NOT NULL DEFAULT 0,
    yes_price DECIMAL(10, 6) DEFAULT 0.5,
    no_price DECIMAL(10, 6) DEFAULT 0.5,
    yes_supply BIGINT DEFAULT 0,
    no_supply BIGINT DEFAULT 0,
    volume_24h DECIMAL(18, 2) DEFAULT 0,
    total_volume DECIMAL(18, 2) DEFAULT 0,
    total_collateral BIGINT DEFAULT 0,
    fee_bps SMALLINT DEFAULT 100,
    oracle VARCHAR(44) NOT NULL,
    collateral_mint VARCHAR(44) NOT NULL,
    yes_mint VARCHAR(44),
    no_mint VARCHAR(44),
    resolution_deadline TIMESTAMPTZ NOT NULL,
    trading_end TIMESTAMPTZ NOT NULL,
    resolved_outcome SMALLINT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);

CREATE INDEX idx_markets_status ON markets(status);
CREATE INDEX idx_markets_category ON markets(category);
CREATE INDEX idx_markets_created ON markets(created_at DESC);

-- Orders table
CREATE TABLE IF NOT EXISTS orders (
    id VARCHAR(64) PRIMARY KEY,
    order_id BIGINT NOT NULL,
    market_id VARCHAR(64) NOT NULL REFERENCES markets(id),
    owner VARCHAR(44) NOT NULL,
    side SMALLINT NOT NULL,
    outcome SMALLINT NOT NULL,
    order_type SMALLINT NOT NULL DEFAULT 0,
    price DECIMAL(10, 6) NOT NULL,
    price_bps SMALLINT NOT NULL,
    quantity BIGINT NOT NULL,
    filled_quantity BIGINT DEFAULT 0,
    remaining_quantity BIGINT NOT NULL,
    status SMALLINT NOT NULL DEFAULT 0,
    is_private BOOLEAN DEFAULT FALSE,
    tx_signature VARCHAR(88),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ
);

CREATE INDEX idx_orders_market ON orders(market_id, status);
CREATE INDEX idx_orders_owner ON orders(owner, status);
CREATE INDEX idx_orders_created ON orders(created_at DESC);

-- Trades table
CREATE TABLE IF NOT EXISTS trades (
    id VARCHAR(64) PRIMARY KEY,
    market_id VARCHAR(64) NOT NULL REFERENCES markets(id),
    buy_order_id VARCHAR(64) NOT NULL REFERENCES orders(id),
    sell_order_id VARCHAR(64) NOT NULL REFERENCES orders(id),
    outcome SMALLINT NOT NULL,
    price DECIMAL(10, 6) NOT NULL,
    price_bps SMALLINT NOT NULL,
    quantity BIGINT NOT NULL,
    collateral_amount BIGINT NOT NULL,
    buyer VARCHAR(44) NOT NULL,
    seller VARCHAR(44) NOT NULL,
    tx_signature VARCHAR(88),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_trades_market ON trades(market_id, created_at DESC);
CREATE INDEX idx_trades_buyer ON trades(buyer);
CREATE INDEX idx_trades_seller ON trades(seller);

-- Positions table
CREATE TABLE IF NOT EXISTS positions (
    id SERIAL PRIMARY KEY,
    market_id VARCHAR(64) NOT NULL REFERENCES markets(id),
    owner VARCHAR(44) NOT NULL,
    yes_balance BIGINT DEFAULT 0,
    no_balance BIGINT DEFAULT 0,
    avg_yes_cost DECIMAL(10, 6),
    avg_no_cost DECIMAL(10, 6),
    locked_collateral BIGINT DEFAULT 0,
    locked_yes BIGINT DEFAULT 0,
    locked_no BIGINT DEFAULT 0,
    total_deposited BIGINT DEFAULT 0,
    total_withdrawn BIGINT DEFAULT 0,
    open_order_count INT DEFAULT 0,
    total_trades INT DEFAULT 0,
    realized_pnl BIGINT DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(market_id, owner)
);

CREATE INDEX idx_positions_owner ON positions(owner);

-- Users table (optional - for profiles)
CREATE TABLE IF NOT EXISTS users (
    wallet VARCHAR(44) PRIMARY KEY,
    username VARCHAR(32) UNIQUE,
    total_trades BIGINT DEFAULT 0,
    total_volume DECIMAL(18, 2) DEFAULT 0,
    pnl_all_time DECIMAL(18, 2) DEFAULT 0,
    default_privacy_mode VARCHAR(16) DEFAULT 'public',
    notifications_enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Transactions table
CREATE TABLE IF NOT EXISTS transactions (
    id VARCHAR(64) PRIMARY KEY,
    owner VARCHAR(44) NOT NULL,
    tx_type SMALLINT NOT NULL,
    market_id VARCHAR(64) REFERENCES markets(id),
    amount BIGINT NOT NULL,
    token VARCHAR(44) NOT NULL,
    tx_signature VARCHAR(88) NOT NULL,
    status SMALLINT DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_transactions_owner ON transactions(owner, created_at DESC);
CREATE INDEX idx_transactions_type ON transactions(tx_type);

-- Private accounts table (for privacy layer)
CREATE TABLE IF NOT EXISTS private_accounts (
    id SERIAL PRIMARY KEY,
    owner VARCHAR(44) NOT NULL UNIQUE,
    elgamal_pubkey BYTEA NOT NULL,
    encrypted_balance BYTEA,
    plaintext_balance BIGINT DEFAULT 0,
    total_deposited BIGINT DEFAULT 0,
    total_withdrawn BIGINT DEFAULT 0,
    private_order_count BIGINT DEFAULT 0,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_private_accounts_owner ON private_accounts(owner);

-- Comments for documentation
COMMENT ON TABLE markets IS 'Prediction markets';
COMMENT ON TABLE orders IS 'Order book entries';
COMMENT ON TABLE trades IS 'Executed trades';
COMMENT ON TABLE positions IS 'User positions per market';
COMMENT ON TABLE private_accounts IS 'Confidential trading accounts';
