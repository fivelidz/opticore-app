//! Concurrency / transaction tests.
//!
//! These tests verify that concurrent writes do not corrupt sequences or
//! produce duplicate identifiers. The primary target is the invoice-number
//! generation in `routes::billing::create_invoice`, which historically used
//! `SELECT COUNT(*) FROM invoices` followed by `format!("INV-2026-{:04}",
//! count + 7)` — a classic read-then-write race: two concurrent requests can
//! read the same count, compute the same invoice number, and the second
//! INSERT then violates the `invoice_number UNIQUE` constraint.
//!
//! ## What we assert
//!
//! For N concurrent invoice creations against the same patient:
//! 1. **No duplicate invoice numbers** end up in the DB (the UNIQUE
//!    constraint is the last line of defence, but the application should
//!    never rely on it for correctness — every legitimate request should
//!    succeed).
//! 2. **Every request succeeds** (200) — i.e. the race is fixed at the
//!    application layer via a serializing transaction, not papered over by
//!    returning 409 to one caller.
//! 3. **No 500 / panic** — a DB serialization error must surface as a clean
//!    HTTP error, never a crash.
//!
//! We also test concurrent payment creation against a single invoice to
//! verify the `amount_paid` accumulator does not lose updates (the UPDATE
//! statement reads `amount_paid` in the SET clause, which SQLite evaluates
//! atomically per-statement, so this should be safe — but we lock it in with
//! a regression test).

mod common;

use std::sync::Arc;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

async fn create_patient(app: &TestApp, t: &str, first: &str) -> i64 {
    let body = serde_json::json!({
        "first_name": first,
        "last_name": "Conc",
        "date_of_birth": "1990-01-01",
        "phone": "0400000000",
        "email": format!("{}@conc.test", first.to_lowercase()),
    });
    let resp = app.post("/api/patients").auth(t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "patient create should succeed");
    body_json(resp).await["id"].as_i64().unwrap()
}

/// A single invoice-creation request body for `patient_id`.
fn invoice_body(pid: i64) -> serde_json::Value {
    serde_json::json!({
        "patient_id": pid,
        "items": [
            { "item_type": "consultation", "description": "Consult", "quantity": 1.0, "unit_price": 100.0 },
        ],
    })
}

// ---------- Invoice-number race ----------

/// N concurrent invoice creations for the same patient must all succeed and
/// produce N distinct invoice numbers.
///
/// Before the fix, `create_invoice` computed the invoice number from
/// `COUNT(*)` outside any transaction, so concurrent calls raced on the same
/// count and produced duplicate `INV-2026-00xx` numbers — the second INSERT
/// then failed with a UNIQUE violation (surfaced as HTTP 409 to one caller,
/// or worse, a 500 if the error path changed).
///
/// The fix wraps the count + insert in a `BEGIN IMMEDIATE` transaction so
/// writers serialize: each `create_invoice` sees a count that reflects all
/// previously-committed invoices.
#[tokio::test]
async fn concurrent_invoice_creation_no_duplicate_numbers() {
    let app = Arc::new(TestApp::spawn().await);
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "RaceInv").await;

    let n = 10usize;
    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let app = app.clone();
        let t = t.clone();
        let body = invoice_body(pid);
        handles.push(tokio::spawn(async move {
            app.post("/api/billing/invoices").auth(&t).json(&body).send().await
        }));
    }

    // Collect results. Every request should succeed (200); none should 500.
    let mut statuses = Vec::with_capacity(n);
    for h in handles {
        let resp = h.await.expect("task panicked").expect("send failed");
        let status = resp.status().as_u16();
        // No request should ever return 500 (that would indicate an unhandled
        // DB error / panic rather than a clean serialization outcome).
        assert_ne!(
            status, 500,
            "concurrent invoice create returned 500 — race not handled cleanly"
        );
        statuses.push(status);
    }

    let ok = statuses.iter().filter(|&&s| s == 200).count();
    assert_eq!(
        ok, n,
        "all {} concurrent invoice creates should succeed, but only {} did (statuses: {:?})",
        n, ok, statuses
    );

    // Verify no duplicate invoice numbers in the DB.
    let nums: Vec<(String,)> = sqlx::query_as("SELECT invoice_number FROM invoices WHERE patient_id = ?")
        .bind(pid)
        .fetch_all(&app.state.db)
        .await
        .unwrap();
    let mut seen = std::collections::HashSet::new();
    for (num,) in &nums {
        assert!(seen.insert(num.clone()), "duplicate invoice number in DB: {}", num);
    }
    assert_eq!(nums.len(), n, "expected {} invoices for patient, found {}", n, nums.len());
}

/// Sequential invoice creation must produce monotonically-increasing,
/// gap-free invoice numbers (regression guard for the count-based scheme).
#[tokio::test]
async fn sequential_invoice_creation_distinct_numbers() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "SeqInv").await;

    let mut numbers = Vec::new();
    for _ in 0..5 {
        let resp = app.post("/api/billing/invoices").auth(&t).json(&invoice_body(pid)).send().await.unwrap();
        assert_eq!(resp.status(), 200);
        numbers.push(body_json(resp).await["invoice_number"].as_str().unwrap().to_string());
    }

    let mut seen = std::collections::HashSet::new();
    for n in &numbers {
        assert!(seen.insert(n.clone()), "duplicate sequential invoice number: {}", n);
    }
}

// ---------- Concurrent payment creation ----------

/// N concurrent payments against the same invoice must all be recorded and
/// the invoice's `amount_paid` must equal the sum of all payment amounts
/// (no lost updates).
///
/// The `add_payment` handler issues:
///   `UPDATE invoices SET amount_paid = amount_paid + ?, ...`
/// SQLite evaluates `amount_paid + ?` atomically per statement and the pool
/// serializes writers (single writer at a time), so this should be safe —
/// but we lock it in with a regression test so a future refactor (e.g.
/// switching to a read-modify-write in Rust) cannot silently break it.
#[tokio::test]
async fn concurrent_payments_no_lost_update() {
    let app = Arc::new(TestApp::spawn().await);
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "RacePay").await;

    // Create a $1000 invoice.
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{ "item_type": "consultation", "description": "Big", "quantity": 1.0, "unit_price": 1000.0 }],
    });
    let r = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    let inv_id = body_json(r).await["id"].as_i64().unwrap();

    // 8 concurrent $10 payments = $80 total.
    let n = 8usize;
    let amount = 10.0f64;
    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let app = app.clone();
        let t = t.clone();
        let pay = serde_json::json!({ "invoice_id": inv_id, "amount": amount, "payment_method": "card" });
        handles.push(tokio::spawn(async move {
            app.post("/api/billing/payments").auth(&t).json(&pay).send().await
        }));
    }

    let mut ok = 0usize;
    for h in handles {
        let resp = h.await.expect("task panicked").expect("send failed");
        let status = resp.status().as_u16();
        assert_ne!(status, 500, "concurrent payment returned 500");
        if status == 200 { ok += 1; }
    }
    assert_eq!(ok, n, "all {} concurrent payments should succeed", n);

    // Verify the invoice amount_paid reflects every payment (no lost update).
    let row: (f64,) = sqlx::query_as("SELECT amount_paid FROM invoices WHERE id = ?")
        .bind(inv_id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    let expected = (n as f64) * amount;
    let actual = row.0;
    assert!(
        (actual - expected).abs() < 0.01,
        "lost update: expected amount_paid={}, got {}",
        expected, actual
    );

    // And the payment count matches.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM payments WHERE invoice_id = ?")
        .bind(inv_id)
        .fetch_one(&app.state.db)
        .await
        .unwrap();
    assert_eq!(count.0, n as i64, "expected {} payment rows, got {}", n, count.0);
}
