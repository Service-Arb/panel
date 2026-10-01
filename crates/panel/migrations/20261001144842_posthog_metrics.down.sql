-- Drops the counts and the import's lease; the journal keeps the events they came from, and
-- `panel rebuild-projections` on a build with this migration makes them again.
SET LOCAL lock_timeout = '3s';

DROP VIEW reporting.experiment_daily;
DROP VIEW reporting.daily_location_metrics;
DROP TABLE posthog_import;
DROP TABLE daily_experiment_metrics;
DROP TABLE daily_location_metrics;
