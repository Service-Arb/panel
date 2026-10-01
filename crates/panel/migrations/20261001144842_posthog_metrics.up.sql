-- The aggregate stages (spec §2, 3–4) and the experiments' daily counts, projected from the
-- `site.metrics`, `contact.metrics` and `experiment.metrics` events the PostHog import
-- journals; and the import's lease. New tables and views only: the build before this one
-- runs on it unchanged.
SET LOCAL lock_timeout = '3s';

-- One count per UTC day, brand, location and slice: page views by traffic source, intents
-- by channel. A later count of the same slice is a new event with the next revision; the
-- row keeps the highest. `location_id` NULL: pages naming no location.
CREATE TABLE daily_location_metrics (
	day date NOT NULL,
	brand_id text NOT NULL,
	location_id text,
	metric text NOT NULL CHECK (metric IN ('visits', 'contact_intent')),
	-- visits: the source (utm_source, the referrer's host, direct, …); contact_intent: the channel.
	dimension text NOT NULL,
	value bigint NOT NULL CHECK (value >= 0),
	revision integer NOT NULL CHECK (revision > 0),
	event_id uuid NOT NULL REFERENCES events (id),
	CONSTRAINT daily_location_metrics_slice UNIQUE NULLS NOT DISTINCT (day, brand_id, location_id, metric, dimension)
);

-- One count per UTC day, brand, experiment and variant, forced (QA) visits left out.
CREATE TABLE daily_experiment_metrics (
	day date NOT NULL,
	brand_id text NOT NULL,
	experiment text NOT NULL,
	variant text NOT NULL,
	exposures bigint NOT NULL CHECK (exposures >= 0),
	leads bigint NOT NULL CHECK (leads >= 0),
	phone bigint NOT NULL CHECK (phone >= 0),
	whatsapp bigint NOT NULL CHECK (whatsapp >= 0),
	form_open bigint NOT NULL CHECK (form_open >= 0),
	booking bigint NOT NULL CHECK (booking >= 0),
	revision integer NOT NULL CHECK (revision > 0),
	event_id uuid NOT NULL REFERENCES events (id),
	PRIMARY KEY (day, brand_id, experiment, variant)
);

-- The one replica that runs the hourly import, and when it last finished: a lease another
-- takes over when its holder is gone, so the import runs once an hour whatever the replicas.
CREATE TABLE posthog_import (
	id boolean PRIMARY KEY DEFAULT true CHECK (id),
	holder uuid,
	leased_until timestamptz,
	-- The last try, successful or not: a failing import is retried, not hammered.
	attempted_at timestamptz,
	-- The last import that finished.
	imported_at timestamptz
);

INSERT INTO posthog_import DEFAULT VALUES;

-- What Grafana may read: the counts, no revisions or event ids (and nothing personal: the
-- events these come from carry no PII).
CREATE VIEW reporting.daily_location_metrics AS
SELECT day, brand_id, location_id, metric, dimension, value
FROM public.daily_location_metrics;

CREATE VIEW reporting.experiment_daily AS
SELECT day, brand_id, experiment, variant, exposures, leads, phone, whatsapp, form_open, booking
FROM public.daily_experiment_metrics;
