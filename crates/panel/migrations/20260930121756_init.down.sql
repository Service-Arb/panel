-- Drops everything, the journal included: only for a database that holds nothing worth
-- keeping. On one that does, the way back is a restore from backup.
DROP SCHEMA reporting CASCADE;
DROP TABLE payments;
DROP TABLE calls;
DROP TABLE leads;
DROP TABLE events;
DROP FUNCTION events_append_only();
DROP TABLE sources;
