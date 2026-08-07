//! Auth / authorization edge-case tests.
//!
//! These tests verify that the JWT verification layer (the `auth_middleware`
//! and `require_admin` middleware, plus the `AuthUser` extractor) handles
//! every malformed / hostile / expired token variant cleanly — returning HTTP
//! 401 (never 500, never a panic, never 200).
//!
//! ## Cases covered
//!
//! ### Token-shape failures (all must return 401):
//! 1. **Missing** `Authorization` header entirely.
//! 2. **`Bearer ` with no token** (empty token after the prefix).
//! 3. **Malformed token** (random garbage string, not a JWT at all).
//! 4. **Expired token** (signed with the correct key but `exp` in the past).
//! 5. **Wrong-signing-key token** (well-formed JWT, but signed with a
//!    different secret than the server's `JWT_SECRET`).
//!
//! ### Authorization failures:
//! 6. **Valid token, non-admin role** hitting an admin-only route -> 403
//!    (the token verifies, but `require_admin` rejects the role).
//! 7. **Valid token, disabled user** (`is_active = 0`) — login itself is
//!    blocked (403), so no valid token can be obtained for a disabled user.
//!    We verify the login endpoint rejects disabled users.
//!
//! ## Why this matters
//!
//! A 500 on a bad token indicates the auth layer is leaking an internal error
//! (e.g. a `jsonwebtoken` error being mapped to `ApiError::Internal` instead
//! of `ApiError::Unauthorized`). A 200 on a bad token is a security hole. A
//! panic crashes the worker. All three are bugs; the only correct response is
//! 401 (or 403 for a valid-but-unauthorized token).

mod common;

use chrono::{Duration, Utc};
use common::{body_json, TestApp};
use jsonwebtoken::{encode, EncodingKey, Header};
use server::auth::Claims;

/// The JWT secret used by `TestApp::spawn` (set in common/mod.rs).
const TEST_SECRET: &str = "test-secret-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Encode arbitrary claims with a given secret. Used to craft hostile tokens
/// (expired, wrong-key) that the server must reject.
fn craft_token(secret: &str, claims: &Claims) -> String {
    encode(&Header::default(), claims, &EncodingKey::from_secret(secret.as_bytes()))
        .expect("encode token")
}

/// A valid admin token (correct secret, future expiry, admin role).
fn valid_admin_token() -> String {
    let claims = Claims {
        sub: 1,
        username: "admin".into(),
        role: "admin".into(),
        exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
    };
    craft_token(TEST_SECRET, &claims)
}

// ---------- Token-shape failures (all -> 401) ----------

#[tokio::test]
async fn missing_authorization_header_returns_401() {
    let app = TestApp::spawn().await;
    // No Authorization header at all.
    let resp = app.get("/api/patients").send().await.unwrap();
    assert_eq!(
        resp.status(),
        401,
        "missing Authorization header must return 401, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn bearer_with_empty_token_returns_401() {
    let app = TestApp::spawn().await;
    // "Bearer " with nothing after it. The middleware strips the prefix and
    // trims, leaving an empty string — verify_token must reject it, not panic.
    let resp = app
        .get("/api/patients")
        .header("authorization", "Bearer ")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        401,
        "empty bearer token must return 401, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn malformed_garbage_token_returns_401() {
    let app = TestApp::spawn().await;
    // A random string that is not a JWT at all. jsonwebtoken::decode returns
    // an error; verify_token maps that to None -> Unauthorized.
    let resp = app
        .get("/api/patients")
        .header("authorization", "Bearer this-is-not-a-jwt")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        401,
        "garbage token must return 401, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn expired_token_returns_401() {
    let app = TestApp::spawn().await;
    // A token signed with the CORRECT key but with exp far in the past.
    // The server's Validation has 60s leeway, so we go well beyond that.
    let claims = Claims {
        sub: 1,
        username: "admin".into(),
        role: "admin".into(),
        exp: (Utc::now() - Duration::hours(24)).timestamp() as usize,
    };
    let token = craft_token(TEST_SECRET, &claims);

    let resp = app
        .get("/api/patients")
        .header("authorization", &format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        401,
        "expired token must return 401, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn wrong_signing_key_token_returns_401() {
    let app = TestApp::spawn().await;
    // A well-formed JWT signed with a DIFFERENT secret. The signature won't
    // verify against the server's decoding key -> Unauthorized.
    let claims = Claims {
        sub: 1,
        username: "admin".into(),
        role: "admin".into(),
        exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
    };
    let token = craft_token("a-completely-different-secret-xxxxxxxxxxxx", &claims);

    let resp = app
        .get("/api/patients")
        .header("authorization", &format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        401,
        "wrong-key token must return 401, got {}",
        resp.status()
    );
}

// ---------- Authorization failures ----------

#[tokio::test]
async fn valid_non_admin_token_on_admin_route_returns_403() {
    let app = TestApp::spawn().await;
    let admin_token = app.admin_token().await;

    // Create a non-admin (doctor) user via the admin API.
    let body = serde_json::json!({
        "username": "dr_auth_edge", "email": "drae@clinic.local",
        "password": "secure123", "role": "doctor",
        "first_name": "Dr", "last_name": "Edge",
    });
    let r = app.post("/api/users").auth(&admin_token).json(&body).send().await.unwrap();
    assert_eq!(r.status(), 201, "user creation should succeed");

    // Log in as the doctor.
    let login = serde_json::json!({ "username": "dr_auth_edge", "password": "secure123" });
    let resp = app.post("/api/auth/login").json(&login).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let doc_token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Doctor hitting an admin-only route (GET /api/users) -> 403, not 401
    // (the token is valid; the role is insufficient) and not 500.
    let resp = app.get("/api/users").auth(&doc_token).send().await.unwrap();
    assert_eq!(
        resp.status(),
        403,
        "non-admin on admin route must return 403, got {}",
        resp.status()
    );

    // The same doctor token should work fine on a regular protected route
    // (proves the token itself is valid — only the admin gate rejected it).
    let resp = app.get("/api/patients").auth(&doc_token).send().await.unwrap();
    assert_eq!(
        resp.status(),
        200,
        "non-admin on regular protected route should succeed, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn admin_route_with_no_token_returns_401_not_403() {
    let app = TestApp::spawn().await;
    // No token on an admin route: the auth check runs before the role check,
    // so this must be 401 (unauthenticated), not 403 (forbidden).
    let resp = app.get("/api/users").send().await.unwrap();
    assert_eq!(
        resp.status(),
        401,
        "no token on admin route must return 401 (unauthenticated), got {}",
        resp.status()
    );
}

#[tokio::test]
async fn disabled_user_login_returns_403() {
    let app = TestApp::spawn().await;
    let admin_token = app.admin_token().await;

    // Create a nurse user.
    let body = serde_json::json!({
        "username": "nurse_dis", "email": "ndis@clinic.local",
        "password": "secure123", "role": "nurse",
        "first_name": "Nurse", "last_name": "Disabled",
    });
    let r = app.post("/api/users").auth(&admin_token).json(&body).send().await.unwrap();
    assert_eq!(r.status(), 201);
    let uid = body_json(r).await["id"].as_i64().unwrap();

    // Verify login works before disabling.
    let login = serde_json::json!({ "username": "nurse_dis", "password": "secure123" });
    let resp = app.post("/api/auth/login").json(&login).send().await.unwrap();
    assert_eq!(resp.status(), 200, "login should work before disabling");

    // Disable the user (toggle is_active from 1 -> 0).
    let resp = app.post(&format!("/api/users/{}/toggle", uid)).auth(&admin_token).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["is_active"], false);

    // Login must now be rejected. The login handler checks is_active and
    // returns Forbidden (403) for disabled accounts — not 200, not 500.
    let resp = app.post("/api/auth/login").json(&login).send().await.unwrap();
    assert_eq!(
        resp.status(),
        403,
        "disabled user login must return 403, got {}",
        resp.status()
    );
}

// ---------- Sanity: a valid admin token still works ----------

#[tokio::test]
async fn valid_admin_token_succeeds() {
    let app = TestApp::spawn().await;
    // A correctly-crafted admin token (correct secret, future exp, admin
    // role) must be accepted — guards against the auth layer being
    // over-aggressive and rejecting good tokens.
    let token = valid_admin_token();
    let resp = app
        .get("/api/patients")
        .header("authorization", &format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "valid admin token must succeed, got {}",
        resp.status()
    );
}
