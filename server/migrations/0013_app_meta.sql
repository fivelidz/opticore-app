-- Small key/value table for app-level bookkeeping.
-- Used to record one-time events like 'clean_started' so that a production
-- launcher which always sets CLEAN_START only wipes seed data ONCE (on the very
-- first boot of an empty database) and never deletes real data afterwards.

CREATE TABLE IF NOT EXISTS app_meta (
    key   TEXT PRIMARY KEY,
    value TEXT
);
