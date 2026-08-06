//! Clinical (notes, allergies, OSDI, IPL) + billing (catalog, invoices,
//! payments) + analytics tests.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

/// Create a patient and return its id.
async fn create_patient(app: &TestApp, t: &str, first: &str) -> i64 {
    let body = serde_json::json!({
        "first_name": first, "last_name": "Clinical", "date_of_birth": "1990-01-01",
    });
    let r = app.post("/api/patients").auth(t).json(&body).send().await.unwrap();
    body_json(r).await["id"].as_i64().unwrap()
}

// ---------- Clinical notes ----------

#[tokio::test]
async fn add_and_list_clinical_note() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Note").await;

    let body = serde_json::json!({
        "patient_id": pid, "category": "general", "note": "Test clinical note",
        "author": "Dr. Test",
    });
    let resp = app.post(&format!("/api/patients/{}/notes", pid)).auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["note"], "Test clinical note");

    // List should now contain it.
    let resp = app.get(&format!("/api/patients/{}/notes", pid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn delete_clinical_note() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "NoteDel").await;
    let body = serde_json::json!({ "patient_id": pid, "note": "To be deleted" });
    let r = app.post(&format!("/api/patients/{}/notes", pid)).auth(&t).json(&body).send().await.unwrap();
    let nid = body_json(r).await["id"].as_i64().unwrap();

    let resp = app.delete(&format!("/api/patients/{}/notes/{}", pid, nid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn clinical_notes_require_auth() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/patients/1/notes").send().await.unwrap().status(), 401);
}

// ---------- Allergies ----------

#[tokio::test]
async fn add_and_list_allergy() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Allergy").await;

    let body = serde_json::json!({
        "patient_id": pid, "substance": "Penicillin", "severity": "severe",
    });
    let resp = app.post("/api/allergies").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["substance"], "Penicillin");
    assert_eq!(v["severity"], "severe");

    // List for the patient.
    let resp = app.get(&format!("/api/patients/{}/allergies", pid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn delete_allergy() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "AlgDel").await;
    let body = serde_json::json!({ "patient_id": pid, "substance": "Latex" });
    let r = app.post("/api/allergies").auth(&t).json(&body).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let resp = app.delete(&format!("/api/allergies/{}", id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ---------- OSDI ----------

#[tokio::test]
async fn add_and_list_osdi() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Osdi").await;
    let body = serde_json::json!({
        "patient_id": pid, "score_date": "2026-01-15",
        "total_score": 32.5, "ocular_symptoms": 12.0, "vision_function": 10.5,
        "environmental_triggers": 10.0,
    });
    let resp = app.post(&format!("/api/patients/{}/osdi", pid)).auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["total_score"], 32.5);

    let resp = app.get(&format!("/api/patients/{}/osdi", pid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
}

// ---------- IPL ----------

#[tokio::test]
async fn add_and_list_ipl() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Ipl").await;
    let body = serde_json::json!({
        "patient_id": pid, "treatment_date": "2026-02-20",
        "session_number": 1, "fluence_j_cm2": 12.0, "number_of_pulses": 15,
        "operator_name": "Dr. Test",
    });
    let resp = app.post(&format!("/api/patients/{}/ipl", pid)).auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["session_number"], 1);
    assert_eq!(v["fluence_j_cm2"], 12.0);

    let resp = app.get(&format!("/api/patients/{}/ipl", pid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
}

// ---------- Billing catalog ----------

#[tokio::test]
async fn consultation_types_returns_catalog() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/billing/consultation-types").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v.is_array(), "consultation types should be an array");
}

#[tokio::test]
async fn services_returns_catalog() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/billing/services").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v.is_array(), "services should be an array");
}

#[tokio::test]
async fn service_categories_returns_distinct() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/billing/service-categories").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v.is_array(), "categories should be an array");
}

// ---------- Invoices + payments ----------

#[tokio::test]
async fn create_invoice_computes_totals() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Invoice").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [
            { "item_type": "consultation", "description": "Dry Eye Consult", "quantity": 1.0, "unit_price": 200.0, "tax_rate": 0.10 },
            { "item_type": "service", "description": "IPL Treatment", "quantity": 1.0, "unit_price": 300.0, "tax_rate": 0.10 },
        ],
    });
    let resp = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    // subtotal = 500, tax = 50, total = 550.
    assert_eq!(v["subtotal"], 500.0);
    assert_eq!(v["tax_amount"], 50.0);
    assert_eq!(v["total_amount"], 550.0);
    assert_eq!(v["balance_due"], 550.0);
    assert_eq!(v["amount_paid"], 0.0);
    assert_eq!(v["status"], "issued");
    assert_eq!(v["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn invoices_by_patient_returns_them() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "InvList").await;
    // Create one.
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{ "item_type": "consultation", "description": "Consult", "quantity": 1.0, "unit_price": 100.0 }],
    });
    app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();

    let resp = app.get(&format!("/api/billing/invoices/patient/{}", pid)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn add_payment_updates_invoice_balance() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "Pay").await;
    // Create a $550 invoice.
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{ "item_type": "consultation", "description": "Consult", "quantity": 1.0, "unit_price": 500.0, "tax_rate": 0.10 }],
    });
    let r = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    let inv_id = body_json(r).await["id"].as_i64().unwrap();

    // Pay $200 (partial).
    let pay = serde_json::json!({
        "invoice_id": inv_id, "amount": 200.0, "payment_method": "card",
    });
    let resp = app.post("/api/billing/payments").auth(&t).json(&pay).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["amount"], 200.0);

    // Verify invoice balance updated.
    let resp = app.get(&format!("/api/billing/invoices/patient/{}", pid)).auth(&t).send().await.unwrap();
    let v = body_json(resp).await;
    let inv = &v[0];
    assert_eq!(inv["amount_paid"], 200.0);
    assert_eq!(inv["balance_due"], 350.0);
    assert_eq!(inv["status"], "partially_paid");
}

#[tokio::test]
async fn full_payment_marks_invoice_paid() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "FullPay").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{ "item_type": "consultation", "description": "Consult", "quantity": 1.0, "unit_price": 100.0 }],
    });
    let r = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    let inv = body_json(r).await;
    let inv_id = inv["id"].as_i64().unwrap();
    let total = inv["total_amount"].as_f64().unwrap();

    let pay = serde_json::json!({
        "invoice_id": inv_id, "amount": total, "payment_method": "cash",
    });
    app.post("/api/billing/payments").auth(&t).json(&pay).send().await.unwrap();

    let resp = app.get(&format!("/api/billing/invoices/patient/{}", pid)).auth(&t).send().await.unwrap();
    let v = body_json(resp).await;
    assert_eq!(v[0]["status"], "paid");
    assert_eq!(v[0]["balance_due"], 0.0);
}

#[tokio::test]
async fn payments_by_invoice_returns_them() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let pid = create_patient(&app, &t, "PayList").await;
    let body = serde_json::json!({
        "patient_id": pid,
        "items": [{ "item_type": "consultation", "description": "Consult", "quantity": 1.0, "unit_price": 100.0 }],
    });
    let r = app.post("/api/billing/invoices").auth(&t).json(&body).send().await.unwrap();
    let inv_id = body_json(r).await["id"].as_i64().unwrap();

    let pay = serde_json::json!({ "invoice_id": inv_id, "amount": 50.0, "payment_method": "card" });
    app.post("/api/billing/payments").auth(&t).json(&pay).send().await.unwrap();

    let resp = app.get(&format!("/api/billing/payments/invoice/{}", inv_id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn billing_endpoints_require_auth() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/billing/consultation-types").send().await.unwrap().status(), 401);
    assert_eq!(app.get("/api/billing/services").send().await.unwrap().status(), 401);
}

// ---------- Analytics ----------

#[tokio::test]
async fn analytics_overview_returns_counts() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/analytics/overview").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v["total_patients"].as_i64().unwrap() > 0, "seeded patients");
    assert!(v["total_appointments"].as_i64().unwrap() > 0, "seeded appointments");
    assert!(v.get("total_revenue").is_some());
    assert!(v.get("outstanding_balance").is_some());
}

#[tokio::test]
async fn analytics_revenue_series_returns_array() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/analytics/revenue/30").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v.is_array(), "revenue series should be an array");
}

#[tokio::test]
async fn analytics_appointment_series_returns_array() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/analytics/appointments/30").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v.is_array());
}

#[tokio::test]
async fn analytics_no_show_rate_returns_value() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/analytics/no-show-rate").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    // Seed includes one noshow (migration 0011), so the rate should be present.
    assert!(v.get("rate").is_some() || v.get("no_show_rate").is_some() || v.is_number());
}

#[tokio::test]
async fn analytics_endpoints_require_auth() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/analytics/overview").send().await.unwrap().status(), 401);
}
