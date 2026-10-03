# Running the stack locally

The panel and both landings (aquafix, vifnet) on one machine, wired as in the cluster: a lead
posted on a site arrives in the panel through ingest, and a place's phone or hours edited in
the panel shows on the site through `/api/internal`. No concierge, no PostHog, no Telegram:
the panel signs you in by itself (dev sign-in, below).

## Prerequisites

- Nix with flakes. Everything else (Rust, Node, the sites' toolchains) comes from the flakes.
- The sites checked out beside the panel, as `service_arb/` holds them:

  ```text
  service_arb/
    panel/      this repository (its worktrees work too: the sites are found beside the main checkout)
    aquafix/    Service-Arb/aquafix, up to date — the Next.js (kitstart) site
    vifnet/     Service-Arb/vifnet, likewise
  ```

  Elsewhere: `AQUAFIX_DIR=… VIFNET_DIR=… nix run .#local-stack`. A checkout without
  `package.json` (the old Rust sites) is skipped with a message: `git pull` it.
- Ports 59120 (panel), 59081 (aquafix) and 59082 (vifnet) free.

## The command

```sh
nix run .#local-stack                  # build, seed, start all three; Ctrl-C stops everything
nix run .#local-stack -- --reset       # wipe .local/ first: database, keys, the sites' leads
nix run .#local-stack -- --no-sites    # the panel alone
nix run .#local-stack -- --role operator
```

The first run builds the panel and its front end with Nix (minutes), and each site's
`nix run .#dev` installs its `node_modules` (minutes again). Later runs start in seconds.

What it does (`scripts/local-stack.sh`; the flake app is the same script with this flake's
builds in `PANEL_BIN` and `PANEL_WEB_DIR`):

1. Keeps its state in `.local/` (ignored by its own `.gitignore`): `panel.db`, the
   `data-key`, the sources' secrets in `secrets/`, each site's `<site>-leads.db`, and the
   logs in `logs/`. It persists between runs; `--reset` wipes it.
2. Seeds, idempotently: the sources `aquafix-site` (brand `aquafix`) and `vifnet-site`
   (brand `vifnet`), each secret kept in `.local/secrets/` and reused; the places
   `aquafix/{royat, clermont-ferrand, desgenettes, lyon-est, la-mouche, lyon-nord}` and
   `vifnet/vifnet`, registered with nothing set (`panel place register`).
3. Starts `panel serve` on `127.0.0.1:59120` in development, with dev sign-in and the front
   end's static export served on the same origin.
4. Starts each site with its own `nix run .#dev` (kitstart's `next dev`), with:

   | Variable | Value |
   | --- | --- |
   | `LEAD_WEBHOOK_URL` | `http://127.0.0.1:59120/api/ingest/v1/events` |
   | `LEAD_WEBHOOK_KEY_ID` / `LEAD_WEBHOOK_SECRET` | the seeded source's |
   | `LOCATIONS_API_URL` | `http://127.0.0.1:59120/api/internal/brands/<brand>` |
   | `LEADS_DB_PATH` | `.local/<site>-leads.db` |
   | `POSTHOG_KEY` | unset: analytics off |

5. Prints the URLs, prefixes every log line with whose it is, and stops everything on Ctrl-C
   (or when the panel stops). Started from a non-interactive shell (an agent's, in the
   background), SIGINT does not reach it: stop it with `kill <pid>` (SIGTERM), which does the
   same. Each part runs in a process group of its own, so the caller's shell is never hit.

Iterating on the backend: `cargo build -p panel_server`, then
`PANEL_BIN=target/debug/panel nix run .#local-stack` (or `scripts/local-stack.sh`, which
builds what you do not name with `nix build`).

## URLs

| What | URL |
| --- | --- |
| Panel | <http://127.0.0.1:59120> — opens signed in |
| Panel: leads, locations | <http://127.0.0.1:59120/leads/>, <http://127.0.0.1:59120/places/> |
| aquafix, a point | `http://<slug>.localhost:59081/fr`, e.g. <http://royat.localhost:59081/fr> |
| aquafix, the brand page | <http://localhost:59081/fr> |
| vifnet | <http://localhost:59082/fr> |

`*.localhost` resolves to this machine in every current browser and in curl; nothing to add
to `/etc/hosts`.

## Dev sign-in

`PANEL_DEV_SIGN_IN=admin|operator` (and optionally `PANEL_DEV_SIGN_IN_EMAIL`, by default
`dev-<role>@localhost`) replaces concierge: `/auth/login` sends the browser straight to
`/auth/callback`, which opens a real panel session for a made-up user with that role. The
sidebar shows the user as **Dev sign-in (admin)**, `GET /api/v1/me` answers
`"dev_sign_in": true` (for a UI badge), and `serve` logs `DEV SIGN-IN ON` at start.

The binary refuses it at start (exit 78, every command) in any `APP_ENV` but `development`,
beside any concierge variable, and unless `PANEL_PUBLIC_ORIGIN` is `http://localhost[:port]`
or `http://127.0.0.1[:port]` — so it cannot be switched on in the image, which sets
`APP_ENV=production`. `cargo test -p panel_server --test dev_sign_in` holds the binary to it.

Without the script:

```sh
APP_ENV=development PANEL_DB_PATH=./panel.db PANEL_DATA_KEY="$(panel gen-data-key)" \
PANEL_PUBLIC_ORIGIN=http://127.0.0.1:59120 PANEL_DEV_SIGN_IN=admin \
PANEL_WEB_DIR="$(nix build .#frontend --print-out-paths)" panel serve
```

The front end's dev server instead of the export: start the stack, then
`cd frontend && PANEL_DEV_BACKEND=http://127.0.0.1:59120 npm run dev` serves on `:3120`. The
sign-in's redirect goes to `PANEL_PUBLIC_ORIGIN`, so for cookies on `:3120` run the panel by
hand as above with `PANEL_PUBLIC_ORIGIN=http://localhost:3120`.

A session in curl (what a browser does):

```sh
jar=$(mktemp)
curl -s -c "$jar" -b "$jar" -L -o /dev/null http://127.0.0.1:59120/auth/login
curl -s -b "$jar" http://127.0.0.1:59120/api/v1/me
# a write needs the CSRF cookie echoed in x-sa-csrf; expected_updated_at is the place's
# current updated_at (GET …/settings), null for one never set
csrf=$(awk '$6 == "sa_csrf" { print $7 }' "$jar")
curl -s -b "$jar" -H "x-sa-csrf: $csrf" -H 'content-type: application/json' \
  -X PUT http://127.0.0.1:59120/api/v1/places/aquafix/royat/settings \
  -d '{"settings": {"phone": "+33499887766"}, "expected_updated_at": null}'
```

Or skip the session: `panel place set aquafix royat --phone +33499887766` with the stack's
`PANEL_DB_PATH=.local/panel.db PANEL_DATA_KEY=$(cat .local/data-key)`.

## The end-to-end check

With the stack up:

**A lead, site → panel**

1. Open <http://royat.localhost:59081/fr> and submit the quote form (any valid French mobile,
   e.g. `0612345678`, and a postcode such as `63130`). The site answers with its thanks page.
2. The site keeps the lead in `.local/aquafix-leads.db` and its outbox posts it, signed, to
   the panel within about 10 s (kitstart's `WEBHOOK_TICK_MS`). `[panel]` logs
   `ingested a batch key_id="aquafix-site" accepted=1`; the site's side is the row in
   `sqlite3 .local/aquafix-leads.db 'select id, state, attempts, last_error from webhook_outbox'`
   (`delivered`).
3. Open <http://127.0.0.1:59120/leads/>: the lead is there, stage *created*, brand aquafix,
   location royat, with what you typed.

The same for vifnet at <http://localhost:59082/fr>. Without a browser, the form's no-JS path:

```sh
t=$(( $(date +%s) * 1000 - 5000 ))   # rendered five seconds ago, as a person would have
curl -si http://royat.localhost:59081/quote --data-urlencode locale=fr --data-urlencode form_id=quote \
  --data-urlencode t=$t --data-urlencode location=royat --data-urlencode job=blocked_drain \
  --data-urlencode zip=63130 --data-urlencode mobile=0612345678 | head -1      # 303 → /fr/thanks
t=$(( $(date +%s) * 1000 - 5000 ))
curl -si http://localhost:59082/quote --data-urlencode locale=fr --data-urlencode form_id=quote \
  --data-urlencode t=$t --data-urlencode location=vifnet --data-urlencode subject=deep \
  --data-urlencode locality=83702 --data-urlencode mobile=2085550192 \
  --data-urlencode 'name=Smoke Test' --data-urlencode bedrooms=3 | head -1
```

**A place, panel → site**

1. Open <http://127.0.0.1:59120/places/>, pick aquafix / royat, set a phone (e.g.
   `+33 4 23 50 06 40`) and save.
2. `curl -s http://127.0.0.1:59120/api/internal/brands/aquafix/locations/royat` answers the
   new phone at once: that is what the site reads.
3. Reload <http://royat.localhost:59081/fr>: the new number shows — see the cache note below.

**The cache.** kitstart fetches a place with `cache: "force-cache"` and a 600 s revalidate
(`PLACE_REVALIDATE_SECONDS`), and the point pages are ISR with `revalidate = 600`. In
production a change therefore shows within about 10 minutes (the first request after the
TTL re-renders in the background, the next one gets it). Under `next dev` the fetch is cached
the same way (in memory, 600 s): a plain reload keeps the old number. To see a change at once:

- a **hard reload** (Cmd-Shift-R / Ctrl-Shift-R): the browser sends `Cache-Control: no-cache`,
  and `next dev` then fetches afresh — later plain reloads get the new value too. In curl:
  `curl -H 'cache-control: no-cache' http://royat.localhost:59081/fr`;
- or restart the stack (the cache is in memory).

Production has no such bypass: there, wait out the 600 s.

## Troubleshooting

- **`port 59120 is taken`**: another stack, or something else —
  `lsof -nP -iTCP:59120 -sTCP:LISTEN` names it. The same for 59081/59082.
- **A site is skipped**: its checkout is missing or predates the Next site; `git pull` it, or
  point `AQUAFIX_DIR` / `VIFNET_DIR` at another one.
- **`source … exists but its secret is not in .local/secrets`**: the secret is shown once and
  `.local/secrets/` lost it. `--reset`.
- **The lead does not arrive**: look at the outbox row (`webhook_outbox` in
  `.local/<site>-leads.db`): `last_error` holds the panel's answer, and the outbox retries with
  backoff, so a lead posted while the panel was down arrives after it is back. A `401` means
  the site's key id and secret are not the panel's — e.g. a site started by hand with old
  values after a `--reset`; the script always passes the current ones.
- **The site shows the old phone**: the cache, above. A `404` for the place in the panel's
  answer means it was withdrawn (Locations → restore).
- **The sites' checkouts get `AGENTS.md` and `CLAUDE.md`**: `next dev` writes them unless a
  site's `next.config.ts` sets `agentRules: false` (the panel's front end does).
- **Start over**: `nix run .#local-stack -- --reset`.
