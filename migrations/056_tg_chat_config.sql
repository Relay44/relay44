-- Per-Telegram-chat config for the Relay44Bot.
--
-- Backs the state-changing commands (/mute, /threshold, /cooldown, /link,
-- /verify, /unlink) introduced on top of the read-only command handler in
-- `telegram_commands.rs`. One row per chat_id. Overrides are nullable so
-- NULL means "fall back to env default".
--
-- `linked_wallet` is a read-only identity binding established via EIP-191
-- signature verification. No spending authority is granted by the binding:
-- trade execution remains deferred to a future, user-reviewed phase.
--
-- The existing `users` table is keyed on `wallet VARCHAR(44)` (Solana
-- pubkey length) rather than a UUID id column, so `linked_user_id` is kept
-- as a nullable UUID without a foreign key for forward compatibility with
-- a future EVM-native user identifier.

CREATE TABLE IF NOT EXISTS tg_chat_config (
    chat_id            BIGINT PRIMARY KEY,
    threshold_override REAL,
    cooldown_override  INTEGER,
    muted_markets      JSONB NOT NULL DEFAULT '[]'::jsonb,
    allow_categories   JSONB NOT NULL DEFAULT '[]'::jsonb,
    linked_user_id     UUID,
    linked_wallet      TEXT,
    linked_at          TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tg_chat_config_linked_wallet
    ON tg_chat_config (linked_wallet);
