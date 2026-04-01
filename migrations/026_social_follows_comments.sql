-- Social layer: follow relations and market comments

-- ============================================================
-- Trader follow relations
-- ============================================================
CREATE TABLE trader_follows (
    follower    VARCHAR(42) NOT NULL,
    following   VARCHAR(42) NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (follower, following)
);

CREATE INDEX idx_follows_follower  ON trader_follows(follower);
CREATE INDEX idx_follows_following ON trader_follows(following);

-- ============================================================
-- Market comments (hybrid: local DB + Farcaster cross-post)
-- ============================================================
CREATE TABLE market_comments (
    id              VARCHAR(64) PRIMARY KEY,
    market_id       VARCHAR(64) NOT NULL REFERENCES markets(id),
    wallet          VARCHAR(42) NOT NULL,
    text            TEXT NOT NULL,
    parent_id       VARCHAR(64) REFERENCES market_comments(id),
    farcaster_hash  VARCHAR(66),           -- cast hash if cross-posted to Farcaster
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_comments_market ON market_comments(market_id, created_at DESC);
CREATE INDEX idx_comments_wallet ON market_comments(wallet);
CREATE INDEX idx_comments_parent ON market_comments(parent_id) WHERE parent_id IS NOT NULL;
