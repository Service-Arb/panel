-- no-transaction
-- The journal takes the source kind `booking`: the panel's booking adapters (a provider's
-- webhook, the calendar it pulls) write `booking.created` / `booking.canceled` as it
-- (FORM-VARIANTS-SPEC, "Booking amendments": one kind for every provider, the provider in
-- the properties).
--
-- SQLite cannot alter a CHECK, so `events` is made again with the wider one, every row copied
-- as it is, and its indexes, triggers and the one view that reads it made again unchanged.
-- Five projections reference `events (id)` by foreign key; dropping the table they point at
-- is only possible with foreign keys off, and PRAGMA foreign_keys is a no-op inside a
-- transaction — hence `-- no-transaction` above, the pragma outside, and the copy in a
-- transaction of its own (IMMEDIATE: no writer between the copy and the swap). The rows keep
-- their ids, so every reference resolves to the new table under its old name; the
-- migration's test checks the copy and `PRAGMA foreign_key_check`.
--
-- `sources.kind` keeps its CHECK: no key is issued for kind booking.

PRAGMA foreign_keys = OFF;

BEGIN IMMEDIATE;

-- A rename re-reads every view; one naming a table that is gone fails it.
DROP VIEW reporting_ingest_daily;

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
