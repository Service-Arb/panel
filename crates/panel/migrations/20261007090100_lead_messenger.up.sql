-- Messenger leads (MESSENGER-CHANNELS-SPEC §3): `lead.created` gains the channels `whatsapp` and
-- `telegram` and the customer's `message_ref`; `lead.messaged` says when they actually wrote.
--
-- SQLite cannot alter a CHECK, so `leads` is made again with the wider channel one and the new
-- columns, as 20261003120000_lead_channel_callback did: a new table, the rows copied, the old one
-- dropped and the new one renamed into its place, its index made again. The two views that read
-- it are dropped first (a rename re-checks every view naming the table) and made again with the
-- new columns — none is PII: the ref is a code the landing made. Nothing references `leads`,
-- and it is a projection: the new columns, NULL for every row copied, `panel rebuild-projections`
-- fills from the journal.

DROP VIEW reporting_funnel_daily;
DROP VIEW reporting_leads;

CREATE TABLE leads_new (
	brand_id TEXT NOT NULL,
	lead_id TEXT NOT NULL,
	location_id TEXT,
	job_id TEXT,
	stage TEXT NOT NULL CHECK (stage IN ('created', 'contacted', 'quoted', 'won', 'completed', 'paid', 'lost')),
	channel TEXT CHECK (channel IN ('form', 'phone_inbound', 'callback', 'whatsapp', 'telegram')),
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
	suspect TEXT CHECK (suspect IN ('rate_limited', 'too_fast')),
	flow TEXT CHECK (flow IN ('quote', 'estimate', 'fixed')),
	-- Integer cents, EUR TTC.
	quoted_cents INTEGER CHECK (quoted_cents >= 0),
	pricing_valid_from TEXT CHECK (date(pricing_valid_from) IS pricing_valid_from),
	-- A JSON object, input id → value id.
	estimate_inputs TEXT CHECK (json_valid(estimate_inputs) AND json_type(estimate_inputs) = 'object'),
	booking_status TEXT CHECK (booking_status IN ('requested', 'booked', 'canceled', 'done', 'no_show')),
	booking_provider TEXT CHECK (booking_provider IN ('manual', 'link', 'google_calendar', 'cal_com')),
	booking_start_at INTEGER,
	booking_end_at INTEGER,
	booking_external_ref TEXT,
	booking_match TEXT CHECK (booking_match IN ('ref', 'contact', 'manual')),
	booking_preferred_date TEXT CHECK (date(booking_preferred_date) IS booking_preferred_date),
	booking_preferred_part TEXT CHECK (booking_preferred_part IN ('morning', 'afternoon', 'evening')),
	-- The ref the customer carries into a messenger (`AQ-7K3F`), from its lead.created. Not unique:
	-- a ref names the brand's newest lead carrying it.
	message_ref TEXT CHECK (message_ref GLOB '[A-Z][A-Z]*-[0-9A-Z][0-9A-Z][0-9A-Z][0-9A-Z]*'),
	-- When the customer first wrote on a messenger (lead.messaged), and on which.
	messaged_at INTEGER,
	messaged_channel TEXT CHECK (messaged_channel IN ('whatsapp', 'telegram')),
	CHECK ((messaged_at IS NULL) = (messaged_channel IS NULL)),
	PRIMARY KEY (brand_id, lead_id)
) STRICT;

INSERT INTO leads_new (brand_id, lead_id, location_id, job_id, stage, channel, manual, created_at, contacted_at, quoted_at, won_at, completed_at,
	paid_at, lost_at, lost_reason, last_event_id, last_event_at, suspect, flow, quoted_cents, pricing_valid_from, estimate_inputs,
	booking_status, booking_provider, booking_start_at, booking_end_at, booking_external_ref, booking_match, booking_preferred_date,
	booking_preferred_part)
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual, created_at, contacted_at, quoted_at, won_at, completed_at,
	paid_at, lost_at, lost_reason, last_event_id, last_event_at, suspect, flow, quoted_cents, pricing_valid_from, estimate_inputs,
	booking_status, booking_provider, booking_start_at, booking_end_at, booking_external_ref, booking_match, booking_preferred_date,
	booking_preferred_part
FROM leads;

DROP TABLE leads;
ALTER TABLE leads_new RENAME TO leads;

CREATE INDEX leads_by_booking ON leads (booking_status) WHERE booking_status IS NOT NULL;
-- `lead.messaged` and the bots' lookup find a lead by its ref.
CREATE INDEX leads_by_message_ref ON leads (brand_id, message_ref) WHERE message_ref IS NOT NULL;

CREATE VIEW reporting_leads AS
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual, suspect,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_at,
	flow, quoted_cents, pricing_valid_from, estimate_inputs,
	message_ref, messaged_at, messaged_channel
FROM leads;

-- The personal funnel (stages 5–10) by the day leads came in: how many reached each stage;
-- `estimate` and `fixed` the leads of the priced flows (the rest said quote or nothing); `messaged`
-- those whose customer wrote on a messenger.
CREATE VIEW reporting_funnel_daily AS
SELECT date(created_at / 1000000, 'unixepoch') AS day, brand_id, location_id,
	count(*) AS leads,
	count(contacted_at) AS contacted,
	count(quoted_at) AS quoted,
	count(won_at) AS won,
	count(completed_at) AS completed,
	count(paid_at) AS paid,
	count(*) FILTER (WHERE stage = 'lost') AS lost_now,
	count(*) FILTER (WHERE manual) AS manual,
	count(suspect) AS suspect,
	count(*) FILTER (WHERE flow = 'estimate') AS estimate,
	count(*) FILTER (WHERE flow = 'fixed') AS fixed,
	count(messaged_at) AS messaged
FROM leads
WHERE created_at IS NOT NULL
GROUP BY 1, 2, 3;
