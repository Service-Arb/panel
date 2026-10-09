-- A signed-in user asking the admins for a permission or an alias of the `sa` catalog: one row per
-- (user, need), refreshed at most once a day, dropped once concierge shows the need held. The
-- email and name are concierge's at the request, for the admins' message.
CREATE TABLE access_requests (
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	need TEXT NOT NULL,
	email TEXT NOT NULL,
	name TEXT NOT NULL,
	requested_at INTEGER NOT NULL,
	-- What the admins' message fans out under; a new one each time the request is refreshed.
	event_id BLOB NOT NULL UNIQUE CHECK (length(event_id) = 16),
	PRIMARY KEY (user_id, need)
) STRICT;

CREATE INDEX access_requests_by_time ON access_requests (requested_at);

-- The rule `access_requested`. SQLite cannot alter a CHECK: the table is made again with the
-- wider one. Nothing references it.
CREATE TABLE telegram_rules_new (
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	rule TEXT NOT NULL CHECK (rule IN ('new_lead', 'contact_overdue', 'payment_received', 'source_silent', 'booked', 'access_requested')),
	enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
	PRIMARY KEY (user_id, rule)
) STRICT;

INSERT INTO telegram_rules_new (user_id, rule, enabled) SELECT user_id, rule, enabled FROM telegram_rules;
DROP TABLE telegram_rules;
ALTER TABLE telegram_rules_new RENAME TO telegram_rules;
