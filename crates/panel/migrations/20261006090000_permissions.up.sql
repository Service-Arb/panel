-- A Telegram link carries the concrete `sa` permissions concierge confirmed, not a role.
-- `operator` and `admin` become what `sa:operator` and `sa:admin` held when this was written
-- (sa_auth's aliases), sorted as the panel serializes a set. SQLite cannot alter a CHECK:
-- both tables are made again. Nothing references them.

CREATE TABLE telegram_link_tokens_new (
	token_hash BLOB PRIMARY KEY CHECK (length(token_hash) = 32),
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	permissions TEXT NOT NULL CHECK (json_valid(permissions) AND json_type(permissions) = 'array'),
	display_name TEXT NOT NULL,
	expires_at INTEGER NOT NULL,
	issued_at INTEGER
) STRICT;

INSERT INTO telegram_link_tokens_new (token_hash, user_id, permissions, display_name, expires_at, issued_at)
SELECT token_hash, user_id,
	CASE role
		WHEN 'operator' THEN '["sa:analysis:read","sa:work:leads:edit","sa:work:pii:see","sa:work:read"]'
		WHEN 'admin' THEN '["sa:admin:sources:manage","sa:analysis:experiments:edit","sa:analysis:read","sa:playbook:mcp:use","sa:review_archive:archive:operate","sa:review_archive:members:act_as","sa:review_archive:tokens:grant","sa:work:leads:edit","sa:work:pii:see","sa:work:places:edit","sa:work:pricing:edit","sa:work:read"]'
	END,
	display_name, expires_at, issued_at
FROM telegram_link_tokens;
DROP TABLE telegram_link_tokens;
ALTER TABLE telegram_link_tokens_new RENAME TO telegram_link_tokens;
CREATE INDEX telegram_link_tokens_by_expiry ON telegram_link_tokens (expires_at);

CREATE TABLE telegram_links_new (
	user_id BLOB PRIMARY KEY CHECK (length(user_id) = 16),
	chat_id INTEGER NOT NULL UNIQUE,
	linked_at INTEGER NOT NULL,
	-- What concierge last confirmed the user holds, and when; NULL once their access is known
	-- lost. Nothing is sent on a confirmation older than the engine's ACCESS_TTL.
	permissions TEXT CHECK (json_valid(permissions) AND json_type(permissions) = 'array'),
	permissions_checked_at INTEGER NOT NULL,
	access_tried_at INTEGER,
	display_name TEXT NOT NULL,
	dead_at INTEGER,
	tg_username TEXT,
	tg_first_name TEXT
) STRICT;

INSERT INTO telegram_links_new (user_id, chat_id, linked_at, permissions, permissions_checked_at, access_tried_at, display_name, dead_at, tg_username, tg_first_name)
SELECT user_id, chat_id, linked_at,
	CASE role
		WHEN 'operator' THEN '["sa:analysis:read","sa:work:leads:edit","sa:work:pii:see","sa:work:read"]'
		WHEN 'admin' THEN '["sa:admin:sources:manage","sa:analysis:experiments:edit","sa:analysis:read","sa:playbook:mcp:use","sa:review_archive:archive:operate","sa:review_archive:members:act_as","sa:review_archive:tokens:grant","sa:work:leads:edit","sa:work:pii:see","sa:work:places:edit","sa:work:pricing:edit","sa:work:read"]'
	END,
	role_checked_at, access_tried_at, display_name, dead_at, tg_username, tg_first_name
FROM telegram_links;
DROP TABLE telegram_links;
ALTER TABLE telegram_links_new RENAME TO telegram_links;
