//! Booking settings + public API tests.
//!
//! - booking_settings: GET/PUT the single config row, GET notifications.
//! - public_api: availability, appointment-types, match-patient (all unauth).

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

// ---------- Booking settings ----------

#[tokio::test]
async fn get_booking_settings_returns_seeded_row() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/booking-settings").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    // 0008_booking_settings.sql seeds id=1 with booking_mode='approval'.
    assert_eq!(v["booking_mode"], "approval");
    assert_eq!(v["reminder_hours_before"], 24);
    assert!(v.get("template_booking_received").is_some());
}

#[tokio::test]
async fn update_booking_settings_changes_fields() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "booking_mode": "automatic",
        "reminder_hours_before": 48,
    });
    let resp = app.put("/api/booking-settings").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["booking_mode"], "automatic");
    assert_eq!(v["reminder_hours_before"], 48);
    // Untouched fields retain their seeded values.
    assert!(v.get("template_booking_received").is_some());
}

#[tokio::test]
async fn get_booking_notifications_returns_array() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/booking-notifications").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v.is_array(), "notifications should be an array");
}

#[tokio::test]
async fn booking_settings_require_auth() {
    let app = TestApp::spawn().await;
    assert_eq!(app.get("/api/booking-settings").send().await.unwrap().status(), 401);
    assert_eq!(app.get("/api/booking-notifications").send().await.unwrap().status(), 401);
}

// ---------- Public API (no auth) ----------

#[tokio::test]
async fn public_availability_returns_slots() {
    let app = TestApp::spawn().await;
    // No auth header — this is a public endpoint.
    let resp = app.get("/api/public/availability/7").send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    let slots = v.as_array().unwrap();
    // 7 days, weekdays only (9am-5pm = 8 slots/day). At least some slots.
    assert!(!slots.is_empty(), "should have availability slots");
    // Each slot has the expected fields.
    assert!(slots[0].get("date").is_some());
    assert!(slots[0].get("time").is_some());
    assert!(slots[0]["available"].is_boolean());
}

#[tokio::test]
async fn public_availability_clamps_days_param() {
    let app = TestApp::spawn().await;
    // 100 days requested but endpoint clamps to 30.
    let resp = app.get("/api/public/availability/100").send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    // 30 weekdays max = ~22 days * 8 slots = ~176. Just confirm reasonable.
    assert!(v.as_array().unwrap().len() <= 30 * 8);
}

#[tokio::test]
async fn public_appointment_types_returns_catalog() {
    let app = TestApp::spawn().await;
    let resp = app.get("/api/public/appointment-types").send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert!(v.is_array(), "appointment types should be an array");
    // 0003_clinical_billing.sql seeds consultation types.
    if !v.as_array().unwrap().is_empty() {
        let first = &v[0];
        assert!(first.get("code").is_some());
        assert!(first.get("name").is_some());
        assert!(first.get("price").is_some());
    }
}

#[tokio::test]
async fn public_match_patient_high_confidence_on_name_plus_dob() {
    let app = TestApp::spawn().await;
    // Seed patient: Sarah Johnson, 1985-04-12.
    let body = serde_json::json!({
        "first_name": "Sarah", "last_name": "Johnson", "date_of_birth": "1985-04-12",
    });
    let resp = app.post("/api/public/match-patient").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["matched"], true);
    assert_eq!(v["confidence"], "high");
}

#[tokio::test]
async fn public_match_patient_medium_confidence_on_phone() {
    let app = TestApp::spawn().await;
    // Seed patient Sarah Johnson phone: "0412 345 678" — match with dashes.
    let body = serde_json::json!({
        "first_name": "Someone", "last_name": "Else", "phone": "0412-345-678",
    });
    let resp = app.post("/api/public/match-patient").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["matched"], true);
    assert_eq!(v["confidence"], "medium");
}

#[tokio::test]
async fn public_match_patient_no_match() {
    let app = TestApp::spawn().await;
    let body = serde_json::json!({
        "first_name": "Definitely", "last_name": "NotHere", "date_of_birth": "1900-01-01",
    });
    let resp = app.post("/api/public/match-patient").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["matched"], false);
    assert_eq!(v["confidence"], "none");
}

#[tokio::test]
async fn public_endpoints_do_not_require_auth() {
    let app = TestApp::spawn().await;
    // All three public endpoints should succeed without a token (no 401).
    assert_eq!(app.get("/api/public/availability/3").send().await.unwrap().status(), 200);
    assert_eq!(app.get("/api/public/appointment-types").send().await.unwrap().status(), 200);
}

// ---------- Booking settings validation (session 10) ----------
//
// The PUT /api/booking-settings endpoint accepted any value for
// booking_mode (not just "automatic"/"approval") and negative
// reminder_hours_before (a negative reminder lead-time is nonsensical).
// These tests lock down the hardened validation.

#[tokio::test]
async fn update_booking_settings_rejects_invalid_booking_mode() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "booking_mode": "nonsense-mode",
    });
    let resp = app
        .put("/api/booking-settings")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "invalid booking_mode must be 400, not 200"
    );
}

#[tokio::test]
async fn update_booking_settings_rejects_negative_reminder_hours() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "reminder_hours_before": -5,
    });
    let resp = app
        .put("/api/booking-settings")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "negative reminder_hours_before must be 400"
    );
}

#[tokio::test]
async fn update_booking_settings_accepts_zero_reminder_hours() {
    // Zero is borderline but legitimate (reminder at appointment time).
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "reminder_hours_before": 0,
    });
    let resp = app
        .put("/api/booking-settings")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "zero reminder_hours_before should succeed");
}

#[tokio::test]
async fn update_booking_settings_accepts_valid_modes() {
    // Both documented modes must remain accepted (regression guard).
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    for mode in ["automatic", "approval"] {
        let body = serde_json::json!({ "booking_mode": mode });
        let resp = app
            .put("/api/booking-settings")
            .auth(&t)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "booking_mode '{mode}' should be accepted");
    }
}
