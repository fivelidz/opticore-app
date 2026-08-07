//! Security-audit tests for the authentication core.
//!
//! These tests were added during a dedicated auth security audit. They cover
//! three vulnerability classes:
//!
//! 1. **Password policy** — the old code only required 4 characters with no
//!    complexity. A medical PMS handling patient health data must enforce a
//!    stronger baseline (>= 8 chars). Tests verify the policy is enforced at
//!    every password-setting surface: user creation, user update, and
//!    change-password.
//! 2. **Login timing oracle** — the old login path returned 401 immediately
//!    for a nonexistent user but ran argon2 verification (~tens of ms) for an
//!    existing user with a wrong password. That timing difference lets an
//!    attacker enumerate valid usernames. The fix runs a dummy argon2 verify
//!    on the not-found path so both branches take comparable time.
//! 3. **Documented decisions** — risks that are accepted trade-offs for a
//!    small single-clinic PMS (no rate limiting; old JWTs valid until expiry
//!    after password change) are recorded as `*_documented_decision` tests so
//!    future auditors see the reasoning.

mod common;

use common::{body_json, TestApp};
use std::time::{Duration, Instant};

async fn token(app: &TestApp) -> String {
    app.admin_token().await
}

// ===========================================================================
// FIX 1: Password policy — minimum length must be >= 8 (was 4)
// ===========================================================================
//
// The old code (`users.rs` create/update and `change_password.rs`) only
// rejected passwords shorter than 4 characters. "1234", "aaaa", "pw" were
// all accepted for a medical PMS. OWASP NIST-800-63B recommends a minimum of
// 8 characters. We enforce 8 at every password-setting surface.
//
// These tests prove that a 7-character password (which the old 4-char policy
// allowed) is now rejected, and an 8-character password is accepted.

const SEVEN_CHARS: &str = "1234567"; // accepted by old policy, must be rejected
const EIGHT_CHARS: &str = "12345678"; // minimum acceptable length

// ---------- change-password surface ----------

#[tokio::test]
async fn change_password_rejects_7_char_new_password() {
    // OLD BEHAVIOR: 7 chars > 4, so it was accepted. NEW: must be rejected.
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

    // Verify the new password actually works for login.
    let login = serde_json::json!({ "username": "admin", "password": EIGHT_CHARS });
    let resp = app.post("/api/auth/login").json(&login).send().await.unwrap();
    assert_eq!(resp.status(), 200, "login with the new 8-char password should work");
}

// ---------- user creation surface ----------

#[tokio::test]
async fn create_user_rejects_7_char_password() {
    // OLD BEHAVIOR: 7 chars > 4, accepted. NEW: must be rejected.
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
    // OLD BEHAVIOR: 7 chars > 4, accepted. NEW: must be rejected.
    let app = TestApp::spawn().await;
    let t = token(&app).await;

    // Create a user first.
    let body = serde_json::json!({
        "username": "upd7", "email": "upd7@clinic.local",
        "password": "initial-good-pw", "role": "nurse",
        "first_name": "Upd", "last_name": "Seven",
    });
    let resp = app.post("/api/users").auth(&t).json(&body).send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let uid = body_json(resp).await["id"].as_i64().unwrap();

    // Try to set a 7-char password via update.
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

// ===========================================================================
// FIX 2: Login timing oracle — nonexistent user must take comparable time
// ===========================================================================
//
// The old login handler returned 401 immediately when the username was not
// found (`.ok_or(ApiError::Unauthorized)?` on the fetch_optional result),
// but ran argon2 verification (~tens of ms) for an existing user with a
// wrong password. That timing delta lets an attacker enumerate valid
// usernames: a slow 401 = user exists, a fast 401 = user doesn't exist.
//
// The fix runs a dummy argon2 verification on the not-found path so both
// branches perform the same expensive work. We can't assert exact timing
// (argon2 is non-deterministic and CI machines vary), but we CAN assert that
// the not-found path is no longer near-instant: it must take a meaningful
// fraction of the time a real verification takes.

#[tokio::test]
async fn login_nonexistent_user_takes_comparable_time_to_wrong_password() {
    // This is a timing-leak guard. The threshold is deliberately conservative:
    // we only fail if the not-found path is more than 10x faster than the
    // wrong-password path (the old code was ~100x+ faster). This avoids flakes
    // from argon2 variance while still catching a regression to the immediate-
    // return behavior.
    let app = TestApp::spawn().await;

    // Warm up the connection pool so the first request doesn't pay pool-init
    // cost (which would skew the timing).
    let _ = app
        .post("/api/auth/login")
        .json(&serde_json::json!({ "username": "admin", "password": "wrong" }))
        .send()
        .await
        .unwrap();

    // Time a login for a NONEXISTENT user (old code: returns immediately).
    let t_nf_start = Instant::now();
    let resp = app
        .post("/api/auth/login")
        .json(&serde_json::json!({ "username": "definitely-not-a-real-user-xyz", "password": "wrong" }))
        .send()
        .await
        .unwrap();
    let t_not_found = t_nf_start.elapsed();
    assert_eq!(resp.status(), 401, "nonexistent user must return 401");

    // Time a login for an EXISTING user with a wrong password (runs argon2).
    let t_wp_start = Instant::now();
    let resp = app
        .post("/api/auth/login")
        .json(&serde_json::json!({ "username": "admin", "password": "wrong" }))
        .send()
        .await
        .unwrap();
    let t_wrong_pw = t_wp_start.elapsed();
    assert_eq!(resp.status(), 401, "wrong password must return 401");

    // The not-found path must NOT be dramatically faster than the wrong-
    // password path. If it is, the dummy verify was removed/regressed.
    // We allow the not-found path to be up to 5x faster (generous, to avoid
    // flakes), but the old immediate-return behavior made it ~50-100x faster.
    let ratio = if t_not_found.as_micros() == 0 {
        u128::MAX
    } else {
        t_wrong_pw.as_micros() / t_not_found.as_micros()
    };
    assert!(
        ratio < 5,
        "login timing leak: nonexistent-user path ({:?}) was {:.1}x faster than \
         wrong-password path ({:?}). The not-found branch must run a dummy \
         argon2 verify to match timing.",
        t_not_found,
        ratio as f64,
        t_wrong_pw
    );

    // Sanity: the wrong-password path itself must take a meaningful amount of
    // time (argon2 is deliberately slow). If it's under 5ms the dummy verify
    // on the not-found path can't help, so we'd have a different problem.
    assert!(
        t_wrong_pw > Duration::from_millis(5),
        "argon2 verify should take >5ms, got {:?} — test environment issue?",
        t_wrong_pw
    );
}

// ===========================================================================
// Documented decisions — accepted risks for a small single-clinic PMS
// ===========================================================================

#[tokio::test]
async fn no_rate_limiting_on_login_documented_decision() {
    // ACCEPTED RISK: there is no brute-force protection (rate limiting /
    // account lockout) on the login endpoint. For a small single-clinic PMS
    // deployed on-premise behind a VPN/firewall, the threat model does not
    // include online brute-force attacks against the login form. The argon2
    // hash makes each guess expensive (~tens of ms), and the deployment is
    // not internet-exposed.
    //
    // If this app is ever exposed to the internet, rate limiting MUST be added
    // (e.g. tower::limit, or a reverse-proxy-level limiter). This test exists
    // to flag the decision for future auditors.
    //
    // We verify the current behavior: many rapid failed logins are all
    // processed (not blocked), confirming no rate limiter is active.
    let app = TestApp::spawn().await;
    for _ in 0..5 {
        let resp = app
            .post("/api/auth/login")
            .json(&serde_json::json!({ "username": "admin", "password": "wrong" }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 401, "each failed login returns 401 (no lockout)");
    }
    // A correct password still works immediately after failures (no lockout).
    let resp = app
        .post("/api/auth/login")
        .json(&serde_json::json!({ "username": "admin", "password": app.admin_password }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "no lockout — correct password works after failures");
}

#[tokio::test]
async fn old_jwt_valid_after_password_change_documented_decision() {
    // ACCEPTED RISK: JWTs are stateless — after a password change, previously
    // issued tokens remain valid until their 8h expiry. There is no server-
    // side revocation list. This means a stolen token cannot be invalidated by
    // changing the password.
    //
    // For a small single-clinic PMS this is an acceptable trade-off: the token
    // lifetime is short (8h), the deployment is on-premise, and adding a
    // revocation list / token version column would add complexity
    // disproportionate to the threat. If session hijacking becomes a realistic
    // threat, add a `token_version` column to users and include it in the JWT
    // claims; bump it on password change and verify it in the middleware.
    //
    // This test documents the current behavior so a future auditor sees it.
    let app = TestApp::spawn().await;
    let old_token = token(&app).await;

    // Change the password.
    let body = serde_json::json!({
        "current_password": app.admin_password,
        "new_password": "brand-new-pw-99",
    });
    let resp = app
        .post("/api/auth/change-password")
        .auth(&old_token)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // The OLD token is still valid (stateless JWT, no revocation).
    let resp = app.get("/api/auth/me").auth(&old_token).send().await.unwrap();
    assert_eq!(
        resp.status(),
        200,
        "old JWT remains valid after password change (documented: stateless, 8h expiry)"
    );
}
