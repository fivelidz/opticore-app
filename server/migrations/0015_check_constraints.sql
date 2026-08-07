-- Defense-in-depth: mirror the handler-level input validations (added in
-- commits 297c6ec, e586101, 1491ebe, 3620dee) as CHECK constraints at the
-- database layer. A handler bug, a future code path that bypasses validation,
-- or a bulk import (data_io) would otherwise let semantically-invalid rows
-- into the DB. With these CHECKs in place, the DB itself rejects the row and
-- the error mapper (commit 44a8e60) surfaces it as a 400 Bad Request.
--
-- Constraints added (mirroring the handler rules):
--   appointments.duration_minutes      >= 1
--   blocked_times:                     start_at < end_at
--   clinical_notes.note                non-empty after whitespace trim
--   allergies.substance                non-empty after whitespace trim
--   osdi_scores.total_score            >= 0
--   ipl_treatments.session_number      >= 1
--   booking_settings.reminder_hours_before >= 0
--   booking_settings.booking_mode      IN ('automatic','approval')
--   patients.first_name / last_name    non-empty after whitespace trim
--   intake_submissions.first_name/last_name non-empty after whitespace trim
--
-- NOTE on whitespace: the handlers use Rust's str::trim(), which strips ALL
-- Unicode whitespace (spaces, tabs, newlines, etc.). SQLite's bare trim(x)
-- only strips spaces. To mirror the handler exactly we use
-- trim(x, char(9)||char(10)||char(13)||' ') which strips tab, LF, CR, and
-- space — the four characters that appear in practice in these text fields.
-- (A field containing only exotic Unicode whitespace like U+00A0 would slip
-- past this CHECK; that is an acceptable edge — the handler still catches it.)
--
-- =====================================================================
-- WHY THE PHASE ORDERING MATTERS (SQLite FK rename-retarget hazard)
-- =====================================================================
-- SQLite cannot add a CHECK constraint to an existing table in place — the
-- only supported method is the documented table rebuild. Two SQLite behaviours
-- make a naive rebuild catastrophically unsafe here:
--
--   1. CASCADE-on-DROP: with `PRAGMA foreign_keys = ON` (which this app
--      enables globally), `DROP TABLE patients` CASCADE-deletes every row in
--      the 7 child tables that reference patients with ON DELETE CASCADE
--      (appointments, clinical_notes, allergies, osdi_scores, ipl_treatments,
--      invoices, patient_photos). Silent mass data loss.
--   2. Rename-retarget: `ALTER TABLE patients RENAME TO patients_old`
--      REWRITES every incoming FK in the schema (including in not-yet-swapped
--      _new tables) to point at `patients_old`. So a _new child created BEFORE
--      the parent rename ends up referencing the _old shadow.
--
-- sqlx wraps each migration in a TRANSACTION, and `PRAGMA foreign_keys` cannot
-- be changed inside a transaction — so we cannot disable FK enforcement.
--
-- The correct in-transaction technique (used below) works around both traps.
-- The rule: rename each parent BEFORE creating/rebuilding its children, so
-- each child's FK is defined fresh against the already-canonical-named new
-- parent. Parents are renamed top-down (patients first, then appointments).
-- Old shadows are dropped LAST, after every child has been rebuilt.
--
--   Phase A — pre-clean bad rows (so the copies succeed).
--   Phase B — create patients_new (root parent) + copy.
--   Phase C — rename patients: original -> _old, _new -> original.
--   Phase D — rebuild appointments (child of patients; parent of invoices/
--             patient_photos): create _new against new patients, copy, rename
--             original -> _old, _new -> original.
--   Phase E — rebuild the remaining children (invoices, patient_photos,
--             invoice_items, payments, clinical_notes, allergies, osdi_scores,
--             ipl_treatments) against the now-canonical new parents.
--   Phase F — rebuild the FK-free tables (blocked_times, booking_settings,
--             intake_submissions) with the simple create-copy-drop-rename.
--   Phase G — drop the _old shadows (safe: no live FK references them).
--   Phase H — recreate indexes + preserve AUTOINCREMENT sequences.
--
-- Every column definition, default, NOT NULL, FK, and CHECK is reproduced
-- EXACTLY from the original migrations (cited per table). The ONLY changes
-- are the new CHECK clauses on the 9 validated tables.
--
-- This migration is idempotent-safe: it only ever runs once (sqlx tracks
-- applied migrations). On a fresh DB the pre-clean UPDATEs are no-ops.

-- =====================================================================
-- Phase A — pre-clean: coerce any rows that would violate the new CHECKs so
-- the copy step succeeds on databases that accumulated bad data before the
-- handler guards existed. (On a fresh/seeded DB all rows are already valid,
-- so every statement here is a no-op.)
-- =====================================================================

UPDATE appointments SET duration_minutes = 1 WHERE duration_minutes < 1;
DELETE FROM blocked_times WHERE start_at >= end_at;
UPDATE clinical_notes SET note = '[imported blank note]'
 WHERE length(trim(note, char(9)||char(10)||char(13)||' ')) = 0;
UPDATE allergies SET substance = '[unknown]'
 WHERE length(trim(substance, char(9)||char(10)||char(13)||' ')) = 0;
UPDATE osdi_scores SET total_score = 0 WHERE total_score < 0;
UPDATE ipl_treatments SET session_number = 1 WHERE session_number < 1;
UPDATE booking_settings SET reminder_hours_before = 0 WHERE reminder_hours_before < 0;
UPDATE booking_settings SET booking_mode = 'approval'
 WHERE booking_mode NOT IN ('automatic', 'approval');
UPDATE patients SET first_name = 'Unknown'
 WHERE length(trim(first_name, char(9)||char(10)||char(13)||' ')) = 0;
UPDATE patients SET last_name  = 'Unknown'
 WHERE length(trim(last_name, char(9)||char(10)||char(13)||' ')) = 0;
UPDATE intake_submissions SET first_name = 'Unknown'
 WHERE length(trim(first_name, char(9)||char(10)||char(13)||' ')) = 0;
UPDATE intake_submissions SET last_name  = 'Unknown'
 WHERE length(trim(last_name, char(9)||char(10)||char(13)||' ')) = 0;

-- =====================================================================
-- Phase B — create patients_new (root parent) + copy.
-- original: 0001_init.sql + ALTERs in 0007_photos.sql, 0009_sms_fixes.sql
-- =====================================================================

CREATE TABLE patients_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mrn VARCHAR(20) UNIQUE NOT NULL,
    first_name VARCHAR(100) NOT NULL CHECK (length(trim(first_name, char(9)||char(10)||char(13)||' ')) > 0),
    last_name VARCHAR(100) NOT NULL CHECK (length(trim(last_name, char(9)||char(10)||char(13)||' ')) > 0),
    date_of_birth DATE NOT NULL,
    gender VARCHAR(20),
    phone VARCHAR(20),
    email VARCHAR(100),
    address TEXT,
    medicare_number VARCHAR(20),
    profile_photo_id INTEGER,
    sms_opt_out INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO patients_new (
    id, mrn, first_name, last_name, date_of_birth, gender, phone, email,
    address, medicare_number, profile_photo_id, sms_opt_out, created_at, updated_at
)
SELECT
    id, mrn, first_name, last_name, date_of_birth, gender, phone, email,
    address, medicare_number, profile_photo_id, sms_opt_out, created_at, updated_at
FROM patients;

-- =====================================================================
-- Phase C — rename patients: original -> _old, _new -> original.
-- After this the new patients holds the canonical name. The still-original
-- child tables' FKs transiently point at patients_old; Phase D-E rebuilds
-- each child so its FK references the new patients.
-- =====================================================================

ALTER TABLE patients RENAME TO patients_old;
ALTER TABLE patients_new RENAME TO patients;

-- =====================================================================
-- Phase D — rebuild appointments (child of patients; parent of invoices/
-- patient_photos). Created against the new patients; then renamed into place.
-- original: 0001_init.sql
-- =====================================================================

CREATE TABLE appointments_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    appointment_type VARCHAR(50) NOT NULL,
    appointment_date DATETIME NOT NULL,
    duration_minutes INTEGER NOT NULL CHECK (duration_minutes >= 1),
    practitioner VARCHAR(100),
    status VARCHAR(20) DEFAULT 'scheduled',
    notes TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);
INSERT INTO appointments_new (
    id, patient_id, appointment_type, appointment_date, duration_minutes,
    practitioner, status, notes, created_at
)
SELECT
    id, patient_id, appointment_type, appointment_date, duration_minutes,
    practitioner, status, notes, created_at
FROM appointments;
ALTER TABLE appointments RENAME TO appointments_old;
ALTER TABLE appointments_new RENAME TO appointments;

-- =====================================================================
-- Phase E — rebuild the remaining children. Each _new table is created AFTER
-- its parent(s) were renamed, so its FK references the canonical-named new
-- parent. Order: invoices/patient_photos (depend on patients+appointments)
-- first, then invoice_items/payments (depend on invoices), then the clinical
-- children (depend on patients only).
-- =====================================================================

-- ---- invoices (rebuilt IDENTICALLY to 0014 — retarget FK only) ----
CREATE TABLE invoices_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_number VARCHAR(30) UNIQUE NOT NULL,
    patient_id INTEGER NOT NULL,
    appointment_id INTEGER,
    invoice_date DATETIME DEFAULT CURRENT_TIMESTAMP,
    due_date DATETIME,
    subtotal REAL NOT NULL DEFAULT 0,
    tax_amount REAL NOT NULL DEFAULT 0,
    discount_amount REAL NOT NULL DEFAULT 0,
    total_amount REAL NOT NULL DEFAULT 0,
    amount_paid REAL NOT NULL DEFAULT 0,
    balance_due REAL NOT NULL DEFAULT 0,
    status VARCHAR(20) DEFAULT 'issued',
    payment_method VARCHAR(30),
    notes TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE,
    FOREIGN KEY (appointment_id) REFERENCES appointments(id) ON DELETE SET NULL
);
INSERT INTO invoices_new (
    id, invoice_number, patient_id, appointment_id, invoice_date, due_date,
    subtotal, tax_amount, discount_amount, total_amount, amount_paid,
    balance_due, status, payment_method, notes, created_at
)
SELECT
    id, invoice_number, patient_id, appointment_id, invoice_date, due_date,
    subtotal, tax_amount, discount_amount, total_amount, amount_paid,
    balance_due, status, payment_method, notes, created_at
FROM invoices;
DROP TABLE invoices;
ALTER TABLE invoices_new RENAME TO invoices;

-- ---- patient_photos (rebuilt IDENTICALLY to 0014 — retarget FK only) ----
CREATE TABLE patient_photos_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    appointment_id INTEGER,
    category VARCHAR(30) NOT NULL DEFAULT 'document',
    filename VARCHAR(300) NOT NULL,
    mime_type VARCHAR(100) DEFAULT 'image/jpeg',
    caption VARCHAR(300),
    data_base64 TEXT NOT NULL,
    file_size INTEGER,
    captured_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE,
    FOREIGN KEY (appointment_id) REFERENCES appointments(id) ON DELETE SET NULL
);
INSERT INTO patient_photos_new (
    id, patient_id, appointment_id, category, filename, mime_type, caption,
    data_base64, file_size, captured_at, created_at
)
SELECT
    id, patient_id, appointment_id, category, filename, mime_type, caption,
    data_base64, file_size, captured_at, created_at
FROM patient_photos;
DROP TABLE patient_photos;
ALTER TABLE patient_photos_new RENAME TO patient_photos;

-- ---- invoice_items (rebuilt IDENTICALLY — retarget FK to new invoices) ----
CREATE TABLE invoice_items_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id INTEGER NOT NULL,
    item_type VARCHAR(30) NOT NULL,
    description VARCHAR(200) NOT NULL,
    quantity REAL NOT NULL DEFAULT 1,
    unit_price REAL NOT NULL DEFAULT 0,
    discount_percent REAL NOT NULL DEFAULT 0,
    tax_rate REAL NOT NULL DEFAULT 0,
    total REAL NOT NULL DEFAULT 0,
    FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE CASCADE
);
INSERT INTO invoice_items_new (
    id, invoice_id, item_type, description, quantity, unit_price,
    discount_percent, tax_rate, total
)
SELECT
    id, invoice_id, item_type, description, quantity, unit_price,
    discount_percent, tax_rate, total
FROM invoice_items;
DROP TABLE invoice_items;
ALTER TABLE invoice_items_new RENAME TO invoice_items;

-- ---- payments (rebuilt IDENTICALLY — retarget FK to new invoices) ----
CREATE TABLE payments_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id INTEGER NOT NULL,
    payment_date DATETIME DEFAULT CURRENT_TIMESTAMP,
    amount REAL NOT NULL,
    payment_method VARCHAR(30) NOT NULL,
    reference_number VARCHAR(100),
    notes TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE CASCADE
);
INSERT INTO payments_new (
    id, invoice_id, payment_date, amount, payment_method, reference_number,
    notes, created_at
)
SELECT
    id, invoice_id, payment_date, amount, payment_method, reference_number,
    notes, created_at
FROM payments;
DROP TABLE payments;
ALTER TABLE payments_new RENAME TO payments;

-- ---- clinical_notes (NEW: note non-empty CHECK) ----
-- original: 0003_clinical_billing.sql
CREATE TABLE clinical_notes_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    author VARCHAR(100),
    category VARCHAR(50) DEFAULT 'general',
    note TEXT NOT NULL CHECK (length(trim(note, char(9)||char(10)||char(13)||' ')) > 0),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);
INSERT INTO clinical_notes_new (id, patient_id, author, category, note, created_at)
SELECT id, patient_id, author, category, note, created_at FROM clinical_notes;
DROP TABLE clinical_notes;
ALTER TABLE clinical_notes_new RENAME TO clinical_notes;

-- ---- allergies (NEW: substance non-empty CHECK) ----
-- original: 0003_clinical_billing.sql
CREATE TABLE allergies_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    substance VARCHAR(200) NOT NULL CHECK (length(trim(substance, char(9)||char(10)||char(13)||' ')) > 0),
    severity VARCHAR(20) DEFAULT 'mild',
    noted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);
INSERT INTO allergies_new (id, patient_id, substance, severity, noted_at)
SELECT id, patient_id, substance, severity, noted_at FROM allergies;
DROP TABLE allergies;
ALTER TABLE allergies_new RENAME TO allergies;

-- ---- osdi_scores (NEW: total_score >= 0 CHECK) ----
-- original: 0003_clinical_billing.sql
CREATE TABLE osdi_scores_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    score_date DATE NOT NULL,
    total_score REAL NOT NULL CHECK (total_score >= 0),
    ocular_symptoms REAL,
    vision_function REAL,
    environmental_triggers REAL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);
INSERT INTO osdi_scores_new (
    id, patient_id, score_date, total_score, ocular_symptoms,
    vision_function, environmental_triggers, created_at
)
SELECT
    id, patient_id, score_date, total_score, ocular_symptoms,
    vision_function, environmental_triggers, created_at
FROM osdi_scores;
DROP TABLE osdi_scores;
ALTER TABLE osdi_scores_new RENAME TO osdi_scores;

-- ---- ipl_treatments (NEW: session_number >= 1 CHECK) ----
-- original: 0003_clinical_billing.sql
CREATE TABLE ipl_treatments_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    treatment_date DATETIME NOT NULL,
    session_number INTEGER NOT NULL CHECK (session_number >= 1),
    fluence_j_cm2 REAL,
    number_of_pulses INTEGER,
    operator_name VARCHAR(200),
    clinical_notes TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);
INSERT INTO ipl_treatments_new (
    id, patient_id, treatment_date, session_number, fluence_j_cm2,
    number_of_pulses, operator_name, clinical_notes, created_at
)
SELECT
    id, patient_id, treatment_date, session_number, fluence_j_cm2,
    number_of_pulses, operator_name, clinical_notes, created_at
FROM ipl_treatments;
DROP TABLE ipl_treatments;
ALTER TABLE ipl_treatments_new RENAME TO ipl_treatments;

-- =====================================================================
-- Phase F — rebuild the FK-free tables (simple create-copy-drop-rename).
-- =====================================================================

-- ---- blocked_times (NEW: start_at < end_at CHECK) ----
-- original: 0001_init.sql + ALTERs in 0006_versioning_blocked.sql
CREATE TABLE blocked_times_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    start_at DATETIME NOT NULL,
    end_at DATETIME NOT NULL,
    reason TEXT,
    practitioner VARCHAR(100),
    all_day INTEGER DEFAULT 0,
    is_recurring INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    CHECK (start_at < end_at)
);
INSERT INTO blocked_times_new (
    id, start_at, end_at, reason, practitioner, all_day, is_recurring, created_at
)
SELECT
    id, start_at, end_at, reason, practitioner, all_day, is_recurring, created_at
FROM blocked_times;
DROP TABLE blocked_times;
ALTER TABLE blocked_times_new RENAME TO blocked_times;

-- ---- booking_settings (NEW: reminder + mode CHECKs) ----
-- original: 0008_booking_settings.sql + ALTER in 0009_sms_fixes.sql
CREATE TABLE booking_settings_new (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    booking_mode VARCHAR(20) DEFAULT 'approval'
        CHECK (booking_mode IN ('automatic', 'approval')),
    auto_confirm_message INTEGER DEFAULT 1,
    auto_reminder_message INTEGER DEFAULT 1,
    reminder_hours_before INTEGER DEFAULT 24
        CHECK (reminder_hours_before >= 0),
    email_provider VARCHAR(20) DEFAULT 'postmark',
    email_api_key TEXT,
    email_from VARCHAR(200) DEFAULT 'bookings@clinic.local',
    sms_provider VARCHAR(20) DEFAULT 'clicksend',
    sms_api_key TEXT,
    sms_sender VARCHAR(20) DEFAULT 'OptiCore',
    sms_username TEXT,
    template_booking_received TEXT DEFAULT 'Hi {{name}}, we received your booking request for {{date}} at {{time}} ({{type}}). We will confirm shortly. Reply STOP to opt out. — OptiCore',
    template_booking_confirmed TEXT DEFAULT 'Hi {{name}}, your appointment is confirmed for {{date}} at {{time}} ({{type}}). Reply STOP to opt out. — OptiCore',
    template_booking_declined TEXT DEFAULT 'Hi {{name}}, unfortunately the requested time ({{date}} {{time}}) is no longer available. Please book again at a different time. Reply STOP to opt out. — OptiCore',
    template_reminder TEXT DEFAULT 'Reminder: {{name}}, you have an appointment tomorrow at {{time}} ({{type}}). Reply STOP to opt out. — OptiCore',
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO booking_settings_new (
    id, booking_mode, auto_confirm_message, auto_reminder_message,
    reminder_hours_before, email_provider, email_api_key, email_from,
    sms_provider, sms_api_key, sms_sender, sms_username,
    template_booking_received, template_booking_confirmed,
    template_booking_declined, template_reminder, updated_at
)
SELECT
    id, booking_mode, auto_confirm_message, auto_reminder_message,
    reminder_hours_before, email_provider, email_api_key, email_from,
    sms_provider, sms_api_key, sms_sender, sms_username,
    template_booking_received, template_booking_confirmed,
    template_booking_declined, template_reminder, updated_at
FROM booking_settings;
DROP TABLE booking_settings;
ALTER TABLE booking_settings_new RENAME TO booking_settings;

-- ---- intake_submissions (NEW: first_name / last_name non-empty CHECK) ----
-- original: 0004_intake.sql + ALTERs in 0010_intake_matching.sql
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
    claimed_no_match INTEGER DEFAULT 0
);
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
DROP TABLE intake_submissions;
ALTER TABLE intake_submissions_new RENAME TO intake_submissions;

-- =====================================================================
-- Phase G — drop the _old parent shadows. Safe now: every child was rebuilt
-- in Phases D-E with FKs pointing at the canonical-named (new) parents.
-- =====================================================================

DROP TABLE appointments_old;
DROP TABLE patients_old;

-- =====================================================================
-- Phase H — recreate indexes (DROP TABLE dropped them) and preserve the
-- AUTOINCREMENT sequences across the rebuilds.
-- =====================================================================

CREATE INDEX IF NOT EXISTS idx_patients_name ON patients(last_name, first_name);
CREATE INDEX IF NOT EXISTS idx_patients_dob ON patients(date_of_birth);
CREATE INDEX IF NOT EXISTS idx_appointments_date ON appointments(appointment_date);
CREATE INDEX IF NOT EXISTS idx_appointments_patient ON appointments(patient_id);
CREATE INDEX IF NOT EXISTS idx_blocked_start ON blocked_times(start_at);
CREATE INDEX IF NOT EXISTS idx_notes_patient ON clinical_notes(patient_id);
CREATE INDEX IF NOT EXISTS idx_osdi_patient ON osdi_scores(patient_id);
CREATE INDEX IF NOT EXISTS idx_ipl_patient ON ipl_treatments(patient_id);
CREATE INDEX IF NOT EXISTS idx_invoices_patient ON invoices(patient_id);
CREATE INDEX IF NOT EXISTS idx_invoice_items_invoice ON invoice_items(invoice_id);
CREATE INDEX IF NOT EXISTS idx_payments_invoice ON payments(invoice_id);
CREATE INDEX IF NOT EXISTS idx_photos_patient ON patient_photos(patient_id);
CREATE INDEX IF NOT EXISTS idx_photos_category ON patient_photos(category);
CREATE INDEX IF NOT EXISTS idx_photos_appointment ON patient_photos(appointment_id);
CREATE INDEX IF NOT EXISTS idx_intake_status ON intake_submissions(status);
CREATE INDEX IF NOT EXISTS idx_intake_date ON intake_submissions(submitted_at);
CREATE INDEX IF NOT EXISTS idx_intake_claimed_no_match ON intake_submissions(claimed_no_match);

-- Preserve AUTOINCREMENT sequences (DROP TABLE removed the sqlite_sequence
-- rows; re-insert at current max(id) so future inserts don't reuse ids).
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'patients', COALESCE(MAX(id), 0) FROM patients;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'appointments', COALESCE(MAX(id), 0) FROM appointments;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'blocked_times', COALESCE(MAX(id), 0) FROM blocked_times;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'clinical_notes', COALESCE(MAX(id), 0) FROM clinical_notes;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'allergies', COALESCE(MAX(id), 0) FROM allergies;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'osdi_scores', COALESCE(MAX(id), 0) FROM osdi_scores;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'ipl_treatments', COALESCE(MAX(id), 0) FROM ipl_treatments;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'invoices', COALESCE(MAX(id), 0) FROM invoices;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'invoice_items', COALESCE(MAX(id), 0) FROM invoice_items;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'payments', COALESCE(MAX(id), 0) FROM payments;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'patient_photos', COALESCE(MAX(id), 0) FROM patient_photos;
INSERT OR REPLACE INTO sqlite_sequence (name, seq) SELECT 'intake_submissions', COALESCE(MAX(id), 0) FROM intake_submissions;
