-- `lead.created` gains `suspect`: the landing's antispam doubted the submission
-- (`rate_limited`, `too_fast`) but kept it, since it may be a person. NULL is an ordinary lead.
--
-- A nullable column with a CHECK is added in place: every existing row gets NULL, which the
-- CHECK lets through, and the build before this one never names the column. Both reporting
-- views are made again to carry it: `reporting_leads` shows the mark, and
-- `reporting_funnel_daily` counts the marked leads apart (`suspect`) while still counting them
-- in `leads` — a report that wants them out subtracts. `leads` is a projection: what this
-- leaves NULL for an older suspect lead (none exists: no landing sent the field before this)
-- `panel rebuild-projections` would fill from the journal.

DROP VIEW reporting_funnel_daily;
DROP VIEW reporting_leads;

ALTER TABLE leads ADD COLUMN suspect TEXT CHECK (suspect IN ('rate_limited', 'too_fast'));

CREATE VIEW reporting_leads AS
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual, suspect,
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
	count(*) FILTER (WHERE manual) AS manual,
	count(suspect) AS suspect
FROM leads
WHERE created_at IS NOT NULL
GROUP BY 1, 2, 3;
