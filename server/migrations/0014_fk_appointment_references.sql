-- Add the two missing FOREIGN KEY declarations identified in the prior
-- referential-integrity audit:
--
--   * invoices.appointment_id        -> appointments(id)  ON DELETE SET NULL
--   * patient_photos.appointment_id  -> appointments(id)  ON DELETE SET NULL
--
-- NOTE: the actual table rebuild that adds these FKs is performed by the NEXT
-- migration (0015_check_constraints.sql), which rebuilds invoices,
-- patient_photos, invoice_items and payments together using a cascade-safe
-- ordering (it stashes the CASCADE children before rebuilding the parent).
--
-- This migration does ONLY the safe, idempotent orphan-cleanup that must run
-- BEFORE 0015: it NULLs out any appointment_id that points at an appointment
-- row that no longer exists. If we did not do this, 0015's copy into the new
-- FK-bearing table would fail the new constraint on databases that accumulated
-- dangling references while the constraint was absent.
--
-- (An earlier version of this migration rebuilt invoices + patient_photos
-- here using a naive CREATE-copy-DROP-rename. With PRAGMA foreign_keys=ON that
-- DROP cascaded through invoice_items/payments ON DELETE CASCADE and silently
-- wiped all billing line-items and payments on upgrade. That rebuild is now
-- done cascade-safely in 0015 instead.)

UPDATE invoices SET appointment_id = NULL
 WHERE appointment_id IS NOT NULL
   AND appointment_id NOT IN (SELECT id FROM appointments);

UPDATE patient_photos SET appointment_id = NULL
 WHERE appointment_id IS NOT NULL
   AND appointment_id NOT IN (SELECT id FROM appointments);
