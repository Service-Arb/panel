-- A brand's experiments as configuration (the statistics are PostHog's): what its landing
-- declares at start (`experiments.declared`) and what an admin lays over it
-- (`experiment.configured`), folded per brand by `panel_core::experiment::fold` from every such
-- event of the brand, recomputed in the transaction that journals one. A projection: `panel
-- rebuild-projections` makes it again from the journal.

-- The fold reads every experiment event of a brand.
CREATE INDEX events_of_experiments ON events (brand_id, occurred_at, id)
WHERE type IN ('experiments.declared', 'experiment.configured');

CREATE TABLE experiments (
	brand_id TEXT NOT NULL,
	key TEXT NOT NULL CHECK (length(key) BETWEEN 1 AND 64 AND key NOT GLOB '*[^a-z0-9_]*'),
	-- As the latest declaration naming it said; JSON arrays, the control first.
	variants TEXT NOT NULL CHECK (json_valid(variants) AND json_type(variants) = 'array'),
	declared_weights TEXT NOT NULL CHECK (json_valid(declared_weights) AND json_type(declared_weights) = 'array'),
	declared_enabled INTEGER NOT NULL CHECK (declared_enabled IN (0, 1)),
	declared_holdout REAL CHECK (declared_holdout >= 0 AND declared_holdout < 1),
	summary TEXT,
	declared_at INTEGER NOT NULL,
	first_declared_at INTEGER NOT NULL,
	-- What an admin set; NULL follows the declaration. Kept as set: the landings (and the
	-- screens' `effective`) ignore a field that no longer fits the declaration.
	override_enabled INTEGER CHECK (override_enabled IN (0, 1)),
	override_weights TEXT CHECK (json_valid(override_weights) AND json_type(override_weights) = 'array'),
	override_holdout REAL CHECK (override_holdout >= 0 AND override_holdout < 1),
	-- The admin's concierge id and when, while anything is set.
	changed_by TEXT,
	changed_at INTEGER,
	weights_changed_at INTEGER,
	-- Not in the brand's latest declaration.
	retired INTEGER NOT NULL CHECK (retired IN (0, 1)),
	CHECK ((changed_by IS NULL) = (changed_at IS NULL)),
	PRIMARY KEY (brand_id, key)
) STRICT;
