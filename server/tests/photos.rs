//! Photos / attachments route tests.
//!
//! Covers: upload (patient-level + appointment-level), list, fetch raw base64,
//! delete, make-profile, category validation, size guard, and auth gating.
//!
//! Uses a hardcoded 1x1 transparent PNG (well-known base64) so tests are
//! self-contained — no disk reads.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

/// Well-known base64 of a 1x1 transparent PNG (68 bytes raw).
/// Decoded length is small enough to stay well under the 14_000_000-char guard.
const TINY_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/// Create a patient and return its id (photos require an existing patient).
async fn create_patient(app: &TestApp, t: &str) -> i64 {
    let body = serde_json::json!({
        "first_name": "Photo", "last_name": "Test",
        "date_of_birth": "1990-01-01", "phone": "0400000000",
    });
    let resp = app.post("/api/patients").auth(t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    body_json(resp).await["id"].as_i64().unwrap()
}

/// Create an appointment for the given patient and return its id.
async fn create_appointment(app: &TestApp, t: &str, pid: i64) -> i64 {
    let body = serde_json::json!({
        "patient_id": pid,
        // Valid future RFC3339 date (passes date validation).
        "appointment_date": "2099-08-07T09:00:00Z",
        "appointment_type": "consultation",
        "duration_minutes": 30,
        "status": "scheduled",
    });
    let resp = app.post("/api/appointments").auth(t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    body_json(resp).await["id"].as_i64().unwrap()
}

fn upload_body(category: &str) -> serde_json::Value {
    serde_json::json!({
        "category": category,
        "filename": "tiny.png",
        "mime_type": "image/png",
        "caption": "a 1x1 test pixel",
        "data_base64": TINY_PNG_B64,
    })
}

// ---------- upload + fetch round-trip (patient-level) ----------

#[tokio::test]
async fn upload_photo_returns_created_and_metadata() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t).await;

    let resp = app
        .post(&format!("/api/patients/{pid}/photos"))
        .auth(&t)
        .json(&upload_body("document"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["patient_id"], pid);
    assert_eq!(v["category"], "document");
    assert_eq!(v["filename"], "tiny.png");
    assert_eq!(v["mime_type"], "image/png");
    // Metadata responses must NOT include the base64 blob.
    assert!(v.get("data_base64").is_none(), "list/upload metadata must omit data_base64");
    assert!(v["id"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn fetch_photo_returns_base64_data() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t).await;

    // Upload
    let resp = app
        .post(&format!("/api/patients/{pid}/photos"))
        .auth(&t)
        .json(&upload_body("medical"))
        .send()
        .await
        .unwrap();
    let photo_id = body_json(resp).await["id"].as_i64().unwrap();

    // Fetch the raw data back
    let resp = app
        .get(&format!("/api/patients/{pid}/photos/{photo_id}"))
        .auth(&t)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["data"], TINY_PNG_B64, "base64 round-trips exactly");
    assert_eq!(v["mime"], "image/png");
}

#[tokio::test]
async fn list_photos_returns_only_this_patients() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid_a = create_patient(&app, &t).await;
    let pid_b = create_patient(&app, &t).await;

    // Two photos for A, one for B
    for _ in 0..2 {
        app.post(&format!("/api/patients/{pid_a}/photos"))
            .auth(&t).json(&upload_body("document")).send().await.unwrap();
    }
    app.post(&format!("/api/patients/{pid_b}/photos"))
        .auth(&t).json(&upload_body("document")).send().await.unwrap();

    let resp = app.get(&format!("/api/patients/{pid_a}/photos")).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let arr = body_json(resp).await.as_array().unwrap().clone();
    assert_eq!(arr.len(), 2, "patient A has exactly 2 photos");
    assert!(arr.iter().all(|p| p["patient_id"] == pid_a));
}

// ---------- appointment-level attachments ----------

#[tokio::test]
async fn upload_attachment_links_to_appointment_and_patient() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t).await;
    let aid = create_appointment(&app, &t, pid).await;

    // 'profile' category is coerced to 'document' for appointment uploads.
    let resp = app
        .post(&format!("/api/appointments/{aid}/attachments"))
        .auth(&t)
        .json(&upload_body("profile"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["patient_id"], pid, "patient_id resolved from appointment");
    assert_eq!(v["appointment_id"], aid);
    assert_eq!(v["category"], "document", "profile coerced to document for attachments");

    // The file should also appear in the patient's photo list.
    let resp = app.get(&format!("/api/patients/{pid}/photos")).auth(&t).send().await.unwrap();
    let arr = body_json(resp).await.as_array().unwrap().clone();
    assert!(arr.iter().any(|p| p["appointment_id"] == aid), "attachment shows on patient file");

    // And in the appointment's attachment list.
    let resp = app.get(&format!("/api/appointments/{aid}/attachments")).auth(&t).send().await.unwrap();
    let arr = body_json(resp).await.as_array().unwrap().clone();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["appointment_id"], aid);

    // Fetch raw data via the appointment path.
    let photo_id = v["id"].as_i64().unwrap();
    let resp = app
        .get(&format!("/api/appointments/{aid}/attachments/{photo_id}"))
        .auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["data"], TINY_PNG_B64);
}

// ---------- delete ----------

#[tokio::test]
async fn delete_photo_removes_it() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t).await;

    let resp = app
        .post(&format!("/api/patients/{pid}/photos"))
        .auth(&t).json(&upload_body("document")).send().await.unwrap();
    let photo_id = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app
        .delete(&format!("/api/patients/{pid}/photos/{photo_id}"))
        .auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["message"], "Photo deleted");

    // Fetching the deleted photo should 404.
    let resp = app
        .get(&format!("/api/patients/{pid}/photos/{photo_id}"))
        .auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn delete_nonexistent_photo_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t).await;
    let resp = app.delete(&format!("/api/patients/{pid}/photos/999999")).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

// ---------- make-profile ----------

/// Read profile_photo_id for a patient directly from the DB (the patients
/// GET endpoint doesn't surface this column, so we verify at the source).
async fn profile_photo_id(app: &TestApp, pid: i64) -> Option<i64> {
    use sqlx::Row;
    let row = sqlx::query("SELECT profile_photo_id FROM patients WHERE id = ?")
        .bind(pid)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    row.get::<Option<i64>, _>("profile_photo_id")
}

#[tokio::test]
async fn make_profile_sets_profile_photo_id() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t).await;

    // Upload a non-profile photo, then promote it.
    let resp = app
        .post(&format!("/api/patients/{pid}/photos"))
        .auth(&t).json(&upload_body("document")).send().await.unwrap();
    let photo_id = body_json(resp).await["id"].as_i64().unwrap();

    assert_eq!(profile_photo_id(&app, pid).await, None, "no profile photo initially");

    let resp = app
        .post(&format!("/api/patients/{pid}/photos/{photo_id}/make-profile"))
        .auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["message"], "Set as profile photo");

    assert_eq!(profile_photo_id(&app, pid).await, Some(photo_id), "profile_photo_id updated");
}

#[tokio::test]
async fn upload_profile_category_sets_profile_photo_id_automatically() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t).await;

    let resp = app
        .post(&format!("/api/patients/{pid}/photos"))
        .auth(&t).json(&upload_body("profile")).send().await.unwrap();
    let photo_id = body_json(resp).await["id"].as_i64().unwrap();

    assert_eq!(
        profile_photo_id(&app, pid).await,
        Some(photo_id),
        "profile-category upload auto-links profile_photo_id"
    );
}

// ---------- validation + auth ----------

#[tokio::test]
async fn upload_invalid_category_returns_400() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t).await;
    let resp = app
        .post(&format!("/api/patients/{pid}/photos"))
        .auth(&t).json(&upload_body("selfie")).send().await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn upload_to_nonexistent_appointment_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app
        .post("/api/appointments/999999/attachments")
        .auth(&t).json(&upload_body("document")).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn photos_endpoints_require_auth() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/patients/1/photos").send().await.unwrap().status(), 401);
    assert_eq!(app.get("/api/patients/1/photos/1").send().await.unwrap().status(), 401);
    assert_eq!(
        app.post("/api/patients/1/photos").json(&upload_body("document")).send().await.unwrap().status(),
        401
    );
}
