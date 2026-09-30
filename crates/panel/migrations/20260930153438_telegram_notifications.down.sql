-- Unlinks every chat and drops what the outbox still owed; the users link again.
SET LOCAL lock_timeout = '3s';

DROP TABLE telegram_poller;
DROP TABLE telegram_outbox;
DROP TABLE telegram_fanout;
DROP TABLE telegram_rules;
DROP TABLE telegram_links;
DROP TABLE telegram_link_tokens;
