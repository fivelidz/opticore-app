//! Appointments + blocked-times + calendar tests.
//! These three route modules are tightly coupled (calendar joins both), so
//! they're tested together.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

/// Create a patient and return its id (for appointment FK).
async fn create_patient(app: &TestApp, t: &str, first: &str) -> i64 {
    let body = serde_json::json!({
        "first_name": first, "last_name": "Test", "date_of_birth": "1990-01-01",
    });
    let r = app.post("/api/patients").auth(t).json(&body).send().await.unwrap();
    body_json(r).await["id"].as_i64().unwrap()
}

// ---------- Appointments ----------

#[tokio::test]
async fn list_appointments_returns_seeded() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/appointments").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    // 0002_seed.sql (5) + 0011_demo_past_appointments.sql (7) = 12 seeded.
    // Don't assert an exact count (migrations may grow); just confirm non-empty
    // and that the count matches the array length.
    let count = v["count"].as_i64().unwrap() as usize;
    assert!(count > 0, "seeded appointments should exist");
    assert_eq!(v["appointments"].as_array().unwrap().len(), count);
}

#[tokio::test]
async fn create_appointment_returns_created() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Appt").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": "2099-01-15T09:00:00Z",
        "duration_minutes": 30,
        "practitioner": "Dr. Test",
    });
    let resp = app.post("/api/appointments").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["patient_id"], pid);
    assert_eq!(v["appointment_type"], "Consultation");
    assert_eq!(v["status"], "scheduled");
    // The date is normalized to "YYYY-MM-DD HH:MM:SS".
    assert!(v["appointment_date"].as_str().unwrap().starts_with("2099-01-15"));
}

#[tokio::test]
async fn get_appointment_by_id() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Seed appointment id=1.
    let resp = app.get("/api/appointments/1").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["id"], 1);
    assert_eq!(v["appointment_type"], "Dry Eye Consultation");
}

#[tokio::test]
async fn get_nonexistent_appointment_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/appointments/99999").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn update_appointment_changes_status() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Upd").await;
    // Create.
    let body = serde_json::json!({
        "patient_id": pid, "appointment_type": "Follow-up",
        "appointment_date": "2099-02-20T10:00:00Z", "duration_minutes": 15,
    });
    let r = app.post("/api/appointments").auth(&t).json(&body).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    // Update status to "confirmed".
    let upd = serde_json::json!({
        "appointment_type": "Follow-up",
        "appointment_date": "2099-02-20T10:00:00Z",
        "duration_minutes": 15,
        "status": "confirmed",
    });
    let resp = app.put(&format!("/api/appointments/{}", id)).auth(&t).json(&upd).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["status"], "confirmed");
}

#[tokio::test]
async fn delete_appointment_removes_it() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Del").await;
    let body = serde_json::json!({
        "patient_id": pid, "appointment_type": "Imaging",
        "appointment_date": "2099-03-30T14:00:00Z", "duration_minutes": 30,
    });
    let r = app.post("/api/appointments").auth(&t).json(&body).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let resp = app.delete(&format!("/api/appointments/{}", id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    // Confirm gone.
    let resp = app.get(&format!("/api/appointments/{}", id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn delete_nonexistent_appointment_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.delete("/api/appointments/99999").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn appointments_endpoints_require_auth() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/appointments").send().await.unwrap().status(), 401);
    assert_eq!(app.get("/api/appointments/1").send().await.unwrap().status(), 401);
}

// ---------- Blocked times ----------

#[tokio::test]
async fn list_blocked_times_returns_seeded() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/blocked-times").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    // 0002_seed.sql inserts blocked times (time-relative; count may vary by
    // run date). Just confirm the endpoint returns an array.
    assert!(v.is_array(), "blocked times should be an array");
}

#[tokio::test]
async fn create_blocked_time_returns_created() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "start_at": "2099-06-01T12:00:00Z",
        "end_at": "2099-06-01T13:00:00Z",
        "reason": "Lunch",
        "practitioner": "Dr. Test",
    });
    let resp = app.post("/api/blocked-times").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["reason"], "Lunch");
    assert_eq!(v["practitioner"], "Dr. Test");
    assert_eq!(v["all_day"], false);
}

#[tokio::test]
async fn update_blocked_time() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Create.
    let body = serde_json::json!({
        "start_at": "2099-07-01T09:00:00Z", "end_at": "2099-07-01T10:00:00Z",
        "reason": "Original",
    });
    let r = app.post("/api/blocked-times").auth(&t).json(&body).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let upd = serde_json::json!({
        "start_at": "2099-07-01T09:00:00Z", "end_at": "2099-07-01T10:00:00Z",
        "reason": "Updated", "all_day": true,
    });
    let resp = app.put(&format!("/api/blocked-times/{}", id)).auth(&t).json(&upd).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["reason"], "Updated");
    assert_eq!(v["all_day"], true);
}

#[tokio::test]
async fn update_nonexistent_blocked_time_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "start_at": "2099-08-01T09:00:00Z", "end_at": "2099-08-01T10:00:00Z",
    });
    let resp = app.put("/api/blocked-times/99999").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn delete_blocked_time() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "start_at": "2099-09-01T09:00:00Z", "end_at": "2099-09-01T10:00:00Z",
        "reason": "Temp",
    });
    let r = app.post("/api/blocked-times").auth(&t).json(&body).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let resp = app.delete(&format!("/api/blocked-times/{}", id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn delete_nonexistent_blocked_time_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.delete("/api/blocked-times/99999").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn blocked_times_require_auth() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/blocked-times").send().await.unwrap().status(), 401);
}

// ---------- Calendar (combined view) ----------

#[tokio::test]
async fn calendar_range_returns_appointments_and_blocked() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Seed data is "now + 1-4 days". Query a wide window around today.
    let from = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let to = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(30))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();
    let uri = format!("/api/calendar/{}/{}", from, to);
    let resp = app.get(&uri).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    let events = v.as_array().unwrap();
    assert!(!events.is_empty(), "seeded calendar should have events");
    // Should contain at least one appointment and one blocked.
    let has_appt = events.iter().any(|e| e["kind"] == "appointment");
    let has_blocked = events.iter().any(|e| e["kind"] == "blocked");
    assert!(has_appt, "calendar should include appointments");
    assert!(has_blocked, "calendar should include blocked times");
}

#[tokio::test]
async fn calendar_empty_range_returns_empty() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Far-future window with no data.
    let resp = app.get("/api/calendar/2099-01-01/2099-01-02").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn calendar_requires_auth() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/calendar/2020-01-01/2020-01-02").send().await.unwrap().status(), 401);
}

// ---------- Appointment duration validation (session 10) ----------

#[tokio::test]
async fn create_appointment_with_zero_duration_is_rejected() {
    // A zero-minute appointment is nonsensical.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "ZeroDur").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": "2099-01-15T09:00:00Z",
        "duration_minutes": 0,
    });
    let resp = app
        .post("/api/appointments")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "zero duration appointment must be 400"
    );
}

#[tokio::test]
async fn create_appointment_with_negative_duration_is_rejected() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "NegDur").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": "2099-01-15T09:00:00Z",
        "duration_minutes": -15,
    });
    let resp = app
        .post("/api/appointments")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "negative duration appointment must be 400"
    );
}

#[tokio::test]
async fn update_appointment_with_zero_duration_is_rejected() {
    // The update path must enforce the same rule.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "UpdZeroDur").await;
    // Create a valid appointment first.
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": "2099-01-15T09:00:00Z",
        "duration_minutes": 30,
    });
    let r = app
        .post("/api/appointments")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    // Try to update to zero duration.
    let upd = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": "2099-01-15T09:00:00Z",
        "duration_minutes": 0,
        "practitioner": null,
        "status": "scheduled",
        "notes": null,
    });
    let resp = app
        .put(&format!("/api/appointments/{id}"))
        .auth(&t)
        .json(&upd)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "update to zero duration must be 400"
    );
}
