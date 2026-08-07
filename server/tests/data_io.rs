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

// ---------- FK-pragma leak regression ----------

/// Regression: a malformed import that fails AFTER the import path has set
/// `PRAGMA foreign_keys = OFF` (e.g. an unknown table name in the snapshot,
/// which is rejected by the allowlist check) must NOT leave the pooled
/// connection with FK enforcement disabled.
///
/// Before the fix, the early `return Err(...)` dropped the connection back
/// into the pool with `foreign_keys = OFF` still set, so the next request
/// served by that connection would silently bypass referential integrity
/// (orphaned invoices/appointments/clinical notes on patient delete, etc.).
///
/// This test triggers that early-return path and then asserts FK enforcement
/// is still active on the pool afterwards by attempting a write that violates
/// a foreign key — it must be rejected.
#[tokio::test]
async fn failed_import_does_not_leak_fk_off_into_pool() {
    use sqlx::Row;

    let app = TestApp::spawn().await;
    let t = token(&app).await;

    // Craft a snapshot whose `data` contains a table name NOT in the import
    // allowlist. This passes snapshot-version validation but is rejected by
    // the per-table allowlist check — which runs AFTER `PRAGMA foreign_keys =
    // OFF` has been set on the connection.
    let bad = serde_json::json!({
        "meta": { "snapshot_version": 1, "app_version": "x", "exported_at": "x", "table_count": 1, "row_count": 0, "encrypted": false },
        "data": {
            "definitely_not_a_real_table": []
        }
    });
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({ "snapshot": bad.to_string() }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400, "malformed-table import should be rejected");

    // The import acquired a pooled connection, set FK OFF, then errored. That
    // connection has now been returned to the pool. Force the pool to hand out
    // connections (up to max_connections=8) and verify EACH one still enforces
    // foreign keys — i.e. inserting a payment for a nonexistent invoice fails.
    //
    // We check multiple acquires because the poisoned connection could be any
    // one of the pool's slots; checking several raises the chance we'd catch a
    // leaked-FK-OFF connection if the bug regressed. (With max_connections=8
    // and a single prior acquire, at most one slot is poisoned, so we acquire
    // several times to maximize coverage.)
    for _ in 0..8 {
        let res = sqlx::query(
            "INSERT INTO payments (invoice_id, amount, payment_method) \
             VALUES (999999, 1.0, 'cash')",
        )
        .execute(&app.state.db)
        .await;
        assert!(
            res.is_err(),
            "FK enforcement must still be active after a failed import: \
             a payment for a nonexistent invoice should be rejected. \
             (This failing means a pooled connection has foreign_keys = OFF.)"
        );
    }

    // Also directly confirm the pragma value on a fresh acquire.
    let row = sqlx::query("PRAGMA foreign_keys").fetch_one(&app.state.db).await.unwrap();
    let fk: i64 = row.get(0);
    assert_eq!(fk, 1, "PRAGMA foreign_keys must be ON (1) on pooled connections");
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

/// End-to-end BLOB round-trip through export → import.
///
/// The schema has no BLOB columns today, but `row_to_json` must handle them
/// correctly for future schema versions (and for snapshots produced by older
/// versions that did use BLOBs). Since the export endpoint only dumps a
/// hardcoded table list, we `ALTER TABLE` an existing exported table
/// (`patient_photos`) to add a BLOB column, insert binary data, then export
/// and import into a fresh DB and verify the bytes survive.
///
/// Before the fix, the BLOB was silently dropped to JSON null on export, so
/// the imported row would have NULL in the BLOB column. With the b64: tag,
/// the bytes survive the full round-trip.
#[tokio::test]
async fn blob_column_survives_export_import_round_trip() {
    let app_a = TestApp::spawn().await;
    let ta = token(&app_a).await;

    // Add a BLOB column to patient_photos and insert a row with non-UTF-8 bytes.
    let blob_bytes: Vec<u8> = vec![0xff, 0x00, 0xfe, 0x41, 0x42, 0x43, 0xff, 0x01];
    sqlx::query("ALTER TABLE patient_photos ADD COLUMN thumb_blob BLOB")
        .execute(&app_a.state.db)
        .await
        .unwrap();
    // patient_photos requires patient_id (FK). Use patient 1 (seeded).
    sqlx::query(
        "INSERT INTO patient_photos (patient_id, category, filename, data_base64, thumb_blob) \
         VALUES (1, 'document', 'test.png', 'b64data', ?)",
    )
    .bind(&blob_bytes[..])
    .execute(&app_a.state.db)
    .await
    .unwrap();

    // Export A (unencrypted so we can inspect the snapshot).
    let resp = app_a
        .post("/api/data/export").auth(&ta).json(&serde_json::json!({})).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let snapshot_str = body_json(resp).await["snapshot"].as_str().unwrap().to_string();

    // Inspect the exported snapshot: the BLOB should be present as "b64:...".
    let snap: serde_json::Value = serde_json::from_str(&snapshot_str).unwrap();
    let photos = snap["data"]["patient_photos"].as_array().expect("patient_photos exported");
    let our_row = photos
        .iter()
        .find(|r| r.get("filename").and_then(|v| v.as_str()) == Some("test.png"))
        .expect("our inserted row should be in the export");
    let exported_blob = our_row["thumb_blob"].as_str().expect("thumb_blob is a string");
    assert!(
        exported_blob.starts_with("b64:"),
        "exported BLOB should be tagged b64:, got: {exported_blob}"
    );

    // Fresh app B with the same BLOB column added.
    let app_b = TestApp::spawn().await;
    let tb = token(&app_b).await;
    sqlx::query("ALTER TABLE patient_photos ADD COLUMN thumb_blob BLOB")
        .execute(&app_b.state.db)
        .await
        .unwrap();

    // Import A's snapshot into B (merge mode).
    let resp = app_b
        .post("/api/data/import")
        .auth(&tb)
        .json(&serde_json::json!({ "snapshot": snapshot_str, "mode": "merge" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Read the BLOB back from B and verify byte-for-byte equality.
    let row = sqlx::query("SELECT thumb_blob FROM patient_photos WHERE filename = 'test.png'")
        .fetch_one(&app_b.state.db)
        .await
        .unwrap();
    use sqlx::Row;
    let recovered: Vec<u8> = row.get::<Vec<u8>, _>("thumb_blob");
    assert_eq!(recovered, blob_bytes, "BLOB bytes must survive export→import round-trip");
}

// ---------- import: field-level validation (upfront snapshot scan) ----------
//
// With the CHECK constraints (migration 0015) in place, a snapshot containing
// a row that violates a field-level rule would fail mid-insert with an opaque
// "CHECK constraint failed" error. The import path now scans every row UP
// FRONT and rejects the whole snapshot with a clear, enumerated 400 before
// touching the DB. These tests characterize that upfront validation.

/// Build a minimal valid snapshot skeleton with the given `data` object.
fn make_snapshot(data: serde_json::Value) -> String {
    let snap = serde_json::json!({
        "meta": {
            "snapshot_version": 1,
            "app_version": "test",
            "exported_at": "2026-01-01T00:00:00Z",
            "table_count": data.as_object().map(|o| o.len()).unwrap_or(0),
            "row_count": 0,
            "encrypted": false,
        },
        "data": data,
    });
    snap.to_string()
}

/// Helper: import a snapshot and return the (status, body_json).
async fn do_import(app: &TestApp, token: &str, snapshot: &str) -> (u16, serde_json::Value) {
    let resp = app
        .post("/api/data/import")
        .auth(token)
        .json(&serde_json::json!({ "snapshot": snapshot, "mode": "replace" }))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    (status, body_json(resp).await)
}

#[tokio::test]
async fn import_rejects_appointment_with_zero_duration() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "appointments": [
            { "id": 1, "patient_id": 1, "appointment_type": "x",
              "appointment_date": "2099-01-01T09:00:00Z", "duration_minutes": 0 },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("duration_minutes") && body.contains(">= 1"),
        "error should mention duration_minutes >= 1, got: {body}"
    );
}

#[tokio::test]
async fn import_rejects_negative_osdi_score() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "osdi_scores": [
            { "id": 1, "patient_id": 1, "score_date": "2026-01-01", "total_score": -5.0 },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("total_score") && body.contains(">= 0"),
        "error should mention total_score >= 0, got: {body}"
    );
}

#[tokio::test]
async fn import_rejects_empty_patient_name() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "patients": [
            { "id": 1, "mrn": "X", "first_name": "", "last_name": "Doe",
              "date_of_birth": "1990-01-01" },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("first_name") && body.contains("empty"),
        "error should mention first_name empty, got: {body}"
    );
}

#[tokio::test]
async fn import_rejects_empty_clinical_note() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "clinical_notes": [
            { "id": 1, "patient_id": 1, "note": "   " },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("note") && body.contains("empty"),
        "error should mention note empty, got: {body}"
    );
}

#[tokio::test]
async fn import_rejects_ipl_session_zero() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "ipl_treatments": [
            { "id": 1, "patient_id": 1, "treatment_date": "2026-01-01T10:00:00Z",
              "session_number": 0 },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("session_number") && body.contains(">= 1"),
        "error should mention session_number >= 1, got: {body}"
    );
}

#[tokio::test]
async fn import_rejects_blocked_time_start_ge_end() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "blocked_times": [
            { "id": 1, "start_at": "2099-01-01T10:00:00Z", "end_at": "2099-01-01T10:00:00Z" },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("start_at") && body.contains("end_at"),
        "error should mention start_at/end_at, got: {body}"
    );
}

#[tokio::test]
async fn import_rejects_bad_booking_mode() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "booking_settings": [
            { "id": 1, "booking_mode": "bogus", "reminder_hours_before": 24 },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("booking_mode"),
        "error should mention booking_mode, got: {body}"
    );
}

#[tokio::test]
async fn import_rejects_negative_reminder_hours() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "booking_settings": [
            { "id": 1, "reminder_hours_before": -1 },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("reminder_hours_before"),
        "error should mention reminder_hours_before, got: {body}"
    );
}

#[tokio::test]
async fn import_rejects_empty_allergy_substance() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "allergies": [
            { "id": 1, "patient_id": 1, "substance": "" },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("substance") && body.contains("empty"),
        "error should mention substance empty, got: {body}"
    );
}

#[tokio::test]
async fn import_rejects_empty_intake_name() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "intake_submissions": [
            { "id": 1, "first_name": "Jane", "last_name": "" },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("last_name") && body.contains("empty"),
        "error should mention last_name empty, got: {body}"
    );
}

/// A snapshot with a MIX of good and bad rows is rejected in full — no partial
/// import. This is the key correctness property: the caller never ends up with
/// half the snapshot loaded.
#[tokio::test]
async fn import_rejects_mixed_good_and_bad_rows_no_partial_import() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    // Count patients before (seed data).
    let before = count_rows(&app, "patients").await;

    let snap = make_snapshot(serde_json::json!({
        "patients": [
            // Good row.
            { "id": 9991, "mrn": "IMP-GOOD", "first_name": "Good", "last_name": "Row",
              "date_of_birth": "1990-01-01" },
            // Bad row (empty first_name).
            { "id": 9992, "mrn": "IMP-BAD", "first_name": "", "last_name": "Bad",
              "date_of_birth": "1990-01-01" },
        ]
    }));
    let (status, _v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400, "mixed snapshot must be rejected");

    // The good row must NOT have been inserted (no partial import).
    let after = count_rows(&app, "patients").await;
    assert_eq!(
        after, before,
        "no rows should have been imported (partial-import guard): before={before}, after={after}"
    );
    // Specifically, the "good" row's id must not exist.
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM patients WHERE id = 9991")
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(exists, 0, "the good row in a rejected snapshot must not be present");
}

/// A clean snapshot with all-valid rows imports successfully.
#[tokio::test]
async fn import_accepts_clean_snapshot() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    let snap = make_snapshot(serde_json::json!({
        "patients": [
            { "id": 8881, "mrn": "IMP-CLEAN-1", "first_name": "Clean", "last_name": "One",
              "date_of_birth": "1990-01-01" },
            { "id": 8882, "mrn": "IMP-CLEAN-2", "first_name": "Clean", "last_name": "Two",
              "date_of_birth": "1991-02-02" },
        ],
        "appointments": [
            { "id": 7771, "patient_id": 8881, "appointment_type": "consultation",
              "appointment_date": "2099-01-01T09:00:00Z", "duration_minutes": 30 },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 200, "clean snapshot should import; body: {v}");
    assert!(v["imported"].as_i64().unwrap() >= 3, "all 3 rows imported");
}

/// A snapshot with multiple violations reports ALL of them (not just the first).
#[tokio::test]
async fn import_reports_all_violations_not_just_first() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    let snap = make_snapshot(serde_json::json!({
        "appointments": [
            { "id": 1, "patient_id": 1, "appointment_type": "x",
              "appointment_date": "2099-01-01T09:00:00Z", "duration_minutes": 0 },
        ],
        "osdi_scores": [
            { "id": 1, "patient_id": 1, "score_date": "2026-01-01", "total_score": -1.0 },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400);
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("duration_minutes") && body.contains("total_score"),
        "error should enumerate BOTH violations, got: {body}"
    );
}

// ---------- replace-mode structural-invariant preservation ----------
//
// Replace-mode does DELETE FROM <table> for every table in the snapshot. If a
// structural table (users / consultation_types / services) is in the snapshot
// with an EMPTY array, it gets wiped and not repopulated — bricking the system
// (no admin to log in, no billing catalog). The import path now re-seeds any
// structural table that ends up empty after a replace-mode import.

/// Replace-import with an empty `users` array must re-seed an admin row so the
/// system is not bricked (no login possible).
#[tokio::test]
async fn replace_import_empty_users_reseeds_admin() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    // Precondition: an admin exists.
    assert!(count_rows(&app, "users").await >= 1);

    // Replace-import with an empty users array — would brick the system
    // without the re-seed guard.
    let snap = make_snapshot(serde_json::json!({
        "users": []
    }));
    let (status, _v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 200);

    // An admin must still exist (re-seeded).
    let admin_count: i64 = {
        use sqlx::Row;
        sqlx::query("SELECT COUNT(*) FROM users WHERE role = 'admin'")
            .fetch_one(&app.state.db)
            .await
            .unwrap()
            .get(0)
    };
    assert_eq!(admin_count, 1, "admin should be re-seeded after empty-users replace-import");
}

/// Replace-import with an empty `consultation_types` array must re-seed the
/// default catalog.
#[tokio::test]
async fn replace_import_empty_consultation_types_reseeds_catalog() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    let snap = make_snapshot(serde_json::json!({
        "consultation_types": []
    }));
    let (status, _v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 200);

    // The 0003 seed installs 5 consultation types; re-seed should restore them.
    let n = count_rows(&app, "consultation_types").await;
    assert_eq!(n, 5, "consultation_types catalog should be re-seeded with 5 defaults");
}

/// Replace-import with an empty `services` array must re-seed the default catalog.
#[tokio::test]
async fn replace_import_empty_services_reseeds_catalog() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    let snap = make_snapshot(serde_json::json!({
        "services": []
    }));
    let (status, _v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 200);

    // The 0003 seed installs 10 services; re-seed should restore them.
    let n = count_rows(&app, "services").await;
    assert_eq!(n, 10, "services catalog should be re-seeded with 10 defaults");
}

/// Replace-import that OMITS a structural table entirely must NOT delete it.
/// (The DELETE only runs for tables present in the snapshot; an omitted table
/// is untouched. This characterizes that existing behavior so a future
/// refactor doesn't regress it.)
#[tokio::test]
async fn replace_import_omitting_users_preserves_rows() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    let before = count_rows(&app, "users").await;
    assert!(before >= 1);

    // Snapshot with a non-structural table but NO users key at all.
    let snap = make_snapshot(serde_json::json!({
        "patients": []
    }));
    let (status, _v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 200);

    let after = count_rows(&app, "users").await;
    assert_eq!(after, before, "users table should be untouched when omitted from snapshot");
}

/// Merge-mode must NEVER delete, even with an empty structural-table array.
/// (Merge skips existing PKs; an empty array is a no-op. The re-seed guard is
/// replace-only, so this confirms merge doesn't accidentally trigger it or
/// wipe data.)
#[tokio::test]
async fn merge_import_empty_users_preserves_rows() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    let before = count_rows(&app, "users").await;
    assert!(before >= 1);

    let snap = make_snapshot(serde_json::json!({
        "users": []
    }));
    let resp = app
        .post("/api/data/import")
        .auth(&t)
        .json(&serde_json::json!({ "snapshot": snap, "mode": "merge" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let after = count_rows(&app, "users").await;
    assert_eq!(after, before, "merge mode must not delete users");
}

// ---------- import: type-coercion rejection ----------
//
// validate_snapshot uses .as_f64()/.as_str() which return None when the JSON
// type doesn't match. So a snapshot where a numeric field is a STRING skips
// validation entirely. SQLite's flexible typing then stores the string verbatim
// in an INTEGER-affinity column (e.g. duration_minutes = "thirty" persists as
// TEXT) — silent data corruption. The import must reject wrong-type fields.

/// A snapshot row where a numeric field is a non-numeric string must be
/// rejected, not silently stored as TEXT.
#[tokio::test]
async fn import_rejects_string_in_numeric_field() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // duration_minutes is a string "thirty" instead of a number.
    let snap = make_snapshot(serde_json::json!({
        "appointments": [
            { "id": 1, "patient_id": 1, "appointment_type": "x",
              "appointment_date": "2099-01-01T09:00:00Z", "duration_minutes": "thirty" },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400, "string in a numeric field must be rejected; body: {v}");
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("duration_minutes"),
        "error should mention duration_minutes type mismatch, got: {body}"
    );
}

/// A snapshot row where a numeric field is a JSON object must be rejected.
#[tokio::test]
async fn import_rejects_object_in_numeric_field() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "appointments": [
            { "id": 1, "patient_id": 1, "appointment_type": "x",
              "appointment_date": "2099-01-01T09:00:00Z", "duration_minutes": {"oops": 1} },
        ]
    }));
    let (status, _v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400, "object in a numeric field must be rejected");
}

/// A snapshot row where a numeric field is a JSON array must be rejected.
#[tokio::test]
async fn import_rejects_array_in_numeric_field() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "osdi_scores": [
            { "id": 1, "patient_id": 1, "score_date": "2026-01-01", "total_score": [1, 2, 3] },
        ]
    }));
    let (status, _v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400, "array in a numeric field must be rejected");
}

/// A snapshot row where a string field is a number must be rejected (e.g.
/// first_name as a number). SQLite would coerce it, but it's still wrong type.
#[tokio::test]
async fn import_rejects_number_in_string_field() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let snap = make_snapshot(serde_json::json!({
        "patients": [
            { "id": 1, "mrn": "X", "first_name": 123, "last_name": "Doe",
              "date_of_birth": "1990-01-01" },
        ]
    }));
    let (status, v) = do_import(&app, &t, &snap).await;
    assert_eq!(status, 400, "number in a string field must be rejected; body: {v}");
    let body = serde_json::to_string(&v).unwrap();
    assert!(
        body.contains("first_name"),
        "error should mention first_name type mismatch, got: {body}"
    );
}
