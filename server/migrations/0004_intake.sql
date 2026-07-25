-- Public intake submissions (from the localhost input page / future website).
CREATE TABLE IF NOT EXISTS intake_submissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    submitted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
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
    status VARCHAR(20) DEFAULT 'new',  -- new | imported | archived
    matched_patient_id INTEGER
);

CREATE INDEX IF NOT EXISTS idx_intake_status ON intake_submissions(status);
CREATE INDEX IF NOT EXISTS idx_intake_date ON intake_submissions(submitted_at);
