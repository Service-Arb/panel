-- Back to `leads` without `suspect`, and the views as they were. The marks go from the
-- projection only; the journal keeps them in each `lead.created`'s properties. The build this
-- goes back to does not know the field, so it judges those events invalid at its next
-- rebuild, and refuses new ones carrying it: stop the landings sending it first.

DROP VIEW reporting_funnel_daily;
DROP VIEW reporting_leads;

ALTER TABLE leads DROP COLUMN suspect;

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
