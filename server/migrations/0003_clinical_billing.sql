-- Extended clinical + billing tables for the decked-out PMS.

-- ---------- Clinical notes ----------
CREATE TABLE IF NOT EXISTS clinical_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    author VARCHAR(100),
    category VARCHAR(50) DEFAULT 'general', -- general | assessment | treatment | followup | admin
    note TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

-- ---------- Allergies ----------
CREATE TABLE IF NOT EXISTS allergies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    substance VARCHAR(200) NOT NULL,
    severity VARCHAR(20) DEFAULT 'mild', -- mild | moderate | severe
    noted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

-- ---------- OSDI scores ----------
CREATE TABLE IF NOT EXISTS osdi_scores (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    score_date DATE NOT NULL,
    total_score REAL NOT NULL,
    ocular_symptoms REAL,
    vision_function REAL,
    environmental_triggers REAL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

-- ---------- IPL treatments ----------
CREATE TABLE IF NOT EXISTS ipl_treatments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    patient_id INTEGER NOT NULL,
    treatment_date DATETIME NOT NULL,
    session_number INTEGER NOT NULL,
    fluence_j_cm2 REAL,
    number_of_pulses INTEGER,
    operator_name VARCHAR(200),
    clinical_notes TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

-- ---------- Consultation types (billing catalog) ----------
CREATE TABLE IF NOT EXISTS consultation_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    type_code VARCHAR(30) UNIQUE NOT NULL,
    type_name VARCHAR(100) NOT NULL,
    description TEXT,
    default_price REAL NOT NULL DEFAULT 0,
    default_duration_minutes INTEGER NOT NULL DEFAULT 30,
    medicare_item_number VARCHAR(30),
    active INTEGER DEFAULT 1
);

-- ---------- Services / products (billing catalog) ----------
CREATE TABLE IF NOT EXISTS services (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_code VARCHAR(30) UNIQUE NOT NULL,
    service_name VARCHAR(100) NOT NULL,
    category VARCHAR(50) NOT NULL, -- treatment | imaging | medication | supply | consultation
    description TEXT,
    unit_price REAL NOT NULL DEFAULT 0,
    unit_type VARCHAR(20) DEFAULT 'each',
    tax_rate REAL DEFAULT 0.10,
    active INTEGER DEFAULT 1
);

-- ---------- Invoices ----------
CREATE TABLE IF NOT EXISTS invoices (
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
    status VARCHAR(20) DEFAULT 'issued', -- draft | issued | partially_paid | paid | overdue | cancelled
    payment_method VARCHAR(30),
    notes TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

-- ---------- Invoice line items ----------
CREATE TABLE IF NOT EXISTS invoice_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id INTEGER NOT NULL,
    item_type VARCHAR(30) NOT NULL, -- consultation | service | product
    description VARCHAR(200) NOT NULL,
    quantity REAL NOT NULL DEFAULT 1,
    unit_price REAL NOT NULL DEFAULT 0,
    discount_percent REAL NOT NULL DEFAULT 0,
    tax_rate REAL NOT NULL DEFAULT 0,
    total REAL NOT NULL DEFAULT 0,
    FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE CASCADE
);

-- ---------- Payments ----------
CREATE TABLE IF NOT EXISTS payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id INTEGER NOT NULL,
    payment_date DATETIME DEFAULT CURRENT_TIMESTAMP,
    amount REAL NOT NULL,
    payment_method VARCHAR(30) NOT NULL, -- card | cash | eftpos | medicare | insurance
    reference_number VARCHAR(100),
    notes TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (invoice_id) REFERENCES invoices(id) ON DELETE CASCADE
);

-- ---------- Website traffic (for analytics; populated by Worker later) ----------
CREATE TABLE IF NOT EXISTS website_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_date DATE NOT NULL,
    visitors INTEGER DEFAULT 0,
    page_views INTEGER DEFAULT 0,
    bookings INTEGER DEFAULT 0,
    source VARCHAR(50), -- website | facebook | google | direct
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ---------- Indexes ----------
CREATE INDEX IF NOT EXISTS idx_notes_patient ON clinical_notes(patient_id);
CREATE INDEX IF NOT EXISTS idx_osdi_patient ON osdi_scores(patient_id);
CREATE INDEX IF NOT EXISTS idx_ipl_patient ON ipl_treatments(patient_id);
CREATE INDEX IF NOT EXISTS idx_invoices_patient ON invoices(patient_id);
CREATE INDEX IF NOT EXISTS idx_invoice_items_invoice ON invoice_items(invoice_id);
CREATE INDEX IF NOT EXISTS idx_payments_invoice ON payments(invoice_id);
CREATE INDEX IF NOT EXISTS idx_website_date ON website_events(event_date);

-- ---------- Seed: consultation types ----------
INSERT OR IGNORE INTO consultation_types (type_code, type_name, description, default_price, default_duration_minutes, medicare_item_number) VALUES
('DRY-EYE', 'Dry Eye Consultation', 'Comprehensive dry eye assessment', 350, 60, '10910'),
('FOLLOWUP', 'Follow-up', 'Review consultation', 150, 30, '10912'),
('IPL', 'IPL Treatment', 'Intense Pulsed Light therapy session', 300, 45, NULL),
('IMAGING', 'Imaging', 'Diagnostic imaging session', 250, 30, NULL),
('TELEHEALTH', 'Telehealth', 'Remote consultation', 120, 20, '91852');

-- ---------- Seed: services ----------
INSERT OR IGNORE INTO services (service_code, service_name, category, description, unit_price, unit_type, tax_rate) VALUES
('IPL-SESSION', 'IPL Therapy Session', 'treatment', 'Single IPL treatment', 300, 'each', 0.10),
('OPTILIGHT', 'OptiLight Treatment', 'treatment', 'IPL OptiLight session', 275, 'each', 0.10),
('KERATO-5M', 'Keratograph 5M Imaging', 'imaging', 'Ocular surface imaging', 180, 'each', 0.10),
('LIPIVIEW', 'LipiView II', 'imaging', 'Lipid layer imaging', 200, 'each', 0.10),
('TBUT', 'Tear Break-Up Time', 'imaging', 'TBUT test', 90, 'each', 0.10),
('RESTASIS', 'Restasis 0.05%', 'medication', 'Cyclosporine emulsion', 95, 'each', 0.10),
('XIIDRA', 'Xiidra 5%', 'medication', 'Lifitegrast eye drops', 110, 'each', 0.10),
('ARTIFICIAL', 'Artificial Tears', 'medication', 'Lubricant drops', 25, 'each', 0.10),
('WARM-COMP', 'Warm Compress', 'supply', 'Eye mask', 35, 'each', 0.10),
('SUPPLY-OTHER', 'Other Supply', 'supply', 'Misc supply item', 20, 'each', 0.10);

-- ---------- Seed: sample clinical data ----------
INSERT OR IGNORE INTO clinical_notes (patient_id, author, category, note) VALUES
(1, 'Dr. Chapman-Davies', 'assessment', 'Moderate dry eye disease. TBUT 5s. Started on Restasis + warm compresses.'),
(1, 'Regina Chapman-Davies', 'followup', 'Improvement reported after 6 weeks. Continue current management.'),
(2, 'Dr. Chapman-Davies', 'treatment', 'IPL session 2 of 4. Good response. Fluence 12 J/cm2, 15 pulses.'),
(3, 'Regina Chapman-Davies', 'general', 'New patient. Mild symptoms. Baseline imaging completed.');

INSERT OR IGNORE INTO allergies (patient_id, substance, severity) VALUES
(2, 'Chloramphenicol', 'moderate'),
(4, 'Sulfa drugs', 'severe');

INSERT OR IGNORE INTO osdi_scores (patient_id, score_date, total_score, ocular_symptoms, vision_function, environmental_triggers) VALUES
(1, '2026-06-01', 28.5, 30.0, 25.0, 30.0),
(1, '2026-07-01', 22.1, 24.0, 20.0, 22.5),
(2, '2026-06-15', 35.2, 38.0, 32.0, 35.5),
(3, '2026-07-10', 15.0, 16.0, 14.0, 15.0);

INSERT OR IGNORE INTO ipl_treatments (patient_id, treatment_date, session_number, fluence_j_cm2, number_of_pulses, operator_name, clinical_notes) VALUES
(2, '2026-06-20', 1, 10.0, 15, 'Dr. Chapman-Davies', 'First session. Tolerated well.'),
(2, '2026-07-04', 2, 12.0, 15, 'Dr. Chapman-Davies', 'Increased fluence. Good response.'),
(1, '2026-06-25', 1, 11.0, 14, 'Dr. Chapman-Davies', 'Session 1 of 4.');

-- ---------- Seed: invoices ----------
INSERT OR IGNORE INTO invoices (invoice_number, patient_id, invoice_date, subtotal, tax_amount, total_amount, amount_paid, balance_due, status, payment_method) VALUES
('INV-2026-0001', 1, '2026-06-01 10:30:00', 350, 35, 385, 385, 0, 'paid', 'card'),
('INV-2026-0002', 1, '2026-07-01 11:00:00', 150, 15, 165, 165, 0, 'paid', 'card'),
('INV-2026-0003', 2, '2026-06-20 14:00:00', 580, 58, 638, 300, 338, 'partially_paid', 'eftpos'),
('INV-2026-0004', 2, '2026-07-04 14:00:00', 300, 30, 330, 0, 330, 'issued', NULL),
('INV-2026-0005', 3, '2026-07-10 09:30:00', 430, 43, 473, 473, 0, 'paid', 'medicare'),
('INV-2026-0006', 4, '2026-06-12 15:00:00', 250, 25, 275, 275, 0, 'paid', 'cash');

INSERT OR IGNORE INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, discount_percent, tax_rate, total) VALUES
(1, 'consultation', 'Dry Eye Consultation', 1, 350, 0, 0.10, 385),
(2, 'consultation', 'Follow-up', 1, 150, 0, 0.10, 165),
(3, 'consultation', 'Dry Eye Consultation', 1, 350, 0, 0.10, 385),
(3, 'service', 'IPL Therapy Session', 1, 300, 10, 0.10, 297),
(4, 'service', 'IPL Therapy Session', 1, 300, 0, 0.10, 330),
(5, 'consultation', 'Dry Eye Consultation', 1, 350, 0, 0.10, 385),
(5, 'service', 'Keratograph 5M Imaging', 1, 180, 0, 0.10, 198),
(6, 'service', 'Keratograph 5M Imaging', 1, 250, 0, 0.10, 275);

INSERT OR IGNORE INTO payments (invoice_id, payment_date, amount, payment_method, reference_number) VALUES
(1, '2026-06-01 10:35:00', 385, 'card', 'TXN-001'),
(2, '2026-07-01 11:05:00', 165, 'card', 'TXN-002'),
(3, '2026-06-20 14:10:00', 300, 'eftpos', 'TXN-003'),
(5, '2026-07-10 09:35:00', 473, 'medicare', 'TXN-004'),
(6, '2026-06-12 15:05:00', 275, 'cash', 'TXN-005');

-- ---------- Seed: website traffic (last 30 days, synthetic) ----------
-- Generated so analytics charts have something to show.
WITH RECURSIVE days(d) AS (
  SELECT DATE('now','-29 days')
  UNION ALL
  SELECT DATE(d,'+1 day') FROM days WHERE d < DATE('now')
)
INSERT OR IGNORE INTO website_events (event_date, visitors, page_views, bookings, source)
SELECT
  d,
  20 + ABS(RANDOM() % 40) AS visitors,
  60 + ABS(RANDOM() % 120) AS page_views,
  ABS(RANDOM() % 4) AS bookings,
  CASE ABS(RANDOM() % 4) WHEN 0 THEN 'website' WHEN 1 THEN 'google' WHEN 2 THEN 'facebook' ELSE 'direct' END
FROM days;
