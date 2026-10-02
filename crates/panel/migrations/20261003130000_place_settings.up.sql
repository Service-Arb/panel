-- A place's live settings (phones, WhatsApp, hours, service area, …): what the landings lay
-- over their baked config, edited from the panel instead of a release. Conventions as in the
-- init. A place is a brand's location, keyed `(brand_id, location_id)` like everything else.

-- The places the panel was told about by hand, or that an edit made known; and whether an
-- admin took one off the sites. A place is never deleted: its history names it.
CREATE TABLE places (
	brand_id TEXT NOT NULL CHECK (length(brand_id) BETWEEN 1 AND 64 AND brand_id GLOB '[a-z0-9]*' AND brand_id NOT GLOB '*[^a-z0-9_-]*'),
	location_id TEXT NOT NULL CHECK (length(location_id) BETWEEN 1 AND 128),
	registered_at INTEGER NOT NULL,
	-- Who made it known: a user's email, or `cli`.
	registered_by TEXT NOT NULL,
	-- 1: the sites answer it as gone (a JSON 404), whatever its settings.
	withdrawn INTEGER NOT NULL DEFAULT 0 CHECK (withdrawn IN (0, 1)),
	PRIMARY KEY (brand_id, location_id)
) STRICT;

CREATE TRIGGER places_no_delete BEFORE DELETE ON places
BEGIN
	SELECT RAISE(ABORT, 'places are never deleted, only withdrawn: DELETE refused');
END;

CREATE TRIGGER places_only_withdrawn BEFORE UPDATE ON places
WHEN NEW.brand_id IS NOT OLD.brand_id OR NEW.location_id IS NOT OLD.location_id
	OR NEW.registered_at IS NOT OLD.registered_at OR NEW.registered_by IS NOT OLD.registered_by
BEGIN
	SELECT RAISE(ABORT, 'a place only ever gets withdrawn or restored: UPDATE refused');
END;

-- The current settings, kitstart's PlaceLive with only the fields set; `{}` once all are
-- cleared (the row stays: its `updated_at` is what the next edit must name). No row: never
-- set. Either way the site serves its baked config for what is missing.
CREATE TABLE place_settings (
	brand_id TEXT NOT NULL,
	location_id TEXT NOT NULL,
	settings TEXT NOT NULL CHECK (json_valid(settings) AND json_type(settings) = 'object'),
	-- Strictly increasing per place: the token of optimistic concurrency.
	updated_at INTEGER NOT NULL,
	updated_by TEXT NOT NULL,
	-- The concierge user id; NULL for the CLI.
	updated_by_user BLOB CHECK (length(updated_by_user) = 16),
	PRIMARY KEY (brand_id, location_id),
	FOREIGN KEY (brand_id, location_id) REFERENCES places (brand_id, location_id)
) STRICT;

CREATE TRIGGER place_settings_no_delete BEFORE DELETE ON place_settings
BEGIN
	SELECT RAISE(ABORT, 'place settings are cleared to {}, not deleted: DELETE refused');
END;

-- Every change to a place, append-only: what its settings were before and after, who and
-- when. The history the editor shows and the undo reads.
CREATE TABLE place_changes (
	id BLOB PRIMARY KEY CHECK (length(id) = 16),
	brand_id TEXT NOT NULL,
	location_id TEXT NOT NULL,
	at INTEGER NOT NULL,
	kind TEXT NOT NULL CHECK (kind IN ('register', 'set', 'revert', 'withdraw', 'restore')),
	changed_by TEXT NOT NULL,
	changed_by_user BLOB CHECK (length(changed_by_user) = 16),
	before TEXT NOT NULL CHECK (json_valid(before) AND json_type(before) = 'object'),
	after TEXT NOT NULL CHECK (json_valid(after) AND json_type(after) = 'object'),
	-- The change a revert went back before.
	reverts BLOB REFERENCES place_changes (id),
	CHECK ((kind = 'revert') = (reverts IS NOT NULL)),
	FOREIGN KEY (brand_id, location_id) REFERENCES places (brand_id, location_id)
) STRICT;

CREATE INDEX place_changes_by_place ON place_changes (brand_id, location_id, at);

CREATE TRIGGER place_changes_no_delete BEFORE DELETE ON place_changes
BEGIN
	SELECT RAISE(ABORT, 'place_changes is append-only: DELETE refused');
END;

CREATE TRIGGER place_changes_no_update BEFORE UPDATE ON place_changes
BEGIN
	SELECT RAISE(ABORT, 'place_changes is append-only: UPDATE refused');
END;
