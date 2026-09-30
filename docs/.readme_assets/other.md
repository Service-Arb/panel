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
