-- Liquidity Mirror: Cross-venue orderbook mirroring and auto-hedging.
--
-- A mirror_market_link connects an internal relay44 market to an external
-- venue market (Limitless, Polymarket, or Aerodrome). The liquidity mirror
-- service reads the external orderbook and synthesizes resting quotes on
-- the internal CLOB. When fills occur, the hedge engine places the
-- corresponding order on the external venue.

CREATE TABLE IF NOT EXISTS mirror_market_links (
    id              SERIAL PRIMARY KEY,
    internal_market_id BIGINT NOT NULL,
    external_market_id VARCHAR(256) NOT NULL,
    external_provider  VARCHAR(32)  NOT NULL,
    spread_premium_bps INT          NOT NULL DEFAULT 50,
    max_depth_usdc     NUMERIC(18,6) NOT NULL DEFAULT 5000,
    hedge_mode         VARCHAR(32)  NOT NULL DEFAULT 'auto',
    hedge_credential_id VARCHAR(64),
    active             BOOLEAN      NOT NULL DEFAULT true,
    last_mirror_at     TIMESTAMPTZ,
    last_hedge_at      TIMESTAMPTZ,
    mirror_error       TEXT,
    hedge_error        TEXT,
    total_mirrored_usdc NUMERIC(18,6) NOT NULL DEFAULT 0,
    total_hedged_usdc   NUMERIC(18,6) NOT NULL DEFAULT 0,
    net_exposure_usdc   NUMERIC(18,6) NOT NULL DEFAULT 0,
    created_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    UNIQUE(internal_market_id, external_market_id)
);

CREATE TABLE IF NOT EXISTS mirror_hedge_log (
    id              SERIAL PRIMARY KEY,
    mirror_link_id  INT NOT NULL REFERENCES mirror_market_links(id),
    direction       VARCHAR(16)  NOT NULL,
    side            VARCHAR(4)   NOT NULL,
    outcome         VARCHAR(8)   NOT NULL,
    price           NUMERIC(18,6) NOT NULL,
    quantity        NUMERIC(18,6) NOT NULL,
    hedge_status    VARCHAR(32)  NOT NULL DEFAULT 'pending',
    hedge_provider_order_id VARCHAR(256),
    hedge_tx_hash   VARCHAR(66),
    pnl_usdc        NUMERIC(18,6),
    error_message   TEXT,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_mirror_links_active
    ON mirror_market_links(active) WHERE active = true;
CREATE INDEX IF NOT EXISTS idx_mirror_links_internal
    ON mirror_market_links(internal_market_id);
CREATE INDEX IF NOT EXISTS idx_mirror_hedge_log_link
    ON mirror_hedge_log(mirror_link_id);
CREATE INDEX IF NOT EXISTS idx_mirror_hedge_log_status
    ON mirror_hedge_log(hedge_status) WHERE hedge_status = 'pending';
