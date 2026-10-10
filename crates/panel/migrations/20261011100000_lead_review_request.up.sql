-- That a Google review was asked of the customer (Service-Arb/panel#38): `review.requested` says
-- on which messenger, and `leads` keeps the first one's time and channel. A lead is asked once;
-- a later event changes nothing (the fold keeps the first).
--
-- An ADD COLUMN, as lead_locale was: both columns are new and nullable, and every existing row is
-- NULL in both, which the pair CHECK lets through. The CHECK sits on the second column because
-- SQLite takes no table constraint in an ADD COLUMN; it names the first, so the down migration
-- drops the channel before the time. `reporting_leads` is made again to show them; nothing else
-- reads `leads` by `SELECT *`. `panel rebuild-projections` fills both from the journal.

ALTER TABLE leads ADD COLUMN review_requested_at INTEGER;
ALTER TABLE leads ADD COLUMN review_requested_channel TEXT
	CHECK ((review_requested_at IS NULL) = (review_requested_channel IS NULL)
		AND (review_requested_channel IS NULL OR review_requested_channel IN ('whatsapp', 'telegram')));

DROP VIEW reporting_leads;
CREATE VIEW reporting_leads AS
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual, suspect,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_at,
	flow, quoted_cents, pricing_valid_from, estimate_inputs,
	message_ref, messaged_at, messaged_channel, locale,
	review_requested_at, review_requested_channel
FROM leads;
