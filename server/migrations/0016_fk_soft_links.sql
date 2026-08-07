-- Add the two remaining missing FOREIGN KEY declarations on the "soft-link"
-- patient-reference columns, closing the defense-in-depth gap documented in
-- tests/referential_integrity.rs:
--
--   * messages.linked_patient_id            -> patients(id) ON DELETE SET NULL
--   * intake_submissions.matched_patient_id -> patients(id) ON DELETE SET NULL
--
-- Both columns already existed as plain nullable INTEGER columns with no
-- constraint (added in 0005_messages.sql and 0004_intake.sql respectively).
-- The handler routes that write them guard the patient-existence check at
-- the application layer (messages::link_patient, intake::import_*), so under
-- normal use no orphan is created. But the schema itself provided no
-- defense-in-depth: a future code path, a bulk import, or a direct DB write
-- could silently store a dangling reference, and deleting a patient left the
-- soft-link pointing at a row that no longer existed.
--
-- Action: ON DELETE SET NULL. A message / intake submission is an incoming
-- communication that may still need processing even after the patient it was
-- matched to is deleted; we must NOT lose it (ON DELETE CASCADE would), we
-- just clear the now-dangling pointer. This matches the policy chosen in
-- migration 0014 for invoices.appointment_id and patient_photos.appointment_id.
--
-- SQLite cannot add a FK constraint to an existing column in place. The only
-- supported method is the documented table rebuild (create _new with the FK,
-- copy data, drop old, rename _new into place, recreate indexes). Both
-- `messages` and `intake_submissions` are LEAF tables (nothing references
-- them via a declared FK), so the rebuild is the simple variant — no
-- FK-retargeting / rename-ordering hazard like migration 0015 had to handle
-- for the patients->appointments->invoices chain.
--
-- Before the copy we NULL OUT any pre-existing orphaned references, so the
-- new FK is satisfiable on databases that accumulated dangling rows while the
-- constraint was absent. This does NOT delete any message or intake row — it
-- only clears the dangling pointer value (the row itself is preserved, per
-- the project's never-delete-without-explicit-request rule). On a fresh DB
-- these UPDATEs are no-ops.
--
-- This migration is idempotent-safe: it only ever runs once (sqlx tracks
-- applied migrations) and reproduces every column, default, CHECK, and index
-- exactly from the latest prior migration that defined each table.

-- =====================================================================
-- 1) messages.linked_patient_id  -> patients(id) ON DELETE SET NULL
-- =====================================================================
-- original: 0005_messages.sql (no later ALTERs; not rebuilt by 0015)

-- 1a. Null out orphaned references from the FK-free era (no-op on fresh DB).
UPDATE messages SET linked_patient_id = NULL
 WHERE linked_patient_id IS NOT NULL
   AND linked_patient_id NOT IN (SELECT id FROM patients);

-- 1b. Rebuild the table with the FK added. Column definitions reproduced
--     EXACTLY from 0005_messages.sql with the single addition of the
--     linked_patient_id FOREIGN KEY ... ON DELETE SET NULL clause.
CREATE TABLE messages_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    received_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    channel VARCHAR(20) NOT NULL,
    from_name VARCHAR(200),
    from_contact VARCHAR(200),
    subject VARCHAR(300),
    body TEXT NOT NULL,
    status VARCHAR(20) DEFAULT 'unread',
    linked_patient_id INTEGER,
    thread_id VARCHAR(60),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (linked_patient_id) REFERENCES patients(id) ON DELETE SET NULL
);

-- 1c. Copy all rows across (explicit column list so a future column add
--     can't silently shift positions).
INSERT INTO messages_new (
    id, received_at, channel, from_name, from_contact, subject, body,
    status, linked_patient_id, thread_id, created_at
)
SELECT
    id, received_at, channel, from_name, from_contact, subject, body,
    status, linked_patient_id, thread_id, created_at
FROM messages;

-- 1d. Swap: drop old, rename new, recreate the three indexes.
DROP TABLE messages;
ALTER TABLE messages_new RENAME TO messages;
CREATE INDEX IF NOT EXISTS idx_messages_status ON messages(status);
CREATE INDEX IF NOT EXISTS idx_messages_channel ON messages(channel);
CREATE INDEX IF NOT EXISTS idx_messages_received ON messages(received_at);

-- Preserve the AUTOINCREMENT sequence across the rebuild.
INSERT OR REPLACE INTO sqlite_sequence (name, seq)
SELECT 'messages', COALESCE(MAX(id), 0) FROM messages;

-- =====================================================================
-- 2) intake_submissions.matched_patient_id  -> patients(id) ON DELETE SET NULL
-- =====================================================================
-- original: 0004_intake.sql + ALTERs in 0010_intake_matching.sql + CHECK
-- constraints added in 0015_check_constraints.sql. Reproduced EXACTLY from
-- the 0015 rebuild (the canonical current definition) with the single
-- addition of the matched_patient_id FOREIGN KEY ... ON DELETE SET NULL.

-- 2a. Null out orphaned references from the FK-free era (no-op on fresh DB).
UPDATE intake_submissions SET matched_patient_id = NULL
 WHERE matched_patient_id IS NOT NULL
   AND matched_patient_id NOT IN (SELECT id FROM patients);

-- 2b. Rebuild with the FK. Column definitions + CHECKs reproduced EXACTLY
--     from 0015_check_constraints.sql (Phase F) with the new FK appended.
CREATE TABLE intake_submissions_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    submitted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    first_name VARCHAR(100) NOT NULL CHECK (length(trim(first_name, char(9)||char(10)||char(13)||' ')) > 0),
    last_name VARCHAR(100) NOT NULL CHECK (length(trim(last_name, char(9)||char(10)||char(13)||' ')) > 0),
    date_of_birth DATE,
    phone VARCHAR(30),
    email VARCHAR(100),
    address TEXT,
    medicare_number VARCHAR(30),
    preferred_date DATE,
    preferred_time VARCHAR(30),
    appointment_type VARCHAR(50),
    symptoms TEXT,
    source VARCHAR(30) DEFAULT 'input-page',
    status VARCHAR(20) DEFAULT 'new',
    matched_patient_id INTEGER,
    claimed_returning INTEGER,
    claimed_no_match INTEGER DEFAULT 0,
    FOREIGN KEY (matched_patient_id) REFERENCES patients(id) ON DELETE SET NULL
);

-- 2c. Copy.
INSERT INTO intake_submissions_new (
    id, submitted_at, first_name, last_name, date_of_birth, phone, email,
    address, medicare_number, preferred_date, preferred_time,
    appointment_type, symptoms, source, status, matched_patient_id,
    claimed_returning, claimed_no_match
)
SELECT
    id, submitted_at, first_name, last_name, date_of_birth, phone, email,
    address, medicare_number, preferred_date, preferred_time,
    appointment_type, symptoms, source, status, matched_patient_id,
    claimed_returning, claimed_no_match
FROM intake_submissions;

-- 2d. Swap + recreate the three indexes.
DROP TABLE intake_submissions;
ALTER TABLE intake_submissions_new RENAME TO intake_submissions;
CREATE INDEX IF NOT EXISTS idx_intake_status ON intake_submissions(status);
CREATE INDEX IF NOT EXISTS idx_intake_date ON intake_submissions(submitted_at);
CREATE INDEX IF NOT EXISTS idx_intake_claimed_no_match ON intake_submissions(claimed_no_match);

-- Preserve the AUTOINCREMENT sequence for intake_submissions too.
INSERT OR REPLACE INTO sqlite_sequence (name, seq)
SELECT 'intake_submissions', COALESCE(MAX(id), 0) FROM intake_submissions;
