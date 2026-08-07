//! Booking settings + public API tests.
//!
//! - booking_settings: GET/PUT the single config row, GET notifications.
//! - public_api: availability, appointment-types, match-patient (all unauth).

mod common;

use common::{body_json, TestApp};
use sqlx::Row;

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

// ---------- booking_settings lazy-init invariant (id=1 always exists) ----------
//
// booking_settings is a single-row config table with CHECK (id = 1). The
// schema guarantees any row present has id=1, but does NOT guarantee a row
// exists. A `DELETE FROM booking_settings` (out-of-band via the sqlite CLI,
// a bug, or a future code path) would leave the table empty and break every
// GET/PUT/approve/decline flow. The handlers now lazy-init the default row
// (INSERT OR IGNORE ... VALUES (1)) whenever it's missing.

/// GET on an empty booking_settings table must NOT 404 — it re-creates the
/// default row and returns it.
#[tokio::test]
async fn get_booking_settings_lazy_inits_when_row_deleted() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    // Simulate an out-of-band DELETE wiping the config row.
    sqlx::query("DELETE FROM booking_settings")
        .execute(&app.state.db)
        .await
        .unwrap();

    // Confirm the table is genuinely empty.
    let count: i64 = sqlx::query("SELECT COUNT(*) FROM booking_settings")
        .fetch_one(&app.state.db)
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0, "precondition: booking_settings should be empty");

    // GET must self-heal: return 200 with the default config, not 404.
    let resp = app.get("/api/booking-settings").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200, "GET should lazy-init, not 404");
    let v = body_json(resp).await;
    assert_eq!(v["booking_mode"], "approval", "default booking_mode");
    assert_eq!(v["reminder_hours_before"], 24, "default reminder_hours_before");

    // The row must now physically exist in the DB (lazy-init persisted it).
    let count: i64 = sqlx::query("SELECT COUNT(*) FROM booking_settings")
        .fetch_one(&app.state.db)
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1, "lazy-init should have re-created the row");
}

/// PUT on an empty booking_settings table must upsert: re-create the default
/// row AND apply the caller's requested fields on top of it (not silently
/// drop them).
#[tokio::test]
async fn put_booking_settings_upserts_when_row_deleted() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    sqlx::query("DELETE FROM booking_settings")
        .execute(&app.state.db)
        .await
        .unwrap();

    let body = serde_json::json!({
        "booking_mode": "automatic",
        "reminder_hours_before": 12,
    });
    let resp = app.put("/api/booking-settings").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200, "PUT should upsert, not 404");
    let v = body_json(resp).await;
    // The caller's fields must be applied (not dropped in favour of defaults).
    assert_eq!(v["booking_mode"], "automatic");
    assert_eq!(v["reminder_hours_before"], 12);
}

/// Repeated GETs after a delete must be stable — the row is created once and
/// stays (no flapping between missing/present).
#[tokio::test]
async fn get_booking_settings_stable_after_repeated_delete() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    for _ in 0..3 {
        sqlx::query("DELETE FROM booking_settings")
            .execute(&app.state.db)
            .await
            .unwrap();
        let resp = app.get("/api/booking-settings").auth(&t).send().await.unwrap();
        assert_eq!(resp.status(), 200, "GET should always self-heal");
        let v = body_json(resp).await;
        assert_eq!(v["booking_mode"], "approval", "default should be re-seeded each time");
    }
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
