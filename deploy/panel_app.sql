-- What the runtime role may do: `panel serve`, `panel source …`, `panel rebuild-projections`.
-- The schema belongs to the role `panel migrate` connects as (MIGRATE_DATABASE_URL); this
-- one (DATABASE_URL) may not change it. Both roles are created by the deploy, not here.
-- Run as the schema's owner after every migration that adds a table or a view; each grant
-- is idempotent. The crate's tests apply this very file (tests/grants.rs).

GRANT USAGE ON SCHEMA public, reporting TO panel_app;

-- `Store::connect` checks that every migration of the build is applied.
GRANT SELECT ON _sqlx_migrations TO panel_app;

-- The journal: appended to, and re-judged by the rebuild. Never deleted from.
GRANT SELECT, INSERT ON events TO panel_app;
GRANT UPDATE (status, status_reason) ON events TO panel_app;

-- The projections are derived; the rebuild empties them.
GRANT SELECT, INSERT, UPDATE, DELETE, TRUNCATE ON leads, calls, payments TO panel_app;

-- Sources are added and revoked, never edited otherwise or deleted.
GRANT SELECT, INSERT ON sources TO panel_app;
GRANT UPDATE (revoked_at) ON sources TO panel_app;

GRANT SELECT ON ALL TABLES IN SCHEMA reporting TO panel_app;

-- Sign-in sessions: opened at the callback, their tokens rotated, closed at sign-out or expiry.
GRANT SELECT, INSERT, UPDATE, DELETE ON sessions TO panel_app;

-- Callback states, redeemed once; the expired ones are dropped on the way.
GRANT SELECT, INSERT, DELETE ON consumed_states TO panel_app;

-- Telegram: link tokens redeemed once, links made and dropped, the users' rules, the fan-out
-- marks and the outbox (pruned once delivered), the poller's lease and offset.
GRANT SELECT, INSERT, DELETE ON telegram_link_tokens TO panel_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON telegram_links, telegram_rules, telegram_outbox TO panel_app;
GRANT SELECT, INSERT, DELETE ON telegram_fanout TO panel_app;
GRANT SELECT, UPDATE ON telegram_poller TO panel_app;
