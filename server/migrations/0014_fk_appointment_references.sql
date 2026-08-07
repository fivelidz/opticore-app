-- Add the two missing FOREIGN KEY declarations identified in the prior
-- referential-integrity audit:
--
--   * invoices.appointment_id        -> appointments(id)
--   * patient_photos.appointment_id  -> appointments(id)
--
-- Both columns already existed as plain INTEGER columns (no constraint), so
-- deleting an appointment left dangling references — the invoice/photo kept
-- pointing at a row that no longer existed. Other "soft-link" columns
-- (audit_log.user_id, intake_submissions.matched_patient_id,
-- messages.linked_patient_id, booking_notifications.*) are intentionally
-- unconstrained and are NOT touched here.
--
-- Action: ON DELETE SET NULL. Deleting an appointment must NOT delete the
-- invoice (financial record) or the photo (clinical document); it should just
-- clear the now-dangling pointer. The invoice/photo remain attached to the
-- patient.
--
-- SQLite cannot add a FK constraint to an existing column in place. The only
-- supported method is the documented "12-step" table rebuild:
--     1.  put the old table in a safe name
--     2.  create the new table with the FK
--     3.  copy data
--     4.  drop the old table
--     5.  rename the new table to the original name
--     6.  recreate indexes
-- We additionally null out any pre-existing orphaned appointment_id values
-- BEFORE the copy, so the FK is satisfiable on databases that accumulated
-- dangling rows while the constraint was absent.
--
-- This migration is idempotent-safe: it only ever runs once (sqlx tracks
-- applied migrations) and preserves every column, default, and index.

-- =====================================================================
-- 1) invoices.appointment_id  -> appointments(id) ON DELETE SET NULL
-- =====================================================================

-- 1a. Clean up any orphaned references that exist from before the FK was
--     declared. (On a fresh DB there are none; this is for upgrading installs.)
UPDATE invoices SET appointment_id = NULL
 WHERE appointment_id IS NOT NULL
   AND appointment_id NOT IN (SELECT id FROM appointments);

-- 1b. Rebuild the table with the FK added. Column definitions are reproduced
--     EXACTLY from 0003_clinical_billing.sql (types, defaults, NOT NULL,
--     existing patient_id FK) with the single addition of the appointment_id
--     FOREIGN KEY ... ON DELETE SET NULL clause.
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

-- 1c. Copy all rows across (column list is explicit so a future column add
--     can't silently shift positions).
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

-- 1d. Swap: drop old, rename new, recreate the single index.
DROP TABLE invoices;
ALTER TABLE invoices_new RENAME TO invoices;
CREATE INDEX IF NOT EXISTS idx_invoices_patient ON invoices(patient_id);

-- Preserve the AUTOINCREMENT sequence across the rebuild. SQLite stores the
-- next-rowid in sqlite_sequence; the DROP above removed the invoices row, so
-- re-insert it at the current max(id) so future inserts don't reuse ids.
INSERT OR REPLACE INTO sqlite_sequence (name, seq)
SELECT 'invoices', COALESCE(MAX(id), 0) FROM invoices;

-- =====================================================================
-- 2) patient_photos.appointment_id  -> appointments(id) ON DELETE SET NULL
-- =====================================================================

-- 2a. Null out orphaned references (upgrade safety; no-op on a fresh DB).
UPDATE patient_photos SET appointment_id = NULL
 WHERE appointment_id IS NOT NULL
   AND appointment_id NOT IN (SELECT id FROM appointments);

-- 2b. Rebuild with the FK. Column definitions reproduced EXACTLY from
--     0007_photos.sql + the appointment_id column added in
--     0012_appointment_attachments.sql, with the new FK appended.
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

-- 2c. Copy.
INSERT INTO patient_photos_new (
    id, patient_id, appointment_id, category, filename, mime_type, caption,
    data_base64, file_size, captured_at, created_at
)
SELECT
    id, patient_id, appointment_id, category, filename, mime_type, caption,
    data_base64, file_size, captured_at, created_at
FROM patient_photos;

-- 2d. Swap + recreate all three indexes.
DROP TABLE patient_photos;
ALTER TABLE patient_photos_new RENAME TO patient_photos;
CREATE INDEX IF NOT EXISTS idx_photos_patient ON patient_photos(patient_id);
CREATE INDEX IF NOT EXISTS idx_photos_category ON patient_photos(category);
CREATE INDEX IF NOT EXISTS idx_photos_appointment ON patient_photos(appointment_id);

-- Preserve the AUTOINCREMENT sequence for patient_photos too.
INSERT OR REPLACE INTO sqlite_sequence (name, seq)
SELECT 'patient_photos', COALESCE(MAX(id), 0) FROM patient_photos;
