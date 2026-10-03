-- The hourly PostHog import is gone (owner, 2026-10-04: what PostHog shows, the panel does not
-- count again). Its projections, their reporting views and its lease go with it.
--
-- Nothing is lost that the journal does not keep: the counts were projected from the
-- `site.metrics`, `contact.metrics` and `experiment.metrics` events, which stay in `events`
-- and still pass the registry (as counts nothing is projected from). Going back to a build
-- that reads these tables: apply the `down` (it makes them again, empty), then
-- `panel rebuild-projections` on that build fills them from the journal.

DROP VIEW reporting_experiment_daily;
DROP VIEW reporting_daily_location_metrics;
DROP TABLE daily_experiment_metrics;
DROP TABLE daily_location_metrics;
DROP TABLE posthog_import;
