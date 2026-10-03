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
  src/metrics.rs                     the day counts of stages 3–4 and of the experiments; a
                                     recount's revision; traffic sources bounded
  src/experiment.rs                  a variant against its control: Wilson, Newcombe, the
                                     `insufficient` rule
  src/place.rs                       a place's live settings (kitstart's PlaceLive), checked
                                     field by field; a change and who made it
crates/panel/                        the engine
  src/lib.rs                         the `Panel` facade: ingest, sources, PII, rebuild
  src/wire.rs                        protojson → the core: one event decoded and checked; the
                                     registry (type@version → properties message → Fact)
  src/seal.rs                        XChaCha20-Poly1305 at rest: PII, sources' HMAC secrets
  src/store/                         SQLite (runtime sqlx queries, embedded migrations/,
                                     applied on open; writes BEGIN IMMEDIATE)
  src/store/events.rs                the journal
  src/store/projections.rs           leads, calls, payments
  src/store/sources.rs               the signing keys
  src/store/telegram.rs              links, rules, fan-out marks, the outbox and its pacing
  src/telegram.rs                    linking, the rules' fan-out, delivery, the buttons; the
                                     Bot and Directory ports
  src/posthog.rs                     the hourly PostHog import: HogQL → counts → journal; the
                                     Hogql port
  src/counts.rs                      what the screens read of the counts
  src/place.rs                       a place's settings changed (optimistic concurrency, revert,
                                     withdraw) and what a site is answered
  src/store/places.rs                places, place_settings, the place_changes history
  src/store/metrics.rs               daily_location_metrics, daily_experiment_metrics, the
                                     import's lease
  src/testing.rs                     (feature `testing`) throwaway SQLite files, signed batches
  migrations/                        the schema: the init (`reporting_*` views and the
                                     journal's append-only triggers included), then one file
                                     per change, each with its `down`
crates/panel_server/                 the `panel` binary: CLI and HTTP, thin over `Panel`
  src/http.rs                        POST /api/ingest/v1/events, GET /health; the sign-in and
                                     /api/v1 mounted on top when signing in is configured
  src/signin.rs                      /auth/login, /auth/callback, /auth/logout; the /api/v1 gate
  src/concierge.rs                   concierge over gRPC: ExchangeCode, RefreshClientToken, GetMe;
                                     its development stand-in (PANEL_DEV_SIGN_IN)
  src/cookies.rs                     __Host- cookies, the double-submit CSRF check
  src/api.rs                         the operator API: JSON over the engine's `operator` module
  src/places.rs                      a place's settings: the editor's routes, and the sites'
                                     GET /api/internal/…, which has no session
  src/telegram.rs                    the Bot API (reqwest), the bot's background work in
                                     `serve`, and /api/v1/telegram
  src/posthog.rs                     PostHog's query API (reqwest) and the import's schedule
  src/counts.rs                      stages 3–4 in /api/v1/funnel, GET /api/v1/experiments
  src/web.rs                         the front end's static export (PANEL_WEB_DIR), behind
                                     every other route
  src/settings.rs                    the environment (ev_lib `settings!`)
contracts/proto/concierge/v1/        concierge's auth and directory protos, vendored at the
                                     commit in REV (`sync.sh` refreshes them)
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
               └─ new: a call / payment / count row; its lead recomputed → accepted
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
  row. Across processes, a lease on the row (`sessions.rotating_until`, taken by one
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
- **Dev sign-in** (`PANEL_DEV_SIGN_IN=admin|operator`, docs/LOCAL.md): `Concierge::dev`
  answers ExchangeCode, RefreshClientToken and GetMe for one made-up user, and `/auth/login`
  redirects straight to `/auth/callback?code=dev-sign-in&state=…`; the pre-login, the state,
  the session and the gate run unchanged. Refused at boot (exit 78, every command) in any
  profile but development, beside any concierge variable, and unless `PANEL_PUBLIC_ORIGIN`
  is `http://localhost[:port]` or `http://127.0.0.1[:port]` — the image is
  `APP_ENV=production`, so it cannot be on there.

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
GET    /leads?stage&brand&location&overdue&created_from&created_to&cursor&limit
                                                  {leads: [Lead], next_cursor}; newest created
                                                  first, limit ≤ 200 (default 50); created_*
                                                  UTC days, both included
GET    /leads/counts?brand&location               {stages: {created: n, …, lost: n} (every
                                                  stage, 0 included), overdue, total}
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
GET    /funnel?from&to&brand&by                   days, UTC, default the last 30, ≤ 366
                                                  {from, to, brand, min_sample, stages: [{stage,
                                                   reached, of_previous, of_leads}], lost, manual,
                                                   payments: [Paid]}
                                                  by=location: {from, to, brand, min_sample,
                                                   by, locations: [{brand, location, stages, lost,
                                                   manual, payments}]}, one per brand's location
                                                   (location null for the leads naming none);
                                                   an empty window has no rows
                                                  every answer: aggregate_source {source:
                                                   "posthog", kind: "aggregate", imported_at};
                                                   each slice: aggregate (stages 3–4, below)
GET    /experiments?from&to&brand                 days as /funnel; below
GET    /places                                    {places: [{brand, location, last_lead_at,
                                                  has_settings, withdrawn}]}: every location a
                                                  lead names or PostHog counted, and every
                                                  place registered (below)
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

The funnel counts the leads that came in (were created) within the window, and `Paid` —
`{currency, billed, commission, count}` in minor units, one per currency, never converted —
sums the payments of those same leads, whenever they were paid; so a slice's money and its
`paid` step are about the same leads.

**Stages 3–4 beside 5–10, never divided by them** (§10.1). A slice's `aggregate` is
`{stages: [{stage: "site.visit", total, by_source: {source: n}, days: [{day, n}]},
{stage: "contact.intent", total, by_channel: {phone, whatsapp, form_open, booking}, days:
[{day, n, by_channel}]}]}` — page views and intents as PostHog counted them, per UTC day (the
days with a count only; a day missing is nothing counted, or not imported yet — see
`imported_at`). By location, a location with visits and no lead is a row too. Stages 1–2
(Maps) come with the GBP import.

`/experiments` answers `{from, to, brand, min_sample, min_exposures: 100, confidence: 0.95,
z, interval: "newcombe_hybrid_score", source, experiments: [{brand, experiment, first_day,
last_day, control, variants: [{variant, control, exposures, leads, intents: {phone, whatsapp,
form_open, booking}, rates: {lead: Share, contact: Share}, vs_control: null | {lead: Cmp,
contact: Cmp}}]}]}`, the control first. `lead` is leads per exposure, `contact` (leads +
phone + WhatsApp) per exposure, as the landings' own reports define them; successes past the
exposures are capped. `Cmp` is `{difference: null | {estimate, low, high, decimals},
insufficient, reason}` in percentage points, treatment − control: `difference` is null and
`reason` `small_sample` while either arm has under `min_exposures`; `insufficient` with
`interval_includes_zero` while the 95 % interval holds 0 (judged before rounding). Rounding
follows the interval: whole points while it is 2 points wide or more, tenths when narrower
(`decimals`). The control is the variant named `control`, else `a`, else the first by name —
PostHog never sees the landing's config. There is no winner field, and none is to be drawn.

Why Newcombe's hybrid score interval (method 10 of Newcombe 1998), not `d ± z·SE`: the Wald
interval collapses at 0 successes, leaves [−1, 1] and undercovers badly at the rates (a few
percent) and arm sizes (hundreds) the landings have; Newcombe's, built from the two Wilson
intervals, has none of these faults. The tests check it against the paper's published
examples. It treats page views as independent trials, which they only approximately are; no
correction is made for several variants against one control.

`Lead` is the projection row (`stage`, the time of each stage, `manual`, `lost_reason`, …)
plus `sla` while it waits for its first contact — `{waiting_since, waiting_seconds,
overdue}`, overdue after 30 minutes — and `pii` (the customer's name, phone, need) for the
roles that see it. A share is `{n, of, percent, small_sample}`; `percent` is null while `of`
is under `min_sample` (§10.1), so the front end can only draw "n of of".

## Place settings

A landing bakes its places into its build and lays over each, field by field, what the
panel answers for it (kitstart's `createPlaceSource`, `PlaceLive`): `phone`, `whatsapp`
(E.164), `hours` (`[{days, opens, closes}]`), `serviceArea` (commune names), and for
storefronts `address`, `geo`, `storefrontPhoto` (https), `landmark` (a text per locale),
`rating`. Hours are the place's local time, Europe/Paris for every place in v1 (no
time zone per place). kitstart fetches every 10 minutes at most, times out after 3 s, and keeps its baked
place on a 5xx or no answer; a JSON 404 is a place withdrawn, and the site 404s it.

```text
GET  /api/internal/brands/{brand}/locations/{slug}?locale   no session; ≤ 32 at once, 3 s
       200 the settings, only the fields set; {} for none, for a place the panel does not
       know, and for a brand or slug that cannot name one — never a 404 for those, or an
       empty database would take the sites down
       404 {"error": "not_found"}: only a place an admin withdrew
```

Under `/api/v1`, CSRF on writes like the rest; writes are an admin's and ask concierge afresh
(`gate_fresh`), operators read (`can_edit` false, `403` on a write):

```text
GET  /places/{brand}/{slug}/settings          {brand, slug, withdrawn, settings, updated_at,
                                              updated_by, can_edit}; updated_at null: never set
PUT  /places/{brand}/{slug}/settings          {settings, expected_updated_at} → 200 as GET; a
                                              full replace; 409 {"error": "conflict"} when
                                              expected_updated_at is not the current one;
                                              422 {"error": "invalid", "fields": {key: why}},
                                              a key the field or its path in a list:
                                              hours[0].opens, hours[1].days, serviceArea[2]
GET  /places/{brand}/{slug}/settings/history  {changes: [{id, at, by, kind, before, after,
                                              reverts}]}, newest first; kind register | set |
                                              revert | withdraw | restore
POST /places/{brand}/{slug}/settings/revert/{id}  the `before` of that change made current,
                                              journaled as a revert → 200 as GET; 404; body
                                              {expected_updated_at} optional, 409 as a PUT
POST /places/{brand}/{slug}/withdraw|restore  → 200 as GET; the settings are kept
POST /places                                  {brand, slug} → 201 as GET; 409 {"error":
                                              "exists"} once registered (by hand or an edit)
```

- **Checked as kitstart reads them, refused instead of dropped.** kitstart drops a field it
  cannot read so a page never fails on one; the panel names it in a 422 so the editor sees
  why. Stricter than kitstart where kitstart is loose (a phone is E.164, not just `+…`;
  hours close after they open, a night across midnight is two rows; a day once per row),
  never looser. kitstart keeps a landmark only when it has every locale the site speaks,
  which the panel does not know.
- **A change is one write transaction**: the current settings read under the write lock,
  `expected_updated_at` compared, the new ones written, the change journaled
  (`place_changes`: before, after, who — the user's email and concierge id, or `cli` — and
  when). `updated_at` only grows, a microsecond at least per change, so two edits never share
  a token. A change that changes nothing writes nothing.
- **The CLI** (`panel place set|show|history|revert|withdraw|restore`) goes through the same
  calls, `by = cli`, patching over what is there when its transaction begins.
- **History is append-only, places never deleted**: triggers refuse any UPDATE or DELETE of
  `place_changes`, a DELETE of `places` or `place_settings` (cleared is `{}`), and any UPDATE
  of `places` but `withdrawn`.

## Telegram (§8)

A bot (`TELEGRAM_BOT_TOKEN`; without it, or without the sign-in, none of this runs) writes to
each user in a private chat. Rules: `new_lead` and `contact_overdue` (every role, on by
default), `payment_received` and `source_silent` (admins, off by default). A 3★ review, a
funnel drop and Grafana alerts are variants to come, once their sources exist.

```text
POST /api/v1/telegram/link  GetMe asked afresh; 256 random bits, base64url; SHA-256 stored
                            with the caller's role and the time, 10 min, replacing the user's
                            earlier token → t.me/<bot>?start=<token>
/start <token>              private chats only, not forwarded (groups are ignored whatever they
                            say); the token redeemed once → telegram_links(user ⇄ chat, the
                            role confirmed as of the token's issue, the Telegram @username);
                            a chat linked to another account is refused ("/stop first"), the
                            token kept; the reply names the panel account
/stop, DELETE …/link        unlinked; what the outbox still owed them is dropped
fan-out (2 s)               new leads (their counted creation ≤ 1 h old, still `created`;
                            not to whoever typed one in),
                            leads created 30 min – 6.5 h ago never contacted (once each),
                            payments (≤ 24 h), sources silent ≥ 24 h (once per full day of
                            it) → telegram_fanout claims (rule, event) once, and in the same
                            transaction one outbox row per recipient, UNIQUE (rule, event, chat)
delivery (0.5 s)            lead messages queued > 1 h ago dead ("stale"); > 20 waiting for a
                            chat folded into one "N new leads, open the panel" without PII;
                            claim in one write transaction (nothing while the outbox is paused):
                            ≤ 25 tries started in any second, one per chat per second, none to
                            a chat with a send in flight; leased 60 s → the user's access
                            checked again (below) → sendMessage → sent | retry | dead
```

- **Updates by long polling.** A webhook would need a public route through the Cloudflare
  tunnel and a secret-header check on it; `getUpdates` needs egress to `api.telegram.org`
  only. One process polls, under a lease in `telegram_poller` (60 s, renewed every poll) that
  another takes over when it lapses; the offset is stored there after each update, and every
  update is idempotent, so one handled twice across a takeover does nothing twice.
- **Retries.** 429 pauses the whole outbox for `retry_after` and costs the message no try;
  5xx and timeouts back off from 10 s, doubling, to 15 min; 10 tries, then dead. 403 (the bot
  blocked) and 400 "chat not found" / "user is deactivated" mark the chat dead, give up what
  it was owed, and nothing more goes there until the user links again. Any other 4xx is dead
  at once; so is a queued message that no longer opens.
- **Quiet.** The bot answers commands only: other messages get nothing, and its unsolicited
  replies (help, "not linked", "invalid link") go at most once per chat per 10 min.
- **PII.** A new lead's message carries the brand, location, need and phone — for roles that
  see PII in the panel (every role today, §5.4). Queued texts are sealed under
  `PANEL_DATA_KEY` like the journal's PII and dropped once sent or dead. What a customer typed
  is put on one line (control characters, line separators and bidi marks become spaces),
  bounded (name 80, need 500, phone 40 of digits and `+()-`, the message 3 500), and the name
  and need are `code` entities — no parse mode, so nothing is markup, no link is made of it,
  and no line of it can read as one of ours ("Взял: …"). Link previews are off.
- **Who gets a message: a role concierge confirmed within the hour.** The panel learns a role
  only from `GetMe`, which takes the user's own access token. Each link keeps the role last
  confirmed and when: the `/api/v1` gate records every `GetMe` answer, and every 15 min the
  bot asks again for a link not confirmed since, through the user's newest panel session
  used within 7 days (`sessions.last_seen_at`, set by the gate; rotated when due, as a
  request would). The role is checked again when a message is sent, not only when it is
  queued. A grant revoked at concierge, or a session concierge refuses to rotate (or whose
  `GetMe` it refuses twice running), stops messages at the next check and drops what was
  queued; no session used for a week, or concierge unreachable, stops them after an hour.
  The risk left: up to an hour of messages (with PII) to someone whose grant was revoked
  while concierge could not be asked.
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

## The PostHog import (§3.4, §7)

With `POSTHOG_PROJECT_ID` and `POSTHOG_PERSONAL_API_KEY` (a personal key, `query:read` on the
project the landings send to), `serve` imports once an hour; without them it warns and does
not, and one without the other fails the boot. `POSTHOG_API_HOST` defaults to
`https://us.posthog.com`: the query API is on the app host, not on the capture host the
landings' `POSTHOG_HOST` names (`us.i.posthog.com`). `panel import-posthog --days N` runs one
now (a backfill: up to 400 days, as long as each query stays under 10 000 rows).

```text
every 5 min      posthog_import: leased (10 min) if none holds it, the last import finished
                 ≥ 1 h ago and the last try ≥ 10 min ago — one process, once an hour
three HogQL      the last 3 UTC days (today included), days cut in UTC whatever the project's
queries          zone, LIMIT 10 000 (a full table is an error, not a short count):
                 location_page_view{brand_id, location_id, source}         → visits by source
                 contact_intent_click{brand_id, location_id, channel}      → intents by channel
                 experiment_exposed / experiment_contact{channel} / experiment_lead
                   {brand_id, experiment, variant, forced}, forced (QA) left out
rows → counts    brands a source key writes for only (a landing's PostHog key is public: anyone
                 can send events naming any brand); malformed days, locations, names, channels
                 left out and counted; sources lowercased, ≤ 20 per location and day, the rest
                 "other"
compared         with the projection's counts of those days: a slice new or changed → an event
                 at the next revision; a slice no longer found → 0 at the next revision; the
                 same count → nothing
journaled        site.metrics / contact.metrics / experiment.metrics, source.kind posthog,
                 source.id posthog-<project>, no key; occurred_at the day's start; the id from
                 (type, day, brand, location, slice, revision)
```

- **A recount replaces, the journal stays append-only.** A day's count changes for days as
  late events land, so the projection holds the highest revision of each slice
  (`INSERT … ON CONFLICT … WHERE revision < EXCLUDED.revision`, so a rebuild in any order
  lands on the newest). Upserting the projection without an event was the other way; it would
  make the counts the one projection the journal cannot rebuild. Writing only on a change
  keeps the journal's growth to the changes, not 24 × 3 copies a day.
- **Two writers of one revision.** The id derives from the revision and the content is
  deterministic, so the same recount from two processes (a lease lapsed mid-import) is a
  duplicate; different counts under one revision is a conflict, the second refused, and the
  next import writes the revision after.
- **Only the import writes counts**: `may_write` lets `posthog` alone write the three types.
  A count names no lead and no job; an experiment's no location (the landings' server-side
  `experiment_lead` names none). `experiment_step` (vifnet) is not imported.
- **Reporting.** `reporting_daily_location_metrics` (day, brand, location, metric `visits` |
  `contact_intent`, dimension: the source or channel, value) and `reporting_experiment_daily`;
  neither has anything personal — the events they come from carry no PII.

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
  `(occurred_at, id)`, so arrival order does not matter), inside the transaction that
  journaled the event. The rebuild runs the same code in one transaction, so it lands on the
  same state; the tests check that it does. Ingest and the rebuild do not interleave, nor two
  events of one lead: every transaction that writes begins `BEGIN IMMEDIATE`, taking SQLite's
  one write lock up front, so writers queue (up to `busy_timeout`, 10 s) and readers, in WAL,
  go on reading the last commit. Calls and payments are one row per event.
- **Stages move forward** through created → contacted → quoted → won → completed → paid; a
  `lead.lost` moves a lead to `lost` from anywhere, and later progress reopens it. Stage times
  are when each stage was first reached.
- **PII never sits in the clear.** `pii` is sealed per event (XChaCha20-Poly1305, the event id
  as associated data) under `PANEL_DATA_KEY`, whose fingerprint is stored beside every blob.
  The free text a customer typed goes in `pii`, not `properties`. Sources' HMAC secrets are
  sealed the same way (they must be usable to verify, so they cannot be hashed).
- **Reporting has no PII.** The `reporting_*` views over the projections and a daily ingest
  count select no `properties`, `pii_sealed` or secret: what the screens read of the counts,
  and what anything reading a copy of the database (a dashboard on the replica, an export)
  should be pointed at instead of the tables. SQLite has no roles to enforce that; whoever
  holds the file holds everything, sealed PII included — which is why PII is sealed.
- **The schema guards the journal.** SQLite has no roles, so what a runtime role's grants
  said on Postgres the tables' triggers say: `events` refuses DELETE and any UPDATE but of
  `status`/`status_reason` (compared NULL-safe, so writing a value back unchanged passes);
  `sources` refuses DELETE and any UPDATE but of `revoked_at`. Tables are STRICT, foreign keys
  enforced (`PRAGMA foreign_keys = ON` on every connection), and every Postgres CHECK kept.
- **One file, migrated on open.** `PANEL_DB_PATH` names it (`/data/panel.db` in the image);
  every command, `serve` included, creates it if missing and applies the migrations it lacks
  before anything else (`panel migrate` does only that). A migration newer than the build
  is let be: that is a rollback onto a schema moved on. Timestamps are INTEGER microseconds
  since the epoch, days `YYYY-MM-DD` text, UUIDs 16-byte blobs, JSON text that must parse.
- **Secrets come from the environment only** (`PANEL_DATA_KEY`, `SENTRY_DSN`,
  `RP_CLIENT_SECRET_SA`, `TELEGRAM_BOT_TOKEN`, `POSTHOG_PERSONAL_API_KEY`),
  through `ev_lib::settings`; with `APP_ENV=production`, `PANEL_DB_PATH`, `PANEL_DATA_KEY` and
  the four sign-in variables are required at boot (`panel --print-required-vars` lists them).

## Deploy requirements

- **One image, one origin.** The image carries the binary and the front end's static
  export, and sets `PANEL_WEB_DIR` to it; `serve` answers `/api`, `/auth` and `/health`
  first, then the files (a directory by its `index.html`, anything else `404.html` with a
  404). `/grafana` is held: a JSON 404 until the dashboards move there. `/_next/static/*`
  is `immutable` for a year, everything else `no-cache` (the store's 1970 mtimes make
  date revalidation meaningless, so the panel answers none); every page carries a CSP of
  `default-src 'self'` with `'unsafe-inline'` for scripts and styles (Next's inline
  bootstrap), `frame-ancestors 'none'`, `Referrer-Policy: same-origin`. Without
  `PANEL_WEB_DIR`, `serve` answers the API alone and says so.
- **The contract** (`nix eval .#containers.<system>.panel.contract`) names the port
  (59120), `/health`, `APP_ENV=production`, the variables required, secret and optional,
  `PANEL_DB_PATH`, the volume (`mounts = [ "/data" ]`) and the SQLite file on it to replicate
  (`sqlite = [ "/data/panel.db" ]`), what the ingress must not publish or must rate-limit, and
  the egress the pods need. `checks.contract-env` fails the flake when its
  list of required variables and the binary's `--print-required-vars` disagree.

- **`/api/internal` stays inside the cluster.** The landings read their places' settings
  there by service DNS; it has no session, so the IngressRoute must exclude the prefix
  (`excludePathPrefixes`) and the NetworkPolicy admit only the landings' pods.
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
- **PostHog, outbound only.** With the import configured the pods need egress to
  `us.posthog.com:443` (or wherever `POSTHOG_API_HOST` points); the key is a personal API key
  of someone with access to the project, scoped to `query:read` alone, in sops.
- **Rate-limit `/auth` per client IP at Traefik** (a `RateLimit` middleware on the
  IngressRoute's `/auth` prefix, e.g. 10/min with a burst of 20). The panel bounds how many
  sign-ins run at once, not who starts them; per-IP limits are the edge's.
- **One pod, a volume, litestream.** The database is the file on `/data`, a volume writable
  by uid 65534 that the cluster replicates off the pod with litestream (to R2) and restores
  on an empty volume, as for the tenant's other apps. One writer: one replica, rolled out
  with `Recreate`, never two pods on the volume at once. No init container: `serve` migrates
  on start, so rolling out an image with a new migration applies it, and rolling the image
  back does not undo it.
