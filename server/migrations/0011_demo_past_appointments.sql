-- Demo past (completed) appointments — for demonstrating editing past notes.
--
-- IMPORTANT: added as a NEW migration. Never modify an already-shipped migration
-- or SQLx rejects the checksum on existing databases and the app fails to start
-- ("migration N was previously applied but has been modified"). Adding new
-- migrations is the ONLY safe way to evolve the schema/data of a live install.
--
-- Safe on BOTH demo and production databases:
--   * INSERT ... SELECT ... WHERE the referenced demo patient exists, so on a
--     production (CLEAN_START) DB where patients 1-5 don't exist, nothing is
--     inserted (no FK errors, no junk data).
--   * A NOT EXISTS guard on the first note text makes it idempotent, so an
--     upgraded demo DB won't get duplicate history rows.

INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes, practitioner, status, notes)
SELECT patient_id, appointment_type, appointment_date, duration_minutes, practitioner, status, notes
FROM (
  SELECT 1 AS patient_id, 'Dry Eye Consultation' AS appointment_type, datetime('now','-42 days','start of day','+9 hours') AS appointment_date, 60 AS duration_minutes, 'Dr. Chapman-Davies' AS practitioner, 'completed' AS status, 'Initial consultation. OSDI 32 (moderate-severe). TBUT 4s OU. Meibomian gland dysfunction grade 2. Started on warm compresses BID + preservative-free tears QID. Discussed IPL as next step.' AS notes
  UNION ALL SELECT 1, 'Follow-up',            datetime('now','-28 days','start of day','+10 hours'),30, 'Dr. Chapman-Davies', 'completed', '2-week review. Symptoms improving. TBUT now 6s. Compliant with compresses. Proceeding with IPL course of 4.'
  UNION ALL SELECT 2, 'IPL Treatment',        datetime('now','-35 days','start of day','+11 hours'),45, 'Dr. Chapman-Davies', 'completed', 'IPL session 1 of 4. Settings 12 J/cm2, 15 pulses. Well tolerated. Mild transient erythema. Next session in 3 weeks.'
  UNION ALL SELECT 2, 'IPL Treatment',        datetime('now','-14 days','start of day','+11 hours'),45, 'Dr. Chapman-Davies', 'completed', 'IPL session 2 of 4. Increased to 14 J/cm2. Good response. Patient reports less grittiness.'
  UNION ALL SELECT 3, 'Dry Eye Consultation', datetime('now','-21 days','start of day','+13 hours'),60, 'Regina Chapman-Davies', 'completed', 'New patient assessment. Mild dry eye. OSDI 15. Advised lifestyle measures and lubricants. Review in 6 weeks.'
  UNION ALL SELECT 4, 'Imaging',              datetime('now','-7 days','start of day','+15 hours'), 30, 'Dr. Chapman-Davies', 'completed', 'Keratograph imaging. Tear meniscus height reduced. NIBUT 5.2s. Lipid layer thin. Consistent with evaporative dry eye.'
  UNION ALL SELECT 5, 'Follow-up',            datetime('now','-3 days','start of day','+14 hours'), 30, 'Regina Chapman-Davies', 'noshow', 'Patient did not attend. Reception to reschedule.'
) AS seed
WHERE EXISTS (SELECT 1 FROM patients p WHERE p.id = seed.patient_id)
  AND NOT EXISTS (
    SELECT 1 FROM appointments a
    WHERE a.notes = 'Initial consultation. OSDI 32 (moderate-severe). TBUT 4s OU. Meibomian gland dysfunction grade 2. Started on warm compresses BID + preservative-free tears QID. Discussed IPL as next step.'
  );
