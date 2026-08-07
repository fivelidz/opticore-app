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
