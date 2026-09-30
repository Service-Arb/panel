# Architecture

What the panel is for, and the phases it is built in, is `SA-PANEL-SPEC.md` (§ numbers in the
code refer to it). This is where things live.

A cargo workspace of four crates; dependencies point inwards only.

```text
contracts/proto/sa/v1/events.proto   sa.funnel.v1: the Event envelope, the ingest request and
                                     answer, one properties message per type@version
crates/panel_contracts/              that proto, generated at build time: prost messages + pbjson
                                     serde (protojson); no checks beyond the JSON shape
crates/panel_core/                   no I/O: no database, network, clock or randomness
  src/ids.rs                         EventId (UUIDv7, ev_lib's Id), brand/location/lead/job ids
  src/event.rs                       SourceKind, Envelope, TypeKey (type@version), KeyGrant:
                                     which kind and brands a signing key may write
  src/fact.rs                        the registered types, typed (Fact), and what each needs of
                                     its subject
  src/lead.rs                        Stage, and fold: a lead's facts → its stage and stage times
  src/signature.rs                   the HMAC scheme of a batch and its replay window
  src/role.rs                        operator / admin and what each may do (§5.4)
crates/panel/                        the engine
  src/lib.rs                         the `Panel` facade: ingest, sources, PII, rebuild
  src/wire.rs                        protojson → the core: one event decoded and checked; the
                                     registry (type@version → properties message → Fact)
  src/seal.rs                        XChaCha20-Poly1305 at rest: PII, sources' HMAC secrets
  src/store/                         Postgres (runtime sqlx queries, embedded migrations/)
  src/store/events.rs                the journal
  src/store/projections.rs           leads, calls, payments
  src/store/sources.rs               the signing keys
  src/testing.rs                     (feature `testing`) throwaway databases, signed batches
  migrations/                        the schema, `reporting` included
crates/panel_server/                 the `panel` binary: CLI and HTTP, thin over `Panel`
  src/http.rs                        POST /api/ingest/v1/events, GET /health
  src/settings.rs                    the environment (ev_lib `settings!`)
deploy/panel_app.sql                 the runtime role's grants (applied by the tests too)
```

## Ingest → journal → projections

```text
POST /api/ingest/v1/events        at most 32 at once (else 503), 30 s in all (else 408)
  │ headers present, |now − x-sa-timestamp| ≤ 5 min, x-sa-key-id a slug     else 401, body unread
  │ body read: ≤ 4 MiB within 10 s                                          else 413 / 408
  │ x-sa-key-id → sources (not revoked) → secret, unsealed
  │ HMAC over "sa-ingest/v1.<x-sa-timestamp>.<body>"                        else 401
  │ body → {"events": [1..500]}                                             else 400
  ▼
each event on its own ─ decode (protojson) ─ envelope checks ─ key may write this kind and brand?
  │                                                                        no → rejected{reason}
  ▼
registry: type@version known?
  ├─ no  → journal, status unregistered                                   → accepted (not projected)
  ├─ yes, properties or subject wrong                                     → rejected{reason}
  └─ yes → one transaction:
             INSERT events … ON CONFLICT (id) DO NOTHING
               ├─ id taken, same content                                  → duplicate
               ├─ id taken, other content                                 → rejected
               └─ new: calls / payments row; lead recomputed from all its events → accepted
```

## Invariants

- **The journal is append-only.** `events` rows are inserted once. Triggers refuse `DELETE`,
  `TRUNCATE`, and any `UPDATE` but of `status`/`status_reason`, which the rebuild rewrites when
  the registry has changed its mind about an event. Everything else is derived from it.
- **An event is its id.** The source issues a UUIDv7; the journal is keyed by it. A resend is
  `duplicate`; the same id with other content (compared as a MAC of the event in canonical
  form, so field spelling and key order do not matter, under a key derived from
  `PANEL_DATA_KEY`, so a dump cannot confirm a guess at the PII it covers) is `rejected`. Delivery is at-least-once
  on the sources' side, exactly-once in the journal.
- **Unknown types are kept, not dropped** (§3.2): stored as `unregistered`, not projected, and
  judged again by `panel rebuild-projections` — which is how a type registered later picks up
  what arrived before it. Known types are checked strictly: unknown fields, a wrong vocabulary
  word, a lead event without a lead are rejected, and not journaled.
- **A key writes what it was registered for.** Each source key has one `kind` and a set of
  brands, and names its source: `source.id` must be the key id. An event claiming another
  source, another brand, or another kind (a site key cannot pass its events
  off as typed in by hand), is rejected. Events of kind `panel` are the manual ones (§10a), and
  every projection row carries `manual`.
- **A kind writes only its types** (`panel_core::event::may_write`): `lead.created` from `site`
  or `panel`; `lead.contacted` and the calls from `panel` or `telephony`; quotes, wins, losses,
  completed jobs and payments from `panel` alone (payments are entered by hand). Unknown types
  are open to every kind. Checked on the key at ingest and again by the registry, so the
  rebuild drops anything that slipped in.
- **A lead is created once.** Of several `lead.created`, the first the panel journaled counts
  (by `received_at`, then id) and the others are left out of the projection, so a source
  cannot back-date a creation to take over someone else's lead.
- **The timestamp is signed.** The MAC covers `sa-ingest/v1.<timestamp>.<body>`, so the 5-minute window
  cannot be dodged by re-stamping a captured request (the weakness concierge's Didit handler
  works around). An unknown key and a bad signature answer the same.
- **Projections are functions of the journal.** A lead is never patched: every event about it
  recomputes its row from all its registered events (`panel_core::lead::fold`, ordered by
  `(occurred_at, id)`, so arrival order does not matter), under a per-lead advisory lock. The
  rebuild runs the same code in one transaction, so it lands on the same state; the tests
  check that it does. Ingest and the rebuild do not interleave: every ingest transaction
  holds an advisory lock shared, the rebuild holds it exclusively from before it empties the
  projections until it commits. Calls and payments are one row per event.
- **Stages move forward** through created → contacted → quoted → won → completed → paid; a
  `lead.lost` moves a lead to `lost` from anywhere, and later progress reopens it. Stage times
  are when each stage was first reached.
- **PII never sits in the clear.** `pii` is sealed per event (XChaCha20-Poly1305, the event id
  as associated data) under `PANEL_DATA_KEY`, whose fingerprint is stored beside every blob.
  The free text a customer typed goes in `pii`, not `properties`. Sources' HMAC secrets are
  sealed the same way (they must be usable to verify, so they cannot be hashed).
- **Reporting has no PII.** The `reporting` schema holds views over the projections and a daily
  ingest count; none selects `properties`, `pii_sealed` or a secret. They run with their
  owner's rights, so a Grafana role granted `SELECT` on `reporting` alone reads nothing else.
  That role (`sa_grafana`) is the deploy's to create, not a migration's.
- **Two database roles.** `panel migrate` applies the migrations as the schema's owner
  (`MIGRATE_DATABASE_URL`); everything else runs as the runtime role (`DATABASE_URL`), holding
  only the grants of [`deploy/panel_app.sql`](../deploy/panel_app.sql) — append to the journal
  and re-judge its status, derive the projections, add and revoke sources, read `reporting` —
  and refuses to start on a database that lacks a migration of its build. No command migrates
  on its own, not even in development: run `panel migrate` there too, with both URLs the same.
- **Secrets come from the environment only** (`DATABASE_URL`, `MIGRATE_DATABASE_URL`,
  `PANEL_DATA_KEY`, `SENTRY_DSN`),
  through `ev_lib::settings`; both of the first are required at boot when `APP_ENV=production`.

## Deploy requirements

- **Ingest stays inside the cluster.** Its sources (the landings, review_archive) reach it
  by service DNS (§3.3); the IngressRoute that publishes `sa.evinvest.ltd` must not route
  `/api/ingest`, and the NetworkPolicy lets in only the pods that send. The signature is
  what authenticates a batch; keeping the route off the internet is what keeps its cost —
  a database lookup and a MAC per request — away from anyone who can reach a URL.
- **Migrate, grant, then roll out.** A release that carries a migration runs `panel migrate`
  (as a Job or an init container with `MIGRATE_DATABASE_URL`) before the new pods start, then
  `deploy/panel_app.sql` as the owner, so a new table is granted too. Both roles, `panel_app`
  included, are the deploy's to create (devops); `sa_grafana` gets `USAGE` on `reporting` and
  `SELECT` on its views, nothing else.
