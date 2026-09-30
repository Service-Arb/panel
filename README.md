# panel
![Minimum Supported Rust Version](https://img.shields.io/badge/nightly-1.100+-ab6000.svg)
![Lines Of Code](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/valeratrades/b48e6f02c61942200e7d1e3eeabf9bcb/raw/panel-loc.json)
<br>
[<img alt="ci errors" src="https://img.shields.io/github/actions/workflow/status/@@REPO_SLUG@@/errors.yml?branch=main&style=for-the-badge&style=flat-square&label=errors&labelColor=420d09" height="20">](https://github.com/@@REPO_SLUG@@/actions?query=branch%3Amain) <!--NB: Won't find it if repo is private-->
[<img alt="ci warnings" src="https://img.shields.io/github/actions/workflow/status/@@REPO_SLUG@@/warnings.yml?branch=main&style=for-the-badge&style=flat-square&label=warnings&labelColor=d16002" height="20">](https://github.com/@@REPO_SLUG@@/actions?query=branch%3Amain) <!--NB: Won't find it if repo is private-->

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

Not here yet: sign-in (concierge as identity provider), the UI, Telegram, the GBP and PostHog
imports. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for where things live.
<!-- markdownlint-disable -->
<details>
<summary>
<h2>Installation</h2>
</summary>

nix build

</details>
<!-- markdownlint-restore -->

## Usage
```sh
# Secrets come from the environment only: DATABASE_URL, and PANEL_DATA_KEY (64 hex characters)
# that seals PII and the sources' HMAC secrets. `panel --print-required-vars` lists what
# production needs.
export PANEL_DATA_KEY="$(panel gen-data-key)"
export DATABASE_URL=postgres://postgres@localhost:5432/service_arb_panel

# A source: its key may write events of one kind, for the brands named. The secret is printed once.
panel source add aquafix-site --kind site --brand aquafix
panel source list
panel source revoke aquafix-site

# HTTP on 127.0.0.1:59120 (migrations are applied on connect)
panel serve

# leads, calls and payments again from the journal, against the registry as it is now
panel rebuild-projections
```

## Sending events

```text
POST /api/ingest/v1/events
x-sa-key-id:    aquafix-site
x-sa-timestamp: 1790762400                       unix seconds; ±5 minutes of the panel's clock
x-sa-signature: hex(HMAC-SHA256(secret, "<x-sa-timestamp>." + <raw body>))

{"events": [ …1 to 500 sa.v1.Event, protojson… ]}
```

The answer is `207` with a verdict per event, in order: `accepted`, `duplicate` (that id is
journaled already), or `rejected` with a reason. A type the panel does not know yet is
`accepted` and stored, and projected once it is registered. `401` refuses the whole batch
(key, signature or timestamp), `400` a body that is not a batch. The contract is
[`contracts/proto/sa/v1/events.proto`](contracts/proto/sa/v1/events.proto).

## Tests

`cargo test` runs everything; the database tests need `DATABASE_URL` pointing at a Postgres
server they may `CREATE DATABASE` on (each test makes and drops its own `panel_test_*`). Without
it they are skipped — in CI too, whose runners have no Postgres yet — so run them locally:

```sh
DATABASE_URL=postgres://postgres@localhost:5432/postgres cargo test
```


<br>

<sup>
	This repository follows <a href="https://github.com/valeratrades/.github/tree/master/best_practices">my best practices</a> and <a href="https://github.com/tigerbeetle/tigerbeetle/blob/main/docs/TIGER_STYLE.md">Tiger Style</a> (except "proper capitalization for acronyms": (VsrState, not VSRState) and formatting). For project's architecture, see <a href="./docs/ARCHITECTURE.md">ARCHITECTURE.md</a>.
</sup>

#### License

<sup>
	Licensed under <a href="LICENSE">Blue Oak 1.0.0</a>
</sup>

<br>

<sub>
	Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be licensed as above, without any additional terms or conditions.
</sub>

