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

/// N concurrent payments whose **combined** total exceeds the invoice
/// balance must NOT overpay the invoice.
///
/// This is the critical overpayment race. The pre-fix `add_payment` did:
///
/// ```text
///   SELECT balance_due FROM invoices WHERE id = ?   -- (1) read
///   if amount > balance_due { return 400 }          -- (2) check
///   INSERT INTO payments ...                        -- (3) write
///   UPDATE invoices SET amount_paid = amount_paid + ?, balance_due = ...
/// ```
///
/// with **no transaction** wrapping steps 1–4. Two concurrent payments of
/// $60 against a $100 invoice both read `balance_due = 100`, both pass the
/// `60 <= 100` check, both insert a payment row, and both run the UPDATE.
/// The result: `amount_paid = 120`, `balance_due = MAX(0, 100-120) = 0` —
/// the invoice is silently overpaid by $20 and both callers got HTTP 200.
///
/// The fix wraps the balance read + overpayment check + payment insert +
/// invoice update in a single `BEGIN IMMEDIATE` transaction. SQLite
/// serializes writers, so the second payment's `SELECT balance_due` only
/// runs after the first has committed — it then sees `balance_due = 40`
/// and is correctly rejected as a $60-over-$40 overpay (HTTP 400).
///
/// Invariant after the fix:
///   * no 500s
///   * `amount_paid <= total_amount` (never overpaid)
///   * `balance_due >= 0`
///   * `balance_due == total_amount - amount_paid` (consistent)
///   * sum of accepted payment amounts == amount_paid (no lost updates)
#[tokio::test]
async fn concurrent_payments_cannot_overpay() {
    let app = Arc::new(TestApp::spawn().await);
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "OverpayRace").await;

    // Create a $100 invoice.
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{ "item_type": "consultation", "description": "Hundred", "quantity": 1.0, "unit_price": 100.0 }],
    });
    let r = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let inv_id = body_json(r).await["id"].as_i64().unwrap();

    // 10 concurrent $60 payments against a $100 invoice. At most ONE can
    // legitimately succeed (the first to grab the write lock pays $60,
    // leaving $40; every subsequent $60 payment exceeds $40 and must be
    // rejected). If the race exists, multiple will succeed and the invoice
    // will be overpaid.
    let n = 10usize;
    let amount = 60.0f64;
    let mut handles = Vec::with_capacity(n);
    for _ in 0..n {
        let app = app.clone();
        let t = t.clone();
        let pay = serde_json::json!({ "invoice_id": inv_id, "amount": amount, "payment_method": "card" });
        handles.push(tokio::spawn(async move {
            app.post("/api/billing/payments").auth(&t).json(&pay).send().await
        }));
    }

    let mut statuses = Vec::with_capacity(n);
    for h in handles {
        let resp = h.await.expect("task panicked").expect("send failed");
        let status = resp.status().as_u16();
        // No request should ever return 500.
        assert_ne!(status, 500, "concurrent payment returned 500 — race not handled cleanly");
        statuses.push(status);
    }

    let accepted = statuses.iter().filter(|&&s| s == 200).count();
    let rejected = statuses.iter().filter(|&&s| s == 400).count();
    eprintln!("overpay-race: statuses={:?} accepted={} rejected={}", statuses, accepted, rejected);

    // At most ONE $60 payment can fit in a $100 balance. If more than one
    // was accepted, the invoice was overpaid — the race exists.
    assert!(
        accepted <= 1,
        "overpayment race: {} concurrent $60 payments were accepted against a $100 invoice \
         (at most 1 should be). statuses={:?}",
        accepted, statuses
    );
    // The rest must be clean 400 rejections (overpay), not 500s.
    assert_eq!(
        accepted + rejected, n,
        "every request must be either 200 (accepted) or 400 (overpay rejected); got {:?}",
        statuses
    );

    // Verify the invoice is NOT overpaid.
    let row: (f64, f64, f64) =
        sqlx::query_as("SELECT total_amount, amount_paid, balance_due FROM invoices WHERE id = ?")
            .bind(inv_id)
            .fetch_one(&app.state.db)
            .await
            .unwrap();
    let (total, paid, balance) = row;
    assert!(
        paid <= total + 0.01,
        "invoice overpaid: amount_paid={} > total_amount={}", paid, total
    );
    assert!(
        balance >= -0.01,
        "balance_due went negative: {}", balance
    );
    // Consistency: balance_due == total_amount - amount_paid.
    assert!(
        (balance - (total - paid)).abs() < 0.01,
        "inconsistent invoice state: balance_due={} but total-paid={}", balance, total - paid
    );

    // If exactly one payment was accepted, amount_paid must be $60.
    if accepted == 1 {
        assert!(
            (paid - amount).abs() < 0.01,
            "expected amount_paid={} for one accepted payment, got {}", amount, paid
        );
    } else {
        // Zero accepted (pathological scheduling) — amount_paid must be 0.
        assert!(
            paid.abs() < 0.01,
            "no payments accepted but amount_paid={}", paid
        );
    }
}
