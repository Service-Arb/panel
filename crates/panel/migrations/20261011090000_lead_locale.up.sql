-- The landing's locale on the lead (Service-Arb/panel#38): `lead.created` may carry `locale`, `fr`
-- or `en`, the language any text to the customer is in. NULL is French, a lead from before it or
-- from a landing that did not say.
--
-- An ADD COLUMN, unlike lead_messenger: a new nullable column with its own CHECK needs no table
-- made again (every existing row is NULL, which a CHECK lets through). `reporting_leads` is made
-- again to show it; nothing else reads `leads` by `SELECT *`. `panel rebuild-projections` fills the
-- column from the journal.

ALTER TABLE leads ADD COLUMN locale TEXT CHECK (locale IN ('fr', 'en'));

DROP VIEW reporting_leads;
CREATE VIEW reporting_leads AS
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual, suspect,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_at,
	flow, quoted_cents, pricing_valid_from, estimate_inputs,
	message_ref, messaged_at, messaged_channel, locale
FROM leads;
