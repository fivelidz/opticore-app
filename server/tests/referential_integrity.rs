//! Referential-integrity audit: characterize the foreign-key enforcement
//! state of the schema and prove (via direct DB writes) where orphans can
//! and cannot be created.
//!
//! ## Background
//!
//! SQLite does NOT enforce foreign keys unless `PRAGMA foreign_keys = ON` is
//! set on the connection. This app sets it globally at pool init
//! (`db::init_pool` → `SqliteConnectOptions::foreign_keys(true)`), so every
//! **declared** FK is enforced on every request connection. The data_io
//! import path temporarily flips it OFF per-connection for ordered bulk
//! restore and restores it ON afterwards (documented in `data_io.rs`).
//!
//! However, `PRAGMA foreign_keys = ON` only enforces FKs that are actually
//! **declared** in the schema. Two soft-link columns were created without a
//! FOREIGN KEY declaration and therefore can hold dangling references
//! regardless of the pragma:
//!
//!   * `messages.linked_patient_id`          (migration 0005)
//!   * `intake_submissions.matched_patient_id` (migration 0004)
//!
//! The handler routes that write these columns both have application-level
//! guards (see `messages::link_patient`, `intake::import_*`), so under normal
//! use no orphan is created. But the schema itself provides no defense-in-
//! depth: a future code path, a bulk import, or a direct DB write can create
//! a dangling row that the DB will not reject. This file proves that gap and
//! documents the intentionally-unconstrained soft links.
//!
//! ## What we characterize
//!
//!   1. **FK declaration audit** — for every dependent table, assert whether
//!      a FOREIGN KEY exists (via `pragma foreign_key_list`). This is a
//!      living map of the schema's referential-integrity surface.
//!   2. **Declared FKs are enforced** — inserting a row that violates a
//!      declared FK (e.g. appointment → nonexistent patient) is rejected by
//!      the DB. This proves `PRAGMA foreign_keys = ON` is effective.
//!   3. **Undeclared soft-links allow orphans (the gap)** — inserting a
//!      message / intake_submission with a dangling patient reference
//!      SUCCEEDS because no FK is declared. These tests assert the current
//!      broken behaviour with a TODO comment, matching the project's
//!      `*_documented_decision` convention. When a migration adds the FK,
//!      these tests must be flipped to expect rejection.
//!   4. **Intentionally-unconstrained soft-links** — `audit_log.user_id` and
//!      `booking_notifications.{booking_id,intake_submission_id}` are
//!      deliberately FK-free (documented in migration 0014). We characterize
//!      that they accept dangling values by design.

mod common;

use common::TestApp;
use sqlx::Row;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the list of (column, references_table) pairs declared as FKs on
/// `table`, via SQLite's `pragma foreign_key_list`. Each entry is
/// `(from_column, to_table)`. Empty vec = no FKs declared.
async fn fk_columns(pool: &sqlx::SqlitePool, table: &str) -> Vec<(String, String)> {
    // table name is a static literal in every call below; not user input.
    let rows = sqlx::query(&format!("PRAGMA foreign_key_list({})", table))
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter()
        .map(|r| {
            let from: String = r.get("from");
            let to: String = r.get("table");
            (from, to)
        })
        .collect()
}

/// Does `table` declare an FK from `col` -> `ref_table`?
async fn has_fk(pool: &sqlx::SqlitePool, table: &str, col: &str, ref_table: &str) -> bool {
    fk_columns(pool, table)
        .await
        .iter()
        .any(|(c, t)| c == col && t == ref_table)
}

// ===========================================================================
// 1. FK declaration audit — the schema's referential-integrity surface
// ===========================================================================
//
// This is a characterization test: it documents, for every dependent table,
// whether a FOREIGN KEY is declared. If a future migration adds or removes a
// declaration, this test will fail and force the author to update the map
// deliberately (no silent schema drift).

#[tokio::test]
async fn fk_declaration_audit() {
    let app = TestApp::spawn().await;
    let pool = &app.state.db;

    // --- Tables that SHOULD have an FK to patients(id) ---
    //
    // Every clinical/scheduling/billing child of `patients` declares
    // `patient_id ... REFERENCES patients(id) ON DELETE CASCADE`.
    assert!(has_fk(pool, "appointments", "patient_id", "patients").await);
    assert!(has_fk(pool, "clinical_notes", "patient_id", "patients").await);
    assert!(has_fk(pool, "allergies", "patient_id", "patients").await);
    assert!(has_fk(pool, "osdi_scores", "patient_id", "patients").await);
    assert!(has_fk(pool, "ipl_treatments", "patient_id", "patients").await);
    assert!(has_fk(pool, "invoices", "patient_id", "patients").await);
    assert!(has_fk(pool, "patient_photos", "patient_id", "patients").await);

    // --- invoices.appointment_id -> appointments(id) ON DELETE SET NULL ---
    // (added in migration 0014 via table rebuild)
    assert!(has_fk(pool, "invoices", "appointment_id", "appointments").await);

    // --- patient_photos.appointment_id -> appointments(id) ON DELETE SET NULL ---
    // (added in migration 0014 via table rebuild)
    assert!(
        has_fk(pool, "patient_photos", "appointment_id", "appointments").await
    );

    // --- invoice_items.invoice_id -> invoices(id) ON DELETE CASCADE ---
    assert!(has_fk(pool, "invoice_items", "invoice_id", "invoices").await);

    // --- payments.invoice_id -> invoices(id) ON DELETE CASCADE ---
    assert!(has_fk(pool, "payments", "invoice_id", "invoices").await);

    // --- The GAP: soft-link columns with NO FK declaration ---
    //
    // These two columns point at patients(id) semantically but have no
    // FOREIGN KEY clause, so the DB cannot reject dangling references.
    // Documented here so the gap is visible in the test suite. See the
    // orphan-creation tests below for the behavioural proof.
    assert!(
        !has_fk(pool, "messages", "linked_patient_id", "patients").await,
        "messages.linked_patient_id has NO FK declaration — orphans are silently possible \
         (defense-in-depth gap; handler guards compensate but schema does not enforce)"
    );
    assert!(
        !has_fk(pool, "intake_submissions", "matched_patient_id", "patients").await,
        "intake_submissions.matched_patient_id has NO FK declaration — orphans are silently possible \
         (defense-in-depth gap; handler guards compensate but schema does not enforce)"
    );

    // --- Intentionally-unconstrained soft-links (documented decision) ---
    //
    // audit_log.user_id: an audit entry may outlive the user it references
    // (e.g. a deleted user's actions must remain auditable). FK-free by design.
    assert!(
        !has_fk(pool, "audit_log", "user_id", "users").await,
        "audit_log.user_id is intentionally FK-free (audit entries outlive users)"
    );
    // booking_notifications.{booking_id, intake_submission_id}: a notification
    // log row may reference a booking/intake that is later deleted; the log
    // is append-only history. FK-free by design.
    assert!(
        !has_fk(pool, "booking_notifications", "booking_id", "appointments").await,
        "booking_notifications.booking_id is intentionally FK-free (append-only history)"
    );
}

// ===========================================================================
// 2. Declared FKs ARE enforced (PRAGMA foreign_keys = ON is effective)
// ===========================================================================

/// Inserting an appointment with a `patient_id` that does not exist in
/// `patients` must be rejected by the DB, because `appointments.patient_id`
/// has a declared FOREIGN KEY and the pool enables `PRAGMA foreign_keys = ON`.
///
/// This proves the global FK enable is actually effective on request
/// connections (not just documented). If this test fails, the pragma is not
/// being applied — every other FK guarantee is then void.
#[tokio::test]
async fn declared_fk_blocks_orphan_appointment() {
    let app = TestApp::spawn().await;
    let pool = &app.state.db;

    // 999999 does not exist in patients.
    let res = sqlx::query(
        "INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes)
         VALUES (999999, 'consultation', '2099-01-01T09:00:00Z', 30)",
    )
    .execute(pool)
    .await;

    assert!(
        res.is_err(),
        "INSERT into appointments with a nonexistent patient_id must be rejected by the \
         declared FOREIGN KEY (PRAGMA foreign_keys = ON is set at pool init)"
    );
    // sqlx surfaces an sqlite FOREIGN KEY constraint error.
    let msg = format!("{}", res.unwrap_err());
    assert!(
        msg.to_lowercase().contains("foreign key"),
        "expected a FOREIGN KEY constraint error, got: {msg}"
    );
}

/// Symmetric check on the billing side: an invoice_item pointing at a
/// nonexistent invoice is rejected.
#[tokio::test]
async fn declared_fk_blocks_orphan_invoice_item() {
    let app = TestApp::spawn().await;
    let pool = &app.state.db;

    let res = sqlx::query(
        "INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, total)
         VALUES (999999, 'service', 'orphan probe', 1, 10.0, 10.0)",
    )
    .execute(pool)
    .await;

    assert!(
        res.is_err(),
        "INSERT into invoice_items with a nonexistent invoice_id must be rejected by the \
         declared FOREIGN KEY"
    );
    let msg = format!("{}", res.unwrap_err());
    assert!(
        msg.to_lowercase().contains("foreign key"),
        "expected a FOREIGN KEY constraint error, got: {msg}"
    );
}

// ===========================================================================
// 3. The GAP: undeclared soft-links allow orphans (defense-in-depth hole)
// ===========================================================================
//
// These two tests prove the orphan-creation bug exists TODAY: because
// `messages.linked_patient_id` and `intake_submissions.matched_patient_id`
// have no FOREIGN KEY declaration, a direct DB write (or a future code path
// that bypasses the handler guard) can store a dangling reference. The DB
// does not reject it.
//
// We assert the CURRENT broken behaviour (insert succeeds) with a TODO
// comment, matching the project's `*_documented_decision` convention. When a
// migration adds the FK declarations, flip these to expect rejection and
// move them into section 2.

/// PROOF OF GAP: a message can be linked to a nonexistent patient via a
/// direct DB write, because `messages.linked_patient_id` has no FK.
///
/// The `messages::link_patient` handler guards against this with an EXISTS
/// check, so the HTTP path is safe today. But the schema provides no
/// defense-in-depth: any other writer (bulk import, a new route, a migration
/// that forgets the guard) will silently create an orphan that the DB cannot
/// catch.
///
/// TODO(fk-defense-in-depth): once a migration adds
/// `FOREIGN KEY (linked_patient_id) REFERENCES patients(id) ON DELETE SET NULL`
/// to `messages`, flip the assertion below to expect the INSERT to FAIL.
#[tokio::test]
async fn orphan_message_linked_patient_id_is_currently_allowed_documented_gap() {
    let app = TestApp::spawn().await;
    let pool = &app.state.db;

    // Insert a message with no patient link first (the normal seed path).
    let res = sqlx::query(
        "INSERT INTO messages (channel, from_name, body, status, linked_patient_id)
         VALUES ('website', 'Orphan Probe', 'test', 'unread', 999999)",
    )
    .execute(pool)
    .await;

    // CURRENT BEHAVIOUR (the bug): succeeds because no FK is declared.
    assert!(
        res.is_ok(),
        "DOCUMENTED GAP: messages.linked_patient_id has no FOREIGN KEY declaration, so a \
         dangling insert silently succeeds. The handler guards the HTTP path, but the schema \
         does not enforce referential integrity. TODO: add the FK via a new migration and \
         flip this assertion to expect rejection."
    );

    // Confirm the orphan row is actually persisted with the dangling value.
    let row = sqlx::query("SELECT linked_patient_id FROM messages WHERE linked_patient_id = 999999")
        .fetch_one(pool)
        .await
        .expect("orphan message row should be readable");
    let v: Option<i64> = row.get("linked_patient_id");
    assert_eq!(v, Some(999999), "the dangling linked_patient_id was persisted");
}

/// PROOF OF GAP: an intake submission can be matched to a nonexistent
/// patient via a direct DB write, because
/// `intake_submissions.matched_patient_id` has no FK.
///
/// The intake import/approve/merge handlers all set `matched_patient_id` to
/// a patient they just created or verified, so the HTTP path is safe today.
/// But the schema provides no defense-in-depth.
///
/// TODO(fk-defense-in-depth): once a migration adds
/// `FOREIGN KEY (matched_patient_id) REFERENCES patients(id) ON DELETE SET NULL`
/// to `intake_submissions`, flip the assertion below to expect the INSERT to
/// FAIL.
#[tokio::test]
async fn orphan_intake_matched_patient_id_is_currently_allowed_documented_gap() {
    let app = TestApp::spawn().await;
    let pool = &app.state.db;

    let res = sqlx::query(
        "INSERT INTO intake_submissions (first_name, last_name, status, matched_patient_id)
         VALUES ('Orphan', 'Probe', 'imported', 999999)",
    )
    .execute(pool)
    .await;

    // CURRENT BEHAVIOUR (the bug): succeeds because no FK is declared.
    assert!(
        res.is_ok(),
        "DOCUMENTED GAP: intake_submissions.matched_patient_id has no FOREIGN KEY declaration, \
         so a dangling insert silently succeeds. The handler guards the HTTP path, but the \
         schema does not enforce referential integrity. TODO: add the FK via a new migration \
         and flip this assertion to expect rejection."
    );

    let row = sqlx::query(
        "SELECT matched_patient_id FROM intake_submissions WHERE matched_patient_id = 999999",
    )
    .fetch_one(pool)
    .await
    .expect("orphan intake row should be readable");
    let v: Option<i64> = row.get("matched_patient_id");
    assert_eq!(v, Some(999999), "the dangling matched_patient_id was persisted");
}

// ===========================================================================
// 4. Orphan on parent delete: declared CASCADE works, soft-links dangle
// ===========================================================================

/// When a patient with declared CASCADE dependents is deleted (via a path
/// that bypasses the handler's referential guard — here, a direct DB DELETE
/// with FKs ON), the cascade removes the dependent rows. This proves the
/// ON DELETE CASCADE declarations are live.
#[tokio::test]
async fn deleting_patient_cascades_to_declared_dependents() {
    let app = TestApp::spawn().await;
    let pool = &app.state.db;

    // Create a patient directly.
    let row = sqlx::query(
        "INSERT INTO patients (mrn, first_name, last_name, date_of_birth)
         VALUES ('CASC-1', 'Cascade', 'Test', '1990-01-01') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let pid: i64 = row.get("id");

    // Create a declared dependent (appointment).
    sqlx::query(
        "INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes)
         VALUES (?, 'consultation', '2099-01-01T09:00:00Z', 30)",
    )
    .bind(pid)
    .execute(pool)
    .await
    .unwrap();

    // Direct DB delete (bypasses the handler guard). With FKs ON and the
    // declared ON DELETE CASCADE, the appointment row is removed too.
    sqlx::query("DELETE FROM patients WHERE id = ?")
        .bind(pid)
        .execute(pool)
        .await
        .expect("direct DELETE should succeed (no RESTRICT dependents block it here)");

    let appt_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM appointments WHERE patient_id = ?")
            .bind(pid)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        appt_count, 0,
        "ON DELETE CASCADE should have removed the appointment when the patient was deleted"
    );
}

/// When a patient is deleted, a `messages.linked_patient_id` pointing at it
/// becomes a dangling reference (NOT nulled, NOT cascaded) because there is
/// no FK declaration. This is the delete-side face of the gap.
#[tokio::test]
async fn deleting_patient_dangles_messages_linked_patient_id_documented_gap() {
    let app = TestApp::spawn().await;
    let pool = &app.state.db;

    // Create a patient.
    let row = sqlx::query(
        "INSERT INTO patients (mrn, first_name, last_name, date_of_birth)
         VALUES ('DANGLE-1', 'Dangle', 'Test', '1990-01-01') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let pid: i64 = row.get("id");

    // Link a message to it (direct DB write; no FK to enforce).
    sqlx::query(
        "INSERT INTO messages (channel, from_name, body, status, linked_patient_id)
         VALUES ('website', 'Dangle Probe', 'test', 'unread', ?)",
    )
    .bind(pid)
    .execute(pool)
    .await
    .unwrap();

    // Delete the patient directly.
    sqlx::query("DELETE FROM patients WHERE id = ?")
        .bind(pid)
        .execute(pool)
        .await
        .unwrap();

    // The message row survives with a DANGLING linked_patient_id.
    let row = sqlx::query("SELECT linked_patient_id FROM messages WHERE linked_patient_id = ?")
        .bind(pid)
        .fetch_one(pool)
        .await
        .expect("message row should still exist (no CASCADE/SET NULL declared)");
    let v: Option<i64> = row.get("linked_patient_id");
    assert_eq!(
        v,
        Some(pid),
        "DOCUMENTED GAP: messages.linked_patient_id is now a dangling reference — the patient \
         was deleted but the message still points at its id. No FK declaration means no \
         ON DELETE SET NULL / CASCADE. TODO: add the FK and this becomes NULL (SET NULL)."
    );
}
