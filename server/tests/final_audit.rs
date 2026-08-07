//! Final deep-audit tests (session 8).
//!
//! Areas covered:
//!   - Analytics time-series `:days` path-param validation (negative / zero /
//!     excessively large values). Previously unvalidated, producing malformed
//!     SQLite date-modifier strings like `"--5 days"` that silently yield NULL
//!     date math and empty result sets.
//!   - Pagination edge cases (negative limit/offset, limit=0, very large
//!     offset beyond data size).
//!   - HTTP status-code consistency for create handlers (201 vs 200).
//!   - Error-response content-type (all errors must be JSON, not HTML/text).
//!   - Unicode / emoji round-trip through text fields.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

// ===========================================================================
// A. Analytics `:days` path-param validation
// ===========================================================================
//
// The revenue / appointment / traffic / patient-growth series endpoints take a
// raw `:days` path param (i64) and interpolate it into a SQLite date modifier:
//   `WHERE date >= date('now', ?)`  with  `? = format!("-{days} days")`
//
// If `days` is negative, the modifier becomes e.g. `"--5 days"`, which SQLite
// cannot parse — `date('now','--5 days')` returns NULL, so the `>= NULL`
// predicate is never true and the endpoint silently returns an empty array
// instead of an error. If `days` is 0, the modifier is `"-0 days"` (today),
// which is semantically odd for a "series over the last N days" but not
// broken. Excessively large values (e.g. 1_000_000) are harmless (just a wide
// range) but should be bounded to prevent surprising query plans.
//
// Fix: clamp `days` to [1, 3650] (10 years) in each handler — matches the
// `public_api::availability` clamp(1, 30) pattern.

#[tokio::test]
async fn analytics_revenue_series_rejects_negative_days() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/analytics/revenue/-5").auth(&t).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(
        status, 400,
        "negative days should be rejected as bad input (got {})", status
    );
}

#[tokio::test]
async fn analytics_revenue_series_rejects_zero_days() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/analytics/revenue/0").auth(&t).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(
        status, 400,
        "zero days should be rejected as bad input (got {})", status
    );
}

#[tokio::test]
async fn analytics_appointment_series_rejects_negative_days() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/analytics/appointments/-1").auth(&t).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(
        status, 400,
        "negative days should be rejected (got {})", status
    );
}

#[tokio::test]
async fn analytics_traffic_series_rejects_negative_days() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/analytics/traffic/-100").auth(&t).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(
        status, 400,
        "negative days should be rejected (got {})", status
    );
}

#[tokio::test]
async fn analytics_patient_growth_rejects_zero_days() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/analytics/patient-growth/0").auth(&t).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(
        status, 400,
        "zero days should be rejected (got {})", status
    );
}

/// A valid positive `days` value still works after the clamp is added.
#[tokio::test]
async fn analytics_revenue_series_accepts_valid_days() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/analytics/revenue/30").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v.is_array(), "revenue series should be an array");
}

// ===========================================================================
// B. Pagination edge cases
// ===========================================================================

/// `limit=0` is clamped to 1 (the handler does `.clamp(1, 500)`), so it must
/// not error and must return at most 1 row.
#[tokio::test]
async fn patients_list_limit_zero_is_clamped_to_one() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/patients?limit=0").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    let count = v["count"].as_i64().unwrap();
    assert!(count <= 1, "limit=0 should clamp to 1, got count={}", count);
}

/// Negative offset is clamped to 0 (`.max(0)`), so it must not error.
#[tokio::test]
async fn patients_list_negative_offset_is_clamped_to_zero() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/patients?offset=-50").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200, "negative offset should be clamped to 0");
}

/// Offset far beyond the data size returns an empty page (not an error).
#[tokio::test]
async fn patients_list_offset_beyond_data_returns_empty() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/patients?offset=999999").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["count"].as_i64().unwrap(), 0, "offset beyond data should be empty");
}

/// Negative limit is clamped to 1.
#[tokio::test]
async fn patients_list_negative_limit_is_clamped() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/patients?limit=-5").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v["count"].as_i64().unwrap() <= 1, "negative limit should clamp to 1");
}

// ===========================================================================
// C. HTTP status-code consistency for create handlers
// ===========================================================================

/// `POST /api/billing/invoices` creates a resource and should return 201
/// (Created), not 200. Most other create handlers in this app already return
/// 201 (patients, appointments, users, photos, notes, blocked-times, intake,
/// messages). The invoice + payment handlers were inconsistent (200).
#[tokio::test]
async fn create_invoice_returns_201() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // create a patient first (invoice requires a valid patient_id)
    let pbody = serde_json::json!({
        "first_name": "Inv", "last_name": "Status",
        "date_of_birth": "1990-01-01",
    });
    let resp = app.post("/api/patients").auth(&t).json(&pbody).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let pid = body_json(resp).await["id"].as_i64().unwrap();

    let ibody = serde_json::json!({
        "patient_id": pid,
        "items": [{ "item_type": "consultation", "description": "Consult",
                    "quantity": 1.0, "unit_price": 100.0 }],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&ibody).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(status, 201, "create invoice should return 201 Created (got {})", status);
}

/// `POST /api/billing/payments` creates a resource and should return 201.
#[tokio::test]
async fn add_payment_returns_201() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // patient + invoice
    let pbody = serde_json::json!({
        "first_name": "Pay", "last_name": "Status",
        "date_of_birth": "1990-01-01",
    });
    let resp = app.post("/api/patients").auth(&t).json(&pbody).send().await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();
    let ibody = serde_json::json!({
        "patient_id": pid,
        "items": [{ "item_type": "consultation", "description": "Consult",
                    "quantity": 1.0, "unit_price": 100.0 }],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&ibody).send().await.unwrap();
    let inv_id = body_json(resp).await["id"].as_i64().unwrap();

    let paybody = serde_json::json!({
        "invoice_id": inv_id,
        "amount": 50.0,
        "payment_method": "card",
    });
    let resp = app.post("/api/billing/payments").auth(&t).json(&paybody).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(status, 201, "add payment should return 201 Created (got {})", status);
}

/// `POST /api/allergies` creates a resource and should return 201.
#[tokio::test]
async fn add_allergy_returns_201() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pbody = serde_json::json!({
        "first_name": "Alg", "last_name": "Status",
        "date_of_birth": "1990-01-01",
    });
    let resp = app.post("/api/patients").auth(&t).json(&pbody).send().await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();

    let abody = serde_json::json!({
        "patient_id": pid,
        "substance": "Penicillin",
        "severity": "moderate",
    });
    let resp = app.post("/api/allergies").auth(&t).json(&abody).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(status, 201, "add allergy should return 201 Created (got {})", status);
}

/// `POST /api/patients/:id/osdi` creates a resource and should return 201.
#[tokio::test]
async fn add_osdi_returns_201() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pbody = serde_json::json!({
        "first_name": "Osd", "last_name": "Status",
        "date_of_birth": "1990-01-01",
    });
    let resp = app.post("/api/patients").auth(&t).json(&pbody).send().await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();

    let obody = serde_json::json!({
        "patient_id": pid,
        "score_date": "2026-01-01",
        "total_score": 25.0,
    });
    let resp = app.post(&format!("/api/patients/{}/osdi", pid)).auth(&t).json(&obody).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(status, 201, "add osdi should return 201 Created (got {})", status);
}

/// `POST /api/patients/:id/ipl` creates a resource and should return 201.
#[tokio::test]
async fn add_ipl_returns_201() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pbody = serde_json::json!({
        "first_name": "Ipl", "last_name": "Status",
        "date_of_birth": "1990-01-01",
    });
    let resp = app.post("/api/patients").auth(&t).json(&pbody).send().await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();

    let ibody = serde_json::json!({
        "patient_id": pid,
        "treatment_date": "2026-01-01",
        "session_number": 1,
    });
    let resp = app.post(&format!("/api/patients/{}/ipl", pid)).auth(&t).json(&ibody).send().await.unwrap();
    let status = resp.status().as_u16();
    assert_eq!(status, 201, "add ipl should return 201 Created (got {})", status);
}

// ===========================================================================
// D. Error responses are always JSON (never HTML / plain text / empty)
// ===========================================================================

/// A 404 must return a JSON error object with an `error` key, not HTML.
#[tokio::test]
async fn not_found_returns_json_error() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/patients/99999999").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap().to_string();
    assert!(
        ct.starts_with("application/json"),
        "404 content-type should be application/json, got: {}", ct
    );
    let v = body_json(resp).await;
    assert!(v.get("error").is_some(), "404 body should have an 'error' key: {:?}", v);
}

/// A 400 (bad request) must return a JSON error object.
#[tokio::test]
async fn bad_request_returns_json_error() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // empty names -> 400 (include required date_of_birth so the body
    // deserializes and reaches the handler's validation check)
    let body = serde_json::json!({ "first_name": "", "last_name": "", "date_of_birth": "1990-01-01" });
    let resp = app.post("/api/patients").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap().to_string();
    assert!(
        ct.starts_with("application/json"),
        "400 content-type should be application/json, got: {}", ct
    );
    let v = body_json(resp).await;
    assert!(v.get("error").is_some(), "400 body should have an 'error' key: {:?}", v);
}

/// A malformed-JSON body (axum `Json` extractor rejection) must ALSO return a
/// JSON error object, not plain text. Without a custom rejection handler,
/// axum's default `Json` extractor returns a `422 Unprocessable Entity` with a
/// plain-text body — breaking API consistency (every other error path returns
/// JSON). This test locks in the expectation that ALL error responses are JSON.
#[tokio::test]
async fn malformed_json_body_returns_json_error() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Send syntactically-invalid JSON.
    let resp = app.post("/api/patients").auth(&t)
        .body(b"{ this is not valid json".to_vec())
        .header("content-type", "application/json")
        .send().await.unwrap();
    let status = resp.status().as_u16();
    assert!(
        status == 400 || status == 422,
        "malformed JSON should be 400 or 422, got {}", status
    );
    let ct = resp.headers().get("content-type")
        .map(|h| h.to_str().unwrap().to_string())
        .unwrap_or_default();
    assert!(
        ct.starts_with("application/json"),
        "malformed-JSON rejection content-type should be application/json, got: {}", ct
    );
}

/// A missing-required-field body (axum `Json` extractor rejection) must return
/// a JSON error object, not plain text.
#[tokio::test]
async fn missing_required_field_returns_json_error() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Omit required `date_of_birth` — axum's Json extractor rejects with 422.
    let body = serde_json::json!({ "first_name": "X", "last_name": "Y" });
    let resp = app.post("/api/patients").auth(&t).json(&body).send().await.unwrap();
    let status = resp.status().as_u16();
    assert!(
        status == 400 || status == 422,
        "missing required field should be 400 or 422, got {}", status
    );
    let ct = resp.headers().get("content-type")
        .map(|h| h.to_str().unwrap().to_string())
        .unwrap_or_default();
    assert!(
        ct.starts_with("application/json"),
        "missing-field rejection content-type should be application/json, got: {}", ct
    );
}

/// A 409 (conflict) must return a JSON error object.
#[tokio::test]
async fn conflict_returns_json_error() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // create a patient, then try to delete one that has an appointment
    let pbody = serde_json::json!({
        "first_name": "Conf", "last_name": "Lict",
        "date_of_birth": "1990-01-01",
    });
    let resp = app.post("/api/patients").auth(&t).json(&pbody).send().await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();
    let abody = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consult",
        "appointment_date": "2099-01-01T09:00:00Z",
        "duration_minutes": 60,
    });
    let _ = app.post("/api/appointments").auth(&t).json(&abody).send().await.unwrap();

    let resp = app.delete(&format!("/api/patients/{}", pid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 409);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap().to_string();
    assert!(
        ct.starts_with("application/json"),
        "409 content-type should be application/json, got: {}", ct
    );
    let v = body_json(resp).await;
    assert!(v.get("error").is_some(), "409 body should have an 'error' key: {:?}", v);
}

// ===========================================================================
// E. Unicode / emoji round-trip through text fields
// ===========================================================================

/// Unicode names (including emoji) must round-trip through create + read
/// without corruption. SQLite TEXT stores UTF-8 verbatim, so this should work
/// — but it has never been tested, and a charset bug (e.g. a Latin-1
/// assumption in a downstream consumer) would silently mangle data.
#[tokio::test]
async fn unicode_emoji_name_round_trips() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "first_name": "José María",
        "last_name": "O'Brien- Müller 🎉",
        "date_of_birth": "1990-01-01",
        "phone": "0400000000",
        "email": "josé@exämple.test",
    });
    let resp = app.post("/api/patients").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let pid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.get(&format!("/api/patients/{}", pid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["first_name"].as_str().unwrap(), "José María");
    assert_eq!(v["last_name"].as_str().unwrap(), "O'Brien- Müller 🎉");
    assert_eq!(v["email"].as_str().unwrap(), "josé@exämple.test");
}
