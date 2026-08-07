//! Error-handling coverage: assert that routes return a clean 500
//! (`ApiError::Internal`) when the database fails — never a panic, never a 200
//! with empty data, never a wrong status.
//!
//! ## How errors are injected
//!
//! We close the app's `SqlitePool` (`state.db.close()`) before calling the
//! route. Every subsequent `acquire()` on that pool returns
//! `sqlx::Error::PoolClosed`, which the `From<sqlx::Error> for ApiError` impl
//! maps to `ApiError::Internal` → HTTP 500. This is the same code path a real
//! DB outage (disk full, locked, connection lost) takes.
//!
//! ## Why pool.close() (not a lock or a second connection)
//!
//! - Deterministic: no timing window, no busy-timeout guessing.
//! - No false positives: the error is guaranteed, not probabilistic.
//! - No deep changes: we don't need to inject a trait object or mock the pool.
//!
//! ## What we assert
//!
//! 1. The status is exactly 500 (not 200, not 4xx).
//! 2. The body is a JSON error object `{ "error": "..." }` (the ApiError
//!    IntoResponse shape), not an empty body or HTML stack trace.
//! 3. The server did not panic (the request completed and returned a response).
//!
//! The pattern below covers representative routes across the major route
//! groups (patients, appointments, analytics, data_io). The same pattern
//! applies to every other route — they all use `?` on sqlx results.

mod common;

use axum::http::StatusCode;
use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

/// Close the app's DB pool so all subsequent queries fail with PoolClosed.
async fn kill_db(app: &TestApp) {
    app.state.db.close().await;
}

/// Assert a response is a clean 500 (no panic).
fn assert_500(resp: axum::response::Response) {
    assert_eq!(
        resp.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "DB failure should surface as 500, got {}",
        resp.status()
    );
}

// ---------- patients ----------

#[tokio::test]
async fn patients_list_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/patients").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn patient_create_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/patients")
        .auth(&t)
        .json(&serde_json::json!({
            "first_name": "X", "last_name": "Y", "date_of_birth": "2000-01-01"
        }))
        .send()
        .await
        .expect("no panic");
    assert_500(resp);
}

// ---------- appointments ----------

#[tokio::test]
async fn appointments_list_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/appointments").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- analytics ----------

#[tokio::test]
async fn analytics_overview_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/analytics/overview").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- data_io ----------

#[tokio::test]
async fn data_export_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/data/export")
        .auth(&t)
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn data_import_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Build a valid (minimal) snapshot string before killing the DB, so the
    // only failure is the import's INSERT, not snapshot parsing.
    let snapshot = serde_json::json!({
        "meta": {
            "snapshot_version": 1,
            "app_version": "0.0.0",
            "exported_at": "2024-01-01T00:00:00Z",
            "table_count": 0,
            "row_count": 0,
            "encrypted": false
        },
        "data": {}
    });
    let snapshot_str = serde_json::to_string(&snapshot).unwrap();
    kill_db(&app).await;
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({ "snapshot": snapshot_str, "mode": "merge" }))
        .send()
        .await
        .expect("no panic");
    assert_500(resp);
}

// ---------- body shape ----------

/// The 500 response must be a JSON `{ "error": "..." }` object, matching the
/// `ApiError::IntoResponse` shape — not an empty body, not HTML, not a stack
/// trace. This guards against a regression where errors are silently swallowed.
#[tokio::test]
async fn db_failure_response_body_is_json_error_object() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/patients").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 500);
    let v = body_json(resp).await;
    assert!(v.is_object(), "error body should be a JSON object");
    let err = v.get("error").and_then(|e| e.as_str());
    assert!(err.is_some(), "error body should contain an \"error\" string field");
    assert!(!err.unwrap().is_empty(), "error message should not be empty");
}

// ===========================================================================
// Full route-group coverage (session 4).
//
// Each block below covers one route group's representative endpoints. The
// pattern is identical to the patients/appointments/analytics/data_io tests
// above: kill the pool, call the route, assert 500.
// ===========================================================================

// ---------- clinical (notes / allergies / osdi / ipl) ----------

#[tokio::test]
async fn clinical_list_notes_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/patients/1/notes").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn clinical_add_note_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/patients/1/notes")
        .auth(&t)
        .json(&serde_json::json!({ "patient_id": 1, "author": "x", "category": "general", "note": "y" }))
        .send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn clinical_list_allergies_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/patients/1/allergies").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn clinical_list_osdi_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/patients/1/osdi").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn clinical_list_ipl_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/patients/1/ipl").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- billing (catalog / invoices / payments) ----------

#[tokio::test]
async fn billing_consultation_types_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/billing/consultation-types").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn billing_services_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/billing/services").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn billing_invoices_by_patient_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/billing/invoices/patient/1").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn billing_create_invoice_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/billing/invoices")
        .auth(&t)
        .json(&serde_json::json!({
            "patient_id": 1, "appointment_id": null, "payment_method": "cash", "notes": null,
            "items": [{ "item_type": "consult", "description": "x", "quantity": 1.0,
                        "unit_price": 100.0, "discount_percent": 0.0, "tax_rate": 0.0 }]
        }))
        .send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn billing_payments_by_invoice_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/billing/payments/invoice/1").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn billing_add_payment_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/billing/payments")
        .auth(&t)
        .json(&serde_json::json!({
            "invoice_id": 1, "amount": 50.0, "payment_method": "cash",
            "reference_number": null, "notes": null
        }))
        .send().await.expect("no panic");
    assert_500(resp);
}

// ---------- messages ----------

#[tokio::test]
async fn messages_list_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/messages").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn messages_receive_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/messages")
        .auth(&t)
        .json(&serde_json::json!({
            "channel": "email", "from_name": "x", "from_contact": "y",
            "subject": "s", "body": "b", "thread_id": null
        }))
        .send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn messages_mark_read_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.post("/api/messages/1/read").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- intake ----------

#[tokio::test]
async fn intake_list_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/intake").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn intake_submit_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    kill_db(&app).await;
    let resp = app
        .post("/api/intake/submit")
        .json(&serde_json::json!({
            "first_name": "A", "last_name": "B", "date_of_birth": "2000-01-01",
            "phone": "123", "email": null, "address": null, "medicare_number": null,
            "preferred_date": null, "preferred_time": null, "appointment_type": null,
            "symptoms": null
        }))
        .send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn intake_archive_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.post("/api/intake/1/archive").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

/// Regression: auto_import previously swallowed per-submission import errors
/// via `if import_one(...).await.is_ok()`, returning 200 with a partial count
/// even when the DB was completely down. Now propagates as 500.
#[tokio::test]
async fn intake_auto_import_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.post("/api/intake/auto-import").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- photos ----------

#[tokio::test]
async fn photos_list_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/patients/1/photos").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn photos_upload_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/patients/1/photos")
        .auth(&t)
        .json(&serde_json::json!({
            "patient_id": 1, "appointment_id": null, "category": "profile",
            "filename": "x.png", "mime_type": "image/png", "caption": null,
            "data_base64": "iVBORw0KGgo="
        }))
        .send().await.expect("no panic");
    assert_500(resp);
}

// ---------- users (CRUD) ----------

#[tokio::test]
async fn users_list_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/users").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn users_create_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/users")
        .auth(&t)
        .json(&serde_json::json!({
            "username": "newuser", "email": "n@e.com", "password": "1234",
            "role": "doctor", "first_name": "A", "last_name": "B"
        }))
        .send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn users_toggle_active_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.post("/api/users/1/toggle").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn users_delete_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.delete("/api/users/1").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- calendar ----------

#[tokio::test]
async fn calendar_range_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/calendar/2026-01-01/2026-12-31").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- blocked ----------

#[tokio::test]
async fn blocked_list_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/blocked-times").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn blocked_create_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/blocked-times")
        .auth(&t)
        .json(&serde_json::json!({
            "start_at": "2026-01-01 09:00", "end_at": "2026-01-01 17:00",
            "reason": "lunch", "practitioner": null,
            "all_day": null, "is_recurring": null
        }))
        .send().await.expect("no panic");
    assert_500(resp);
}

// ---------- booking_settings ----------

#[tokio::test]
async fn booking_settings_get_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/booking-settings").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn booking_notifications_list_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/booking-notifications").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- change_password ----------

#[tokio::test]
async fn change_password_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/auth/change-password")
        .auth(&t)
        .json(&serde_json::json!({
            "current_password": "test-admin-pw", "new_password": "new-pass-123"
        }))
        .send().await.expect("no panic");
    assert_500(resp);
}

// ---------- patient_detail ----------

#[tokio::test]
async fn patient_detail_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/patients/1/detail").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- public_api ----------

#[tokio::test]
async fn public_availability_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    kill_db(&app).await;
    let resp = app.get("/api/public/availability/7").send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn public_appointment_types_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    kill_db(&app).await;
    let resp = app.get("/api/public/appointment-types").send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn public_match_patient_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    kill_db(&app).await;
    let resp = app
        .post("/api/public/match-patient")
        .json(&serde_json::json!({
            "first_name": "John", "last_name": "Doe",
            "date_of_birth": "2000-01-01", "phone": null, "email": null
        }))
        .send().await.expect("no panic");
    assert_500(resp);
}

// ---------- auth (me) ----------

#[tokio::test]
async fn auth_me_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/auth/me").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- sync (lib.rs handlers) ----------

/// Regression: sync_status previously swallowed DB errors via `.ok().flatten()`
/// on both queries, returning 200 with `pending_intake: 0` and null
/// `last_worker_intake` — a DB outage was indistinguishable from "no pending
/// intake." Now propagates as 500.
#[tokio::test]
async fn sync_status_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/sync/status").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

/// Regression: sync_now previously caught ALL errors (including DB errors) and
/// returned 200 with `{"ok": false, "error": "..."}`. A DB outage returned 200.
/// Now propagates DB errors as 500.
///
/// NOTE: run_sync_cycle returns Ok(()) early when WORKER_URL is unset (no DB
/// access), so we set a dummy WORKER_URL to force the DB-touching push/pull
/// path. The env var is read fresh on each call; no other test touches it, so
/// there is no parallel-test race.
#[tokio::test]
async fn sync_now_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    std::env::set_var("WORKER_URL", "https://dummy-worker.example.com");
    kill_db(&app).await;
    let resp = app.post("/api/sync/now").auth(&t).send().await.expect("no panic");
    std::env::remove_var("WORKER_URL");
    assert_500(resp);
}

// ---------- analytics (remaining series endpoints) ----------

#[tokio::test]
async fn analytics_revenue_series_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/analytics/revenue/30").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn analytics_no_show_rate_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/analytics/no-show-rate").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn analytics_hour_distribution_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/analytics/hour-distribution").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- appointments (remaining endpoints) ----------

#[tokio::test]
async fn appointments_today_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/appointments/today").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn appointments_create_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app
        .post("/api/appointments")
        .auth(&t)
        .json(&serde_json::json!({
            "patient_id": 1, "appointment_type": "consult",
            "appointment_date": "2026-01-01 09:00", "duration_minutes": 60,
            "practitioner": "Dr X", "notes": null
        }))
        .send().await.expect("no panic");
    assert_500(resp);
}

// ---------- patients (remaining endpoints) ----------

#[tokio::test]
async fn patients_get_one_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/patients/1").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn patients_enriched_list_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/patients/enriched/list").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

#[tokio::test]
async fn patients_delete_returns_500_on_db_failure() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.delete("/api/patients/1").auth(&t).send().await.expect("no panic");
    assert_500(resp);
}

// ---------- data_io (version endpoint) ----------

#[tokio::test]
async fn data_version_returns_200_without_db() {
    // version_info is a pure constant — it does not touch the DB, so it should
    // still return 200 even when the pool is closed. This documents that
    // intentional behaviour (not every route must 500 on DB failure).
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    kill_db(&app).await;
    let resp = app.get("/api/data/version").auth(&t).send().await.expect("no panic");
    assert_eq!(resp.status(), StatusCode::OK);
}
