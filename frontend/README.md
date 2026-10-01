# Panel front end

Next.js (App Router), Feature-Sliced Design, `@evinvest/uikit`. The screens are the
ones of `SA-PANEL-SPEC.md` §10 (hybrid 1+2): Overview, Leads, Locations, Sources
(admin), and on a phone a bottom tab bar with "More" in place of the sidebar.

```text
app/                 routes: (panel)/ is everything behind sign-in, signed-out/ is not
src/views/           one slice per screen, plus shell/ (sidebar, tab bar, access states)
src/features/        call-lead · move-stage · record-payment · create-lead ·
                     lead-filters · funnel-filters · manage-sources · sign-out
src/entities/        session · lead · funnel · place · source — types, response checks, requests
src/shared/          api/ (fetch, CSRF, the gate's answers), i18n/, lib/, ui/
messages/            en.json (the source of keys) and ru.json
scripts/dev-stub.ts  a stand-in backend for local work
tests/               vitest, in Node: the rules the screens obey live in plain modules
```

## Running it

```sh
npm ci
npm run dev:stub     # :3121 — the operator API over made-up leads; STUB_ROLE=admin, STUB_ME=401|403|503,
                     # STUB_MIN_SAMPLE=2 for percents on so few leads
npm run dev          # :3120 — proxies /api and /auth to PANEL_DEV_BACKEND (default the stub)
```

Against the real backend, point `PANEL_DEV_BACKEND` at `panel serve` with sign-in
configured and `PANEL_PUBLIC_ORIGIN=http://localhost:3120`, so the callback lands here.

Checks: `npm run typecheck`, `npm run lint`, `npm test`, `npm run build`.

## How it is served

`next build` is a static export (`out/`): every screen reads `/api/v1` from the
browser with the session cookie, so a Node server would have nothing to render,
and the session cookie is `HttpOnly` anyway. The Rust binary serves `out/` on the
same origin as `/api` and `/auth` — one image, one Service, one IngressRoute, as
aquafix has one. What that needs of the backend and the flake:

- `panel serve` gets a fallback: files from a directory (say `PANEL_WEB_DIR`),
  a directory answering with its `index.html` (the export writes `leads/index.html`
  and links `/leads/`), and `404.html` with status 404 for anything else. `/api`,
  `/auth`, `/health` and later `/grafana` keep their routes ahead of it.
- Cache headers: `/_next/static/*` is content-hashed — `public, max-age=31536000,
  immutable`; every HTML file `no-cache`. A CSP (`default-src 'self'`, plus
  `'unsafe-inline'` for `script-src` while Next inlines its bootstrap).
- The flake builds `frontend/` with `buildNpmPackage` (needs `npmDepsHash`) and puts
  `out/` in the container beside the binary; the entrypoint sets the directory.
- The IngressRoute for `sa.evinvest.ltd` sends everything to the panel's Service
  except `/api/ingest` (in-cluster only, `docs/ARCHITECTURE.md`).

## What the API does not answer yet

- **Stages 1–4** (Maps, site) arrive with the GBP and PostHog imports (phase 2);
  until then the overview says so rather than showing zeros.
- **Lost reasons** are a slug the backend takes freely; the panel offers a fixed
  list (`features/move-stage/model/moves.ts`) so the reports can group by it.
