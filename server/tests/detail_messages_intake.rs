//! Patient detail aggregate + messages inbox + intake submissions tests.
//! Covers the remaining route modules: patient_detail, messages, intake.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

/// Create a patient and return its id.
async fn create_patient(app: &TestApp, t: &str, first: &str) -> i64 {
    let body = serde_json::json!({
        "first_name": first, "last_name": "Detail", "date_of_birth": "1990-01-01",
    });
    let r = app.post("/api/patients").auth(t).json(&body).send().await.unwrap();
    body_json(r).await["id"].as_i64().unwrap()
}

// ---------- Patient detail ----------

#[tokio::test]
async fn patient_detail_returns_aggregate() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Detail").await;

    let resp = app.get(&format!("/api/patients/{}/detail", pid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["patient"]["id"], pid);
    assert_eq!(v["patient"]["first_name"], "Detail");
    // Aggregate sections are all present (arrays).
    assert!(v["appointments"].is_array());
    assert!(v["notes"].is_array());
    assert!(v["allergies"].is_array());
    assert!(v["osdi_scores"].is_array());
    assert!(v["ipl_treatments"].is_array());
    assert!(v["invoices"].is_array());
    assert!(v["stats"].is_object());
}

#[tokio::test]
async fn patient_detail_includes_created_data() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Populated").await;

    // Add a note + allergy so the detail view has content.
    app.post(&format!("/api/patients/{}/notes", pid)).auth(&t)
        .json(&serde_json::json!({ "patient_id": pid, "note": "detail test" }))
        .send().await.unwrap();
    app.post("/api/allergies").auth(&t)
        .json(&serde_json::json!({ "patient_id": pid, "substance": "Pollen" }))
        .send().await.unwrap();

    let resp = app.get(&format!("/api/patients/{}/detail", pid)).auth(&t).send().await.unwrap();
    let v = body_json(resp).await;
    assert_eq!(v["notes"].as_array().unwrap().len(), 1);
    assert_eq!(v["allergies"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn patient_detail_nonexistent_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/patients/99999/detail").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn patient_detail_requires_auth() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/patients/1/detail").send().await.unwrap().status(), 401);
}

// ---------- Messages ----------

#[tokio::test]
async fn receive_message_is_public() {
    let app = TestApp::spawn().await;
    // /api/messages/receive is public (no auth) — for webhooks/contact forms.
    let body = serde_json::json!({
        "channel": "email", "from_name": "John Doe",
        "from_contact": "john@example.com", "subject": "Question",
        "body": "I have a question about my appointment",
    });
    let resp = app.post("/api/messages/receive").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["channel"], "email");
    assert_eq!(v["from_name"], "John Doe");
    assert_eq!(v["body"], "I have a question about my appointment");
}

#[tokio::test]
async fn list_messages_returns_array() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Seed a message via the public receive endpoint.
    let body = serde_json::json!({ "channel": "sms", "body": "test msg" });
    app.post("/api/messages/receive").json(&body).send().await.unwrap();

    let resp = app.get("/api/messages").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v.as_array().unwrap().len() >= 1);
}

#[tokio::test]
async fn list_messages_filters_by_channel() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    app.post("/api/messages/receive").json(&serde_json::json!({ "channel": "email", "body": "a" })).send().await.unwrap();
    app.post("/api/messages/receive").json(&serde_json::json!({ "channel": "sms", "body": "b" })).send().await.unwrap();

    let resp = app.get("/api/messages?channel=email").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    let arr = v.as_array().unwrap();
    assert!(arr.iter().all(|m| m["channel"] == "email"));
}

#[tokio::test]
async fn mark_message_read() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let r = app.post("/api/messages/receive").json(&serde_json::json!({ "channel": "email", "body": "x" })).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let resp = app.post(&format!("/api/messages/{}/read", id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn archive_message() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let r = app.post("/api/messages/receive").json(&serde_json::json!({ "channel": "email", "body": "y" })).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let resp = app.post(&format!("/api/messages/{}/archive", id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn link_message_to_patient() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Link").await;
    let r = app.post("/api/messages/receive").json(&serde_json::json!({ "channel": "email", "body": "z" })).send().await.unwrap();
    let mid = body_json(r).await["id"].as_i64().unwrap();

    let resp = app.post(&format!("/api/messages/{}/link/{}", mid, pid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn messages_list_requires_auth() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/messages").send().await.unwrap().status(), 401);
}

// ---------- Intake submissions ----------

#[tokio::test]
async fn submit_intake_is_public() {
    let app = TestApp::spawn().await;
    // /api/intake/submit is public (no auth).
    let body = serde_json::json!({
        "first_name": "Intake", "last_name": "Test",
        "phone": "0400 123 456", "email": "intake@example.com",
        "preferred_date": "2099-01-15", "preferred_time": "09:00",
        "appointment_type": "Dry Eye Consultation",
        "symptoms": "Gritty eyes",
    });
    let resp = app.post("/api/intake/submit").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["first_name"], "Intake");
    assert_eq!(v["status"], "new");
    assert_eq!(v["source"], "input-page");
}

#[tokio::test]
async fn submit_intake_minimal_fields() {
    let app = TestApp::spawn().await;
    let body = serde_json::json!({
        "first_name": "Min", "last_name": "Intake",
    });
    let resp = app.post("/api/intake/submit").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn submit_intake_claimed_returning_with_no_match_sets_flag() {
    let app = TestApp::spawn().await;
    // Claim to be a returning patient, but use a name that doesn't exist.
    let body = serde_json::json!({
        "first_name": "Definitely", "last_name": "NotARealPatient",
        "claimed_returning": true,
    });
    let resp = app.post("/api/intake/submit").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["claimed_returning"], true);
    assert_eq!(v["claimed_no_match"], true);
}

#[tokio::test]
async fn submit_intake_claimed_returning_with_match_no_flag() {
    let app = TestApp::spawn().await;
    // Seed patient Sarah Johnson exists. Claim returning + match her.
    let body = serde_json::json!({
        "first_name": "Sarah", "last_name": "Johnson",
        "date_of_birth": "1985-04-12",
        "claimed_returning": true,
    });
    let resp = app.post("/api/intake/submit").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["claimed_returning"], true);
    assert_eq!(v["claimed_no_match"], false);
}

#[tokio::test]
async fn list_intake_requires_auth() {
    let app = TestApp::spawn().await;
    // Submit one (public).
    app.post("/api/intake/submit").json(&serde_json::json!({ "first_name": "A", "last_name": "B" })).send().await.unwrap();
    // List is protected.
    assert_eq!(app.get("/api/intake").send().await.unwrap().status(), 401);
}

#[tokio::test]
async fn list_intake_returns_submissions() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    app.post("/api/intake/submit").json(&serde_json::json!({ "first_name": "List", "last_name": "Me" })).send().await.unwrap();

    let resp = app.get("/api/intake").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v.as_array().unwrap().len() >= 1);
}

// ---------- Intake validation (session 10) ----------

#[tokio::test]
async fn submit_intake_with_empty_first_name_is_rejected() {
    // first_name is a required field (non-Option in CreateIntake), but the
    // empty string still deserializes. A nameless intake submission is
    // useless to staff. Reject it.
    let app = TestApp::spawn().await;
    let body = serde_json::json!({
        "first_name": "", "last_name": "Nonempty",
    });
    let resp = app
        .post("/api/intake/submit")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "empty first_name must be 400, not 201"
    );
}

#[tokio::test]
async fn submit_intake_with_empty_last_name_is_rejected() {
    let app = TestApp::spawn().await;
    let body = serde_json::json!({
        "first_name": "Nonempty", "last_name": "",
    });
    let resp = app
        .post("/api/intake/submit")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "empty last_name must be 400, not 201"
    );
}

#[tokio::test]
async fn submit_intake_with_whitespace_only_name_is_rejected() {
    let app = TestApp::spawn().await;
    let body = serde_json::json!({
        "first_name": "   ", "last_name": "  \t ",
    });
    let resp = app
        .post("/api/intake/submit")
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "whitespace-only names must be 400"
    );
}
