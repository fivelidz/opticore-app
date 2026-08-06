//! Users (admin-only staff management) + change-password tests.

mod common;

use common::{body_json, TestApp};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

// ---------- Users CRUD (admin-gated) ----------

#[tokio::test]
async fn list_users_returns_admin() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.get("/api/users").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    let arr = v.as_array().unwrap();
    assert!(!arr.is_empty(), "should have at least the seeded admin");
    let admin = arr.iter().find(|u| u["username"] == "admin").expect("admin user exists");
    assert_eq!(admin["role"], "admin");
    assert_eq!(admin["is_active"], true);
}

#[tokio::test]
async fn create_user_returns_created() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "username": "drsmith", "email": "smith@clinic.local",
        "password": "secure123", "role": "doctor",
        "first_name": "Jane", "last_name": "Smith",
    });
    let resp = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let v = body_json(resp).await;
    assert_eq!(v["username"], "drsmith");
    assert_eq!(v["role"], "doctor");
    assert!(v["id"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn create_user_invalid_role_is_bad_request() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "username": "badrole", "email": "bad@clinic.local",
        "password": "secure123", "role": "superuser",
        "first_name": "Bad", "last_name": "Role",
    });
    let resp = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn create_user_short_password_is_bad_request() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "username": "shortpw", "email": "short@clinic.local",
        "password": "ab", "role": "nurse",
        "first_name": "Short", "last_name": "Pw",
    });
    let resp = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn create_duplicate_user_is_conflict() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "username": "dupuser", "email": "dup@clinic.local",
        "password": "secure123", "role": "nurse",
        "first_name": "Dup", "last_name": "User",
    });
    let r1 = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(r1.status(), 201);
    // Same username again -> conflict.
    let r2 = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(r2.status(), 409);
}

#[tokio::test]
async fn update_user_changes_fields() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // Create.
    let body = serde_json::json!({
        "username": "upduser", "email": "upd@clinic.local",
        "password": "secure123", "role": "receptionist",
        "first_name": "Before", "last_name": "Upd",
    });
    let r = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let upd = serde_json::json!({ "first_name": "After", "role": "doctor" });
    let resp = app.put(&format!("/api/users/{}", id)).auth(&t).json(&upd).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["first_name"], "After");
    assert_eq!(v["role"], "doctor");
}

#[tokio::test]
async fn toggle_user_active_flips_state() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "username": "toggle", "email": "tog@clinic.local",
        "password": "secure123", "role": "nurse",
        "first_name": "Tog", "last_name": "Gle",
    });
    let r = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let resp = app.post(&format!("/api/users/{}/toggle", id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["is_active"], false);

    // Toggle back.
    let resp = app.post(&format!("/api/users/{}/toggle", id)).auth(&t).send().await.unwrap();
    let v = body_json(resp).await;
    assert_eq!(v["is_active"], true);
}

#[tokio::test]
async fn delete_user_removes_it() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "username": "delme", "email": "del@clinic.local",
        "password": "secure123", "role": "nurse",
        "first_name": "Del", "last_name": "Me",
    });
    let r = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    let id = body_json(r).await["id"].as_i64().unwrap();

    let resp = app.delete(&format!("/api/users/{}", id)).auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn delete_nonexistent_user_returns_404() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let resp = app.delete("/api/users/99999").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn cannot_delete_last_admin() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    // The seeded admin (id=1) is the only admin. Deleting it must be refused.
    let resp = app.delete("/api/users/1").auth(&t).send().await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn users_endpoints_require_admin() {
    let app = TestApp::spawn().await;
    // No token -> 401 (auth middleware runs before admin check).
    assert_eq!(app.get("/api/users").send().await.unwrap().status(), 401);
}

#[tokio::test]
async fn users_endpoints_reject_non_admin() {
    let app = TestApp::spawn().await;
    let admin_token = token(&app).await;

    // Create a non-admin (receptionist) user and log in as them.
    let body = serde_json::json!({
        "username": "recep", "email": "recep@clinic.local",
        "password": "secure123", "role": "receptionist",
        "first_name": "Rec", "last_name": "Eptionist",
    });
    let r = app.post("/api/users").auth(&admin_token).json(&body).send().await.unwrap();
    assert_eq!(r.status(), 201);

    let login = serde_json::json!({ "username": "recep", "password": "secure123" });
    let resp = app.post("/api/auth/login").json(&login).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let recep_token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Non-admin hitting /api/users -> 403 Forbidden.
    let resp = app.get("/api/users").auth(&recep_token).send().await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ---------- Change password ----------

#[tokio::test]
async fn change_password_succeeds_with_correct_current() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "current_password": "test-admin-pw",
        "new_password": "new-admin-pw-99",
    });
    let resp = app.post("/api/auth/change-password").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Verify the new password works for login.
    let login = serde_json::json!({ "username": "admin", "password": "new-admin-pw-99" });
    let resp = app.post("/api/auth/login").json(&login).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn change_password_rejects_wrong_current() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "current_password": "wrong",
        "new_password": "new-admin-pw-99",
    });
    let resp = app.post("/api/auth/change-password").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn change_password_rejects_too_short_new() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "current_password": "test-admin-pw",
        "new_password": "ab",
    });
    let resp = app.post("/api/auth/change-password").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn change_password_rejects_same_password() {
    let app = TestApp::spawn().await;
    let t = token(&app).await;
    let body = serde_json::json!({
        "current_password": "test-admin-pw",
        "new_password": "test-admin-pw",
    });
    let resp = app.post("/api/auth/change-password").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn change_password_requires_auth() {
    let app = TestApp::spawn().await;
    let body = serde_json::json!({
        "current_password": "x", "new_password": "yyyyyy",
    });
    let resp = app.post("/api/auth/change-password").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 401);
}
