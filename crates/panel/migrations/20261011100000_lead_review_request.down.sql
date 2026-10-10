-- Back to `leads` without the review request. Its time and channel are lost; the build this goes
-- back to does not know `review.requested` and would judge it unregistered at its next rebuild
-- anyway. The view goes first (a column a view names cannot be dropped), and the channel before
-- the time (its CHECK names the time, and a column a constraint names cannot be dropped).

DROP VIEW reporting_leads;

ALTER TABLE leads DROP COLUMN review_requested_channel;
ALTER TABLE leads DROP COLUMN review_requested_at;

CREATE VIEW reporting_leads AS
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual, suspect,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_at,
	flow, quoted_cents, pricing_valid_from, estimate_inputs,
	message_ref, messaged_at, messaged_channel, locale
FROM leads;
