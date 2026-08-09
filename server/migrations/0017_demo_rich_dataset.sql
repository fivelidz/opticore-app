-- Comprehensive DEMO dataset — a realistic-looking clinic so every screen,
-- chart, and report has plenty to show. This is SEPARATE from the minimal
-- 5-patient seed in 0002 (which exists so even a bare boot has something).
--
-- SAFETY (same rules as 0011):
--   * Only runs when the demo patients exist. On a production (CLEAN_START)
--     DB the WHERE EXISTS guards short-circuit every insert, so nothing is
--     added to a real clinic. No FK errors, no junk.
--   * Idempotent: a NOT EXISTS guard on a sentinel row means re-running on an
--     already-seeded demo DB is a no-op (so upgrades never duplicate).
--   * New migration (never edit a shipped one) — upgrades cleanly.
--
-- Adds: 18 more patients (IDs 6-23) each with a clinical profile — allergies,
-- OSDI scores, IPL treatments where relevant, clinical notes, a spread of past
-- and upcoming appointments, and invoices (with line items + payments) so the
-- billing and analytics screens are populated.

-- Sentinel: if our "William" demo patient already exists, this whole migration
-- has already run — stop here.
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000006', 'William', 'Anderson', '1972-06-18', 'male', '0410 111 222', 'william.anderson@example.com', '5 Awaba St, Mosman NSW 2088', '6789 01234 5'
WHERE NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000006')
  AND EXISTS (SELECT 1 FROM patients WHERE id = 1);  -- only on a demo DB

-- Remaining demo patients (7-23). Each guarded the same way.
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000007', 'Sophia', 'Martinez', '1988-11-02', 'female', '0410 222 333', 'sophia.martinez@example.com', '18 Bradleys Head Rd, Mosman 2088', '7890 12345 6'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000007');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000008', 'James', 'Thompson', '1965-03-25', 'male', '0410 333 444', 'james.thompson@example.com', '29 Military Rd, Neutral Bay 2089', '8901 23456 7'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000008');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000009', 'Isabella', 'Lee', '1993-08-14', 'female', '0410 444 555', 'isabella.lee@example.com', '7 Spit Rd, Mosman 2088', '9012 34567 8'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000009');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000010', 'Benjamin', 'Walker', '1979-12-30', 'male', '0410 555 666', 'benjamin.walker@example.com', '44 Raglan St, Mosman 2088', '0123 45678 9'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000010');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000011', 'Charlotte', 'Harris', '1990-05-09', 'female', '0410 666 777', 'charlotte.harris@example.com', '11 Belmont Rd, Mosman 2088', '1234 56789 0'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000011');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000012', 'Henry', 'Clark', '1958-09-21', 'male', '0410 777 888', 'henry.clark@example.com', '3 Mosman St, Mosman 2088', '2345 67890 1'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000012');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000013', 'Amelia', 'Robinson', '1996-01-17', 'female', '0410 888 999', 'amelia.robinson@example.com', '26 Military Rd, Neutral Bay 2089', '3456 78901 2'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000013');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000014', 'Alexander', 'Lewis', '1976-07-03', 'male', '0410 999 000', 'alexander.lewis@example.com', '15 Awaba St, Mosman 2088', '4567 89012 3'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000014');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000015', 'Mia', 'Young', '1983-04-28', 'female', '0411 111 222', 'mia.young@example.com', '9 Bradleys Head Rd, Mosman 2088', '5678 90123 4'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000015');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000016', 'Daniel', 'King', '1969-10-12', 'male', '0411 222 333', 'daniel.king@example.com', '32 Spit Rd, Mosman 2088', '6789 01234 5'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000016');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000017', 'Grace', 'Wright', '1991-02-06', 'female', '0411 333 444', 'grace.wright@example.com', '21 Raglan St, Mosman 2088', '7890 12345 6'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000017');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000018', 'Samuel', 'Lopez', '1974-08-19', 'male', '0411 444 555', 'samuel.lopez@example.com', '4 Belmont Rd, Mosman 2088', '8901 23456 7'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000018');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000019', 'Chloe', 'Hill', '1998-11-23', 'female', '0411 555 666', 'chloe.hill@example.com', '13 Mosman St, Mosman 2088', '9012 34567 8'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000019');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000020', 'Joseph', 'Green', '1962-06-07', 'male', '0411 666 777', 'joseph.green@example.com', '27 Military Rd, Neutral Bay 2089', '0123 45678 9'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000020');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000021', 'Zoe', 'Adams', '1986-03-15', 'female', '0411 777 888', 'zoe.adams@example.com', '8 Awaba St, Mosman 2088', '1234 56789 0'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000021');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000022', 'Oliver', 'Baker', '1971-12-01', 'male', '0411 888 999', 'oliver.baker@example.com', '19 Bradleys Head Rd, Mosman 2088', '2345 67890 1'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000022');
INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
SELECT 'MOS-2026000023', 'Lily', 'Nelson', '1994-09-09', 'female', '0411 999 000', 'lily.nelson@example.com', '2 Spit Rd, Mosman 2088', '3456 78901 2'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 1) AND NOT EXISTS (SELECT 1 FROM patients WHERE mrn = 'MOS-2026000023');

-- ===== Allergies (a subset of patients) =====
INSERT INTO allergies (patient_id, substance, severity)
SELECT patient_id, substance, severity FROM (
  SELECT 6 AS patient_id, 'Sulfonamides' AS substance, 'moderate' AS severity
  UNION ALL SELECT 7, 'Penicillin', 'severe'
  UNION ALL SELECT 9, 'Latex', 'mild'
  UNION ALL SELECT 12, 'Aspirin', 'moderate'
  UNION ALL SELECT 14, 'Penicillin', 'mild'
  UNION ALL SELECT 17, 'Sulfonamides', 'mild'
  UNION ALL SELECT 20, 'Codeine', 'severe'
) s
WHERE EXISTS (SELECT 1 FROM patients p WHERE p.id = s.patient_id)
  AND NOT EXISTS (SELECT 1 FROM allergies a WHERE a.patient_id = s.patient_id AND a.substance = s.substance);

-- ===== OSDI scores (baseline + follow-up for several patients) =====
INSERT INTO osdi_scores (patient_id, score_date, total_score, ocular_symptoms, vision_function, environmental_triggers)
SELECT patient_id, score_date, total_score, ocular_symptoms, vision_function, environmental_triggers FROM (
  SELECT 6 AS patient_id, date('now','-90 days') AS score_date, 28.0 AS total_score, 30.0 AS ocular_symptoms, 25.0 AS vision_function, 28.0 AS environmental_triggers
  UNION ALL SELECT 6, date('now','-30 days'), 18.0, 20.0, 15.0, 19.0
  UNION ALL SELECT 7, date('now','-60 days'), 35.0, 40.0, 30.0, 35.0
  UNION ALL SELECT 7, date('now','-20 days'), 22.0, 25.0, 18.0, 23.0
  UNION ALL SELECT 8, date('now','-45 days'), 42.0, 45.0, 38.0, 43.0
  UNION ALL SELECT 9, date('now','-15 days'), 12.0, 15.0, 8.0, 13.0
  UNION ALL SELECT 10, date('now','-75 days'), 31.0, 33.0, 28.0, 32.0
  UNION ALL SELECT 11, date('now','-50 days'), 19.0, 22.0, 15.0, 20.0
  UNION ALL SELECT 12, date('now','-100 days'), 45.0, 50.0, 40.0, 45.0
  UNION ALL SELECT 12, date('now','-40 days'), 30.0, 33.0, 25.0, 32.0
  UNION ALL SELECT 15, date('now','-25 days'), 24.0, 28.0, 20.0, 24.0
  UNION ALL SELECT 16, date('now','-35 days'), 38.0, 42.0, 33.0, 39.0
  UNION ALL SELECT 18, date('now','-10 days'), 16.0, 18.0, 12.0, 18.0
  UNION ALL SELECT 20, date('now','-65 days'), 33.0, 36.0, 30.0, 33.0
  UNION ALL SELECT 21, date('now','-55 days'), 27.0, 30.0, 24.0, 27.0
) s
WHERE EXISTS (SELECT 1 FROM patients p WHERE p.id = s.patient_id);

-- ===== IPL treatments (patients undergoing IPL courses) =====
INSERT INTO ipl_treatments (patient_id, treatment_date, session_number, fluence_j_cm2, number_of_pulses, operator_name, clinical_notes)
SELECT patient_id, treatment_date, session_number, fluence_j_cm2, number_of_pulses, operator_name, clinical_notes FROM (
  SELECT 6 AS patient_id, datetime('now','-70 days','start of day','+11 hours') AS treatment_date, 1 AS session_number, 12.0 AS fluence_j_cm2, 15 AS number_of_pulses, 'Dr. Chapman-Davies' AS operator_name, 'First session. Tolerated well. Mild erythema.' AS clinical_notes
  UNION ALL SELECT 6, datetime('now','-49 days','start of day','+11 hours'), 2, 13.0, 15, 'Dr. Chapman-Davies', 'Increased fluence. Good response.'
  UNION ALL SELECT 6, datetime('now','-28 days','start of day','+11 hours'), 3, 14.0, 16, 'Dr. Chapman-Davies', 'Continued improvement. TBUT 7s.'
  UNION ALL SELECT 7, datetime('now','-55 days','start of day','+11 hours'), 1, 11.0, 14, 'Dr. Chapman-Davies', 'First session. Cautious start.'
  UNION ALL SELECT 7, datetime('now','-34 days','start of day','+11 hours'), 2, 13.0, 15, 'Dr. Chapman-Davies', 'Good tolerance. Symptoms easing.'
  UNION ALL SELECT 8, datetime('now','-40 days','start of day','+11 hours'), 1, 12.0, 15, 'Dr. Chapman-Davies', 'Severe MGD. Started course.'
  UNION ALL SELECT 10, datetime('now','-60 days','start of day','+11 hours'), 1, 12.5, 15, 'Regina Chapman-Davies', 'First session.'
  UNION ALL SELECT 10, datetime('now','-39 days','start of day','+11 hours'), 2, 13.5, 16, 'Regina Chapman-Davies', 'Improved gland expression.'
  UNION ALL SELECT 12, datetime('now','-80 days','start of day','+11 hours'), 1, 11.0, 14, 'Dr. Chapman-Davies', 'Elderly patient, conservative settings.'
  UNION ALL SELECT 15, datetime('now','-22 days','start of day','+11 hours'), 1, 12.0, 15, 'Regina Chapman-Davies', 'First session. Well tolerated.'
) s
WHERE EXISTS (SELECT 1 FROM patients p WHERE p.id = s.patient_id);

-- ===== Clinical notes (varied categories) =====
INSERT INTO clinical_notes (patient_id, author, category, note)
SELECT patient_id, author, category, note FROM (
  SELECT 6 AS patient_id, 'Dr. Chapman-Davies' AS author, 'clinical' AS category, 'Moderate dry eye with MGD. IPL course recommended. Started Omega-3 supplements.' AS note
  UNION ALL SELECT 6, 'Regina Chapman-Davies', 'general', 'Patient reports significant improvement after 3 IPL sessions. Less grittiness.'
  UNION ALL SELECT 7, 'Dr. Chapman-Davies', 'clinical', 'Severe dry eye. OSDI 35. Allergic to penicillin — note for prescriptions. IPL in progress.'
  UNION ALL SELECT 8, 'Dr. Chapman-Davies', 'clinical', 'Advanced MGD, grade 3. Atrophic glands inferiorly. IPL + thermal pulsation recommended.'
  UNION ALL SELECT 9, 'Regina Chapman-Davies', 'general', 'Mild dry eye, lifestyle advice given. Preservative-free tears PRN.'
  UNION ALL SELECT 10, 'Dr. Chapman-Davies', 'clinical', 'Bilateral dry eye worse AM. TBUT 5s. Commenced LipiFlow + IPL.'
  UNION ALL SELECT 11, 'Regina Chapman-Davies', 'clinical', 'Contact lens intolerance related to dry eye. Refit with scleral lenses planned.'
  UNION ALL SELECT 12, 'Dr. Chapman-Davies', 'clinical', 'Elderly patient, severe dry eye. Comorbid hypertension. Cautious IPL settings.'
  UNION ALL SELECT 13, 'Regina Chapman-Davies', 'general', 'Young patient, screen-related dry eye. 20-20-20 rule advised.'
  UNION ALL SELECT 14, 'Dr. Chapman-Davies', 'clinical', 'Post-LASIK dry eye, 6 months out. Improving. Continue lubricants.'
  UNION ALL SELECT 15, 'Regina Chapman-Davies', 'clinical', 'Moderate MGD. IPL course started. Omega-3 + warm compresses.'
  UNION ALL SELECT 16, 'Dr. Chapman-Davies', 'clinical', 'Sjogren''s screening negative. Aqueous-deficient component. Punctal plugs considered.'
  UNION ALL SELECT 17, 'Regina Chapman-Davies', 'general', 'Pregnant patient — avoid IPL and Rx drops. Lubricants only. Review postpartum.'
  UNION ALL SELECT 18, 'Dr. Chapman-Davies', 'clinical', 'Mild-moderate dry eye. Good response to OTC tears. IPL not indicated.'
  UNION ALL SELECT 19, 'Regina Chapman-Davies', 'general', 'New patient, mild symptoms. Baseline OSDI 12. Lifestyle measures.'
  UNION ALL SELECT 20, 'Dr. Chapman-Davies', 'clinical', 'Severe dry eye with filamentary keratitis. Bandage CL + serum tears.'
  UNION ALL SELECT 21, 'Regina Chapman-Davies', 'clinical', 'Moderate dry eye, IPL candidate. Discussed treatment plan, patient keen.'
  UNION ALL SELECT 22, 'Dr. Chapman-Davies', 'clinical', 'Chronic blepharitis with MGD. Lid hygiene + IPL planned.'
  UNION ALL SELECT 23, 'Regina Chapman-Davies', 'general', 'Mild dry eye, screen-related. Review in 3 months.'
) s
WHERE EXISTS (SELECT 1 FROM patients p WHERE p.id = s.patient_id);

-- ===== Past + upcoming appointments (spread across patients) =====
INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes, practitioner, status, notes)
SELECT patient_id, appointment_type, appointment_date, duration_minutes, practitioner, status, notes FROM (
  SELECT 6 AS patient_id, 'IPL Treatment' AS appointment_type, datetime('now','-7 days','start of day','+11 hours') AS appointment_date, 45 AS duration_minutes, 'Dr. Chapman-Davies' AS practitioner, 'completed' AS status, 'IPL session 3 of 4. Excellent response.' AS notes
  UNION ALL SELECT 6, 'Follow-up',   datetime('now','+5 days','start of day','+10 hours'), 30, 'Dr. Chapman-Davies', 'scheduled', '4-week review post-IPL course.'
  UNION ALL SELECT 7, 'IPL Treatment', datetime('now','-3 days','start of day','+11 hours'),45, 'Dr. Chapman-Davies', 'completed', 'IPL session 2 of 4.'
  UNION ALL SELECT 7, 'IPL Treatment', datetime('now','+18 days','start of day','+11 hours'),45,'Dr. Chapman-Davies', 'scheduled', 'IPL session 3 of 4.'
  UNION ALL SELECT 8, 'Dry Eye Consultation', datetime('now','-12 days','start of day','+9 hours'),60,'Dr. Chapman-Davies','completed','Severe MGD assessment. IPL recommended.'
  UNION ALL SELECT 8, 'IPL Treatment', datetime('now','+7 days','start of day','+11 hours'), 45, 'Dr. Chapman-Davies', 'confirmed', 'IPL session 1 of 4.'
  UNION ALL SELECT 9, 'Follow-up',   datetime('now','-20 days','start of day','+14 hours'), 30, 'Regina Chapman-Davies', 'completed', 'Mild dry eye review. Stable.'
  UNION ALL SELECT 10, 'IPL Treatment', datetime('now','-5 days','start of day','+11 hours'),45,'Regina Chapman-Davies','completed','IPL session 2 of 4.'
  UNION ALL SELECT 10, 'Imaging',    datetime('now','+3 days','start of day','+14 hours'),  30, 'Dr. Chapman-Davies', 'scheduled', 'Keratograph to track gland changes.'
  UNION ALL SELECT 11, 'Dry Eye Consultation', datetime('now','-8 days','start of day','+9 hours'),60,'Regina Chapman-Davies','completed','CL intolerance assessment.'
  UNION ALL SELECT 12, 'Follow-up',  datetime('now','-15 days','start of day','+10 hours'), 30, 'Dr. Chapman-Davies', 'completed', 'Elderly patient review. Stable on current regimen.'
  UNION ALL SELECT 13, 'Telehealth', datetime('now','+2 days','start of day','+15 hours'),  20, 'Regina Chapman-Davies', 'scheduled', 'Telehealth follow-up.'
  UNION ALL SELECT 14, 'Follow-up',  datetime('now','-30 days','start of day','+10 hours'), 30, 'Dr. Chapman-Davies', 'completed', 'Post-LASIK review. Dry eye improving.'
  UNION ALL SELECT 15, 'IPL Treatment', datetime('now','-1 days','start of day','+11 hours'),45,'Regina Chapman-Davies','completed','IPL session 1 of 4.'
  UNION ALL SELECT 16, 'Dry Eye Consultation', datetime('now','-18 days','start of day','+9 hours'),60,'Dr. Chapman-Davies','completed','Aqueous-deficient workup.'
  UNION ALL SELECT 18, 'Follow-up',  datetime('now','+9 days','start of day','+10 hours'),  30, 'Dr. Chapman-Davies', 'scheduled', '3-month review.'
  UNION ALL SELECT 20, 'Dry Eye Consultation', datetime('now','-6 days','start of day','+9 hours'),60,'Dr. Chapman-Davies','completed','Filamentary keratitis review.'
  UNION ALL SELECT 21, 'IPL Treatment', datetime('now','+12 days','start of day','+11 hours'),45,'Regina Chapman-Davies','scheduled','IPL session 1 of 4.'
  UNION ALL SELECT 22, 'Dry Eye Consultation', datetime('now','-4 days','start of day','+13 hours'),60,'Dr. Chapman-Davies','completed','Blepharitis + MGD assessment.'
  UNION ALL SELECT 23, 'Follow-up',  datetime('now','+90 days','start of day','+10 hours'), 30, 'Regina Chapman-Davies', 'scheduled', '3-month review.'
) s
WHERE EXISTS (SELECT 1 FROM patients p WHERE p.id = s.patient_id);

-- ===== Invoices with line items + payments (billing history) =====
-- Helper: we build invoices one at a time. Each block: insert invoice, then its
-- item(s), then a payment (full or partial). All guarded on the patient existing.
-- Invoice 1: patient 6, Dry Eye Consultation, paid in full.
INSERT INTO invoices (invoice_number, patient_id, appointment_id, subtotal, tax_amount, total_amount, amount_paid, balance_due, status, payment_method)
SELECT 'INV-2026-0013', 6, NULL, 350.00, 0.00, 350.00, 350.00, 0.00, 'paid', 'card'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 6);
INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
SELECT id, 'consultation', 'Dry Eye Consultation', 1, 350.00, 350.00 FROM invoices WHERE invoice_number = 'INV-2026-0013';
INSERT INTO payments (invoice_id, amount, payment_method, reference_number)
SELECT id, 350.00, 'card', 'EFT-0001' FROM invoices WHERE invoice_number = 'INV-2026-0013';

-- Invoice 2: patient 7, IPL course (partial — outstanding balance).
INSERT INTO invoices (invoice_number, patient_id, appointment_id, subtotal, tax_amount, total_amount, amount_paid, balance_due, status, payment_method)
SELECT 'INV-2026-0014', 7, NULL, 1200.00, 0.00, 1200.00, 300.00, 900.00, 'partial', 'card'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 7);
INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
SELECT id, 'treatment', 'IPL Treatment course (4 sessions)', 4, 300.00, 1200.00 FROM invoices WHERE invoice_number = 'INV-2026-0014';
INSERT INTO payments (invoice_id, amount, payment_method, reference_number)
SELECT id, 300.00, 'card', 'EFT-0002' FROM invoices WHERE invoice_number = 'INV-2026-0014';

-- Invoice 3: patient 8, consultation + imaging, paid.
INSERT INTO invoices (invoice_number, patient_id, appointment_id, subtotal, tax_amount, total_amount, amount_paid, balance_due, status, payment_method)
SELECT 'INV-2026-0009', 8, NULL, 600.00, 0.00, 600.00, 600.00, 0.00, 'paid', 'cash'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 8);
INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
SELECT id, 'consultation', 'Dry Eye Consultation', 1, 350.00, 350.00 FROM invoices WHERE invoice_number = 'INV-2026-0009';
INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
SELECT id, 'imaging', 'Keratograph Imaging', 1, 250.00, 250.00 FROM invoices WHERE invoice_number = 'INV-2026-0009';
INSERT INTO payments (invoice_id, amount, payment_method, reference_number)
SELECT id, 600.00, 'cash', 'CASH-0003' FROM invoices WHERE invoice_number = 'INV-2026-0009';

-- Invoice 4: patient 10, IPL session, paid.
INSERT INTO invoices (invoice_number, patient_id, appointment_id, subtotal, tax_amount, total_amount, amount_paid, balance_due, status, payment_method)
SELECT 'INV-2026-0010', 10, NULL, 300.00, 0.00, 300.00, 300.00, 0.00, 'paid', 'card'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 10);
INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
SELECT id, 'treatment', 'IPL Treatment session', 1, 300.00, 300.00 FROM invoices WHERE invoice_number = 'INV-2026-0010';
INSERT INTO payments (invoice_id, amount, payment_method, reference_number)
SELECT id, 300.00, 'card', 'EFT-0004' FROM invoices WHERE invoice_number = 'INV-2026-0010';

-- Invoice 5: patient 12, consultation, unpaid (issued).
INSERT INTO invoices (invoice_number, patient_id, appointment_id, subtotal, tax_amount, total_amount, amount_paid, balance_due, status)
SELECT 'INV-2026-0011', 12, NULL, 350.00, 0.00, 350.00, 0.00, 350.00, 'issued'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 12);
INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
SELECT id, 'consultation', 'Dry Eye Consultation', 1, 350.00, 350.00 FROM invoices WHERE invoice_number = 'INV-2026-0011';

-- Invoice 6: patient 15, IPL session, paid.
INSERT INTO invoices (invoice_number, patient_id, appointment_id, subtotal, tax_amount, total_amount, amount_paid, balance_due, status, payment_method)
SELECT 'INV-2026-0012', 15, NULL, 300.00, 0.00, 300.00, 300.00, 0.00, 'paid', 'card'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 15);
INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
SELECT id, 'treatment', 'IPL Treatment session', 1, 300.00, 300.00 FROM invoices WHERE invoice_number = 'INV-2026-0012';
INSERT INTO payments (invoice_id, amount, payment_method, reference_number)
SELECT id, 300.00, 'card', 'EFT-0006' FROM invoices WHERE invoice_number = 'INV-2026-0012';

-- Invoice 7: patient 20, consultation + imaging, partial.
INSERT INTO invoices (invoice_number, patient_id, appointment_id, subtotal, tax_amount, total_amount, amount_paid, balance_due, status, payment_method)
SELECT 'INV-2026-0015', 20, NULL, 600.00, 0.00, 600.00, 250.00, 350.00, 'partial', 'card'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 20);
INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
SELECT id, 'consultation', 'Dry Eye Consultation', 1, 350.00, 350.00 FROM invoices WHERE invoice_number = 'INV-2026-0015';
INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
SELECT id, 'imaging', 'Keratograph Imaging', 1, 250.00, 250.00 FROM invoices WHERE invoice_number = 'INV-2026-0015';
INSERT INTO payments (invoice_id, amount, payment_method, reference_number)
SELECT id, 250.00, 'card', 'EFT-0007' FROM invoices WHERE invoice_number = 'INV-2026-0015';

-- Invoice 8: patient 22, consultation, paid.
INSERT INTO invoices (invoice_number, patient_id, appointment_id, subtotal, tax_amount, total_amount, amount_paid, balance_due, status, payment_method)
SELECT 'INV-2026-0016', 22, NULL, 350.00, 0.00, 350.00, 350.00, 0.00, 'paid', 'cash'
WHERE EXISTS (SELECT 1 FROM patients WHERE id = 22);
INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
SELECT id, 'consultation', 'Dry Eye Consultation', 1, 350.00, 350.00 FROM invoices WHERE invoice_number = 'INV-2026-0016';
INSERT INTO payments (invoice_id, amount, payment_method, reference_number)
SELECT id, 350.00, 'cash', 'CASH-0008' FROM invoices WHERE invoice_number = 'INV-2026-0016';
