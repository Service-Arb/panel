-- Back to `leads` without the locale. The column's text is lost; the build this goes back to does
-- not know `locale` (its protojson decode would refuse the field anyway), and a rebuild there
-- never refills it. The view goes first: a column a view names cannot be dropped.

DROP VIEW reporting_leads;

ALTER TABLE leads DROP COLUMN locale;

CREATE VIEW reporting_leads AS
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual, suspect,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_at,
	flow, quoted_cents, pricing_valid_from, estimate_inputs,
	message_ref, messaged_at, messaged_channel
FROM leads;
