-- A brand's price list (kitstart's PricingModel, FORM-VARIANTS-SPEC, lib#178): what the landings
-- price their `estimate` and `fixed` needs by, edited from the panel instead of a release, and
-- the locales the brand's sites speak, which every label of its model must be in. Conventions
-- as in the init. New tables only: the build before this one never names them.

-- The locales a brand's sites speak, set from the CLI (`panel pricing locales`). No row: the
-- default in code, fr and en.
CREATE TABLE brand_locales (
	brand_id TEXT PRIMARY KEY CHECK (length(brand_id) BETWEEN 1 AND 64 AND brand_id GLOB '[a-z0-9]*' AND brand_id NOT GLOB '*[^a-z0-9_-]*'),
	-- A JSON array of locales, never empty.
	locales TEXT NOT NULL CHECK (json_valid(locales) AND json_type(locales) = 'array' AND json_array_length(locales) > 0),
	updated_at INTEGER NOT NULL,
	-- A user's email, or `cli`.
	updated_by TEXT NOT NULL
) STRICT;

-- The current model, as checked when it was saved; NULL once removed (the sites go back to
-- their baked model). The row stays when removed: its `updated_at` is what the next edit must
-- name. No row: never set.
CREATE TABLE pricing (
	brand_id TEXT PRIMARY KEY CHECK (length(brand_id) BETWEEN 1 AND 64 AND brand_id GLOB '[a-z0-9]*' AND brand_id NOT GLOB '*[^a-z0-9_-]*'),
	model TEXT CHECK (model IS NULL OR (json_valid(model) AND json_type(model) = 'object')),
	-- Strictly increasing per brand: the token of optimistic concurrency.
	updated_at INTEGER NOT NULL,
	updated_by TEXT NOT NULL,
	-- The concierge user id; NULL for the CLI.
	updated_by_user BLOB CHECK (length(updated_by_user) = 16)
) STRICT;

CREATE TRIGGER pricing_no_delete BEFORE DELETE ON pricing
BEGIN
	SELECT RAISE(ABORT, 'a brand''s pricing is removed by setting it NULL, not deleted: DELETE refused');
END;

-- Every change to a brand's pricing, append-only: the model before and after (NULL: none), who
-- and when. The history the editor shows.
CREATE TABLE pricing_changes (
	id BLOB PRIMARY KEY CHECK (length(id) = 16),
	brand_id TEXT NOT NULL REFERENCES pricing (brand_id),
	at INTEGER NOT NULL,
	kind TEXT NOT NULL CHECK (kind IN ('set', 'remove')),
	changed_by TEXT NOT NULL,
	changed_by_user BLOB CHECK (length(changed_by_user) = 16),
	before TEXT CHECK (before IS NULL OR (json_valid(before) AND json_type(before) = 'object')),
	after TEXT CHECK (after IS NULL OR (json_valid(after) AND json_type(after) = 'object')),
	CHECK ((kind = 'remove') = (after IS NULL)),
	CHECK (kind = 'set' OR before IS NOT NULL)
) STRICT;

CREATE INDEX pricing_changes_by_brand ON pricing_changes (brand_id, at);

CREATE TRIGGER pricing_changes_no_delete BEFORE DELETE ON pricing_changes
BEGIN
	SELECT RAISE(ABORT, 'pricing_changes is append-only: DELETE refused');
END;

CREATE TRIGGER pricing_changes_no_update BEFORE UPDATE ON pricing_changes
BEGIN
	SELECT RAISE(ABORT, 'pricing_changes is append-only: UPDATE refused');
END;
