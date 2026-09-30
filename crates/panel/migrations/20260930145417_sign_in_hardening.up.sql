-- Sign-in hardening: a state is redeemed once, a session's rotation is leased across
-- replicas, and a user's sessions are found to close them all. A new table, a nullable
-- column and an index on a small table: the build before this one runs on it unchanged.
SET LOCAL lock_timeout = '3s';

-- The `state` of every callback that got as far as presenting its code, as SHA-256: a
-- replayed callback (same pre-login cookie, same state) is refused before concierge is
-- asked. Rows are dropped past `expires_at`, when the pre-login they match has expired too.
CREATE TABLE consumed_states (
	state_hash bytea PRIMARY KEY CHECK (octet_length(state_hash) = 32),
	expires_at timestamptz NOT NULL
);

CREATE INDEX consumed_states_by_expiry ON consumed_states (expires_at);

-- Whoever set this (and it is in the future) is rotating the session's tokens: the others
-- wait for the result instead of presenting the same refresh token a second time.
ALTER TABLE sessions ADD COLUMN rotating_until timestamptz;

-- Sign-out closes every session of the user.
CREATE INDEX sessions_by_user ON sessions (user_id);
