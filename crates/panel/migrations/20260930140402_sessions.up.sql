-- Signed-in panel users (spec §4): one row per browser session, opened by the concierge code
-- flow. A new table only, so the build before this one runs on it unchanged.
SET LOCAL lock_timeout = '3s';

CREATE TABLE sessions (
	-- SHA-256 of the session cookie's value: a dump does not hand out sessions.
	id_hash bytea PRIMARY KEY CHECK (octet_length(id_hash) = 32),
	-- The concierge user id, the tokens' `sub`.
	user_id uuid NOT NULL,
	-- The relying party's access and refresh tokens, sealed under PANEL_DATA_KEY
	-- (XChaCha20-Poly1305, the row's id_hash bound as associated data).
	access_sealed bytea NOT NULL,
	access_expires_at timestamptz NOT NULL,
	refresh_sealed bytea NOT NULL,
	data_key_fp bytea NOT NULL CHECK (octet_length(data_key_fp) = 32),
	created_at timestamptz NOT NULL DEFAULT now(),
	-- The refresh family's deadline: past it the session is over, whatever the cookie says.
	expires_at timestamptz NOT NULL
);

CREATE INDEX sessions_by_expiry ON sessions (expires_at);
