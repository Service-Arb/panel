-- Telegram, hardened after review: which Telegram account a chat is, when a link token was
-- issued, a pause of the whole outbox on a 429, a throttle on the bot's unsolicited replies,
-- and when a panel session was last used (the bot does not keep an abandoned one alive).
-- Nullable columns and a new table: the build before this one runs on it unchanged.
SET LOCAL lock_timeout = '3s';

-- The Telegram account that linked the chat, as the profile shows it ("@username").
ALTER TABLE telegram_links ADD COLUMN tg_username text, ADD COLUMN tg_first_name text;

-- When the token was issued: the link's role is confirmed as of then, not of the /start.
ALTER TABLE telegram_link_tokens ADD COLUMN issued_at timestamptz;

-- Telegram asked the bot to wait (429): nothing leaves the outbox before this.
ALTER TABLE telegram_poller ADD COLUMN outbox_paused_until timestamptz;

-- The last unsolicited reply to a chat (help, "not linked", …): at most one per ten minutes.
CREATE TABLE telegram_replies (
	chat_id bigint PRIMARY KEY,
	last_at timestamptz NOT NULL
);

-- The last request the user made with the session; NULL: never since this column came.
ALTER TABLE sessions ADD COLUMN last_seen_at timestamptz;
