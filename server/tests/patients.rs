//! Patients CRUD tests: create, list, get, update, delete, search, not-found,
//! and auth enforcement.

mod common;

use common::{body_json, TestApp};

/// Log in and return a bearer token (every test in this file needs auth).
async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

#[tokio::test]
async fn list_returns_seeded_patients() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/patients").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    // 0002_seed.sql inserts 5 patients. Don't hard-code the count (migrations
    // may grow); confirm non-empty + count matches array length.
    let count = v["count"].as_i64().unwrap() as usize;
    assert!(count >= 5, "should have at least the 5 seeded patients");
    assert_eq!(v["patients"].as_array().unwrap().len(), count);
}

#[tokio::test]
async fn create_patient_returns_created_with_auto_mrn() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "first_name": "Test",
        "last_name": "Patient",
        "date_of_birth": "1990-01-15",
        "gender": "female",
        "phone": "0400 000 000",
        "email": "test@example.com",
    });
    let resp = app.post("/api/patients").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["first_name"], "Test");
    assert_eq!(v["last_name"], "Patient");
    // MRN auto-generated (starts with MOS-).
    assert!(v["mrn"].as_str().unwrap().starts_with("MOS-"));
    assert!(v["id"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn create_patient_with_explicit_mrn_uses_it() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "mrn": "MOS-CUSTOM-1",
        "first_name": "Custom",
        "last_name": "Mrn",
        "date_of_birth": "1980-06-06",
    });
    let resp = app.post("/api/patients").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["mrn"], "MOS-CUSTOM-1");
}

#[tokio::test]
async fn get_patient_by_id_returns_the_patient() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Seed patient id=1 is Sarah Johnson.
    let resp = app.get("/api/patients/1").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["id"], 1);
    assert_eq!(v["first_name"], "Sarah");
    assert_eq!(v["last_name"], "Johnson");
}

#[tokio::test]
async fn get_nonexistent_patient_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/patients/99999").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn update_patient_changes_fields() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Create a patient to update (avoid touching seed data).
    let create = serde_json::json!({
        "first_name": "Before", "last_name": "Edit", "date_of_birth": "1970-01-01",
    });
    let r = app.post("/api/patients").auth(&t).json(&create).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let update = serde_json::json!({
        "first_name": "After", "last_name": "Edit", "date_of_birth": "1970-01-01",
        "phone": "0411 222 333",
    });
    let resp = app.put(&format!("/api/patients/{}", id)).auth(&t).json(&update).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["first_name"], "After");
    assert_eq!(v["phone"], "0411 222 333");
}

#[tokio::test]
async fn delete_patient_removes_it() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Create then delete.
    let create = serde_json::json!({
        "first_name": "Doomed", "last_name": "Patient", "date_of_birth": "1999-12-31",
    });
    let r = app.post("/api/patients").auth(&t).json(&create).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let resp = app.delete(&format!("/api/patients/{}", id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v["message"].as_str().unwrap().contains("deleted"));

    // Confirm it's gone.
    let resp = app.get(&format!("/api/patients/{}", id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn delete_nonexistent_patient_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.delete("/api/patients/99999").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn search_filters_by_name() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // "Sarah" is one of the seeded patients.
    let resp = app.get("/api/patients?search=Sarah").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["count"], 1);
    assert_eq!(v["patients"][0]["first_name"], "Sarah");
}

#[tokio::test]
async fn search_no_match_returns_empty() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/patients?search=zzznomatch").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["count"], 0);
}

#[tokio::test]
async fn enriched_list_works() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/patients/enriched/list").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    let arr = v.as_array().unwrap();
    assert!(!arr.is_empty(), "enriched list should have seeded patients");
    // Each row has the enriched fields.
    assert!(arr[0].get("total_visits").is_some());
    assert!(arr[0].get("total_spent").is_some());
}

#[tokio::test]
async fn patients_endpoints_require_auth() {
    let app = TestApp::spawn().await;
    // No token -> 401 on every method.
    assert_eq!(app.get("/api/patients").send().await.unwrap().status(), 401);
    assert_eq!(app.get("/api/patients/1").send().await.unwrap().status(), 401);
    assert_eq!(app.post("/api/patients").body(b"{}".to_vec()).header("content-type", "application/json").send().await.unwrap().status(), 401);
    assert_eq!(app.put("/api/patients/1").body(b"{}".to_vec()).header("content-type", "application/json").send().await.unwrap().status(), 401);
    assert_eq!(app.delete("/api/patients/1").send().await.unwrap().status(), 401);
}

#[tokio::test]
async fn create_patient_missing_required_field_is_rejected() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Missing last_name (required). axum's Json extractor returns 422 when the
    // body is valid JSON but fails to deserialize into the target type (e.g.
    // missing required field).
    let body = serde_json::json!({ "first_name": "NoLast", "date_of_birth": "1990-01-01" });
    let resp = app.post("/api/patients").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 422);
}

// ---------- Patient name validation (session 10) ----------

#[tokio::test]
async fn create_patient_with_empty_first_name_is_rejected() {
    // first_name is required (non-Option), but the empty string still
    // deserializes. A patient record with no first name is useless.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "first_name": "", "last_name": "Nonempty", "date_of_birth": "1990-01-01",
    });
    let resp = app
        .post("/api/patients")
        .auth(&t)
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
async fn create_patient_with_empty_last_name_is_rejected() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "first_name": "Nonempty", "last_name": "", "date_of_birth": "1990-01-01",
    });
    let resp = app
        .post("/api/patients")
        .auth(&t)
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
async fn create_patient_with_whitespace_only_name_is_rejected() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "first_name": "  \t ", "last_name": "  ", "date_of_birth": "1990-01-01",
    });
    let resp = app
        .post("/api/patients")
        .auth(&t)
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

// =====================================================================
// Patient update: empty / whitespace-only names must be rejected
// =====================================================================
//
// `create` validates this at the handler; `update` previously did not, relying
// solely on the DB CHECK constraint (migration 0015). The DB catches it, but
// the error surfaced as an opaque "value violates a database check constraint"
// 400. The handler now validates upfront for a clear, actionable message.

#[tokio::test]
async fn update_patient_with_empty_last_name_is_rejected() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Create a valid patient first.
    let body = serde_json::json!({
        "first_name": "Upd", "last_name": "Valid", "date_of_birth": "1990-01-01",
    });
    let r = app
        .post("/api/patients")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    let pid = body_json(r).await["id"].as_i64().unwrap();

    // Try to update with an empty last name.
    let upd = serde_json::json!({
        "first_name": "Upd", "last_name": "", "date_of_birth": "1990-01-01",
    });
    let resp = app
        .put(&format!("/api/patients/{pid}"))
        .auth(&t)
        .json(&upd)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "updating a patient with an empty last_name must be 400"
    );
    // The error message should be actionable (mention first_name/last_name),
    // not the opaque DB constraint message.
    let v = body_json(resp).await;
    let err = v["error"].as_str().unwrap();
    assert!(
        err.contains("first_name") || err.contains("last_name"),
        "error should mention the field name, got: {err}"
    );
}

#[tokio::test]
async fn update_patient_with_whitespace_first_name_is_rejected() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "first_name": "Upd2", "last_name": "Valid", "date_of_birth": "1990-01-01",
    });
    let r = app
        .post("/api/patients")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    let pid = body_json(r).await["id"].as_i64().unwrap();

    let upd = serde_json::json!({
        "first_name": "   ", "last_name": "Valid", "date_of_birth": "1990-01-01",
    });
    let resp = app
        .put(&format!("/api/patients/{pid}"))
        .auth(&t)
        .json(&upd)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "updating a patient with whitespace-only first_name must be 400"
    );
}
