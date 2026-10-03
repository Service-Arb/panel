-- `lead.created` says the landing's flow and the price it showed (FORM-VARIANTS-SPEC,
-- lib#178): `flow` (quote | estimate | fixed), `quoted_cents` and `pricing_valid_from` (set
-- exactly when the flow is priced), `estimate_inputs` (an estimate's choices). NULL: a lead
-- that said nothing of them — every lead before this.
--
-- Nullable columns with a CHECK, added in place as `suspect` was: every existing row gets NULL,
-- which the CHECKs let through, and the build before this one never names them. `leads` is a
-- projection: `panel rebuild-projections` fills them from the journal. Both reporting views
-- are made again to carry them; none is PII (inputs are the pricing model's slugs).

DROP VIEW reporting_funnel_daily;
DROP VIEW reporting_leads;

ALTER TABLE leads ADD COLUMN flow TEXT CHECK (flow IN ('quote', 'estimate', 'fixed'));
-- Integer cents, EUR TTC.
ALTER TABLE leads ADD COLUMN quoted_cents INTEGER CHECK (quoted_cents >= 0);
ALTER TABLE leads ADD COLUMN pricing_valid_from TEXT CHECK (date(pricing_valid_from) IS pricing_valid_from);
-- A JSON object, input id → value id.
ALTER TABLE leads ADD COLUMN estimate_inputs TEXT CHECK (json_valid(estimate_inputs) AND json_type(estimate_inputs) = 'object');

CREATE VIEW reporting_leads AS
SELECT brand_id, lead_id, location_id, job_id, stage, channel, manual, suspect,
	created_at, contacted_at, quoted_at, won_at, completed_at, paid_at, lost_at, lost_reason, last_event_at,
	flow, quoted_cents, pricing_valid_from, estimate_inputs
FROM leads;

-- The personal funnel (stages 5–10) by the day leads came in: how many reached each stage;
-- `estimate` and `fixed` the leads of the priced flows (the rest said quote or nothing).
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
	count(*) FILTER (WHERE flow = 'fixed') AS fixed
FROM leads
WHERE created_at IS NOT NULL
GROUP BY 1, 2, 3;
