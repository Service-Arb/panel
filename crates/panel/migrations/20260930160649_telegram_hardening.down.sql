-- Drops the Telegram account names, the reply throttle and the session activity; nothing a
-- running build before this one reads.
SET LOCAL lock_timeout = '3s';

ALTER TABLE sessions DROP COLUMN last_seen_at;
DROP TABLE telegram_replies;
ALTER TABLE telegram_poller DROP COLUMN outbox_paused_until;
ALTER TABLE telegram_link_tokens DROP COLUMN issued_at;
ALTER TABLE telegram_links DROP COLUMN tg_first_name, DROP COLUMN tg_username;
