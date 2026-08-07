//! SQL injection regression tests.
//!
//! These tests audit every route that builds SQL with `format!` (string
//! interpolation) to confirm that **no user-controlled input reaches the SQL
//! string itself**. User input must always be passed via sqlx bind parameters
//! (`?` placeholders), never interpolated into the query text.
//!
//! ## Audit findings
//!
//! 1. **`routes::public_api::availability`** — interpolates `day_offset` into
//!    the SQL, but `day_offset` is a validated integer (`Path<i64>` then
//!    `.clamp(1, 30)`). Axum's `Path<i64>` extractor rejects non-integer path
//!    segments with a 400 before the handler runs, so no string can reach the
//!    `format!`. **Safe** — covered by a sanity test below.
//!
//! 2. **`routes::public_api::match_patient`** — user-supplied name/phone/email
//!    are bound via `?` placeholders. **Safe** — covered by a test that sends
//!    classic injection payloads and confirms they are treated as literal
//!    strings (no match, no error).
//!
//! 3. **`routes::data_io::import_data`** — **WAS VULNERABLE.** The table name
//!    and column names came directly from the imported JSON snapshot keys and
//!    were interpolated into `DELETE FROM {table}` and
//!    `INSERT OR IGNORE INTO {table} ({cols}) ...`. A malicious snapshot could
//!    set a table key to `patients; DROP TABLE users; --` or a column name
//!    containing SQL. The *values* were parameterized, but the *identifiers*
//!    were not. This is now fixed: table names are validated against a static
//!    allowlist (the same 17 tables `export_data` knows about), and column
//!    names are validated to match a strict SQLite-identifier pattern
//!    (`[A-Za-z_][A-Za-z0-9_]*`). Covered by tests below.
//!
//! 4. **`routes::booking_settings::update_settings`** — builds a dynamic
//!    `SET` clause, but every fragment is a hardcoded string literal
//!    (`"booking_mode = ?"`); only the values are bound. **Safe.**
//!
//! 5. **`routes::patients::list` / `list_enriched`** — builds a `LIKE` pattern
//!    with `format!("%{s}%")` but binds the resulting string as a parameter.
//!    **Safe.**

mod common;

use common::{body_json, TestApp};

async fn admin_token(app: &TestApp) -> String {
    app.admin_token().await
}

// ---------- public_api::availability (validated integer path) ----------

/// The `:days` path param is extracted as `Path<i64>`. A non-integer value
/// must be rejected by axum's extractor (400/404) — it must never reach the
/// handler's `format!` and thus can never inject SQL.
#[tokio::test]
async fn availability_non_integer_days_is_rejected() {
    let app = TestApp::spawn().await;
    // A non-numeric path segment. axum's Path<i64> extractor rejects this
    // with a 400/404 before the handler runs, so the `format!` in the handler
    // never sees it. (We use a simple alphabetic string rather than a full
    // SQL payload because URI-unsafe characters like spaces/semicolons are
    // rejected by the HTTP layer before axum even routes the request — that
    // would test HTTP parsing, not our injection resistance.)
    let resp = app.get("/api/public/availability/abc").send().await.unwrap();
    assert!(
        resp.status() == 400 || resp.status() == 404,
        "non-integer days path must be rejected (400/404), got {}",
        resp.status()
    );

    // Confirm the appointments table still exists (no injection happened).
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM appointments")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert!(count.0 >= 0, "appointments table is intact");
}

/// A valid integer `:days` works normally (sanity guard).
#[tokio::test]
async fn availability_valid_integer_days_works() {
    let app = TestApp::spawn().await;
    let resp = app.get("/api/public/availability/3").send().await.unwrap();
    assert_eq!(resp.status(), 200, "valid days=3 should succeed");
}

// ---------- public_api::match_patient (parameterized user input) ----------

/// Injection payloads in the match-patient body must be treated as literal
/// strings (bound via `?`), never executed as SQL. We send classic payloads
/// and confirm: no error (200), and `matched` is a boolean (the query ran
/// safely and found no match for the garbage).
#[tokio::test]
async fn match_patient_injection_payload_is_literal() {
    let app = TestApp::spawn().await;
    let payloads = [
        "' OR 1=1 --",
        "'; DROP TABLE patients; --",
        "x' UNION SELECT id FROM users --",
        "Robert'); DROP TABLE students; --",
    ];
    for p in &payloads {
        let body = serde_json::json!({
            "first_name": p,
            "last_name": p,
            "phone": p,
            "email": p,
        });
        let resp = app
            .post("/api/public/match-patient")
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            200,
            "match-patient with injection payload should not error, got {} for {:?}",
            resp.status(),
            p
        );
        let v = body_json(resp).await;
        assert!(
            v["matched"].is_boolean(),
            "matched must be a boolean (query ran safely) for {:?}",
            p
        );
    }

    // Confirm no table was dropped.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM patients")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert!(count.0 >= 0, "patients table is intact after injection attempts");
}

// ---------- data_io::import_data (identifier injection — THE vector) ----------

/// A snapshot with a malicious **table name** must be rejected (400), and the
/// injected SQL must NOT execute.
///
/// Before the fix, `import_data` did `format!("DELETE FROM {}", table)` and
/// `format!("INSERT OR IGNORE INTO {} (...) ...", table, ...)` with `table`
/// taken directly from the JSON keys. A table key of
/// `patients; DROP TABLE users; --` would run the DROP.
#[tokio::test]
async fn import_rejects_malicious_table_name() {
    let app = TestApp::spawn().await;
    let t = admin_token(&app).await;

    // Count users before — the attack tries to DROP them.
    let before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&app.state.db)
        .await
        .expect("users count before");

    // Craft a snapshot whose "table name" is a SQL injection payload.
    let malicious = serde_json::json!({
        "meta": { "snapshot_version": 1, "app_version": "x", "exported_at": "x", "table_count": 1, "row_count": 0, "encrypted": false },
        "data": {
            "patients; DROP TABLE users; --": []
        }
    });
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({ "snapshot": malicious.to_string(), "mode": "merge" }))
        .send()
        .await
        .unwrap();

    // The import must be rejected — the table name is not in the allowlist.
    assert_eq!(
        resp.status(),
        400,
        "malicious table name must be rejected with 400, got {}",
        resp.status()
    );

    // The users table must still exist and have the same row count (no DROP).
    let after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&app.state.db)
        .await
        .expect("users count after");
    assert_eq!(before.0, after.0, "users table row count unchanged — no injection occurred");
}

/// A snapshot with a malicious **column name** must be rejected (400).
///
/// Before the fix, column names from the JSON row object were interpolated
/// directly into the INSERT column list.
#[tokio::test]
async fn import_rejects_malicious_column_name() {
    let app = TestApp::spawn().await;
    let t = admin_token(&app).await;

    // A column name containing SQL syntax.
    let malicious = serde_json::json!({
        "meta": { "snapshot_version": 1, "app_version": "x", "exported_at": "x", "table_count": 1, "row_count": 1, "encrypted": false },
        "data": {
            "patients": [
                { "id": 999999, "first_name; DROP TABLE users; --": "evil" }
            ]
        }
    });
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({ "snapshot": malicious.to_string(), "mode": "merge" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "malicious column name must be rejected with 400, got {}",
        resp.status()
    );

    // users table intact.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert!(count.0 >= 0, "users table intact after column-name injection attempt");
}

/// A snapshot with an unknown-but-harmless table name (not in the allowlist)
/// must be rejected (400), not silently skipped or error-500'd.
#[tokio::test]
async fn import_rejects_unknown_table_name() {
    let app = TestApp::spawn().await;
    let t = admin_token(&app).await;
    let snap = serde_json::json!({
        "meta": { "snapshot_version": 1, "app_version": "x", "exported_at": "x", "table_count": 1, "row_count": 0, "encrypted": false },
        "data": {
            "this_table_does_not_exist": []
        }
    });
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({ "snapshot": snap.to_string(), "mode": "merge" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "unknown table name must be rejected with 400, got {}",
        resp.status()
    );
}

/// A legitimate snapshot (valid table + valid columns) must still import
/// successfully after the validation fix — guards against the allowlist being
/// too strict and breaking the normal round-trip.
#[tokio::test]
async fn import_valid_snapshot_still_works() {
    let app = TestApp::spawn().await;
    let t = admin_token(&app).await;

    // Minimal valid snapshot: one patient row into the patients table.
    let snap = serde_json::json!({
        "meta": { "snapshot_version": 1, "app_version": "x", "exported_at": "x", "table_count": 1, "row_count": 1, "encrypted": false },
        "data": {
            "patients": [
                { "id": 777777, "first_name": "Valid", "last_name": "Import", "date_of_birth": "1990-01-01", "phone": "0400000000", "email": "valid@import.test", "mrn": "MOS-TEST", "created_at": "2026-01-01 00:00:00" }
            ]
        }
    });
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({ "snapshot": snap.to_string(), "mode": "merge" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "valid snapshot must import successfully, got {}",
        resp.status()
    );
    let v = body_json(resp).await;
    assert_eq!(v["imported"], 1, "one row should be imported");

    // Verify the row landed.
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM patients WHERE id = 777777")
        .fetch_optional(&app.state.db)
        .await
        .unwrap();
    assert!(row.is_some(), "imported patient row should exist in DB");
}
