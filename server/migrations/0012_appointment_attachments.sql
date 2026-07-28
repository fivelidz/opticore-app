-- Link photos/documents to a specific appointment (in addition to the patient).
-- NULL appointment_id = a patient-level document (existing behaviour, unchanged).
-- A set appointment_id = the file is attached to that appointment's notes, so it
-- shows on both the current appointment and when reviewing prior appointments.
--
-- New migration (never edit a shipped one) so existing databases upgrade cleanly.

ALTER TABLE patient_photos ADD COLUMN appointment_id INTEGER;

CREATE INDEX IF NOT EXISTS idx_photos_appointment ON patient_photos(appointment_id);
