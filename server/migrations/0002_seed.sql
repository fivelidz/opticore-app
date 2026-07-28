-- Demo seed data. The admin user is created at runtime by the server
-- (with a generated password printed to the log) so no secret lives in the repo.

INSERT OR IGNORE INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
VALUES
('MOS-2026000001', 'Sarah',   'Johnson', '1985-04-12', 'female', '0412 345 678', 'sarah.johnson@example.com',   '12 Mosman St, Mosman NSW 2088',  '2123 45678 1'),
('MOS-2026000002', 'Michael', 'Chen',    '1979-09-23', 'male',   '0423 456 789', 'michael.chen@example.com',    '45 Military Rd, Neutral Bay 2089', '2987 65432 1'),
('MOS-2026000003', 'Emma',    'Wilson',  '1992-12-03', 'female', '0434 567 890', 'emma.wilson@example.com',     '8 Raglan St, Mosman NSW 2088',   '3456 78901 2'),
('MOS-2026000004', 'David',   'Nguyen',  '1968-07-15', 'male',   '0445 678 901', 'david.nguyen@example.com',    '22 Spit Rd, Mosman NSW 2088',    '4567 89012 3'),
('MOS-2026000005', 'Olivia',  'Brown',   '1995-02-28', 'female', '0456 789 012', 'olivia.brown@example.com',    '3 Belmont Rd, Mosman NSW 2088',  '5678 90123 4');

INSERT OR IGNORE INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes, practitioner, status, notes)
VALUES
(1, 'Dry Eye Consultation', datetime('now','+1 day','start of day','+9 hours'),  60, 'Dr. Chapman-Davies', 'scheduled', 'Initial assessment'),
(2, 'IPL Treatment',        datetime('now','+1 day','start of day','+11 hours'), 45, 'Dr. Chapman-Davies', 'confirmed', 'Session 2 of 4'),
(3, 'Follow-up',            datetime('now','+2 days','start of day','+10 hours'),30, 'Regina Chapman-Davies', 'scheduled', '6-week review'),
(1, 'Imaging',              datetime('now','+3 days','start of day','+14 hours'),30, 'Dr. Chapman-Davies', 'scheduled', 'Keratograph 5M'),
(4, 'Dry Eye Consultation', datetime('now','+4 days','start of day','+9 hours'),  60, 'Regina Chapman-Davies', 'scheduled', 'New patient'),
-- PAST (completed) appointments — for demonstrating editing past notes
(1, 'Dry Eye Consultation', datetime('now','-42 days','start of day','+9 hours'), 60, 'Dr. Chapman-Davies', 'completed', 'Initial consultation. OSDI 32 (moderate-severe). TBUT 4s OU. Meibomian gland dysfunction grade 2. Started on warm compresses BID + preservative-free tears QID. Discussed IPL as next step.'),
(1, 'Follow-up',            datetime('now','-28 days','start of day','+10 hours'),30, 'Dr. Chapman-Davies', 'completed', '2-week review. Symptoms improving. TBUT now 6s. Compliant with compresses. Proceeding with IPL course of 4.'),
(2, 'IPL Treatment',        datetime('now','-35 days','start of day','+11 hours'),45, 'Dr. Chapman-Davies', 'completed', 'IPL session 1 of 4. Settings 12 J/cm2, 15 pulses. Well tolerated. Mild transient erythema. Next session in 3 weeks.'),
(2, 'IPL Treatment',        datetime('now','-14 days','start of day','+11 hours'),45, 'Dr. Chapman-Davies', 'completed', 'IPL session 2 of 4. Increased to 14 J/cm2. Good response. Patient reports less grittiness.'),
(3, 'Dry Eye Consultation', datetime('now','-21 days','start of day','+13 hours'),60, 'Regina Chapman-Davies', 'completed', 'New patient assessment. Mild dry eye. OSDI 15. Advised lifestyle measures and lubricants. Review in 6 weeks.'),
(4, 'Imaging',              datetime('now','-7 days','start of day','+15 hours'), 30, 'Dr. Chapman-Davies', 'completed', 'Keratograph imaging. Tear meniscus height reduced. NIBUT 5.2s. Lipid layer thin. Consistent with evaporative dry eye.'),
(5, 'Follow-up',            datetime('now','-3 days','start of day','+14 hours'), 30, 'Regina Chapman-Davies', 'noshow', 'Patient did not attend. Reception to reschedule.');

INSERT OR IGNORE INTO blocked_times (start_at, end_at, reason, practitioner)
VALUES
(datetime('now','+1 day','start of day','+13 hours'), datetime('now','+1 day','start of day','+14 hours'), 'Lunch', 'Dr. Chapman-Davies'),
(datetime('now','+2 days','start of day','+12 hours'), datetime('now','+2 days','start of day','+13 hours 30 minutes'), 'Lunch', 'Regina Chapman-Davies');
