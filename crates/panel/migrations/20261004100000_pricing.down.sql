-- Drops the brands' pricing, its history and their locales: the sites go back to their baked
-- models at their next fetch. Irreversible but by a restore from the replica (litestream).
DROP TABLE pricing_changes;
DROP TABLE pricing;
DROP TABLE brand_locales;
