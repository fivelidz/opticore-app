//! Regression tests for the CHECK constraints added in migration
//! `0015_check_constraints.sql`.
//!
//! These are DEFENSE-IN-DEPTH tests. The handler-level input validations
//! (commits 297c6ec, e586101, 1491ebe, 3620dee) already reject bad input with
//! a 400 before it reaches the DB. The CHECK constraints exist so that if a
//! handler bug, a future code path, or a bulk import (data_io) ever bypasses
//! the handler guards, the DB ITSELF rejects the row. The error mapper
//! (commit 44a8e60) then surfaces the CHECK violation as a 400 Bad Request.
//!
//! Each test here goes STRAIGHT to the DB pool (sqlx::query), bypassing the
//! handlers entirely, to prove the constraint is enforced at the storage
//! layer — not just in the request handlers.
//!
//! Constraints covered (mirroring the handler rules):
//!   appointments.duration_minutes         >= 1
//!   blocked_times                         start_at < end_at
//!   clinical_notes.note                   non-empty (trim)
//!   allergies.substance                   non-empty (trim)
//!   osdi_scores.total_score               >= 0
//!   ipl_treatments.session_number         >= 1
//!   booking_settings.reminder_hours_before >= 0
//!   booking_settings.booking_mode         IN ('automatic','approval')
//!   patients.first_name / last_name       non-empty (trim)
//!   intake_submissions.first_name/last_name non-empty (trim)

mod common;

use common::TestApp;
use sqlx::Row;

/// Helper: does a direct INSERT against the DB pool succeed or fail?
/// Returns Ok(rows_affected) on success, or Err on a DB constraint error.
async fn try_insert(app: &TestApp, sql: &str) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(sql).execute(&app.state.db).await?;
    Ok(r.rows_affected())
}

/// Insert a valid patient directly and return its id (for FK references).
async fn seed_patient(app: &TestApp) -> i64 {
    let r = sqlx::query(
        "INSERT INTO patients (mrn, first_name, last_name, date_of_birth) \
         VALUES ('TEST-MRN', 'Test', 'Patient', '1990-01-01')",
    )
    .execute(&app.state.db)
    .await
    .expect("seed patient");
    let row = sqlx::query("SELECT MAX(id) AS m FROM patients")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    row.get::<i64, _>("m")
}

// =====================================================================
// appointments.duration_minutes >= 1
// =====================================================================

#[tokio::test]
async fn check_appointments_rejects_zero_duration() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes) \
         VALUES ({pid}, 'consultation', '2099-01-01T09:00:00Z', 0)"
    );
    let res = try_insert(&app, &sql).await;
    assert!(res.is_err(), "duration_minutes = 0 must be rejected by CHECK");
    let err = res.unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("check"),
        "expected a CHECK constraint error, got: {err}"
    );
}

#[tokio::test]
async fn check_appointments_rejects_negative_duration() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes) \
         VALUES ({pid}, 'consultation', '2099-01-01T09:00:00Z', -5)"
    );
    let res = try_insert(&app, &sql).await;
    assert!(res.is_err(), "duration_minutes = -5 must be rejected by CHECK");
}

#[tokio::test]
async fn check_appointments_accepts_positive_duration() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes) \
         VALUES ({pid}, 'consultation', '2099-01-01T09:00:00Z', 30)"
    );
    let n = try_insert(&app, &sql).await.expect("duration_minutes = 30 accepted");
    assert_eq!(n, 1);
}

// =====================================================================
// blocked_times: start_at < end_at
// =====================================================================

#[tokio::test]
async fn check_blocked_times_rejects_start_ge_end() {
    let app = TestApp::spawn().await;
    // start == end (zero duration)
    let sql = "INSERT INTO blocked_times (start_at, end_at) \
               VALUES ('2099-01-01T10:00:00Z', '2099-01-01T10:00:00Z')";
    let res = try_insert(&app, sql).await;
    assert!(res.is_err(), "start_at == end_at must be rejected by CHECK");

    // start > end (negative duration)
    let sql = "INSERT INTO blocked_times (start_at, end_at) \
               VALUES ('2099-01-01T11:00:00Z', '2099-01-01T10:00:00Z')";
    let res = try_insert(&app, sql).await;
    assert!(res.is_err(), "start_at > end_at must be rejected by CHECK");
}

#[tokio::test]
async fn check_blocked_times_accepts_start_lt_end() {
    let app = TestApp::spawn().await;
    let sql = "INSERT INTO blocked_times (start_at, end_at) \
               VALUES ('2099-01-01T10:00:00Z', '2099-01-01T11:00:00Z')";
    let n = try_insert(&app, sql).await.expect("valid range accepted");
    assert_eq!(n, 1);
}

// =====================================================================
// clinical_notes.note non-empty
// =====================================================================

#[tokio::test]
async fn check_clinical_notes_rejects_empty_note() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO clinical_notes (patient_id, note) VALUES ({pid}, '')"
    );
    let res = try_insert(&app, &sql).await;
    assert!(res.is_err(), "empty note must be rejected by CHECK");
}

#[tokio::test]
async fn check_clinical_notes_rejects_whitespace_only_note() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO clinical_notes (patient_id, note) VALUES ({pid}, '   ')"
    );
    let res = try_insert(&app, &sql).await;
    assert!(res.is_err(), "whitespace-only note must be rejected by CHECK (trim)");
}

#[tokio::test]
async fn check_clinical_notes_accepts_nonempty_note() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO clinical_notes (patient_id, note) VALUES ({pid}, 'Patient reports improvement.')"
    );
    let n = try_insert(&app, &sql).await.expect("valid note accepted");
    assert_eq!(n, 1);
}

// =====================================================================
// allergies.substance non-empty
// =====================================================================

#[tokio::test]
async fn check_allergies_rejects_empty_substance() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO allergies (patient_id, substance) VALUES ({pid}, '')"
    );
    let res = try_insert(&app, &sql).await;
    assert!(res.is_err(), "empty substance must be rejected by CHECK");
}

#[tokio::test]
async fn check_allergies_rejects_whitespace_substance() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO allergies (patient_id, substance) VALUES ({pid}, '\t ')"
    );
    let res = try_insert(&app, &sql).await;
    assert!(res.is_err(), "whitespace substance must be rejected by CHECK (trim)");
}

#[tokio::test]
async fn check_allergies_accepts_nonempty_substance() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO allergies (patient_id, substance) VALUES ({pid}, 'Penicillin')"
    );
    let n = try_insert(&app, &sql).await.expect("valid substance accepted");
    assert_eq!(n, 1);
}

// =====================================================================
// osdi_scores.total_score >= 0
// =====================================================================

#[tokio::test]
async fn check_osdi_rejects_negative_total_score() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO osdi_scores (patient_id, score_date, total_score) \
         VALUES ({pid}, '2026-01-01', -1.5)"
    );
    let res = try_insert(&app, &sql).await;
    assert!(res.is_err(), "negative total_score must be rejected by CHECK");
}

#[tokio::test]
async fn check_osdi_accepts_zero_total_score() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO osdi_scores (patient_id, score_date, total_score) \
         VALUES ({pid}, '2026-01-01', 0)"
    );
    let n = try_insert(&app, &sql).await.expect("total_score = 0 accepted");
    assert_eq!(n, 1);
}

// =====================================================================
// ipl_treatments.session_number >= 1
// =====================================================================

#[tokio::test]
async fn check_ipl_rejects_zero_session_number() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO ipl_treatments (patient_id, treatment_date, session_number) \
         VALUES ({pid}, '2026-01-01T10:00:00Z', 0)"
    );
    let res = try_insert(&app, &sql).await;
    assert!(res.is_err(), "session_number = 0 must be rejected by CHECK");
}

#[tokio::test]
async fn check_ipl_rejects_negative_session_number() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO ipl_treatments (patient_id, treatment_date, session_number) \
         VALUES ({pid}, '2026-01-01T10:00:00Z', -1)"
    );
    let res = try_insert(&app, &sql).await;
    assert!(res.is_err(), "session_number = -1 must be rejected by CHECK");
}

#[tokio::test]
async fn check_ipl_accepts_session_one() {
    let app = TestApp::spawn().await;
    let pid = seed_patient(&app).await;
    let sql = format!(
        "INSERT INTO ipl_treatments (patient_id, treatment_date, session_number) \
         VALUES ({pid}, '2026-01-01T10:00:00Z', 1)"
    );
    let n = try_insert(&app, &sql).await.expect("session_number = 1 accepted");
    assert_eq!(n, 1);
}

// =====================================================================
// booking_settings.reminder_hours_before >= 0
// =====================================================================

#[tokio::test]
async fn check_booking_settings_rejects_negative_reminder_hours() {
    let app = TestApp::spawn().await;
    let sql = "UPDATE booking_settings SET reminder_hours_before = -1 WHERE id = 1";
    let res = try_insert(&app, sql).await;
    assert!(res.is_err(), "negative reminder_hours_before must be rejected by CHECK");
}

#[tokio::test]
async fn check_booking_settings_accepts_zero_reminder_hours() {
    let app = TestApp::spawn().await;
    let sql = "UPDATE booking_settings SET reminder_hours_before = 0 WHERE id = 1";
    let n = try_insert(&app, sql).await.expect("reminder_hours_before = 0 accepted");
    assert_eq!(n, 1);
}

// =====================================================================
// booking_settings.booking_mode IN ('automatic','approval')
// =====================================================================

#[tokio::test]
async fn check_booking_settings_rejects_unknown_mode() {
    let app = TestApp::spawn().await;
    let sql = "UPDATE booking_settings SET booking_mode = 'bogus' WHERE id = 1";
    let res = try_insert(&app, sql).await;
    assert!(res.is_err(), "unknown booking_mode must be rejected by CHECK");
}

#[tokio::test]
async fn check_booking_settings_accepts_valid_modes() {
    let app = TestApp::spawn().await;
    for mode in ["automatic", "approval"] {
        let sql = format!("UPDATE booking_settings SET booking_mode = '{mode}' WHERE id = 1");
        let n = try_insert(&app, &sql).await.expect("valid mode accepted");
        assert_eq!(n, 1, "mode {mode} should be accepted");
    }
}

// =====================================================================
// patients.first_name / last_name non-empty
// =====================================================================

#[tokio::test]
async fn check_patients_rejects_empty_first_name() {
    let app = TestApp::spawn().await;
    let sql = "INSERT INTO patients (mrn, first_name, last_name, date_of_birth) \
               VALUES ('M1', '', 'Last', '1990-01-01')";
    let res = try_insert(&app, sql).await;
    assert!(res.is_err(), "empty first_name must be rejected by CHECK");
}

#[tokio::test]
async fn check_patients_rejects_empty_last_name() {
    let app = TestApp::spawn().await;
    let sql = "INSERT INTO patients (mrn, first_name, last_name, date_of_birth) \
               VALUES ('M2', 'First', '', '1990-01-01')";
    let res = try_insert(&app, sql).await;
    assert!(res.is_err(), "empty last_name must be rejected by CHECK");
}

#[tokio::test]
async fn check_patients_rejects_whitespace_names() {
    let app = TestApp::spawn().await;
    let sql = "INSERT INTO patients (mrn, first_name, last_name, date_of_birth) \
               VALUES ('M3', '  ', '\t', '1990-01-01')";
    let res = try_insert(&app, sql).await;
    assert!(res.is_err(), "whitespace names must be rejected by CHECK (trim)");
}

#[tokio::test]
async fn check_patients_accepts_valid_names() {
    let app = TestApp::spawn().await;
    let sql = "INSERT INTO patients (mrn, first_name, last_name, date_of_birth) \
               VALUES ('M4', 'Jane', 'Doe', '1990-01-01')";
    let n = try_insert(&app, sql).await.expect("valid names accepted");
    assert_eq!(n, 1);
}

// =====================================================================
// intake_submissions.first_name / last_name non-empty
// =====================================================================

#[tokio::test]
async fn check_intake_rejects_empty_first_name() {
    let app = TestApp::spawn().await;
    let sql = "INSERT INTO intake_submissions (first_name, last_name) VALUES ('', 'Last')";
    let res = try_insert(&app, sql).await;
    assert!(res.is_err(), "empty first_name must be rejected by CHECK");
}

#[tokio::test]
async fn check_intake_rejects_empty_last_name() {
    let app = TestApp::spawn().await;
    let sql = "INSERT INTO intake_submissions (first_name, last_name) VALUES ('First', '')";
    let res = try_insert(&app, sql).await;
    assert!(res.is_err(), "empty last_name must be rejected by CHECK");
}

#[tokio::test]
async fn check_intake_accepts_valid_names() {
    let app = TestApp::spawn().await;
    let sql = "INSERT INTO intake_submissions (first_name, last_name) VALUES ('Jane', 'Doe')";
    let n = try_insert(&app, sql).await.expect("valid names accepted");
    assert_eq!(n, 1);
}
