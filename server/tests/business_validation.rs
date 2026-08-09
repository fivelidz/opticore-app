//! Business-logic validation tests.
//!
//! These tests assert that the API rejects semantically-invalid inputs that
//! would otherwise corrupt financial/state data:
//!
//! * Invoice items with non-positive quantities or negative unit prices.
//! * Overflow / NaN / Infinity in invoice totals.
//! * Payments against nonexistent invoices, with non-positive amounts, or
//!   exceeding the outstanding balance (overpayment).
//! * Appointments booked in the past.
//!
//! The corresponding handler-level guards live in `routes/billing.rs` and
//! `routes/appointments.rs`.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

/// Create a patient and return its id.
async fn create_patient(app: &TestApp, t: &str, first: &str) -> i64 {
    let body = serde_json::json!({
        "first_name": first, "last_name": "Validation", "date_of_birth": "1990-01-01",
    });
    let r = app.post("/api/patients").auth(t).json(&body).send().await.unwrap();
    body_json(r).await["id"].as_i64().unwrap()
}

/// Create a simple invoice with a single item and return the parsed JSON
/// (used by payment tests to obtain a valid invoice_id).
async fn create_invoice(app: &TestApp, t: &str, pid: i64, qty: f64, price: f64) -> serde_json::Value {
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{
            "item_type": "consultation",
            "description": "Consult",
            "quantity": qty,
            "unit_price": price,
            "tax_rate": 0.0,
        }],
    });
    let resp = app.post("/api/billing/invoices").auth(t).json(&body).send().await.unwrap();
    body_json(resp).await
}

// =====================================================================
// Invoice item quantity / price validation + overflow guards
// =====================================================================
//
// Business rules (documented in the fix commit):
//   * items must be non-empty
//   * quantity  must be > 0  (a zero/negative line item is nonsensical)
//   * unit_price must be >= 0 (free items are allowed; negative prices are not)
//   * discount_percent must be in [0, 100]
//   * tax_rate must be in [0, 1]
//   * no NaN/Infinity inputs
//   * computed subtotal/tax/total must be finite (overflow guard)
//
// Rationale: a negative quantity or price produces a negative line total,
// which can zero-out or invert an invoice's grand total — an obvious
// financial-data-integrity bug. Free items (price 0) are legitimate
// (e.g. a complimentary follow-up), so we only reject negative prices.

#[tokio::test]
async fn invoice_rejects_negative_quantity() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "NegQty").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{
            "item_type": "consultation", "description": "X",
            "quantity": -2.0, "unit_price": 50.0, "tax_rate": 0.0,
        }],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "negative quantity must be 400");
}

#[tokio::test]
async fn invoice_rejects_zero_quantity() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "ZeroQty").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{
            "item_type": "consultation", "description": "X",
            "quantity": 0.0, "unit_price": 50.0, "tax_rate": 0.0,
        }],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "zero quantity must be 400");
}

#[tokio::test]
async fn invoice_rejects_negative_unit_price() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "NegPrice").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{
            "item_type": "consultation", "description": "X",
            "quantity": 1.0, "unit_price": -50.0, "tax_rate": 0.0,
        }],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "negative unit_price must be 400");
}

#[tokio::test]
async fn invoice_allows_zero_unit_price() {
    // Free items are legitimate (complimentary service). Must NOT be rejected.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "FreeItem").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{
            "item_type": "consultation", "description": "Complimentary",
            "quantity": 1.0, "unit_price": 0.0, "tax_rate": 0.0,
        }],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "zero unit_price (free item) should be accepted");
    let v = body_json(resp).await;
    assert_eq!(v["total_amount"], 0.0);
}

#[tokio::test]
async fn invoice_rejects_discount_percent_out_of_range() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "BadDisc").await;
    // > 100% discount would invert the line total.
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{
            "item_type": "consultation", "description": "X",
            "quantity": 1.0, "unit_price": 100.0, "discount_percent": 150.0, "tax_rate": 0.0,
        }],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "discount_percent > 100 must be 400");

    // Negative discount is also invalid.
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{
            "item_type": "consultation", "description": "X",
            "quantity": 1.0, "unit_price": 100.0, "discount_percent": -5.0, "tax_rate": 0.0,
        }],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "negative discount_percent must be 400");
}

#[tokio::test]
async fn invoice_rejects_tax_rate_out_of_range() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "BadTax").await;
    // tax_rate > 1.0 (100%) is implausible for a tax rate.
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{
            "item_type": "consultation", "description": "X",
            "quantity": 1.0, "unit_price": 100.0, "tax_rate": 2.5,
        }],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "tax_rate > 1.0 must be 400");

    // Negative tax rate.
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{
            "item_type": "consultation", "description": "X",
            "quantity": 1.0, "unit_price": 100.0, "tax_rate": -0.1,
        }],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "negative tax_rate must be 400");
}

#[tokio::test]
async fn invoice_rejects_empty_items() {
    // An invoice with no line items is meaningless and would store a zero
    // total with no breakdown.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "NoItems").await;
    let body = serde_json::json!({ "patient_id": pid, "items": [] });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "empty items list must be 400");
}

#[tokio::test]
async fn invoice_rejects_overflowing_totals() {
    // Two huge line items whose sum overflows f64 to +Infinity.
    // f64::MAX is itself finite, but adding two of them (via
    // quantity * unit_price then summing across items) overflows to Inf.
    // We can't write `1.8e308` as a literal (the compiler calls it Inf), so
    // derive f64::MAX at runtime.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Overflow").await;
    let huge = f64::MAX;
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [
            { "item_type": "consultation", "description": "A", "quantity": huge, "unit_price": huge, "tax_rate": 0.0 },
            { "item_type": "consultation", "description": "B", "quantity": huge, "unit_price": huge, "tax_rate": 0.0 },
        ],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "overflowing totals (Infinity) must be 400");
}

// =====================================================================
// Payment validation
// =====================================================================
//
// Business rules (documented in the fix commit):
//   * amount must be > 0 and finite (reject zero, negative, NaN, Infinity)
//   * invoice must exist (404 if not — previously surfaced as 500 from
//     the FK constraint violation on INSERT)
//   * amount must not exceed the outstanding balance (reject overpayment
//     with 400 — conservative policy; previously the SQL MAX(0, ...)
//     silently clamped balance_due to 0, discarding the overpayment)

#[tokio::test]
async fn payment_to_nonexistent_invoice_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pay = serde_json::json!({
        "invoice_id": 999999, "amount": 50.0, "payment_method": "card",
    });
    let resp = app.post("/api/billing/payments").auth(&t).json(&pay).send().await.unwrap();
    assert_eq!(resp.status(), 404, "payment to nonexistent invoice must be 404");
}

#[tokio::test]
async fn payment_rejects_negative_amount() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "NegPay").await;
    let inv = create_invoice(&app, &t, pid, 1.0, 100.0).await;
    let inv_id = inv["id"].as_i64().unwrap();
    let pay = serde_json::json!({
        "invoice_id": inv_id, "amount": -50.0, "payment_method": "card",
    });
    let resp = app.post("/api/billing/payments").auth(&t).json(&pay).send().await.unwrap();
    assert_eq!(resp.status(), 400, "negative payment must be 400");
}

#[tokio::test]
async fn payment_rejects_zero_amount() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "ZeroPay").await;
    let inv = create_invoice(&app, &t, pid, 1.0, 100.0).await;
    let inv_id = inv["id"].as_i64().unwrap();
    let pay = serde_json::json!({
        "invoice_id": inv_id, "amount": 0.0, "payment_method": "card",
    });
    let resp = app.post("/api/billing/payments").auth(&t).json(&pay).send().await.unwrap();
    assert_eq!(resp.status(), 400, "zero payment must be 400");
}

#[tokio::test]
async fn payment_rejects_overpayment() {
    // Conservative policy: reject payments that exceed the outstanding
    // balance. Previously the SQL `MAX(0, ...)` silently clamped the
    // balance to 0, discarding the overpayment amount entirely (the
    // payment row itself was recorded, but the invoice showed no credit /
    // refund owing — a silent data-loss bug for the practice).
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Overpay").await;
    let inv = create_invoice(&app, &t, pid, 1.0, 100.0).await;
    let inv_id = inv["id"].as_i64().unwrap();
    let pay = serde_json::json!({
        "invoice_id": inv_id, "amount": 150.0, "payment_method": "card",
    });
    let resp = app.post("/api/billing/payments").auth(&t).json(&pay).send().await.unwrap();
    assert_eq!(resp.status(), 400, "overpayment must be 400 (conservative policy)");

    // Confirm no payment row was inserted (the fix must validate BEFORE insert).
    let resp = app.get(&format!("/api/billing/payments/invoice/{}", inv_id)).auth(&t).send().await.unwrap();
    let v = body_json(resp).await;
    assert_eq!(v.as_array().unwrap().len(), 0, "no payment row should exist after rejected overpayment");
}

#[tokio::test]
async fn payment_rejects_overpayment_on_second_partial_payment() {
    // First partial payment is fine; second that would overpay must be rejected.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Overpay2").await;
    let inv = create_invoice(&app, &t, pid, 1.0, 100.0).await;
    let inv_id = inv["id"].as_i64().unwrap();

    // Pay 60 — OK (balance 40).
    let pay = serde_json::json!({ "invoice_id": inv_id, "amount": 60.0, "payment_method": "card" });
    let resp = app.post("/api/billing/payments").auth(&t).json(&pay).send().await.unwrap();
    assert_eq!(resp.status(), 201);

    // Pay 50 — would overpay (60 + 50 > 100).
    let pay = serde_json::json!({ "invoice_id": inv_id, "amount": 50.0, "payment_method": "card" });
    let resp = app.post("/api/billing/payments").auth(&t).json(&pay).send().await.unwrap();
    assert_eq!(resp.status(), 400, "second payment overpaying the balance must be 400");
}

#[tokio::test]
async fn payment_exact_balance_is_allowed() {
    // Paying the exact remaining balance is the normal "settle up" case.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "ExactPay").await;
    let inv = create_invoice(&app, &t, pid, 1.0, 100.0).await;
    let inv_id = inv["id"].as_i64().unwrap();
    let pay = serde_json::json!({ "invoice_id": inv_id, "amount": 100.0, "payment_method": "cash" });
    let resp = app.post("/api/billing/payments").auth(&t).json(&pay).send().await.unwrap();
    assert_eq!(resp.status(), 201, "exact-balance payment must be accepted");
}

// =====================================================================
// Appointment date validation
// =====================================================================
//
// Business rules (documented in the fix commit):
//   * appointment_date must parse as RFC3339 or "YYYY-MM-DD"
//     (malformed -> 400; previously normalize_dt stored the raw string)
//   * the parsed instant must not be in the past (past -> 400)
//
// We do NOT constrain far-future dates.

#[tokio::test]
async fn appointment_rejects_past_date() {
    // A practice-management system should not allow booking appointments in
    // the past. We use a clearly-past RFC3339 timestamp.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "PastAppt").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": "2000-01-01T09:00:00Z",
        "duration_minutes": 30,
        "practitioner": "Dr. Test",
    });
    let resp = app.post("/api/appointments").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "past appointment date must be 400");
}

#[tokio::test]
async fn appointment_allows_future_date() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "FutureAppt").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": "2099-06-15T09:00:00Z",
        "duration_minutes": 30,
        "practitioner": "Dr. Test",
    });
    let resp = app.post("/api/appointments").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "future appointment date must be accepted");
}

#[tokio::test]
async fn appointment_rejects_malformed_date() {
    // A garbage string that normalize_dt can't parse should be rejected,
    // not silently stored verbatim.
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "BadDate").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": "not-a-date",
        "duration_minutes": 30,
    });
    let resp = app.post("/api/appointments").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400, "malformed appointment date must be 400");
}

#[tokio::test]
async fn appointment_update_allows_past_date() {
    // Updating an existing appointment must ACCEPT a past appointment_date.
    // The UI re-sends the original (past) date when marking a completed/cancelled
    // appointment or editing its notes, so rejecting past dates on UPDATE would
    // break the most common historical-appointment workflows. (Past-date
    // rejection remains in force on CREATE.)
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "PastUpd").await;
    // Create a valid future appointment first.
    let body = serde_json::json!({
        "patient_id": pid,
        "appointment_type": "Consultation",
        "appointment_date": "2099-06-15T09:00:00Z",
        "duration_minutes": 30,
        "practitioner": "Dr. Test",
    });
    let r = app.post("/api/appointments").auth(&t).json(&body).send().await.unwrap();
    let appt_id = body_json(r).await["id"].as_i64().unwrap();

    // Move it into the past AND change status — this is exactly what the
    // "Mark completed" / "Cancel" buttons do for a historical appointment.
    let body = serde_json::json!({
        "appointment_type": "Consultation",
        "appointment_date": "2000-01-01T09:00:00Z",
        "duration_minutes": 30,
        "practitioner": "Dr. Test",
        "status": "completed",
    });
    let resp = app.put(&format!("/api/appointments/{}", appt_id)).auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200, "updating an appointment to a past date (e.g. marking it completed) must be accepted");
    let j = body_json(resp).await;
    assert_eq!(j["status"], "completed", "status change on a past-dated appointment must persist");
}
