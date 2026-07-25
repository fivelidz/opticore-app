-- Mosman Dry Eye Clinic PMS — Rust server schema
-- Ported from opticore/database/schema.sql + users-auth-schema.sql (cleaned).

-- ---------- Users / auth ----------
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(20) NOT NULL DEFAULT 'receptionist', -- admin | doctor | nurse | receptionist
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    is_active BOOLEAN DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------- Patients ----------
CREATE TABLE IF NOT EXISTS patients (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mrn VARCHAR(20) UNIQUE NOT NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE NOT NULL,
    gender VARCHAR(20),
    phone VARCHAR(20),
    email VARCHAR(100),
    address TEXT,
    medicare_number VARCHAR(20),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------- Appointments ----------
CREATE TABLE IF NOT EXISTS appointments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    appointment_type VARCHAR(50) NOT NULL,
    appointment_date DATETIME NOT NULL,
    duration_minutes INTEGER NOT NULL,
    practitioner VARCHAR(100),
    status VARCHAR(20) DEFAULT 'scheduled', -- scheduled | confirmed | completed | cancelled | noshow
    notes TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

-- ---------- Blocked times (calendar) ----------
CREATE TABLE IF NOT EXISTS blocked_times (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    start_at DATETIME NOT NULL,
    end_at DATETIME NOT NULL,
    reason TEXT,
    practitioner VARCHAR(100),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------- Practice settings (per-clinic config) ----------
CREATE TABLE IF NOT EXISTS practice_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    setting_key VARCHAR(100) UNIQUE NOT NULL,
    setting_value TEXT,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------- Audit log (append-only) ----------
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER,
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50),
    resource_id INTEGER,
    detail TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------- Indexes ----------
CREATE INDEX IF NOT EXISTS idx_patients_name ON patients(last_name, first_name);
CREATE INDEX IF NOT EXISTS idx_patients_dob ON patients(date_of_birth);
CREATE INDEX IF NOT EXISTS idx_appointments_date ON appointments(appointment_date);
CREATE INDEX IF NOT EXISTS idx_appointments_patient ON appointments(patient_id);
CREATE INDEX IF NOT EXISTS idx_blocked_start ON blocked_times(start_at);

-- ---------- Default practice settings ----------
INSERT OR IGNORE INTO practice_settings (setting_key, setting_value) VALUES
('practice_name', 'OptiCore'),
('practice_address', '86 Prince Albert St, Mosman NSW 2088'),
('practice_phone', '(02) 9969 9999'),
('practice_email', 'info@mymosmaneyecare.com.au');
