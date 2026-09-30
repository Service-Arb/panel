-- Telegram notifications in private chats (spec §8): the link between a panel user and their
-- chat with the bot, what each user chose to hear, and the outbox the bot delivers from. New
-- tables only, so the build before this one runs on it unchanged.
SET LOCAL lock_timeout = '3s';

-- One-time link tokens: the SHA-256 of what went into the t.me/<bot>?start=<token> link,
-- redeemed once by `/start <token>` within ten minutes. The role and name are the caller's
-- at the moment the link was asked for, carried to the link it becomes.
CREATE TABLE telegram_link_tokens (
	token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash) = 32),
	user_id uuid NOT NULL,
	role text NOT NULL CHECK (role IN ('operator', 'admin')),
	display_name text NOT NULL,
	expires_at timestamptz NOT NULL
);

CREATE INDEX telegram_link_tokens_by_expiry ON telegram_link_tokens (expires_at);

-- A panel user's private chat with the bot: one each way.
CREATE TABLE telegram_links (
	user_id uuid PRIMARY KEY,
	chat_id bigint NOT NULL UNIQUE,
	linked_at timestamptz NOT NULL,
	-- The role concierge last confirmed for the user, and when; NULL when it last said the
	-- user has none. Nothing is sent on a confirmation older than the engine's ACCESS_TTL.
	role text CHECK (role IN ('operator', 'admin')),
	role_checked_at timestamptz NOT NULL,
	-- When the worker last tried to confirm it, answer or not: it asks at most so often.
	access_tried_at timestamptz,
	-- preferred_name or email: what "taken by" says.
	display_name text NOT NULL,
	-- The bot was blocked (403): nothing goes to the chat until the user links again.
	dead_at timestamptz
);

-- The rules a user turned on or off; a rule without a row is at its default.
CREATE TABLE telegram_rules (
	user_id uuid NOT NULL,
	rule text NOT NULL CHECK (rule IN ('new_lead', 'contact_overdue', 'payment_received', 'source_silent')),
	enabled boolean NOT NULL,
	PRIMARY KEY (user_id, rule)
);

-- What a rule has already been fanned out for: a journal event, a lead's creation, or a
-- source on a day. Taken once, by whichever replica gets there first.
CREATE TABLE telegram_fanout (
	rule text NOT NULL,
	event_id uuid NOT NULL,
	fanned_at timestamptz NOT NULL,
	PRIMARY KEY (rule, event_id)
);

CREATE INDEX telegram_fanout_by_time ON telegram_fanout (fanned_at);

-- The outbox: one row per message per chat. The text holds PII (a customer's phone), so it
-- is sealed under PANEL_DATA_KEY like the journal's, and dropped once the message is sent
-- or given up on. The buttons are named, not stored: their callback data is signed at send
-- time over the row's id.
CREATE TABLE telegram_outbox (
	id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
	user_id uuid NOT NULL,
	chat_id bigint NOT NULL,
	rule text NOT NULL,
	event_id uuid NOT NULL,
	-- The lead the buttons act on.
	brand_id text,
	lead_id text,
	buttons text[] NOT NULL DEFAULT '{}',
	text_sealed bytea,
	data_key_fp bytea CHECK (octet_length(data_key_fp) = 32),
	state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'sent', 'dead')),
	attempts integer NOT NULL DEFAULT 0,
	next_attempt_at timestamptz NOT NULL,
	-- A replica sending it holds it until then; a lease past its time is taken over.
	leased_until timestamptz,
	-- When the last try started: the pacing counts these.
	attempted_at timestamptz,
	sent_at timestamptz,
	tg_message_id bigint,
	last_error text,
	created_at timestamptz NOT NULL,
	UNIQUE (rule, event_id, chat_id),
	CHECK ((state = 'pending') = (text_sealed IS NOT NULL)),
	CHECK ((text_sealed IS NULL) = (data_key_fp IS NULL))
);

CREATE INDEX telegram_outbox_due ON telegram_outbox (next_attempt_at, id) WHERE state = 'pending';
CREATE INDEX telegram_outbox_by_attempt ON telegram_outbox (attempted_at);
CREATE INDEX telegram_outbox_by_chat ON telegram_outbox (chat_id, attempted_at);

-- The one replica that long-polls getUpdates (Telegram answers a second poller 409), and the
-- next update id to ask for, so a new holder carries on where the last one stopped.
CREATE TABLE telegram_poller (
	id boolean PRIMARY KEY DEFAULT true CHECK (id),
	holder uuid,
	leased_until timestamptz,
	next_update_id bigint NOT NULL DEFAULT 0
);

INSERT INTO telegram_poller DEFAULT VALUES;
