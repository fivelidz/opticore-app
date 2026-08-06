//! Data I/O (export/import) route tests.
//!
//! These endpoints are admin-gated and back the clinic's backup/restore.
//! The highest-value test is the round-trip: export a DB, import it into a
//! fresh DB, and confirm the row counts match (export and import are inverse
//! operations).
//!
//! Note: the export endpoint returns the snapshot as a JSON *string* (not a
//! nested object) because the same field carries ciphertext when a passphrase
//! is supplied. Tests parse that string back into JSON to inspect it.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

/// Count rows in a table via the app's DB pool.
async fn count_rows(app: &TestApp, table: &str) -> i64 {
    use sqlx::Row;
    // table name is a hardcoded literal at each call site — not user input
    let row = sqlx::query(&format!("SELECT COUNT(*) AS c FROM {}", table))
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    row.get::<i64, _>("c")
}

// ---------- version ----------

#[tokio::test]
async fn version_info_returns_snapshot_version() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/data/version").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["snapshot_version"], 1);
    assert!(v["app_version"].is_string());
    assert_eq!(v["supported_min_snapshot"], 1);
}

// ---------- export ----------

#[tokio::test]
async fn export_returns_snapshot_string_with_meta() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app
        .post("/api/data/export")
        .auth(&t)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    // The snapshot is a JSON-encoded string (so the same field can hold ciphertext).
    let snap_str = v["snapshot"].as_str().expect("snapshot is a string");
    let snap: serde_json::Value = serde_json::from_str(snap_str).expect("snapshot parses as JSON");
    assert_eq!(snap["meta"]["snapshot_version"], 1);
    assert_eq!(snap["meta"]["encrypted"], false);
    assert!(snap["meta"]["row_count"].as_i64().unwrap() > 0, "seed data present");
    assert!(snap["meta"]["table_count"].as_i64().unwrap() > 0);
    // The data object should contain the exported tables.
    let data = snap["data"].as_object().unwrap();
    assert!(data.contains_key("patients"), "patients table exported");
    assert!(data.contains_key("invoices"), "invoices table exported");
}

#[tokio::test]
async fn export_with_passphrase_produces_ciphertext() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app
        .post("/api/data/export")
        .auth(&t)
        .json(&serde_json::json!({ "passphrase": "s3cret-pass" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    let snap_str = v["snapshot"].as_str().unwrap();
    // Encrypted snapshots are base64 + ":v1" tag and must NOT parse as JSON.
    let snap_str = v["snapshot"].as_str().unwrap();
    assert!(snap_str.ends_with(":v1"), "encrypted snapshot carries the :v1 cipher tag");
    assert!(
        serde_json::from_str::<serde_json::Value>(snap_str).is_err(),
        "encrypted snapshot is not valid JSON"
    );
}

// ---------- import: invalid input ----------

#[tokio::test]
async fn import_missing_snapshot_returns_400() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn import_garbage_snapshot_returns_400() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({ "snapshot": "not valid json at all" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn import_newer_snapshot_version_returns_400() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Hand-craft a snapshot claiming to be from the future.
    let future = serde_json::json!({
        "meta": { "snapshot_version": 999, "app_version": "x", "exported_at": "x", "table_count": 0, "row_count": 0, "encrypted": false },
        "data": {}
    });
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({ "snapshot": future.to_string() }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

// ---------- round-trip: export → import → equivalent state ----------

#[tokio::test]
async fn export_then_import_round_trips_row_counts() {
    // App A: source with seed data.
    let app_a = TestApp::spawn().await;
    let ta = token(&app_a).await;

    // Capture the source row counts for every exported table.
    let tables = [
        "patients", "appointments", "blocked_times", "clinical_notes", "allergies",
        "osdi_scores", "ipl_treatments", "invoices", "invoice_items", "payments",
        "consultation_types", "services", "intake_submissions", "messages",
        "website_events", "users", "patient_photos",
    ];
    let mut before = std::collections::HashMap::new();
    for t_name in &tables {
        before.insert(*t_name, count_rows(&app_a, t_name).await);
    }

    // Export from A.
    let resp = app_a
        .post("/api/data/export").auth(&ta).json(&serde_json::json!({})).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let snapshot_str = body_json(resp).await["snapshot"].as_str().unwrap().to_string();
    // Sanity: the exported snapshot's row_count should equal the sum of our counts.
    let snap: serde_json::Value = serde_json::from_str(&snapshot_str).unwrap();
    let exported_total: i64 = snap["data"].as_object().unwrap()
        .values()
        .map(|arr| arr.as_array().map(|a| a.len() as i64).unwrap_or(0))
        .sum();
    let local_total: i64 = before.values().sum();
    assert_eq!(exported_total, local_total, "exported row total matches DB row total");

    // App B: fresh DB (also seeded, so it has its own rows).
    let app_b = TestApp::spawn().await;
    let tb = token(&app_b).await;

    // Import A's snapshot into B in "replace" mode (overwrites B's data with A's).
    let resp = app_b
        .post("/api/data/import")
        .auth(&tb)
        .json(&serde_json::json!({ "snapshot": snapshot_str, "mode": "replace" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v["imported"].as_i64().unwrap() > 0, "imported at least one row");
    assert_eq!(v["mode"], "replace");
    assert_eq!(v["snapshot_version"], 1);

    // After replace-mode import, B's row counts should match A's for every table.
    for t_name in &tables {
        let after = count_rows(&app_b, t_name).await;
        assert_eq!(
            after, before[t_name],
            "table `{}` row count matches after round-trip ({} -> {})",
            t_name, before[t_name], after
        );
    }
}

// ---------- encrypted round-trip ----------

#[tokio::test]
async fn encrypted_export_then_import_round_trips() {
    let app_a = TestApp::spawn().await;
    let ta = token(&app_a).await;
    let passphrase = "round-trip-key";

    // Export encrypted.
    let resp = app_a
        .post("/api/data/export")
        .auth(&ta)
        .json(&serde_json::json!({ "passphrase": passphrase }))
        .send()
        .await
        .unwrap();
    let cipher = body_json(resp).await["snapshot"].as_str().unwrap().to_string();
    assert!(cipher.ends_with(":v1"));

    // Import with the same passphrase into a fresh DB.
    let app_b = TestApp::spawn().await;
    let tb = token(&app_b).await;
    let resp = app_b
        .post("/api/data/import")
        .auth(&tb)
        .json(&serde_json::json!({ "snapshot": cipher, "passphrase": passphrase, "mode": "replace" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v["imported"].as_i64().unwrap() > 0, "decrypted import restored rows");
}

#[tokio::test]
async fn encrypted_import_wrong_passphrase_returns_400() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    // Export with one passphrase.
    let resp = app
        .post("/api/data/export")
        .auth(&t)
        .json(&serde_json::json!({ "passphrase": "right-key" }))
        .send()
        .await
        .unwrap();
    let cipher = body_json(resp).await["snapshot"].as_str().unwrap().to_string();

    // Import with a different passphrase → decrypt yields garbage UTF-8 → 400.
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({ "snapshot": cipher, "passphrase": "wrong-key" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

// ---------- auth gating ----------

#[tokio::test]
async fn data_endpoints_require_admin() {
    let app = TestApp::spawn().await;
    // No token at all → 401 (auth middleware).
    assert_eq!(app.get("/api/data/version").send().await.unwrap().status(), 401);
    assert_eq!(
        app.post("/api/data/export").json(&serde_json::json!({})).send().await.unwrap().status(),
        401
    );
    assert_eq!(
        app.post("/api/data/import").json(&serde_json::json!({})).send().await.unwrap().status(),
        401
    );
}
