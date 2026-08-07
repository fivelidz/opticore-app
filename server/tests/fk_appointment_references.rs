//! Regression tests for the foreign-key declarations added in migration
//! `0014_fk_appointment_references.sql`:
//!
//!   * `invoices.appointment_id`        -> appointments(id) ON DELETE SET NULL
//!   * `patient_photos.appointment_id`  -> appointments(id) ON DELETE SET NULL
//!
//! These columns previously had NO constraint, so deleting an appointment left
//! dangling references. The migration rebuilds both tables (SQLite's only way
//! to add a FK to an existing column) with `ON DELETE SET NULL`.
//!
//! What we characterize here:
//!   1. Insert with a VALID appointment_id succeeds and the link is stored.
//!   2. Insert with a NONEXISTENT appointment_id is rejected by the FK
//!      (surfaces as HTTP 400 Bad Request — the error mapper maps FK
//!      violations to BadRequest so clients get an actionable error).
//!   3. Deleting the appointment NULLS the appointment_id on the dependent
//!      row (SET NULL) — the invoice/photo itself is preserved.
//!
//! The existing 253-test suite already covers that the rebuild preserves
//! schema/data (those tests create invoices + photos and still pass); this
//! file adds the FK-specific behaviour.

mod common;

use common::{body_json, TestApp};
use sqlx::Row;

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

async fn create_patient(app: &TestApp, t: &str, first: &str) -> i64 {
    let body = serde_json::json!({
        "first_name": first, "last_name": "FK", "date_of_birth": "1990-01-01",
    });
    let r = app.post("/api/patients").auth(t).json(&body).send().await.unwrap();
    body_json(r).await["id"].as_i64().unwrap()
}

async fn create_appointment(app: &TestApp, t: &str, pid: i64) -> i64 {
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_date": "2099-08-07T09:00:00Z",
        "appointment_type": "consultation",
        "duration_minutes": 30,
        "status": "scheduled",
    });
    let resp = app.post("/api/appointments").auth(t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    body_json(resp).await["id"].as_i64().unwrap()
}

/// Well-known base64 of a 1x1 transparent PNG (matches the photos test suite).
const TINY_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/// Read invoices.appointment_id straight from the DB (the list endpoint
/// surfaces it too, but a direct query is unambiguous for the SET NULL check).
async fn invoice_appointment_id(app: &TestApp, inv_id: i64) -> Option<i64> {
    let row = sqlx::query("SELECT appointment_id FROM invoices WHERE id = ?")
        .bind(inv_id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    row.get::<Option<i64>, _>("appointment_id")
}

/// Read patient_photos.appointment_id straight from the DB.
async fn photo_appointment_id(app: &TestApp, photo_id: i64) -> Option<i64> {
    let row = sqlx::query("SELECT appointment_id FROM patient_photos WHERE id = ?")
        .bind(photo_id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    row.get::<Option<i64>, _>("appointment_id")
}

// =====================================================================
// invoices.appointment_id FK
// =====================================================================

#[tokio::test]
async fn invoice_with_valid_appointment_id_links_and_persists() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "InvValid").await;
    let aid = create_appointment(&app, &t, pid).await;

    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_id": aid,
        "items": [
            { "item_type": "consultation", "description": "Consult",
              "quantity": 1.0, "unit_price": 100.0, "tax_rate": 0.10 },
        ],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "invoice with valid appointment_id should succeed");
    let v = body_json(resp).await;
    let inv_id = v["id"].as_i64().unwrap();
    assert_eq!(v["appointment_id"], aid, "response echoes the linked appointment_id");

    // Confirm it landed in the DB.
    assert_eq!(invoice_appointment_id(&app, inv_id).await, Some(aid));
}

#[tokio::test]
async fn invoice_with_nonexistent_appointment_id_is_rejected_by_fk() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "InvBad").await;

    // 9_999_999 does not exist in appointments -> FK violation.
    // The error mapper (error.rs) maps FK violations to 400 Bad Request so
    // clients get an actionable error instead of an opaque 500.
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_id": 9_999_999,
        "items": [
            { "item_type": "consultation", "description": "Consult",
              "quantity": 1.0, "unit_price": 100.0, "tax_rate": 0.10 },
        ],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "FK violation on appointment_id surfaces as 400 Bad Request");
}

#[tokio::test]
async fn deleting_appointment_nulls_invoice_appointment_id() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "InvDel").await;
    let aid = create_appointment(&app, &t, pid).await;

    // Create an invoice linked to the appointment.
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_id": aid,
        "items": [
            { "item_type": "consultation", "description": "Consult",
              "quantity": 1.0, "unit_price": 100.0, "tax_rate": 0.10 },
        ],
    });
    let r = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    let inv_id = body_json(r).await["id"].as_i64().unwrap();
    assert_eq!(invoice_appointment_id(&app, inv_id).await, Some(aid));

    // Delete the appointment.
    let resp = app.delete(&format!("/api/appointments/{aid}")).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // The invoice must still exist, but its appointment_id is now NULL.
    assert_eq!(
        invoice_appointment_id(&app, inv_id).await,
        None,
        "ON DELETE SET NULL: invoice kept, appointment_id cleared"
    );

    // And the invoice row itself is intact (not deleted).
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM invoices WHERE id = ?")
        .bind(inv_id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "invoice row preserved after appointment delete");
}

// =====================================================================
// patient_photos.appointment_id FK
// =====================================================================

#[tokio::test]
async fn photo_with_valid_appointment_id_links_and_persists() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "PhotoValid").await;
    let aid = create_appointment(&app, &t, pid).await;

    // Patient-level upload with an explicit, valid appointment_id. (The
    // appointment-scoped upload endpoint /appointments/:id/attachments would
    // also work, but this path exercises the INSERT directly.)
    let body = serde_json::json!({
        "category": "document",
        "filename": "tiny.png",
        "mime_type": "image/png",
        "caption": "valid appt link",
        "data_base64": TINY_PNG_B64,
        "appointment_id": aid,
    });
    let resp = app
        .post(&format!("/api/patients/{pid}/photos"))
        .auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "photo with valid appointment_id should succeed");
    let v = body_json(resp).await;
    let photo_id = v["id"].as_i64().unwrap();
    assert_eq!(v["appointment_id"], aid);

    assert_eq!(photo_appointment_id(&app, photo_id).await, Some(aid));
}

#[tokio::test]
async fn photo_with_nonexistent_appointment_id_is_rejected_by_fk() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "PhotoBad").await;

    // The patient-level upload route does NOT pre-validate appointment_id
    // (unlike the appointment-scoped route, which 404s on a missing appt).
    // So a bad appointment_id reaches the INSERT and trips the new FK ->
    // sqlx::Error::Database(foreign_key_violation) -> ApiError::BadRequest -> 400.
    let body = serde_json::json!({
        "category": "document",
        "filename": "tiny.png",
        "mime_type": "image/png",
        "caption": "bad appt link",
        "data_base64": TINY_PNG_B64,
        "appointment_id": 9_999_999,
    });
    let resp = app
        .post(&format!("/api/patients/{pid}/photos"))
        .auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "FK violation on appointment_id surfaces as 400 Bad Request");
}

#[tokio::test]
async fn deleting_appointment_nulls_photo_appointment_id() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "PhotoDel").await;
    let aid = create_appointment(&app, &t, pid).await;

    // Upload a photo linked to the appointment.
    let body = serde_json::json!({
        "category": "document",
        "filename": "tiny.png",
        "mime_type": "image/png",
        "caption": "will be unlinked",
        "data_base64": TINY_PNG_B64,
        "appointment_id": aid,
    });
    let r = app
        .post(&format!("/api/patients/{pid}/photos"))
        .auth(&t).json(&body).send().await.unwrap();
    assert_eq!(r.status(), 201);
    let photo_id = body_json(r).await["id"].as_i64().unwrap();
    assert_eq!(photo_appointment_id(&app, photo_id).await, Some(aid));

    // Delete the appointment.
    let resp = app.delete(&format!("/api/appointments/{aid}")).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Photo kept, appointment_id cleared.
    assert_eq!(
        photo_appointment_id(&app, photo_id).await,
        None,
        "ON DELETE SET NULL: photo kept, appointment_id cleared"
    );

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM patient_photos WHERE id = ?")
        .bind(photo_id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "photo row preserved after appointment delete");
}
