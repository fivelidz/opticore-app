//! Security-audit tests for the authentication core — password policy.
//!
//! These tests verify the password policy is enforced at every password-
//! setting surface: user creation, user update, and change-password. The old
//! code only required 4 characters; the strengthened policy requires >= 8.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

const SEVEN_CHARS: &str = "1234567"; // accepted by old policy, must be rejected
const EIGHT_CHARS: &str = "12345678"; // minimum acceptable length

// ---------- change-password surface ----------

#[tokio::test]
async fn change_password_rejects_7_char_new_password() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "current_password": app.admin_password,
        "new_password": SEVEN_CHARS,
    });
    let resp = app
        .post("/api/auth/change-password")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "7-char password must be rejected by the strengthened policy"
    );
}

#[tokio::test]
async fn change_password_accepts_8_char_new_password() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "current_password": app.admin_password,
        "new_password": EIGHT_CHARS,
    });
    let resp = app
        .post("/api/auth/change-password")
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "8-char password must be accepted (meets minimum)"
    );
    let login = serde_json::json!({ "username": "admin", "password": EIGHT_CHARS });
    let resp = app.post("/api/auth/login").json(&login).send().await.unwrap();
    assert_eq!(resp.status(), 200, "login with the new 8-char password should work");
}

// ---------- user creation surface ----------

#[tokio::test]
async fn create_user_rejects_7_char_password() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "username": "pw7", "email": "pw7@clinic.local",
        "password": SEVEN_CHARS, "role": "nurse",
        "first_name": "Seven", "last_name": "Chars",
    });
    let resp = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(
        resp.status(),
        400,
        "7-char password must be rejected on user creation"
    );
}

#[tokio::test]
async fn create_user_accepts_8_char_password() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "username": "pw8", "email": "pw8@clinic.local",
        "password": EIGHT_CHARS, "role": "nurse",
        "first_name": "Eight", "last_name": "Chars",
    });
    let resp = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201, "8-char password must be accepted on user creation");
}

// ---------- user update surface ----------

#[tokio::test]
async fn update_user_rejects_7_char_password() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "username": "upd7", "email": "upd7@clinic.local",
        "password": "initial-good-pw", "role": "nurse",
        "first_name": "Upd", "last_name": "Seven",
    });
    let resp = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let uid = body_json(resp).await["id"].as_i64().unwrap();
    let body = serde_json::json!({ "password": SEVEN_CHARS });
    let resp = app
        .put(&format!("/api/users/{}", uid))
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "7-char password must be rejected on user update"
    );
}

#[tokio::test]
async fn update_user_accepts_8_char_password() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "username": "upd8", "email": "upd8@clinic.local",
        "password": "initial-good-pw", "role": "nurse",
        "first_name": "Upd", "last_name": "Eight",
    });
    let resp = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let uid = body_json(resp).await["id"].as_i64().unwrap();
    let body = serde_json::json!({ "password": EIGHT_CHARS });
    let resp = app
        .put(&format!("/api/users/{}", uid))
        .auth(&t)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "8-char password must be accepted on user update");
}
