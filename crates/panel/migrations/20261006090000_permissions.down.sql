-- Back to roles: whoever manages the sources is an admin, else whoever edits leads an
-- operator; a link holding neither has no role, a token holding neither is dropped.

CREATE TABLE telegram_link_tokens_old (
	token_hash BLOB PRIMARY KEY CHECK (length(token_hash) = 32),
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	role TEXT NOT NULL CHECK (role IN ('operator', 'admin')),
	display_name TEXT NOT NULL,
	expires_at INTEGER NOT NULL,
	issued_at INTEGER
) STRICT;

INSERT INTO telegram_link_tokens_old (token_hash, user_id, role, display_name, expires_at, issued_at)
SELECT token_hash, user_id, role, display_name, expires_at, issued_at FROM (
	SELECT *, CASE
		WHEN EXISTS (SELECT 1 FROM json_each(permissions) WHERE value = 'sa:admin:sources:manage') THEN 'admin'
		WHEN EXISTS (SELECT 1 FROM json_each(permissions) WHERE value = 'sa:work:leads:edit') THEN 'operator'
	END AS role
	FROM telegram_link_tokens
) WHERE role IS NOT NULL;
DROP TABLE telegram_link_tokens;
ALTER TABLE telegram_link_tokens_old RENAME TO telegram_link_tokens;
CREATE INDEX telegram_link_tokens_by_expiry ON telegram_link_tokens (expires_at);

CREATE TABLE telegram_links_old (
	user_id BLOB PRIMARY KEY CHECK (length(user_id) = 16),
	chat_id INTEGER NOT NULL UNIQUE,
	linked_at INTEGER NOT NULL,
	role TEXT CHECK (role IN ('operator', 'admin')),
	role_checked_at INTEGER NOT NULL,
	access_tried_at INTEGER,
	display_name TEXT NOT NULL,
	dead_at INTEGER,
	tg_username TEXT,
	tg_first_name TEXT
) STRICT;

INSERT INTO telegram_links_old (user_id, chat_id, linked_at, role, role_checked_at, access_tried_at, display_name, dead_at, tg_username, tg_first_name)
SELECT user_id, chat_id, linked_at,
	CASE
		WHEN EXISTS (SELECT 1 FROM json_each(permissions) WHERE value = 'sa:admin:sources:manage') THEN 'admin'
		WHEN EXISTS (SELECT 1 FROM json_each(permissions) WHERE value = 'sa:work:leads:edit') THEN 'operator'
	END,
	permissions_checked_at, access_tried_at, display_name, dead_at, tg_username, tg_first_name
FROM telegram_links;
DROP TABLE telegram_links;
ALTER TABLE telegram_links_old RENAME TO telegram_links;
