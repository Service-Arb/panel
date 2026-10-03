-- What the panel owes PostHog: one row per journaled lead event (`panel_core::analytics`),
-- written in the transaction that journals it — never by `panel rebuild-projections` — and
-- sent in batches by `serve` (`panel::capture`), deleted once PostHog took it. The event's id
-- is PostHog's `uuid`, so a row sent twice is one event there. No PII: brand, location, the
-- closed vocabularies and amounts; `distinct_id` is the landing's random analytics id or
-- `sa-lead:<brand>:<lead>`.

CREATE TABLE posthog_outbox (
	event_id BLOB PRIMARY KEY REFERENCES events (id),
	event TEXT NOT NULL CHECK (event GLOB 'sa_[a-z]*'),
	distinct_id TEXT NOT NULL CHECK (length(distinct_id) BETWEEN 1 AND 200),
	properties TEXT NOT NULL CHECK (json_valid(properties) AND json_type(properties) = 'object'),
	occurred_at INTEGER NOT NULL,
	queued_at INTEGER NOT NULL,
	tries INTEGER NOT NULL DEFAULT 0 CHECK (tries >= 0),
	next_attempt_at INTEGER NOT NULL,
	-- Why the last try failed, PostHog's words cut short.
	last_error TEXT
) STRICT;

CREATE INDEX posthog_outbox_due ON posthog_outbox (next_attempt_at, event_id);
