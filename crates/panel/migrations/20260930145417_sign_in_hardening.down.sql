-- Replayed callbacks are no longer caught once this is gone; nothing else is lost.
SET LOCAL lock_timeout = '3s';

DROP INDEX sessions_by_user;
ALTER TABLE sessions DROP COLUMN rotating_until;
DROP TABLE consumed_states;
