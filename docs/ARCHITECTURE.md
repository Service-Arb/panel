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
  src/notify.rs                      Telegram: the rules and who gets each, the texts, the
                                     buttons' signed data, retry and pacing constants
crates/panel/                        the engine
  src/lib.rs                         the `Panel` facade: ingest, sources, PII, rebuild
  src/wire.rs                        protojson → the core: one event decoded and checked; the
                                     registry (type@version → properties message → Fact)
  src/seal.rs                        XChaCha20-Poly1305 at rest: PII, sources' HMAC secrets
  src/store/                         Postgres (runtime sqlx queries, embedded migrations/)
  src/store/events.rs                the journal
  src/store/projections.rs           leads, calls, payments
  src/store/sources.rs               the signing keys
  src/store/telegram.rs              links, rules, fan-out marks, the outbox and its pacing
  src/telegram.rs                    linking, the rules' fan-out, delivery, the buttons; the
                                     Bot and Directory ports
  src/testing.rs                     (feature `testing`) throwaway databases, signed batches
  migrations/                        the schema, `reporting` included
crates/panel_server/                 the `panel` binary: CLI and HTTP, thin over `Panel`
  src/http.rs                        POST /api/ingest/v1/events, GET /health; the sign-in and
                                     /api/v1 mounted on top when signing in is configured
  src/signin.rs                      /auth/login, /auth/callback, /auth/logout; the /api/v1 gate
  src/concierge.rs                   concierge over gRPC: ExchangeCode, RefreshClientToken, GetMe
  src/cookies.rs                     __Host- cookies, the double-submit CSRF check
  src/api.rs                         the operator API: JSON over the engine's `operator` module
  src/telegram.rs                    the Bot API (reqwest), the bot's background work in
                                     `serve`, and /api/v1/telegram
  src/settings.rs                    the environment (ev_lib `settings!`)
contracts/proto/concierge/v1/        concierge's auth and directory protos, vendored at the
                                     commit in REV (`sync.sh` refreshes them)
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

## Signing in (§4)

The panel is a relying party of concierge, client id `sa`, with its own origin and its own
cookies; the browser never holds a concierge token.

```text
GET /auth/login      state + PKCE verifier, sealed into sa_prelogin (10 min)
                     → 302 <concierge>/api/auth/authorize?client_id=sa&redirect_uri=…&state&code_challenge
GET /auth/callback   state = the cookie's (constant time), and not redeemed before
                     (consumed_states), else 400 and the code is never presented;
                     ExchangeCode(code, redirect_uri, verifier, client secret); the browser's
                     previous session, if any, closed → a session row (tokens sealed under
                     PANEL_DATA_KEY, keyed by the hash of a random id) → sa_session (HttpOnly)
                     + sa_csrf → 303 /
/api/v1/*            not GET: x-sa-csrf must equal sa_csrf; the session (access token rotated
                     when within 30 s of expiry, see below); GetMe (cached ≤ 60 s per session,
                     asked afresh for POST/DELETE /sources, one retry when concierge does not
                     answer) → the role, else 401 (cookies cleared) / 403 / 503
POST /auth/logout    CSRF; every session of the user is closed
```

- **The role** is `panel_core::role::Role::admitted`: a grant on `allocation:service_arb`
  gives its role, a global admin/owner is an admin; the higher wins. Nothing else gets in —
  concierge refuses them a code already, and the panel asks again on every request.
- **Revocation.** concierge refusing a refresh or `GetMe` closes the session here; concierge
  unreachable is a 503 and keeps it. A scope revoked at concierge is seen within the 60 s of
  the cache (at once by the source-key mutations). Signing out of evinvest.ltd revokes the
  concierge token family: the panel's session ends at its next rotation, so within the
  access token's lifetime.
- **Rotation, a refresh token presented once.** No pool connection is held while concierge
  is asked. In one process, a single flight per session: the others wait and find the fresh
  row. Across replicas, a lease on the row (`sessions.rotating_until`, taken by one
  conditional UPDATE, 15 s — longer than a call to concierge): only its holder asks, the
  others poll the row for up to 6 s, then answer 503; a lease whose holder died lapses and is
  taken over. The rotated pair is written only if the row still has the pair the rotation
  started from. If the holder's answer is lost after concierge rotated, the next rotation
  presents a spent token and concierge closes the family: it fails closed.
- **Bounded.** `/auth/*`: at most 8 at once (else 503), 10 s each. `/api/v1`: 32, 15 s. The
  pool gives a connection within 3 s or the request fails. Every answer carries
  `X-Content-Type-Options: nosniff`; the sign-in's HTML pages a CSP of `default-src 'self';
  frame-ancestors 'none'; base-uri 'none'; form-action 'self'`.
- **Cookies** are `__Host-` and `Secure` when `PANEL_PUBLIC_ORIGIN` is https, bare over plain
  http (development only), all `SameSite=Lax`: the callback arrives by a top-level navigation.
- `serve` without any of the four sign-in variables answers ingest alone; some but not all
  of them is a configuration error at boot.

## The operator API, `/api/v1`

JSON; timestamps RFC 3339, money in minor units (|amount| ≤ 10^10, currency one of EUR,
USD, GBP, AUD — checked here, not in the registry, so a rebuild never re-judges old events),
errors `{"error": "…"}` with 400 / 403 / 404 / 409 / 503. Writes need the CSRF header and
journal an event of kind `panel` (manual), so they go through the same registry and
projections as ingest.

`POST /leads`, `…/stage` and `…/payments` take an optional `Idempotency-Key` header (1–128
visible ASCII). The event id is then derived from (user, action, lead, key) — shaped as a
UUIDv7, its time field hash — and so is a new lead's id: a retry finds the first attempt in
the journal and is answered `200` with the same body, journaling nothing. Without the key,
every request is a new event (`201`).

```text
GET    /me                                        {user_id, role, email, preferred_name}
GET    /leads?stage&brand&location&overdue&cursor&limit
                                                  {leads: [Lead], next_cursor}; newest created
                                                  first, limit ≤ 200 (default 50)
POST   /leads                                     {brand, location, need, phone?} → 201
                                                  {brand, lead_id: "p-<uuidv7>", event_id}
GET    /leads/{brand}/{lead}                      {lead: Lead, events: [Event]}
POST   /leads/{brand}/{lead}/stage                {stage: contacted, channel?} | {stage: quoted,
                                                  amount?, currency?} | {stage: won, job_id?} |
                                                  {stage: lost, reason, note?} | {stage: completed}
                                                  → 201 {event_id}
POST   /leads/{brand}/{lead}/calls/attempt        → 201 {attempt_id}
POST   /leads/{brand}/{lead}/calls/{attempt}/outcome
                                                  {outcome: answered|no_answer|wrong_number|later}
POST   /leads/{brand}/{lead}/payments             {billed, commission, currency}
GET    /funnel?from&to&brand                      days, UTC, default the last 30, ≤ 366
                                                  {stages: [{stage, reached, of_previous, of_leads}],
                                                   lost, manual, min_sample}
GET    /sources                     admin         {sources: [{key_id, kind, brands, created_at,
                                                  revoked_at}]}
POST   /sources                     admin, fresh  {key_id, kind, brands} → 201 {key_id, secret}
                                                  (shown once), 409 if taken; kind panel → 400
                                                  (the panel writes without a key)
DELETE /sources/{key_id}            admin, fresh  204, 404
```

```text
GET    /telegram                                  {enabled, linked, blocked, rules: {rule: bool}}
                                                  (the rules the caller's role gets)
POST   /telegram/link                             → 201 {url: "https://t.me/<bot>?start=<token>"};
                                                  503 without a bot
DELETE /telegram/link                             204, 404
PUT    /telegram/rules      {rules: {new_lead: false, …}}
                                                  → 200 as GET; 400 for a rule not the role's
```

`Lead` is the projection row (`stage`, the time of each stage, `manual`, `lost_reason`, …)
plus `sla` while it waits for its first contact — `{waiting_since, waiting_seconds,
overdue}`, overdue after 30 minutes — and `pii` (the customer's name, phone, need) for the
roles that see it. A share is `{n, of, percent, small_sample}`; `percent` is null while `of`
is under `min_sample` (§10.1), so the front end can only draw "n of of".

## Telegram (§8)

A bot (`TELEGRAM_BOT_TOKEN`; without it, or without the sign-in, none of this runs) writes to
each user in a private chat. Rules: `new_lead` and `contact_overdue` (every role, on by
default), `payment_received` and `source_silent` (admins, off by default). A 3★ review, a
funnel drop and Grafana alerts are variants to come, once their sources exist.

```text
POST /api/v1/telegram/link  256 random bits, base64url; SHA-256 stored with the caller's
                            role, 10 min → t.me/<bot>?start=<token>
/start <token>              private chats only (groups are ignored whatever they say); the
                            token redeemed once → telegram_links(user ⇄ chat)
/stop, DELETE …/link        unlinked; what the outbox still owed them is dropped
fan-out (2 s)               new leads (their counted creation ≤ 1 h old, still `created`),
                            leads created 30 min – 6.5 h ago never contacted (once each),
                            payments (≤ 24 h), sources silent ≥ 24 h (once per full day of
                            it) → telegram_fanout claims (rule, event) once, and in the same
                            transaction one outbox row per recipient, UNIQUE (rule, event, chat)
delivery (0.5 s)            claim under one advisory lock: ≤ 25 tries started in any second,
                            one per chat per second, none to a chat with a send in flight;
                            leased 60 s → sendMessage → sent | retry | dead
```

- **Updates by long polling.** A webhook would need a public route through the Cloudflare
  tunnel and a secret-header check on it; `getUpdates` needs egress to `api.telegram.org`
  only. One replica polls, under a lease in `telegram_poller` (60 s, renewed every poll) that
  another takes over when it lapses; the offset is stored there after each update, and every
  update is idempotent, so one handled twice across a takeover does nothing twice.
- **Retries.** 429 waits `retry_after` (the whole chat does); 5xx and timeouts back off from
  10 s, doubling, to 15 min; 10 tries, then dead. 403 (the bot blocked) marks the chat dead,
  gives up what it was owed, and nothing more goes there until the user links again. Any
  other 4xx is dead at once.
- **PII.** A new lead's message carries the brand, location, need and phone — for roles that
  see PII in the panel (every role today, §5.4). Queued texts are sealed under
  `PANEL_DATA_KEY` like the journal's PII and dropped once sent or dead; messages are plain
  text, so nothing a customer typed is markup.
- **Who gets a message: a role concierge confirmed within the hour.** The panel learns a role
  only from `GetMe`, which takes the user's own access token. Each link keeps the role last
  confirmed and when: the `/api/v1` gate records every `GetMe` answer, and every 15 min the
  bot asks again for a link not confirmed since, through the user's newest live panel session
  (rotating it when due, as a request would). A grant revoked at concierge stops messages
  within ~15 min; a session concierge refuses stops them at once; concierge unreachable stops
  them after an hour. The risk left: up to that hour of messages (with PII) to someone whose
  grant was revoked while concierge could not be asked — or, with the bot keeping a linked
  user's session rotated, as long as concierge keeps rotating it.
- **Buttons.** "Взял" writes `lead.contacted`, "Не дозвонился" `call.attempted` +
  `call.logged{no_answer}`, through the operator API's path (`source.kind = panel`,
  `source.id` = the user). The callback data is `<button>.<outbox id>.<HMAC-SHA256, 80 bits>`
  under a key derived from `PANEL_DATA_KEY`, bound to the chat: nothing else in it is
  trusted. The message must be that chat's, the chat linked to the message's user, and the
  user's access is asked of concierge at the press — with no live panel session to ask with,
  the answer is "open the panel to confirm your access", never a cached role. The event ids
  derive from (user, outbox id, button), so a press repeated, or its update redelivered,
  records nothing twice. The message is then edited: "Взял: <preferred_name or email>" added
  and the buttons removed ("Не дозвонился" leaves "Взял").
- Language: `TELEGRAM_LOCALE` (`ru` default, `en`).

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
  `PANEL_DATA_KEY`, `SENTRY_DSN`, `RP_CLIENT_SECRET_SA`, `TELEGRAM_BOT_TOKEN`),
  through `ev_lib::settings`; with `APP_ENV=production`, `DATABASE_URL`, `PANEL_DATA_KEY` and the
  four sign-in variables are required at boot (`panel --print-required-vars` lists them).

## Deploy requirements

- **Ingest stays inside the cluster.** Its sources (the landings, review_archive) reach it
  by service DNS (§3.3); the IngressRoute that publishes `sa.evinvest.ltd` must not route
  `/api/ingest`, and the NetworkPolicy lets in only the pods that send. The signature is
  what authenticates a batch; keeping the route off the internet is what keeps its cost —
  a database lookup and a MAC per request — away from anyone who can reach a URL.
- **concierge within reach.** The panel's pods call concierge's gRPC (`CONCIERGE_GRPC_ADDR`)
  for every sign-in, token rotation and `GetMe`; the egress policy must allow it. concierge
  must register client `sa` with redirect URI `<PANEL_PUBLIC_ORIGIN>/auth/callback` exactly,
  and hold the hash of `RP_CLIENT_SECRET_SA`.
- **Telegram, outbound only.** With `TELEGRAM_BOT_TOKEN` (the panel bot's, in sops; not
  `telegram_token_main`) the pods need egress to `api.telegram.org:443`; nothing inbound. No
  webhook is set on the bot (`getUpdates` refuses to run while one is).
- **Rate-limit `/auth` per client IP at Traefik** (a `RateLimit` middleware on the
  IngressRoute's `/auth` prefix, e.g. 10/min with a burst of 20). The panel bounds how many
  sign-ins run at once, not who starts them; per-IP limits are the edge's.
- **Migrate, grant, then roll out.** A release that carries a migration runs `panel migrate`
  (as a Job or an init container with `MIGRATE_DATABASE_URL`) before the new pods start, then
  `deploy/panel_app.sql` as the owner, so a new table is granted too. Both roles, `panel_app`
  included, are the deploy's to create (devops); `sa_grafana` gets `USAGE` on `reporting` and
  `SELECT` on its views, nothing else.
