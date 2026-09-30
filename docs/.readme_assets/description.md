The Service-Arb panel (`sa.evinvest.ltd`): where the funnel of the Service-Arb brands — from a
lead to a payment — is recorded and read. The plan is `SA-PANEL-SPEC.md` (§ references in the
code point there); this repository holds its backend.

What exists so far is ingest. Sources (the landings, and later review_archive, GBP and
PostHog imports, the panel's own screens) send `sa.funnel.v1` events — a versioned protobuf
contract, spoken as protojson — to `POST /api/ingest/v1/events`, signed with a per-source HMAC
key that may write only for its own brands. Every accepted event goes into an append-only
journal in Postgres, PII sealed apart; the funnel's projections (`leads` with their stages,
`calls`, `payments`) are derived from it and can be rebuilt from it at any time. A `reporting`
schema exposes them without PII, for the panel's Grafana.

People sign in through concierge (the panel is its relying party, client `sa`): the scope
`allocation:service_arb` lets them in, as an operator or an admin, and `/api/v1` is the
operator API the panel's front end works through — leads and their stages, semi-manual
calls, payments typed in by hand, the funnel, and (admins) the sources.

A Telegram bot notifies each user in a private chat they link from their profile — a new
lead, with buttons that record "taken" and "no answer" as the operator API would; a lead past
its contact SLA; for admins, payments and a source gone silent — through an outbox in
Postgres, paced to Telegram's limits.

Not here yet: the UI, the GBP and PostHog imports. See
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for where things live.
