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
