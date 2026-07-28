-- Intake matching enhancements.
--
-- 1) Track what the patient CLAIMED about themselves on the public form:
--    whether they said they were an existing/returning patient. Combined with
--    the match-check logic, staff can see "claims to be existing patient — no
--    record found" and act accordingly.
--
--    0 = new patient (claims they have never been here)
--    1 = returning patient (claims they already have a record)
--   NULL/absent = the form did not ask / unknown.
ALTER TABLE intake_submissions ADD COLUMN claimed_returning INTEGER;

-- 2) When a claimed-existing patient has no matching record, staff need a
--    quick, indexable flag rather than recomputing every time. Set at
--    submit/match-check time.
--    0 = not flagged, 1 = claims existing but no record found.
ALTER TABLE intake_submissions ADD COLUMN claimed_no_match INTEGER DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_intake_claimed_no_match ON intake_submissions(claimed_no_match);
