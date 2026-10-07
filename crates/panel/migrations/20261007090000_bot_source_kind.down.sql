-- no-transaction
-- Back to the CHECKs without `bot`. A database holding a bot's key or event cannot go back: the
-- copy fails on the CHECK and the whole transaction is rolled back, nothing lost — the journal is
-- append-only, and the build this goes back to cannot read those events. Roll back only before
-- any bot key is issued.

PRAGMA foreign_keys = OFF;

BEGIN IMMEDIATE;

DROP VIEW reporting_ingest_daily;

CREATE TABLE sources_new (
	key_id TEXT PRIMARY KEY CHECK (length(key_id) BETWEEN 1 AND 64 AND key_id GLOB '[a-z0-9]*' AND key_id NOT GLOB '*[^a-z0-9_-]*'),
	kind TEXT NOT NULL CHECK (kind IN ('site', 'review_archive', 'gbp', 'posthog', 'panel', 'sheet', 'telephony')),
	-- A JSON array of brand ids, never empty.
	brand_ids TEXT NOT NULL CHECK (json_valid(brand_ids) AND json_type(brand_ids) = 'array' AND json_array_length(brand_ids) > 0),
	-- The HMAC secret, sealed under PANEL_DATA_KEY with the key id as associated data.
	secret_sealed BLOB NOT NULL,
	-- Which data key sealed it (a fingerprint, not the key), so a rotated key fails by name.
	data_key_fp BLOB NOT NULL CHECK (length(data_key_fp) = 32),
	created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000000 AS INTEGER)),
	revoked_at INTEGER
) STRICT;

INSERT INTO sources_new (key_id, kind, brand_ids, secret_sealed, data_key_fp, created_at, revoked_at)
SELECT key_id, kind, brand_ids, secret_sealed, data_key_fp, created_at, revoked_at
FROM sources;

DROP TABLE sources;
ALTER TABLE sources_new RENAME TO sources;

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

CREATE TABLE events_new (
	id BLOB PRIMARY KEY CHECK (length(id) = 16),
	schema TEXT NOT NULL,
	type TEXT NOT NULL,
	type_version INTEGER NOT NULL CHECK (type_version > 0),
	occurred_at INTEGER NOT NULL,
	received_at INTEGER NOT NULL,
	source_kind TEXT NOT NULL CHECK (source_kind IN ('site', 'review_archive', 'gbp', 'posthog', 'panel', 'sheet', 'telephony', 'booking')),
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
	-- HMAC-SHA256 of the event in canonical form, under a key derived from PANEL_DATA_KEY.
	content_mac BLOB NOT NULL CHECK (length(content_mac) = 32),
	status TEXT NOT NULL CHECK (status IN ('registered', 'unregistered', 'invalid')),
	status_reason TEXT,
	manual INTEGER GENERATED ALWAYS AS (source_kind = 'panel') STORED,
	CHECK ((pii_sealed IS NULL) = (data_key_fp IS NULL))
) STRICT;

INSERT INTO events_new (id, schema, type, type_version, occurred_at, received_at, source_kind, source_id, key_id, brand_id, location_id,
	lead_id, job_id, properties, pii_sealed, data_key_fp, content_mac, status, status_reason)
SELECT id, schema, type, type_version, occurred_at, received_at, source_kind, source_id, key_id, brand_id, location_id,
	lead_id, job_id, properties, pii_sealed, data_key_fp, content_mac, status, status_reason
FROM events;

DROP TABLE events;
ALTER TABLE events_new RENAME TO events;

CREATE INDEX events_by_lead ON events (brand_id, lead_id, occurred_at, id) WHERE lead_id IS NOT NULL;
CREATE INDEX events_in_order ON events (occurred_at, id);
CREATE INDEX events_not_registered ON events (type, type_version) WHERE status <> 'registered';
CREATE INDEX events_by_key ON events (key_id, received_at) WHERE key_id IS NOT NULL;
CREATE INDEX events_of_experiments ON events (brand_id, occurred_at, id)
WHERE type IN ('experiments.declared', 'experiment.configured');

CREATE TRIGGER events_no_delete BEFORE DELETE ON events
BEGIN
	SELECT RAISE(ABORT, 'events is append-only: DELETE refused');
END;

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

CREATE VIEW reporting_ingest_daily AS
SELECT date(received_at / 1000000, 'unixepoch') AS day, brand_id, source_kind, source_id, type, type_version, status,
	count(*) AS events
FROM events
GROUP BY 1, 2, 3, 4, 5, 6, 7;

COMMIT;

PRAGMA foreign_keys = ON;
