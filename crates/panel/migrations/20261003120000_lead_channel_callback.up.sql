-- `lead.created` gains the channel `callback`: the landing's "call me back" request.
--
-- SQLite cannot alter a CHECK, so `leads` is rebuilt with the wider one: a new table, the
-- rows copied, the old one dropped and the new one renamed into its place. The two views
-- that read `leads` are dropped first (a rename re-checks every view naming the table) and
-- made again, unchanged. Nothing references `leads`, and it is a projection besides: what is
-- copied here `panel rebuild-projections` could make again from the journal.

DROP VIEW reporting_funnel_daily;
DROP VIEW reporting_leads;

CREATE TABLE leads_new (
	brand_id TEXT NOT NULL,
	lead_id TEXT NOT NULL,
	location_id TEXT,
	job_id TEXT,
	stage TEXT NOT NULL CHECK (stage IN ('created', 'contacted', 'quoted', 'won', 'completed', 'paid', 'lost')),
	channel TEXT CHECK (channel IN ('form', 'phone_inbound', 'callback')),
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
	PRIMARY KEY (brand_id, lead_id)
) STRICT;

INSERT INTO leads_new (brand_id, lead_id, location_id, job_id, stage, channel, manual,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_id, last_event_at)
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_id, last_event_at
FROM leads;

DROP TABLE leads;
ALTER TABLE leads_new RENAME TO leads;

CREATE VIEW reporting_leads AS
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_at
FROM leads;

-- The personal funnel (stages 5–10) by the day leads came in: how many reached each stage.
CREATE VIEW reporting_funnel_daily AS
SELECT date(created_at / 1000000, 'unixepoch') AS day, brand_id, location_id,
	count(*) AS leads,
	count(contacted_at) AS contacted,
	count(quoted_at) AS quoted,
	count(won_at) AS won,
	count(completed_at) AS completed,
	count(paid_at) AS paid,
	count(*) FILTER (WHERE stage = 'lost') AS lost_now,
	count(*) FILTER (WHERE manual) AS manual
FROM leads
WHERE created_at IS NOT NULL
GROUP BY 1, 2, 3;
