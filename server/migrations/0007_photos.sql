-- Patient photos/files: profile pictures, medical documentation, other documents.

CREATE TABLE IF NOT EXISTS patient_photos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    category VARCHAR(30) NOT NULL DEFAULT 'document',  -- profile | medical | document
    filename VARCHAR(300) NOT NULL,
    mime_type VARCHAR(100) DEFAULT 'image/jpeg',
    caption VARCHAR(300),
    -- stored as base64 text for simplicity (single-file, no external assets).
    -- For large clinics this would be a file path / blob store instead.
    data_base64 TEXT NOT NULL,
    file_size INTEGER,
    captured_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_photos_patient ON patient_photos(patient_id);
CREATE INDEX IF NOT EXISTS idx_photos_category ON patient_photos(category);

-- Add profile_photo_id to patients for quick lookup of the avatar.
ALTER TABLE patients ADD COLUMN profile_photo_id INTEGER;
