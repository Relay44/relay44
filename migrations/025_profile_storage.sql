-- Profile storage: editable profiles, badge system, Farcaster linking

-- ============================================================
-- User profile fields
-- ============================================================
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS bio             TEXT,
    ADD COLUMN IF NOT EXISTS avatar_url      TEXT,
    ADD COLUMN IF NOT EXISTS website_url     TEXT,
    ADD COLUMN IF NOT EXISTS twitter_handle  VARCHAR(32),
    ADD COLUMN IF NOT EXISTS farcaster_fid   BIGINT,
    ADD COLUMN IF NOT EXISTS updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW();

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_farcaster_fid
    ON users(farcaster_fid) WHERE farcaster_fid IS NOT NULL;

-- ============================================================
-- Badge definitions
-- ============================================================
CREATE TABLE profile_badges (
    id           VARCHAR(32) PRIMARY KEY,
    name         VARCHAR(64) NOT NULL,
    description  TEXT NOT NULL,
    icon         TEXT NOT NULL,
    tier         SMALLINT NOT NULL DEFAULT 0   -- 0=bronze, 1=silver, 2=gold
);

INSERT INTO profile_badges (id, name, description, icon, tier) VALUES
    ('first_trade',    'First Trade',    'Placed their first trade',                   'zap',    0),
    ('century_club',   'Century Club',   'Completed 100+ trades',                      'trophy', 1),
    ('high_volume',    'High Volume',    'Traded $10K+ total volume',                  'trending-up', 1),
    ('top_10',         'Top 10',         'Reached top 10 on any leaderboard',          'award',  2),
    ('market_creator', 'Market Maker',   'Created at least 1 market',                  'plus-circle', 0),
    ('streak_5',       'Hot Streak',     '5 consecutive winning trades',               'flame',  1),
    ('early_adopter',  'Early Adopter',  'Joined during the first month of the platform', 'star', 2)
ON CONFLICT (id) DO NOTHING;

-- ============================================================
-- User badge assignments
-- ============================================================
CREATE TABLE user_badges (
    wallet       VARCHAR(42) NOT NULL,
    badge_id     VARCHAR(32) NOT NULL REFERENCES profile_badges(id),
    earned_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (wallet, badge_id)
);

CREATE INDEX idx_user_badges_wallet ON user_badges(wallet);
