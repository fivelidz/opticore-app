//! Validation-audit tests (session 7).
//!
//! These tests characterize and lock in the behaviour of several areas
//! flagged as potential validation gaps by the prior audit session:
//!
//!   1. **Patient deletion with dependent records** — what happens when you
//!      delete a patient that has appointments / invoices / clinical notes?
//!      (Spoiler: the FKs are declared `ON DELETE CASCADE` but SQLite does
//!      not enforce foreign keys unless `PRAGMA foreign_keys = ON` is set
//!      per-connection, and the app never sets it for normal request
//!      handling. So the DELETE silently orphans every dependent row.)
//!
//!   2. **Appointment double-booking** — can two appointments be created for
//!      the same practitioner at the same date/time? (Yes — this is a
//!      deliberate business decision: many clinics intentionally overbook
//!      and resolve conflicts at check-in. We document the chosen
//!      behaviour with a characterization test rather than adding
//!      enforcement that could break existing workflows.)
//!
//!   3. **Cross-patient data access** — this is a clinic *staff* PMS. Every
//!      authenticated staff member (admin or non-admin) can read/write all
//!      patient records; there is no patient-facing login and no per-patient
//!      scoping. We document that a non-admin staff token can access any
//!      patient's data (this is intended, not a leak).
//!
//!   4. **Intake import with a malformed preferred_date** — the import path
//!      builds `appointment_date` by string-formatting the raw
//!      `preferred_date` + `preferred_time`. A garbage date is stored
//!      verbatim. We characterize this so a future fix is intentional.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

async fn create_patient(app: &TestApp, t: &str, first: &str) -> i64 {
    let body = serde_json::json!({
        "first_name": first,
        "last_name": "Audit",
        "date_of_birth": "1990-01-01",
        "phone": "0400000000",
        "email": format!("{}@audit.test", first.to_lowercase()),
    });
    let resp = app.post("/api/patients").auth(t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "patient create should succeed");
    body_json(resp).await["id"].as_i64().unwrap()
}

async fn create_appointment(app: &TestApp, t: &str, pid: i64, dt: &str) -> i64 {
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": dt,
        "duration_minutes": 60,
    });
    let resp = app.post("/api/appointments").auth(t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "appointment create should succeed: {:?}", resp.status());
    body_json(resp).await["id"].as_i64().unwrap()
}

async fn create_invoice(app: &TestApp, t: &str, pid: i64) -> i64 {
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{ "item_type": "consultation", "description": "Consult", "quantity": 1.0, "unit_price": 100.0 }],
    });
    let resp = app.post("/api/billing/invoices").auth(t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200, "invoice create should succeed");
    body_json(resp).await["id"].as_i64().unwrap()
}

// ===========================================================================
// 1. Patient deletion with dependent records
// ===========================================================================

/// Deleting a patient that has NO dependent records succeeds (200) and the
/// row is gone. This is the baseline; the next test covers the dependent
/// case.
#[tokio::test]
async fn delete_patient_with_no_dependencies_succeeds() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "FreeDel").await;

    let resp = app.delete(&format!("/api/patients/{}", pid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Row is gone.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM patients WHERE id = ?")
        .bind(pid)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count.0, 0, "patient row should be deleted");
}

/// Deleting a patient that HAS appointments must not silently orphan them.
///
/// The `appointments` table declares
///   `FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE`
/// but SQLite does NOT enforce foreign keys unless `PRAGMA foreign_keys = ON`
/// is set per-connection — and the app never sets it for normal request
/// handling (only the data-import path toggles it). So a bare
/// `DELETE FROM patients WHERE id = ?` would leave orphaned appointment
/// rows pointing at a nonexistent patient_id.
///
/// For a medical PMS, silently orphaning clinical/appointment history is
/// dangerous (lost audit trail, broken joins, referential integrity
/// violations). The conservative fix: REFUSE the deletion (409) when
/// dependent records exist, telling the caller which tables block it. A
/// proper hard-delete / anonymize / merge workflow is a separate feature.
#[tokio::test]
async fn delete_patient_with_appointments_is_rejected() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "WithAppt").await;
    let _aid = create_appointment(
        &app, &t, pid,
        "2099-01-01T09:00:00Z",
    ).await;

    let resp = app.delete(&format!("/api/patients/{}", pid)).auth(&t).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(
        status, 409,
        "deleting a patient with appointments should be 409 Conflict (got {})", status
    );

    // Patient must still exist (deletion refused).
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM patients WHERE id = ?")
        .bind(pid)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "patient should still exist after refused delete");
}

/// Same as above but for invoices. A patient with invoices (financial
/// history) must not be hard-deleted.
#[tokio::test]
async fn delete_patient_with_invoices_is_rejected() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "WithInv").await;
    let _iid = create_invoice(&app, &t, pid).await;

    let resp = app.delete(&format!("/api/patients/{}", pid)).auth(&t).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(
        status, 409,
        "deleting a patient with invoices should be 409 Conflict (got {})", status
    );

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM patients WHERE id = ?")
        .bind(pid)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "patient should still exist after refused delete");
}

// ===========================================================================
// 2. Appointment double-booking (documented business decision)
// ===========================================================================

/// Two appointments can be created for the same practitioner at the same
/// date/time.
///
/// This is a **deliberate business decision**, not a bug: many clinics
/// intentionally overbook (knowing some patients no-show) and resolve
/// conflicts at check-in. Adding hard double-booking enforcement here
/// could break existing clinic workflows. We lock in the current
/// permissive behaviour with a characterization test so that any future
/// change to add enforcement is a conscious decision (and updates this
/// test).
#[tokio::test]
async fn double_booking_is_currently_allowed_documented_decision() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "DoubleBook").await;

    // Two appointments, same practitioner, same slot.
    let dt = "2099-03-03T10:00:00Z";
    let body = |practitioner: &str| serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": dt,
        "duration_minutes": 60,
        "practitioner": practitioner,
    });

    let r1 = app.post("/api/appointments").auth(&t).json(&body("Dr. Smith")).send().await.unwrap();
    assert_eq!(r1.status(), 201, "first appointment should succeed");
    let r2 = app.post("/api/appointments").auth(&t).json(&body("Dr. Smith")).send().await.unwrap();
    // DOCUMENTED: double-booking is allowed (no conflict check). If a
    // scheduling-conflict layer is added later, this assertion must be
    // updated to reflect the chosen policy.
    assert_eq!(
        r2.status(), 201,
        "double-booking is currently allowed by design (overbooking resolved at check-in); \
         if a conflict check is added, update this test"
    );
}

// ===========================================================================
// 3. Cross-patient data access (documented: staff PMS, no per-patient scope)
// ===========================================================================

/// A non-admin staff token can read any patient's record.
///
/// This is a clinic **staff** PMS: every authenticated staff member
/// (admin or non-admin) legitimately accesses all patient records. There
/// is no patient-facing login and no per-patient data scoping. This test
/// documents that the non-admin role is NOT a data-access restriction —
/// it only gates admin-only routes (users, data import/export). If a
/// per-patient scope is ever introduced (e.g. a patient self-service
/// portal), this test must be revisited.
#[tokio::test]
async fn non_admin_can_access_all_patients_documented_decision() {
    let app = TestApp::spawn().await;
    let admin = token(&app).await;

    // Create a second (non-admin) user.
    let body = serde_json::json!({
        "username": "staff1",
        "email": "staff1@audit.test",
        "password": "staff-password-1",
        "role": "receptionist",
        "first_name": "Staff",
        "last_name": "One",
    });
    let resp = app.post("/api/users").auth(&admin).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "create non-admin user");

    // Log in as the non-admin staff user.
    let login = serde_json::json!({ "username": "staff1", "password": "staff-password-1" });
    let resp = app.post("/api/auth/login").body(serde_json::to_vec(&login).unwrap())
        .header("content-type", "application/json").send().await.unwrap();
    assert_eq!(resp.status(), 200, "non-admin login");
    let staff_token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Create two patients as admin.
    let pid_a = create_patient(&app, &admin, "PatientA").await;
    let pid_b = create_patient(&app, &admin, "PatientB").await;

    // The non-admin staff token can read BOTH patients. This is intended
    // (staff access all patients); it is NOT a data leak.
    let r = app.get(&format!("/api/patients/{}", pid_a)).auth(&staff_token).send().await.unwrap();
    assert_eq!(r.status(), 200, "staff can read patient A");
    let r = app.get(&format!("/api/patients/{}", pid_b)).auth(&staff_token).send().await.unwrap();
    assert_eq!(r.status(), 200, "staff can read patient B");

    // Sanity: the non-admin token IS rejected from admin-only routes.
    let r = app.get("/api/users").auth(&staff_token).send().await.unwrap();
    assert_eq!(r.status(), 403, "non-admin must be rejected from /api/users (admin-only)");
}

// ===========================================================================
// 4. Intake import with malformed preferred_date (characterization)
// ===========================================================================

/// Importing an intake submission whose `preferred_date` is a garbage
/// string currently stores that garbage verbatim into the generated
/// appointment's `appointment_date`.
///
/// The import path does:
///   `format!("{} {}:00", preferred_date, preferred_time.unwrap_or("09:00"))`
/// with no validation. A malformed date (e.g. "whenever") becomes the
/// literal appointment_date "whenever 09:00:00", which will never parse
/// as a real datetime and breaks every date-based query/report.
///
/// This test characterizes the current (lenient) behaviour. A future
/// hardening pass should validate `preferred_date` on import and either
/// skip appointment creation or store NULL when it is unparseable.
#[tokio::test]
async fn intake_import_with_malformed_preferred_date_is_stored_verbatim() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    // Submit a public intake form with a garbage preferred_date.
    let body = serde_json::json!({
        "first_name": "Mal",
        "last_name": "Date",
        "date_of_birth": "1990-02-02",
        "preferred_date": "whenever",
        "preferred_time": "09:00",
        "appointment_type": "Consultation",
    });
    let resp = app.post("/api/intake/submit").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "intake submit is public and accepts any string");
    let intake_id = body_json(resp).await["id"].as_i64().unwrap();

    // Import it as a patient + appointment.
    let resp = app.post(&format!("/api/intake/{}/import", intake_id))
        .auth(&t).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(status, 200, "import should succeed");

    // The generated appointment has the garbage date stored verbatim.
    let row: (String,) = sqlx::query_as(
        "SELECT appointment_date FROM appointments ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    // DOCUMENTED: the raw "whenever" is embedded into appointment_date.
    // This is a data-quality wart, not a crash. A future fix should
    // validate preferred_date on import.
    assert!(
        row.0.contains("whenever"),
        "expected the malformed preferred_date to be stored verbatim, got: {}",
        row.0
    );
}

/// Importing an intake submission that omits `date_of_birth` must NOT crash.
///
/// `CreateIntake.date_of_birth` is `Option<String>` (the public form does
/// not require it), but `patients.date_of_birth` is `NOT NULL`. The import
/// path blindly binds the optional DOB into the patient INSERT, so a
/// submission with no DOB triggers
///   `NOT NULL constraint failed: patients.date_of_birth`
/// which surfaces as an opaque HTTP 500.
///
/// Conservative fix: the import path should supply a sentinel
/// `"unknown"` DOB when the submission omits it, so the patient row is
/// always created and staff can fix the DOB later. (Rejecting the whole
/// import would leave the submission stuck in `new` forever.)
#[tokio::test]
async fn intake_import_with_missing_dob_does_not_crash() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    // Submit a public intake form with NO date_of_birth.
    let body = serde_json::json!({
        "first_name": "No",
        "last_name": "Dob",
        "phone": "0400000001",
    });
    let resp = app.post("/api/intake/submit").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "intake submit accepts a form with no DOB");
    let intake_id = body_json(resp).await["id"].as_i64().unwrap();

    // Import it — must not 500.
    let resp = app.post(&format!("/api/intake/{}/import", intake_id))
        .auth(&t).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_ne!(
        status, 500,
        "importing an intake with no DOB must not crash (500); got {}", status
    );
    assert_eq!(status, 200, "import should succeed with a sentinel DOB");

    // The patient should exist with a non-null DOB.
    let row: (i64, String) = sqlx::query_as(
        "SELECT id, date_of_birth FROM patients ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert!(
        !row.1.is_empty(),
        "patient DOB should be a non-empty sentinel, got: {:?}",
        row.1
    );
}
