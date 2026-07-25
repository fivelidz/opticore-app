-- Versioning + blocked-time enhancements.

-- ---------- Schema version tracking (for version-control / safe updates) ----------
CREATE TABLE IF NOT EXISTS schema_version (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version INTEGER NOT NULL,
    label VARCHAR(100),
    applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
INSERT OR IGNORE INTO schema_version (version, label) VALUES (6, 'user-mgmt + intake + messages + blocked all-day + versioning');

-- ---------- Blocked time enhancements ----------
-- all_day: 1 = blocks the entire day(s), ignores time-of-day.
-- is_recurring: 1 = repeats weekly on the same weekday(s).
ALTER TABLE blocked_times ADD COLUMN all_day INTEGER DEFAULT 0;
ALTER TABLE blocked_times ADD COLUMN is_recurring INTEGER DEFAULT 0;
