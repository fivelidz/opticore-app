//! Business-logic validation tests for clinical records, blocked times, and
//! messages — added in session 10.
//!
//! These characterize and then lock down input-validation gaps that would
//! otherwise allow semantically-invalid or corrupting data into the DB:
//!
//! * Blocked times with `start_at >= end_at` (a zero/negative-duration block
//!   is nonsensical and breaks calendar rendering).
//! * Clinical notes / allergies with empty required text fields (the schema
//!   uses `TEXT NOT NULL`, but that still accepts the empty string).
//! * IPL treatments with `session_number < 1`.
//! * OSDI scores with negative `total_score`.
//! * Messages linked to a nonexistent patient (the `messages` table has NO
//!   FK on `linked_patient_id`, so without a handler check this is silent
//!   data corruption).
//!
//! Each bug is first characterized (current behaviour), then the handler is
//! hardened with a `BadRequest` guard, and the test is flipped to assert 400.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

/// Create a patient and return its id.
async fn create_patient(app: &TestApp, t: &str, first: &str) -> i64 {
    let body = serde_json::json!({
        "first_name": first, "last_name": "Clin", "date_of_birth": "1990-01-01",
    });
    let r = app
        .post("/api/patients")
        .auth(t)
        .json(&body)
        .send()
        .await
        .unwrap();
    body_json(r).await["id"].as_i64().unwrap()
}

// =====================================================================
// Blocked times: start_at must be strictly before end_at
// =====================================================================

#[tokio::test]
async fn blocked_time_with_end_before_start_is_rejected() {
    // A blocked time where end_at < start_at is nonsensical (negative
    // duration). Must be rejected with 400, not stored.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "start_at": "2099-06-01T13:00:00Z",
        "end_at": "2099-06-01T12:00:00Z", // 1h BEFORE start
        "reason": "Bad",
    });
    let resp = app
        .post("/api/blocked-times")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "blocked time with end < start must be 400, not 201"
    );
}

#[tokio::test]
async fn blocked_time_with_start_equal_end_is_rejected() {
    // Zero-duration block is also nonsensical.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let same = "2099-06-01T12:00:00Z";
    let body = serde_json::json!({
        "start_at": same,
        "end_at": same,
        "reason": "Zero",
    });
    let resp = app
        .post("/api/blocked-times")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "blocked time with start == end must be 400"
    );
}

#[tokio::test]
async fn blocked_time_with_valid_range_is_accepted() {
    // Sanity: a normal start < end is still accepted (regression guard).
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "start_at": "2099-06-01T12:00:00Z",
        "end_at": "2099-06-01T13:00:00Z",
        "reason": "Lunch",
    });
    let resp = app
        .post("/api/blocked-times")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "valid blocked time should succeed");
}

#[tokio::test]
async fn blocked_time_update_with_end_before_start_is_rejected() {
    // The PUT update path must enforce the same rule.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Create a valid one first.
    let body = serde_json::json!({
        "start_at": "2099-07-01T09:00:00Z", "end_at": "2099-07-01T10:00:00Z",
        "reason": "Original",
    });
    let r = app
        .post("/api/blocked-times")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    // Now try to update to an inverted range.
    let upd = serde_json::json!({
        "start_at": "2099-07-01T10:00:00Z",
        "end_at": "2099-07-01T09:00:00Z", // before start
        "reason": "Bad",
    });
    let resp = app
        .put(&format!("/api/blocked-times/{id}"))
        .auth(&t)
        .json(&upd)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "update with end < start must be 400"
    );
}

// =====================================================================
// Clinical notes: note text must be non-empty
// =====================================================================

#[tokio::test]
async fn clinical_note_with_empty_text_is_rejected() {
    // `note` is TEXT NOT NULL, but the empty string still satisfies NOT NULL.
    // An empty clinical note is meaningless, so reject it at the handler.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "EmptyNote").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "note": "",
        "author": "Dr. Test",
    });
    let resp = app
        .post(&format!("/api/patients/{pid}/notes"))
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "empty clinical note must be 400, not 201"
    );
}

#[tokio::test]
async fn clinical_note_with_whitespace_only_text_is_rejected() {
    // Whitespace-only is also effectively empty.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "WsNote").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "note": "   \n\t  ",
        "author": "Dr. Test",
    });
    let resp = app
        .post(&format!("/api/patients/{pid}/notes"))
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "whitespace-only clinical note must be 400"
    );
}

#[tokio::test]
async fn clinical_note_with_valid_text_is_accepted() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "OkNote").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "note": "Patient reports improvement.",
        "author": "Dr. Test",
    });
    let resp = app
        .post(&format!("/api/patients/{pid}/notes"))
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "valid clinical note should succeed");
}

// =====================================================================
// Allergies: substance must be non-empty
// =====================================================================

#[tokio::test]
async fn allergy_with_empty_substance_is_rejected() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "EmptyAllergy").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "substance": "",
        "severity": "mild",
    });
    let resp = app
        .post("/api/allergies")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "allergy with empty substance must be 400"
    );
}

#[tokio::test]
async fn allergy_with_valid_substance_is_accepted() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "OkAllergy").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "substance": "Penicillin",
        "severity": "moderate",
    });
    let resp = app
        .post("/api/allergies")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "valid allergy should succeed");
}

// =====================================================================
// IPL treatments: session_number must be >= 1
// =====================================================================

#[tokio::test]
async fn ipl_with_zero_session_number_is_rejected() {
    // Session 0 is nonsensical (treatment sessions are 1-indexed).
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "IplZero").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "treatment_date": "2099-06-01T10:00:00Z",
        "session_number": 0,
    });
    let resp = app
        .post(&format!("/api/patients/{pid}/ipl"))
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "IPL session_number < 1 must be 400"
    );
}

#[tokio::test]
async fn ipl_with_negative_session_number_is_rejected() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "IplNeg").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "treatment_date": "2099-06-01T10:00:00Z",
        "session_number": -3,
    });
    let resp = app
        .post(&format!("/api/patients/{pid}/ipl"))
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400, "negative IPL session_number must be 400");
}

#[tokio::test]
async fn ipl_with_valid_session_number_is_accepted() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "IplOk").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "treatment_date": "2099-06-01T10:00:00Z",
        "session_number": 1,
    });
    let resp = app
        .post(&format!("/api/patients/{pid}/ipl"))
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "valid IPL session_number should succeed");
}

// =====================================================================
// OSDI scores: total_score must be >= 0
// =====================================================================

#[tokio::test]
async fn osdi_with_negative_total_score_is_rejected() {
    // OSDI total is a severity score; negative values are meaningless.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "OsdiNeg").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "score_date": "2026-06-01",
        "total_score": -5.0,
    });
    let resp = app
        .post(&format!("/api/patients/{pid}/osdi"))
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "OSDI negative total_score must be 400"
    );
}

#[tokio::test]
async fn osdi_with_zero_total_score_is_accepted() {
    // Zero is a valid score (no symptoms).
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "OsdiZero").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "score_date": "2026-06-01",
        "total_score": 0.0,
    });
    let resp = app
        .post(&format!("/api/patients/{pid}/osdi"))
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "OSDI total_score of 0 should succeed");
}

// =====================================================================
// Messages: link_patient must reject a nonexistent patient
// =====================================================================

#[tokio::test]
async fn link_message_to_nonexistent_patient_is_rejected() {
    // The messages table has NO FK on linked_patient_id, so without a handler
    // check this would silently store a dangling reference. Reject with 400.
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    // First create a message (public receive endpoint).
    let msg = serde_json::json!({
        "channel": "website",
        "from_name": "Test Sender",
        "body": "hello",
    });
    let r = app
        .post("/api/messages")
        .auth(&t)
        .json(&msg)
        .send()
        .await
        .unwrap();
    let mid = body_json(r).await["id"].as_i64().unwrap();

    // Try to link to a patient that doesn't exist.
    let resp = app
        .post(&format!("/api/messages/{mid}/link/99999999"))
        .auth(&t)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "linking a message to a nonexistent patient must be 400"
    );
}

#[tokio::test]
async fn link_message_to_existing_patient_succeeds() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "MsgLink").await;

    let msg = serde_json::json!({
        "channel": "website",
        "from_name": "Test Sender",
        "body": "hello",
    });
    let r = app
        .post("/api/messages")
        .auth(&t)
        .json(&msg)
        .send()
        .await
        .unwrap();
    let mid = body_json(r).await["id"].as_i64().unwrap();

    let resp = app
        .post(&format!("/api/messages/{mid}/link/{pid}"))
        .auth(&t)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "linking a message to an existing patient should succeed"
    );
}
