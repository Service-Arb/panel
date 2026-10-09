CREATE TABLE telegram_rules_old (
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	rule TEXT NOT NULL CHECK (rule IN ('new_lead', 'contact_overdue', 'payment_received', 'source_silent', 'booked')),
	enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
	PRIMARY KEY (user_id, rule)
) STRICT;

INSERT INTO telegram_rules_old (user_id, rule, enabled) SELECT user_id, rule, enabled FROM telegram_rules WHERE rule <> 'access_requested';
DROP TABLE telegram_rules;
ALTER TABLE telegram_rules_old RENAME TO telegram_rules;

DROP TABLE access_requests;
