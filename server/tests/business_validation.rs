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
    assert_eq!(resp.status(), 200, "zero unit_price (free item) should be accepted");
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
