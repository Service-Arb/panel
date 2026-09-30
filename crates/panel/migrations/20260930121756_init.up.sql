-- The journal, the sources that write to it, the projections built from it, and the
-- `reporting` schema the panel's Grafana will read (spec §3, §6).

CREATE TABLE sources (
	key_id text PRIMARY KEY CHECK (key_id ~ '^[a-z0-9][a-z0-9_-]{0,63}$'),
	kind text NOT NULL CHECK (kind IN ('site', 'review_archive', 'gbp', 'posthog', 'panel', 'sheet', 'telephony')),
	brand_ids text[] NOT NULL CHECK (cardinality(brand_ids) > 0),
	-- The HMAC secret has to be usable to verify, so it is sealed, not hashed: XChaCha20-Poly1305
	-- under PANEL_DATA_KEY, the key id bound as associated data.
	secret_sealed bytea NOT NULL,
	-- Which data key sealed it (a fingerprint, not the key), so a rotated key fails by name.
	data_key_fp bytea NOT NULL CHECK (octet_length(data_key_fp) = 32),
	created_at timestamptz NOT NULL DEFAULT now(),
	revoked_at timestamptz
);

-- Append-only: rows are inserted once and never deleted; only `status` and
-- `status_reason` ever change, when `panel rebuild-projections` re-reads an event against
-- the registry as it is now. Enforced by the triggers below.
CREATE TABLE events (
	id uuid PRIMARY KEY,
	schema text NOT NULL,
	type text NOT NULL,
	type_version integer NOT NULL CHECK (type_version > 0),
	occurred_at timestamptz NOT NULL,
	received_at timestamptz NOT NULL,
	source_kind text NOT NULL CHECK (source_kind IN ('site', 'review_archive', 'gbp', 'posthog', 'panel', 'sheet', 'telephony')),
	source_id text NOT NULL,
	-- The key that signed it; NULL for events the panel writes itself.
	key_id text REFERENCES sources (key_id),
	brand_id text NOT NULL,
	location_id text,
	lead_id text,
	job_id text,
	properties jsonb NOT NULL,
	pii_sealed bytea,
	data_key_fp bytea CHECK (octet_length(data_key_fp) = 32),
	-- SHA-256 of the event as received; a resend with the same id and other content is refused.
	content_sha256 bytea NOT NULL CHECK (octet_length(content_sha256) = 32),
	-- registered: projected. unregistered: a type@version the panel does not know (yet).
	-- invalid: a type registered after the event arrived, whose checks it fails.
	status text NOT NULL CHECK (status IN ('registered', 'unregistered', 'invalid')),
	status_reason text,
	manual boolean GENERATED ALWAYS AS (source_kind = 'panel') STORED,
	CHECK ((pii_sealed IS NULL) = (data_key_fp IS NULL))
);

CREATE INDEX events_by_lead ON events (brand_id, lead_id, occurred_at, id) WHERE lead_id IS NOT NULL;
CREATE INDEX events_in_order ON events (occurred_at, id);
CREATE INDEX events_not_registered ON events (type, type_version) WHERE status <> 'registered';

CREATE FUNCTION events_append_only() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
	-- `manual` is left out of the comparison because a stored generated column is not
	-- computed yet in a BEFORE trigger (NEW.manual is NULL here); it follows source_kind,
	-- which is compared.
	IF TG_OP = 'UPDATE'
		AND (to_jsonb(NEW) - 'status' - 'status_reason' - 'manual') IS NOT DISTINCT FROM (to_jsonb(OLD) - 'status' - 'status_reason' - 'manual') THEN
		RETURN NEW;
	END IF;
	RAISE EXCEPTION 'events is append-only: % refused', TG_OP;
END
$$;

CREATE TRIGGER events_append_only_rows BEFORE UPDATE OR DELETE ON events
	FOR EACH ROW EXECUTE FUNCTION events_append_only();
CREATE TRIGGER events_append_only_truncate BEFORE TRUNCATE ON events
	FOR EACH STATEMENT EXECUTE FUNCTION events_append_only();

-- ── projections: derived from `events` only, rebuilt by `panel rebuild-projections` ──────

CREATE TABLE leads (
	brand_id text NOT NULL,
	lead_id text NOT NULL,
	location_id text,
	job_id text,
	stage text NOT NULL CHECK (stage IN ('created', 'contacted', 'quoted', 'won', 'completed', 'paid', 'lost')),
	channel text CHECK (channel IN ('form', 'phone_inbound')),
	manual boolean NOT NULL,
	-- When the lead first reached each stage.
	created_at timestamptz,
	contacted_at timestamptz,
	quoted_at timestamptz,
	won_at timestamptz,
	completed_at timestamptz,
	paid_at timestamptz,
	lost_at timestamptz,
	lost_reason text,
	last_event_id uuid NOT NULL REFERENCES events (id),
	last_event_at timestamptz NOT NULL,
	PRIMARY KEY (brand_id, lead_id)
);

CREATE TABLE calls (
	event_id uuid PRIMARY KEY REFERENCES events (id),
	brand_id text NOT NULL,
	lead_id text NOT NULL,
	location_id text,
	kind text NOT NULL CHECK (kind IN ('attempted', 'logged')),
	outcome text CHECK (outcome IN ('answered', 'no_answer', 'wrong_number', 'later')),
	attempt_id text,
	occurred_at timestamptz NOT NULL,
	manual boolean NOT NULL,
	CHECK ((kind = 'logged') = (outcome IS NOT NULL))
);

CREATE INDEX calls_by_lead ON calls (brand_id, lead_id, occurred_at);

CREATE TABLE payments (
	event_id uuid PRIMARY KEY REFERENCES events (id),
	brand_id text NOT NULL,
	lead_id text NOT NULL,
	location_id text,
	job_id text,
	-- Minor units of `currency`.
	billed bigint NOT NULL CHECK (billed >= 0),
	commission bigint NOT NULL CHECK (commission BETWEEN 0 AND billed),
	currency text NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
	occurred_at timestamptz NOT NULL,
	manual boolean NOT NULL
);

CREATE INDEX payments_by_lead ON payments (brand_id, lead_id, occurred_at);

-- ── reporting: what Grafana may read ────────────────────────────────────────────────────
-- No PII and no raw journal: these views never select `properties`, `pii_sealed` or a
-- source's secret. They run with their owner's rights, so a read-only role granted SELECT
-- on this schema alone (sa_grafana, created by the deploy, not here) reads them without
-- any right on the tables underneath. Days are UTC.

CREATE SCHEMA reporting;

CREATE VIEW reporting.leads AS
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_at
FROM public.leads;

CREATE VIEW reporting.calls AS
SELECT event_id, brand_id, lead_id, location_id, kind, outcome, occurred_at, manual
FROM public.calls;

CREATE VIEW reporting.payments AS
SELECT event_id, brand_id, lead_id, location_id, job_id, billed, commission, currency, occurred_at, manual
FROM public.payments;

-- The personal funnel (stages 5–10) by the day leads came in: how many reached each stage.
CREATE VIEW reporting.funnel_daily AS
SELECT (created_at AT TIME ZONE 'UTC')::date AS day, brand_id, location_id,
	count(*) AS leads,
	count(contacted_at) AS contacted,
	count(quoted_at) AS quoted,
	count(won_at) AS won,
	count(completed_at) AS completed,
	count(paid_at) AS paid,
	count(*) FILTER (WHERE stage = 'lost') AS lost_now,
	count(*) FILTER (WHERE manual) AS manual
FROM public.leads
WHERE created_at IS NOT NULL
GROUP BY 1, 2, 3;

-- What each source sent, by day, type and status: the "Sources" screen and a silent source.
CREATE VIEW reporting.ingest_daily AS
SELECT (received_at AT TIME ZONE 'UTC')::date AS day, brand_id, source_kind, source_id, type, type_version, status,
	count(*) AS events
FROM public.events
GROUP BY 1, 2, 3, 4, 5, 6, 7;
