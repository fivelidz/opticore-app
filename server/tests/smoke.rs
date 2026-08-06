//! Smoke tests: prove the test harness boots, migrations apply, and the most
//! basic endpoints (health, login, auth-middleware enforcement) work.

mod common;

use common::{body_json, TestApp};

#[tokio::test]
async fn health_check_returns_ok() {
    let app = TestApp::spawn().await;
    let resp = app.get("/api/health").send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["status"], "ok");
    assert_eq!(v["clinic"], "OptiCore");
}

#[tokio::test]
async fn login_with_seeded_admin_succeeds() {
    let app = TestApp::spawn().await;
    let token = app.admin_token().await;
    // A JWT is three dot-separated base64 segments.
    assert!(token.split('.').count() == 3, "token should be a JWT: {token}");
}

#[tokio::test]
async fn login_with_wrong_password_is_unauthorized() {
    let app = TestApp::spawn().await;
    let body = serde_json::json!({ "username": "admin", "password": "wrong" });
    let resp = app.post("/api/auth/login").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn login_with_unknown_user_is_unauthorized() {
    let app = TestApp::spawn().await;
    let body = serde_json::json!({ "username": "nobody", "password": "x" });
    let resp = app.post("/api/auth/login").json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn protected_route_without_token_is_unauthorized() {
    let app = TestApp::spawn().await;
    // /api/auth/me is behind the auth middleware.
    let resp = app.get("/api/auth/me").send().await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn protected_route_with_valid_token_succeeds() {
    let app = TestApp::spawn().await;
    let token = app.admin_token().await;
    let resp = app.get("/api/auth/me").auth(&token).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let v = body_json(resp).await;
    assert_eq!(v["username"], "admin");
    assert_eq!(v["role"], "admin");
}

#[tokio::test]
async fn admin_route_with_admin_token_succeeds() {
    // /api/users is behind require_admin.
    let app = TestApp::spawn().await;
    let token = app.admin_token().await;
    let resp = app.get("/api/users").auth(&token).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn malformed_json_is_bad_request() {
    let app = TestApp::spawn().await;
    // Send invalid JSON to the login endpoint.
    let resp = app
        .post("/api/auth/login")
        .body(b"{ not json".to_vec())
        .header("content-type", "application/json")
        .send()
        .await
        .unwrap();
    // axum's Json extractor rejects malformed bodies with 400.
    assert_eq!(resp.status(), 400);
}

/// Regression test for the `TestApp::spawn()` env-var race.
///
/// Previously, `spawn()` mutated the process-global `DEV_ADMIN_PASSWORD` env
/// var around `ensure_admin`. Under parallel `cargo test`, two spawns could
/// interleave such that one task restored/cleared the var while another was
/// still inside `ensure_admin` — the latter would then generate a *random*
/// admin password, and that task's `admin_token()` login would fail with 401.
///
/// The fix passes the password through a function parameter instead of the
/// env. This test spawns many apps concurrently and asserts every one can log
/// in with the known password. If the race returned, at least one login would
/// 401 and the test would fail (deterministically, not flakily, because we
/// spawn enough concurrent tasks to reliably trigger the old interleaving).
#[tokio::test]
async fn concurrent_spawns_all_login_successfully() {
    const N: usize = 16;
    let mut handles = Vec::with_capacity(N);
    for _ in 0..N {
        handles.push(tokio::spawn(async move {
            let app = TestApp::spawn().await;
            // This panics (via assert in admin_token) if login != 200, which
            // propagates through JoinHandle::expect below.
            let _token = app.admin_token().await;
        }));
    }
    for h in handles {
        h.await.expect("spawned login task panicked");
    }
}
