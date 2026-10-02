-- Drops the places' settings and their history: the sites go back to their baked config at
-- their next fetch. Irreversible but by a restore from the replica (litestream).
DROP TABLE place_changes;
DROP TABLE place_settings;
DROP TABLE places;
