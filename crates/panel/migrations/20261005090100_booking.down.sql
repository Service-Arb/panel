-- Back to before booking: the projections and the sync state dropped (the journal keeps every
-- booking event; the build this goes back to does not know their types and judges them
-- unregistered at its next rebuild), and the rule `booked` forgotten with the users' choices
-- of it.

CREATE TABLE telegram_rules_old (
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	rule TEXT NOT NULL CHECK (rule IN ('new_lead', 'contact_overdue', 'payment_received', 'source_silent')),
	enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
	PRIMARY KEY (user_id, rule)
) STRICT;

INSERT INTO telegram_rules_old (user_id, rule, enabled) SELECT user_id, rule, enabled FROM telegram_rules WHERE rule <> 'booked';
DROP TABLE telegram_rules;
ALTER TABLE telegram_rules_old RENAME TO telegram_rules;

DROP TABLE booking_sync;
DROP TABLE bookings;
DROP TABLE booking_events;

DROP INDEX leads_by_booking;
ALTER TABLE leads DROP COLUMN booking_preferred_part;
ALTER TABLE leads DROP COLUMN booking_preferred_date;
ALTER TABLE leads DROP COLUMN booking_match;
ALTER TABLE leads DROP COLUMN booking_external_ref;
ALTER TABLE leads DROP COLUMN booking_end_at;
ALTER TABLE leads DROP COLUMN booking_start_at;
ALTER TABLE leads DROP COLUMN booking_provider;
ALTER TABLE leads DROP COLUMN booking_status;
