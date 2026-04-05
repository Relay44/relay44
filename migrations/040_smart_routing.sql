-- Smart order routing: cross-venue price comparison and best execution.

-- Market equivalence: maps the same event across multiple venues.
-- Unlike mirror_market_links (1:1 for hedging), this is N:M for routing.
CREATE TABLE IF NOT EXISTS market_venue_links (
    id              SERIAL PRIMARY KEY,
    market_slug     VARCHAR(256) NOT NULL,
    provider        VARCHAR(32)  NOT NULL,
    provider_market_id VARCHAR(256) NOT NULL,
    fee_bps         INT NOT NULL DEFAULT 0,
    active          BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(market_slug, provider)
);

CREATE INDEX IF NOT EXISTS idx_market_venue_links_slug
    ON market_venue_links(market_slug) WHERE active = true;

-- Routing decisions log for transparency and analytics.
CREATE TABLE IF NOT EXISTS routing_decisions (
    id              SERIAL PRIMARY KEY,
    intent_id       VARCHAR(64),
    market_slug     VARCHAR(256) NOT NULL,
    outcome         VARCHAR(16) NOT NULL,
    side            VARCHAR(16) NOT NULL,
    quantity        NUMERIC(18,6) NOT NULL,
    chosen_provider VARCHAR(32) NOT NULL,
    chosen_price    NUMERIC(18,6) NOT NULL,
    alternatives    JSONB NOT NULL DEFAULT '[]'::jsonb,
    savings_bps     INT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_routing_decisions_market
    ON routing_decisions(market_slug, created_at DESC);

-- Arbitrage opportunities detected by the scanner.
CREATE TABLE IF NOT EXISTS arbitrage_opportunities (
    id              SERIAL PRIMARY KEY,
    market_slug     VARCHAR(256) NOT NULL,
    outcome         VARCHAR(16) NOT NULL,
    buy_provider    VARCHAR(32) NOT NULL,
    buy_price       NUMERIC(18,6) NOT NULL,
    sell_provider   VARCHAR(32) NOT NULL,
    sell_price      NUMERIC(18,6) NOT NULL,
    spread_bps      INT NOT NULL,
    max_size_usdc   NUMERIC(18,6) NOT NULL,
    status          VARCHAR(32) NOT NULL DEFAULT 'detected',
    executed_at     TIMESTAMPTZ,
    pnl_usdc        NUMERIC(18,6),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_arb_opportunities_status
    ON arbitrage_opportunities(status) WHERE status = 'detected';
CREATE INDEX IF NOT EXISTS idx_arb_opportunities_market
    ON arbitrage_opportunities(market_slug, created_at DESC);
