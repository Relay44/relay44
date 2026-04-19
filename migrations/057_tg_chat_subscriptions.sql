-- Per-chat subscription controls layered on top of tg_chat_config.
--
-- `quiet_until` lets a chat snooze digest delivery until a timestamp
-- (NULL = not quiet). The digest_scheduler checks this before sending
-- and silently skips while the window is active.
--
-- `subscribed_kinds` is a JSONB array of SignalKind strings (e.g.
-- "probability_shift", "volume_spike"). Empty array means "all kinds"
-- so existing chats continue to receive everything without backfill.

ALTER TABLE tg_chat_config
    ADD COLUMN IF NOT EXISTS quiet_until      TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS subscribed_kinds JSONB NOT NULL DEFAULT '[]'::jsonb;
