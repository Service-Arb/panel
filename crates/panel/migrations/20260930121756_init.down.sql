-- Drops everything, the journal included: only for a database that holds nothing worth
-- keeping. On one that does, the way back is a restore from the replica (litestream).
DROP VIEW reporting_experiment_daily;
DROP VIEW reporting_daily_location_metrics;
DROP VIEW reporting_ingest_daily;
DROP VIEW reporting_funnel_daily;
DROP VIEW reporting_payments;
DROP VIEW reporting_calls;
DROP VIEW reporting_leads;
DROP TABLE posthog_import;
DROP TABLE telegram_replies;
DROP TABLE telegram_poller;
DROP TABLE telegram_outbox;
DROP TABLE telegram_fanout;
DROP TABLE telegram_rules;
DROP TABLE telegram_links;
DROP TABLE telegram_link_tokens;
DROP TABLE consumed_states;
DROP TABLE sessions;
DROP TABLE daily_experiment_metrics;
DROP TABLE daily_location_metrics;
DROP TABLE payments;
DROP TABLE calls;
DROP TABLE leads;
-- DROP TABLE removes the table's triggers with it; the DELETE trigger does not fire on a drop.
DROP TABLE events;
DROP TABLE sources;
