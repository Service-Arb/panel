-- What was still owed to PostHog is dropped with the table: those events stay in the journal,
-- and are not sent again on the way forward (the outbox is filled only as events arrive).

DROP TABLE posthog_outbox;
