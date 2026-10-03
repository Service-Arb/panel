-- The import's tables as the init made them, empty, and its lease never taken. The counts come
-- back from the journal with `panel rebuild-projections` of the build that reads them.

CREATE TABLE posthog_import (
	id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
	holder BLOB,
	leased_until INTEGER,
	attempted_at INTEGER,
	imported_at INTEGER
) STRICT;

INSERT INTO posthog_import DEFAULT VALUES;

CREATE TABLE daily_location_metrics (
	day TEXT NOT NULL CHECK (date(day) IS day),
	brand_id TEXT NOT NULL,
	location_id TEXT,
	metric TEXT NOT NULL CHECK (metric IN ('visits', 'contact_intent')),
	dimension TEXT NOT NULL,
	value INTEGER NOT NULL CHECK (value >= 0),
	revision INTEGER NOT NULL CHECK (revision > 0),
	event_id BLOB NOT NULL REFERENCES events (id),
	location_key TEXT GENERATED ALWAYS AS (coalesce(location_id, '')) VIRTUAL,
	UNIQUE (day, brand_id, location_key, metric, dimension)
) STRICT;

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

CREATE VIEW reporting_daily_location_metrics AS
SELECT day, brand_id, location_id, metric, dimension, value
FROM daily_location_metrics;

CREATE VIEW reporting_experiment_daily AS
SELECT day, brand_id, experiment, variant, exposures, leads, phone, whatsapp, form_open, booking
FROM daily_experiment_metrics;
