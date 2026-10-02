-- The panel's whole schema (spec §3, §4, §6, §8), in SQLite: the journal, the sources that
-- write to it, the projections built from it, the sign-in sessions, Telegram's tables, the
-- PostHog import's lease, and the `reporting_*` views.
--
-- Conventions, the same in every table:
-- - Tables are STRICT: a value of the wrong type is refused, as Postgres would.
-- - A timestamp is INTEGER microseconds since the Unix epoch, UTC (`store::to_db`): the
--   precision Postgres' timestamptz had, and it compares and sorts as a number.
-- - A day is TEXT `YYYY-MM-DD`, checked to be a real date; it sorts as a day.
-- - A UUID is its 16 bytes as a BLOB (sqlx's encoding), which sorts as Postgres' uuid did.
-- - A boolean is INTEGER 0 or 1.
-- - JSON is TEXT that must parse (`json_valid`).
-- - `unixepoch('subsec')` is SQLite's clock, used only where the caller passes no time.
-- Foreign keys are enforced: the connection sets PRAGMA foreign_keys = ON (`store::options`).

CREATE TABLE sources (
	key_id TEXT PRIMARY KEY CHECK (length(key_id) BETWEEN 1 AND 64 AND key_id GLOB '[a-z0-9]*' AND key_id NOT GLOB '*[^a-z0-9_-]*'),
	kind TEXT NOT NULL CHECK (kind IN ('site', 'review_archive', 'gbp', 'posthog', 'panel', 'sheet', 'telephony')),
	-- A JSON array of brand ids, never empty.
	brand_ids TEXT NOT NULL CHECK (json_valid(brand_ids) AND json_type(brand_ids) = 'array' AND json_array_length(brand_ids) > 0),
	-- The HMAC secret has to be usable to verify, so it is sealed, not hashed: XChaCha20-Poly1305
	-- under PANEL_DATA_KEY, the key id bound as associated data.
	secret_sealed BLOB NOT NULL,
	-- Which data key sealed it (a fingerprint, not the key), so a rotated key fails by name.
	data_key_fp BLOB NOT NULL CHECK (length(data_key_fp) = 32),
	created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000000 AS INTEGER)),
	revoked_at INTEGER
) STRICT;

-- Sources are added and revoked, never edited otherwise or deleted: their events name them.
CREATE TRIGGER sources_no_delete BEFORE DELETE ON sources
BEGIN
	SELECT RAISE(ABORT, 'sources are never deleted: DELETE refused');
END;

CREATE TRIGGER sources_only_revoked BEFORE UPDATE ON sources
WHEN NEW.key_id IS NOT OLD.key_id OR NEW.kind IS NOT OLD.kind OR NEW.brand_ids IS NOT OLD.brand_ids
	OR NEW.secret_sealed IS NOT OLD.secret_sealed OR NEW.data_key_fp IS NOT OLD.data_key_fp OR NEW.created_at IS NOT OLD.created_at
BEGIN
	SELECT RAISE(ABORT, 'a source only ever gets revoked: UPDATE refused');
END;

-- Append-only: rows are inserted once and never deleted; only `status` and
-- `status_reason` ever change, when `panel rebuild-projections` re-reads an event against
-- the registry as it is now. Enforced by the triggers below.
CREATE TABLE events (
	id BLOB PRIMARY KEY CHECK (length(id) = 16),
	schema TEXT NOT NULL,
	type TEXT NOT NULL,
	type_version INTEGER NOT NULL CHECK (type_version > 0),
	occurred_at INTEGER NOT NULL,
	received_at INTEGER NOT NULL,
	source_kind TEXT NOT NULL CHECK (source_kind IN ('site', 'review_archive', 'gbp', 'posthog', 'panel', 'sheet', 'telephony')),
	source_id TEXT NOT NULL,
	-- The key that signed it; NULL for events the panel writes itself.
	key_id TEXT REFERENCES sources (key_id),
	brand_id TEXT NOT NULL,
	location_id TEXT,
	lead_id TEXT,
	job_id TEXT,
	properties TEXT NOT NULL CHECK (json_valid(properties)),
	pii_sealed BLOB,
	data_key_fp BLOB CHECK (length(data_key_fp) = 32),
	-- HMAC-SHA256 of the event in canonical form, under a key derived from PANEL_DATA_KEY: a
	-- resend with the same id and other content is refused. Keyed, not a bare hash, so a
	-- dump cannot confirm a guess at the PII it covers.
	content_mac BLOB NOT NULL CHECK (length(content_mac) = 32),
	-- registered: projected. unregistered: a type@version the panel does not know (yet).
	-- invalid: a type registered after the event arrived, whose checks it fails.
	status TEXT NOT NULL CHECK (status IN ('registered', 'unregistered', 'invalid')),
	status_reason TEXT,
	manual INTEGER GENERATED ALWAYS AS (source_kind = 'panel') STORED,
	CHECK ((pii_sealed IS NULL) = (data_key_fp IS NULL))
) STRICT;

CREATE INDEX events_by_lead ON events (brand_id, lead_id, occurred_at, id) WHERE lead_id IS NOT NULL;
CREATE INDEX events_in_order ON events (occurred_at, id);
CREATE INDEX events_not_registered ON events (type, type_version) WHERE status <> 'registered';
-- When a source last sent: the silent-source rule.
CREATE INDEX events_by_key ON events (key_id, received_at) WHERE key_id IS NOT NULL;

-- SQLite has no TRUNCATE: an unqualified DELETE is a DELETE, and a table with a DELETE
-- trigger is never emptied behind it.
CREATE TRIGGER events_no_delete BEFORE DELETE ON events
BEGIN
	SELECT RAISE(ABORT, 'events is append-only: DELETE refused');
END;

-- Every column but `status` and `status_reason`, compared NULL-safe (`IS NOT`): an UPDATE that
-- writes the same value is let through, as Postgres' IS NOT DISTINCT FROM let it. `manual` is
-- generated and cannot be written at all.
CREATE TRIGGER events_only_status BEFORE UPDATE ON events
WHEN NEW.id IS NOT OLD.id OR NEW.schema IS NOT OLD.schema OR NEW.type IS NOT OLD.type OR NEW.type_version IS NOT OLD.type_version
	OR NEW.occurred_at IS NOT OLD.occurred_at OR NEW.received_at IS NOT OLD.received_at
	OR NEW.source_kind IS NOT OLD.source_kind OR NEW.source_id IS NOT OLD.source_id OR NEW.key_id IS NOT OLD.key_id
	OR NEW.brand_id IS NOT OLD.brand_id OR NEW.location_id IS NOT OLD.location_id OR NEW.lead_id IS NOT OLD.lead_id OR NEW.job_id IS NOT OLD.job_id
	OR NEW.properties IS NOT OLD.properties OR NEW.pii_sealed IS NOT OLD.pii_sealed OR NEW.data_key_fp IS NOT OLD.data_key_fp
	OR NEW.content_mac IS NOT OLD.content_mac
BEGIN
	SELECT RAISE(ABORT, 'events is append-only: UPDATE refused');
END;

-- ── projections: derived from `events` only, rebuilt by `panel rebuild-projections` ──────

CREATE TABLE leads (
	brand_id TEXT NOT NULL,
	lead_id TEXT NOT NULL,
	location_id TEXT,
	job_id TEXT,
	stage TEXT NOT NULL CHECK (stage IN ('created', 'contacted', 'quoted', 'won', 'completed', 'paid', 'lost')),
	channel TEXT CHECK (channel IN ('form', 'phone_inbound')),
	manual INTEGER NOT NULL CHECK (manual IN (0, 1)),
	-- When the lead first reached each stage.
	created_at INTEGER,
	contacted_at INTEGER,
	quoted_at INTEGER,
	won_at INTEGER,
	completed_at INTEGER,
	paid_at INTEGER,
	lost_at INTEGER,
	lost_reason TEXT,
	last_event_id BLOB NOT NULL REFERENCES events (id),
	last_event_at INTEGER NOT NULL,
	PRIMARY KEY (brand_id, lead_id)
) STRICT;

CREATE TABLE calls (
	event_id BLOB PRIMARY KEY REFERENCES events (id),
	brand_id TEXT NOT NULL,
	lead_id TEXT NOT NULL,
	location_id TEXT,
	kind TEXT NOT NULL CHECK (kind IN ('attempted', 'logged')),
	outcome TEXT CHECK (outcome IN ('answered', 'no_answer', 'wrong_number', 'later')),
	attempt_id TEXT,
	occurred_at INTEGER NOT NULL,
	manual INTEGER NOT NULL CHECK (manual IN (0, 1)),
	CHECK ((kind = 'logged') = (outcome IS NOT NULL))
) STRICT;

CREATE INDEX calls_by_lead ON calls (brand_id, lead_id, occurred_at);

CREATE TABLE payments (
	event_id BLOB PRIMARY KEY REFERENCES events (id),
	brand_id TEXT NOT NULL,
	lead_id TEXT NOT NULL,
	location_id TEXT,
	job_id TEXT,
	-- Minor units of `currency`.
	billed INTEGER NOT NULL CHECK (billed >= 0),
	commission INTEGER NOT NULL CHECK (commission BETWEEN 0 AND billed),
	currency TEXT NOT NULL CHECK (length(currency) = 3 AND currency NOT GLOB '*[^A-Z]*'),
	occurred_at INTEGER NOT NULL,
	manual INTEGER NOT NULL CHECK (manual IN (0, 1))
) STRICT;

CREATE INDEX payments_by_lead ON payments (brand_id, lead_id, occurred_at);

-- One count per UTC day, brand, location and slice: page views by traffic source, intents
-- by channel, projected from the `site.metrics` and `contact.metrics` events the PostHog
-- import journals. A later count of the same slice is a new event with the next revision;
-- the row keeps the highest. `location_id` NULL: pages naming no location.
CREATE TABLE daily_location_metrics (
	day TEXT NOT NULL CHECK (date(day) IS day),
	brand_id TEXT NOT NULL,
	location_id TEXT,
	metric TEXT NOT NULL CHECK (metric IN ('visits', 'contact_intent')),
	-- visits: the source (utm_source, the referrer's host, direct, …); contact_intent: the channel.
	dimension TEXT NOT NULL,
	value INTEGER NOT NULL CHECK (value >= 0),
	revision INTEGER NOT NULL CHECK (revision > 0),
	event_id BLOB NOT NULL REFERENCES events (id),
	-- A UNIQUE in SQLite takes NULLs as distinct; the slice must not (no location is one
	-- slice). A location id is a non-empty slug, so '' stands for none without colliding.
	location_key TEXT GENERATED ALWAYS AS (coalesce(location_id, '')) VIRTUAL,
	UNIQUE (day, brand_id, location_key, metric, dimension)
) STRICT;

-- One count per UTC day, brand, experiment and variant, forced (QA) visits left out.
CREATE TABLE daily_experiment_metrics (
	day TEXT NOT NULL CHECK (date(day) IS day),
	brand_id TEXT NOT NULL,
	experiment TEXT NOT NULL,
	variant TEXT NOT NULL,
	exposures INTEGER NOT NULL CHECK (exposures >= 0),
	leads INTEGER NOT NULL CHECK (leads >= 0),
	phone INTEGER NOT NULL CHECK (phone >= 0),
	whatsapp INTEGER NOT NULL CHECK (whatsapp >= 0),
	form_open INTEGER NOT NULL CHECK (form_open >= 0),
	booking INTEGER NOT NULL CHECK (booking >= 0),
	revision INTEGER NOT NULL CHECK (revision > 0),
	event_id BLOB NOT NULL REFERENCES events (id),
	PRIMARY KEY (day, brand_id, experiment, variant)
) STRICT;

-- ── sign-in (spec §4) ───────────────────────────────────────────────────────────────────

-- Signed-in panel users: one row per browser session, opened by the concierge code flow.
CREATE TABLE sessions (
	-- SHA-256 of the session cookie's value: a dump does not hand out sessions.
	id_hash BLOB PRIMARY KEY CHECK (length(id_hash) = 32),
	-- The concierge user id, the tokens' `sub`.
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	-- The relying party's access and refresh tokens, sealed under PANEL_DATA_KEY
	-- (XChaCha20-Poly1305, the row's id_hash bound as associated data).
	access_sealed BLOB NOT NULL,
	access_expires_at INTEGER NOT NULL,
	refresh_sealed BLOB NOT NULL,
	data_key_fp BLOB NOT NULL CHECK (length(data_key_fp) = 32),
	created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000000 AS INTEGER)),
	-- The refresh family's deadline: past it the session is over, whatever the cookie says.
	expires_at INTEGER NOT NULL,
	-- Whoever set this (and it is in the future) is rotating the session's tokens: the others
	-- wait for the result instead of presenting the same refresh token a second time.
	rotating_until INTEGER,
	-- The last request the user made with the session.
	last_seen_at INTEGER
) STRICT;

CREATE INDEX sessions_by_expiry ON sessions (expires_at);
-- Sign-out closes every session of the user.
CREATE INDEX sessions_by_user ON sessions (user_id);

-- The `state` of every callback that got as far as presenting its code, as SHA-256: a
-- replayed callback (same pre-login cookie, same state) is refused before concierge is
-- asked. Rows are dropped past `expires_at`, when the pre-login they match has expired too.
CREATE TABLE consumed_states (
	state_hash BLOB PRIMARY KEY CHECK (length(state_hash) = 32),
	expires_at INTEGER NOT NULL
) STRICT;

CREATE INDEX consumed_states_by_expiry ON consumed_states (expires_at);

-- ── Telegram notifications in private chats (spec §8) ──────────────────────────────────

-- One-time link tokens: the SHA-256 of what went into the t.me/<bot>?start=<token> link,
-- redeemed once by `/start <token>` within ten minutes. The role and name are the caller's
-- at the moment the link was asked for, carried to the link it becomes.
CREATE TABLE telegram_link_tokens (
	token_hash BLOB PRIMARY KEY CHECK (length(token_hash) = 32),
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	role TEXT NOT NULL CHECK (role IN ('operator', 'admin')),
	display_name TEXT NOT NULL,
	expires_at INTEGER NOT NULL,
	-- When the token was issued: the link's role is confirmed as of then, not of the /start.
	issued_at INTEGER
) STRICT;

CREATE INDEX telegram_link_tokens_by_expiry ON telegram_link_tokens (expires_at);

-- A panel user's private chat with the bot: one each way.
CREATE TABLE telegram_links (
	user_id BLOB PRIMARY KEY CHECK (length(user_id) = 16),
	chat_id INTEGER NOT NULL UNIQUE,
	linked_at INTEGER NOT NULL,
	-- The role concierge last confirmed for the user, and when; NULL when it last said the
	-- user has none. Nothing is sent on a confirmation older than the engine's ACCESS_TTL.
	role TEXT CHECK (role IN ('operator', 'admin')),
	role_checked_at INTEGER NOT NULL,
	-- When the worker last tried to confirm it, answer or not: it asks at most so often.
	access_tried_at INTEGER,
	-- preferred_name or email: what "taken by" says.
	display_name TEXT NOT NULL,
	-- The bot was blocked (403): nothing goes to the chat until the user links again.
	dead_at INTEGER,
	-- The Telegram account that linked the chat, as the profile shows it ("@username").
	tg_username TEXT,
	tg_first_name TEXT
) STRICT;

-- The rules a user turned on or off; a rule without a row is at its default.
CREATE TABLE telegram_rules (
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	rule TEXT NOT NULL CHECK (rule IN ('new_lead', 'contact_overdue', 'payment_received', 'source_silent')),
	enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
	PRIMARY KEY (user_id, rule)
) STRICT;

-- What a rule has already been fanned out for: a journal event, a lead's creation, or a
-- source on a day. Taken once.
CREATE TABLE telegram_fanout (
	rule TEXT NOT NULL,
	event_id BLOB NOT NULL,
	fanned_at INTEGER NOT NULL,
	PRIMARY KEY (rule, event_id)
) STRICT;

CREATE INDEX telegram_fanout_by_time ON telegram_fanout (fanned_at);

-- The outbox: one row per message per chat. The text holds PII (a customer's phone), so it
-- is sealed under PANEL_DATA_KEY like the journal's, and dropped once the message is sent
-- or given up on. The buttons are named, not stored: their callback data is signed at send
-- time over the row's id — which AUTOINCREMENT never hands out twice, so an old button
-- cannot land on a newer message after the outbox is pruned.
CREATE TABLE telegram_outbox (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	chat_id INTEGER NOT NULL,
	rule TEXT NOT NULL,
	event_id BLOB NOT NULL,
	-- The lead the buttons act on.
	brand_id TEXT,
	lead_id TEXT,
	-- A JSON array of button names.
	buttons TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(buttons) AND json_type(buttons) = 'array'),
	text_sealed BLOB,
	data_key_fp BLOB CHECK (length(data_key_fp) = 32),
	state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'sent', 'dead')),
	attempts INTEGER NOT NULL DEFAULT 0,
	next_attempt_at INTEGER NOT NULL,
	-- A sender holds it until then; a lease past its time is taken over.
	leased_until INTEGER,
	-- When the last try started: the pacing counts these.
	attempted_at INTEGER,
	sent_at INTEGER,
	tg_message_id INTEGER,
	last_error TEXT,
	created_at INTEGER NOT NULL,
	UNIQUE (rule, event_id, chat_id),
	CHECK ((state = 'pending') = (text_sealed IS NOT NULL)),
	CHECK ((text_sealed IS NULL) = (data_key_fp IS NULL))
) STRICT;

CREATE INDEX telegram_outbox_due ON telegram_outbox (next_attempt_at, id) WHERE state = 'pending';
CREATE INDEX telegram_outbox_by_attempt ON telegram_outbox (attempted_at);
CREATE INDEX telegram_outbox_by_chat ON telegram_outbox (chat_id, attempted_at);

-- The one poller of getUpdates (Telegram answers a second poller 409), and the next update id
-- to ask for, so a new holder carries on where the last one stopped. A lease, so a process
-- that died holding it is taken over, and a second process (a rollout's overlap) waits.
CREATE TABLE telegram_poller (
	id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
	holder BLOB,
	leased_until INTEGER,
	next_update_id INTEGER NOT NULL DEFAULT 0,
	-- Telegram asked the bot to wait (429): nothing leaves the outbox before this.
	outbox_paused_until INTEGER
) STRICT;

INSERT INTO telegram_poller DEFAULT VALUES;

-- The last unsolicited reply to a chat (help, "not linked", …): at most one per ten minutes.
CREATE TABLE telegram_replies (
	chat_id INTEGER PRIMARY KEY,
	last_at INTEGER NOT NULL
) STRICT;

-- ── the PostHog import ──────────────────────────────────────────────────────────────────

-- Who runs the hourly import, and when it last finished: a lease another process takes over
-- when its holder is gone, so the import runs once an hour whatever runs beside it.
CREATE TABLE posthog_import (
	id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
	holder BLOB,
	leased_until INTEGER,
	-- The last try, successful or not: a failing import is retried, not hammered.
	attempted_at INTEGER,
	-- The last import that finished.
	imported_at INTEGER
) STRICT;

INSERT INTO posthog_import DEFAULT VALUES;

-- ── reporting: what may be read without PII ─────────────────────────────────────────────
-- These views never select `properties`, `pii_sealed` or a source's secret: what the screens
-- read of the counts, and what a reader of a replica (a dashboard, an export) should be
-- pointed at instead of the tables. Days are UTC.

CREATE VIEW reporting_leads AS
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_at
FROM leads;

CREATE VIEW reporting_calls AS
SELECT event_id, brand_id, lead_id, location_id, kind, outcome, occurred_at, manual
FROM calls;

CREATE VIEW reporting_payments AS
SELECT event_id, brand_id, lead_id, location_id, job_id, billed, commission, currency, occurred_at, manual
FROM payments;

-- The personal funnel (stages 5–10) by the day leads came in: how many reached each stage.
CREATE VIEW reporting_funnel_daily AS
SELECT date(created_at / 1000000, 'unixepoch') AS day, brand_id, location_id,
	count(*) AS leads,
	count(contacted_at) AS contacted,
	count(quoted_at) AS quoted,
	count(won_at) AS won,
	count(completed_at) AS completed,
	count(paid_at) AS paid,
	count(*) FILTER (WHERE stage = 'lost') AS lost_now,
	count(*) FILTER (WHERE manual) AS manual
FROM leads
WHERE created_at IS NOT NULL
GROUP BY 1, 2, 3;

-- What each source sent, by day, type and status: the "Sources" screen and a silent source.
CREATE VIEW reporting_ingest_daily AS
SELECT date(received_at / 1000000, 'unixepoch') AS day, brand_id, source_kind, source_id, type, type_version, status,
	count(*) AS events
FROM events
GROUP BY 1, 2, 3, 4, 5, 6, 7;

-- The counts, no revisions or event ids (and nothing personal: the events these come from
-- carry no PII).
CREATE VIEW reporting_daily_location_metrics AS
SELECT day, brand_id, location_id, metric, dimension, value
FROM daily_location_metrics;

CREATE VIEW reporting_experiment_daily AS
SELECT day, brand_id, experiment, variant, exposures, leads, phone, whatsapp, form_open, booking
FROM daily_experiment_metrics;
