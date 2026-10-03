-- Booking (FORM-VARIANTS-SPEC, "Booking providers contract"): a lead's booking on its row, the
-- providers' bookings and their events, the pull adapters' sync state, and the Telegram rule
-- `booked`. Conventions as in the init. Everything here but `booking_sync` is a projection:
-- `panel rebuild-projections` makes it again from the journal.

-- A lead's booking, folded from its booking facts and the providers' bookings joined to it
-- (`panel_core::booking::fold`). NULL status: none.
ALTER TABLE leads ADD COLUMN booking_status TEXT CHECK (booking_status IN ('requested', 'booked', 'canceled', 'done', 'no_show'));
ALTER TABLE leads ADD COLUMN booking_provider TEXT CHECK (booking_provider IN ('manual', 'link', 'google_calendar', 'cal_com'));
ALTER TABLE leads ADD COLUMN booking_start_at INTEGER;
ALTER TABLE leads ADD COLUMN booking_end_at INTEGER;
ALTER TABLE leads ADD COLUMN booking_external_ref TEXT;
ALTER TABLE leads ADD COLUMN booking_match TEXT CHECK (booking_match IN ('ref', 'contact', 'manual'));
ALTER TABLE leads ADD COLUMN booking_preferred_date TEXT CHECK (date(booking_preferred_date) IS booking_preferred_date);
ALTER TABLE leads ADD COLUMN booking_preferred_part TEXT CHECK (booking_preferred_part IN ('morning', 'afternoon', 'evening'));

CREATE INDEX leads_by_booking ON leads (booking_status) WHERE booking_status IS NOT NULL;

-- One row per registered booking event: what a provider's booking is folded from (by its
-- key), and what the Telegram rule `booked` finds.
CREATE TABLE booking_events (
	event_id BLOB PRIMARY KEY REFERENCES events (id),
	brand_id TEXT NOT NULL,
	kind TEXT NOT NULL CHECK (kind IN ('requested', 'created', 'canceled', 'set', 'status_changed', 'cleared', 'attached')),
	provider TEXT CHECK (provider IN ('manual', 'link', 'google_calendar', 'cal_com')),
	-- A provider's booking: set exactly for its own events and an attachment.
	external_ref TEXT,
	-- The lead the event names (a provider's: the one it was matched to as it arrived).
	lead_id TEXT,
	-- `status_changed`: done | no_show | canceled.
	status TEXT CHECK (status IN ('done', 'no_show', 'canceled')),
	-- `created` and `set`: the slot's start.
	start_at INTEGER,
	occurred_at INTEGER NOT NULL,
	CHECK ((external_ref IS NOT NULL) = (kind IN ('created', 'canceled', 'attached'))),
	CHECK ((status IS NOT NULL) = (kind = 'status_changed'))
) STRICT;

CREATE INDEX booking_events_by_booking ON booking_events (brand_id, provider, external_ref) WHERE external_ref IS NOT NULL;

-- A provider's booking as it stands: its slot (status booked) or none (canceled), and the
-- lead it is joined to — NULL while unmatched ("bookings without a lead").
CREATE TABLE bookings (
	-- Derived from (brand, provider, external_ref): what the API names it by.
	id BLOB PRIMARY KEY CHECK (length(id) = 16),
	brand_id TEXT NOT NULL,
	provider TEXT NOT NULL CHECK (provider IN ('google_calendar', 'cal_com')),
	external_ref TEXT NOT NULL,
	lead_id TEXT,
	match TEXT CHECK (match IN ('ref', 'contact', 'manual')),
	status TEXT NOT NULL CHECK (status IN ('booked', 'canceled')),
	-- The latest slot, kept once canceled.
	start_at INTEGER NOT NULL,
	end_at INTEGER,
	-- When the customer booked, as the provider says.
	booked_at INTEGER,
	-- The latest booking.created: the attendee's sealed PII is its.
	created_event_id BLOB NOT NULL REFERENCES events (id),
	first_event_id BLOB NOT NULL REFERENCES events (id),
	last_event_id BLOB NOT NULL REFERENCES events (id),
	last_event_at INTEGER NOT NULL,
	UNIQUE (brand_id, provider, external_ref),
	CHECK ((lead_id IS NULL) = (match IS NULL))
) STRICT;

CREATE INDEX bookings_unmatched ON bookings (brand_id, start_at) WHERE lead_id IS NULL;
CREATE INDEX bookings_by_lead ON bookings (brand_id, lead_id) WHERE lead_id IS NOT NULL;

-- A pull adapter's place in a brand's calendar, and who is pulling it: a lease another
-- process takes over when its holder is gone, as the PostHog import's. Not a projection.
CREATE TABLE booking_sync (
	provider TEXT NOT NULL CHECK (provider IN ('google_calendar', 'cal_com')),
	brand_id TEXT NOT NULL,
	-- The provider's cursor (Google's nextSyncToken); NULL: the next pull is a full one.
	cursor TEXT,
	holder BLOB,
	leased_until INTEGER,
	-- The last try, and the last pull that finished.
	attempted_at INTEGER,
	synced_at INTEGER,
	PRIMARY KEY (provider, brand_id)
) STRICT;

-- The rule `booked`. SQLite cannot alter a CHECK: the table is made again with the wider one.
-- Nothing references it.
CREATE TABLE telegram_rules_new (
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	rule TEXT NOT NULL CHECK (rule IN ('new_lead', 'contact_overdue', 'payment_received', 'source_silent', 'booked')),
	enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
	PRIMARY KEY (user_id, rule)
) STRICT;

INSERT INTO telegram_rules_new (user_id, rule, enabled) SELECT user_id, rule, enabled FROM telegram_rules;
DROP TABLE telegram_rules;
ALTER TABLE telegram_rules_new RENAME TO telegram_rules;
