//! Integration-test harness for the OptiCore server.
//!
//! Each test gets a **fresh, isolated** SQLite database backed by a `tempfile`
//! temp dir, so tests never share state and can run in parallel safely.
//!
//! Usage:
//!   ```
//!   use common::TestApp;
//!   let app = TestApp::spawn().await;
//!   let token = app.admin_token().await;          // Bearer token for the seeded admin
//!   let resp = app.get("/api/patients").auth(&token).send().await.unwrap();
//!   ```

use std::sync::Arc;

use server::auth::JwtCfg;
use server::{build_app, db, AppState};

/// A test fixture: owns a temp DB file + the built axum app, exposed for both
/// `oneshot` (in-process) and bound-socket (reqwest) testing.
pub struct TestApp {
    pub state: AppState,
    pub app: axum::Router,
    /// The admin password (known, deterministic for tests).
    pub admin_password: &'static str,
}

impl TestApp {
    /// Create a fresh test app with its own isolated temp SQLite database.
    ///
    /// Migrations are applied and a single admin user (`admin` / `test-admin-pw`)
    /// is provisioned. No demo seed data is wiped (CLEAN_START is not set), so
    /// the seed patients/appointments from `0002_seed.sql` are present — tests
    /// that need a known-empty table should insert/assert their own rows.
    pub async fn spawn() -> Self {
        // temp dir keeps the .db file alive for the test's lifetime; dropped
        // (and cleaned up) when TestApp is dropped.
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("test.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        let pool = db::init_pool(&db_url).await.expect("init pool");
        db::run_migrations(&pool).await.expect("run migrations");

        // Provision a deterministic admin so tests can log in without reading
        // a random password from the log.
        //
        // We pass the password directly to `ensure_admin_with_password` rather
        // than going through the `DEV_ADMIN_PASSWORD` env var. The env var is
        // process-global: under parallel `cargo test`, two concurrent spawns
        // could race on set/restore and one would end up provisioning the admin
        // with a random password (its `admin_token()` login would then fail).
        // Passing the value through the function signature eliminates that race
        // entirely — no synchronization needed.
        db::ensure_admin_with_password(&pool, "test-admin-pw")
            .await
            .expect("ensure admin");

        // Use a fixed JWT secret so tokens are deterministic and verifiable.
        // JWT_SECRET is read once per process at JwtCfg construction; tests all
        // set the SAME value, so even though the mutation is process-global the
        // worst case is a redundant write of an identical value (no race).
        std::env::set_var("JWT_SECRET", "test-secret-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let jwt = Arc::new(JwtCfg::from_env());

        let state = AppState { db: pool, jwt };
        let app = build_app(state.clone());

        // Leak the temp dir so it lives as long as the TestApp (the DB file
        // must remain on disk for the duration of the test). The OS reclaims
        // the space when the process exits.
        std::mem::forget(dir);

        Self { state, app, admin_password: "test-admin-pw" }
    }

    /// Log in as the seeded admin and return a fresh JWT bearer token.
    pub async fn admin_token(&self) -> String {
        let body = serde_json::json!({
            "username": "admin",
            "password": self.admin_password,
        });
        let resp = self
            .post("/api/auth/login")
            .body(serde_json::to_vec(&body).unwrap())
            .header("content-type", "application/json")
            .send()
            .await
            .expect("login request");
        assert_eq!(resp.status(), 200, "admin login should succeed");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["token"].as_str().unwrap().to_string()
    }
}

// ---------- request-builder helpers (tower::ServiceExt::oneshot) ----------

use axum::body::Body;
use axum::http::{Method, Request};
use tower::ServiceExt;

/// A fluent wrapper that builds a `Request<Body>` and runs it against the app
/// via `oneshot`. Mirrors reqwest's ergonomics but stays fully in-process.
pub struct RequestBuilder<'a> {
    app: &'a axum::Router,
    method: Method,
    uri: String,
    body: Vec<u8>,
    headers: Vec<(String, String)>,
}

impl TestApp {
    pub fn get(&self, uri: &str) -> RequestBuilder<'_> {
        RequestBuilder::new(&self.app, Method::GET, uri)
    }
    pub fn post(&self, uri: &str) -> RequestBuilder<'_> {
        RequestBuilder::new(&self.app, Method::POST, uri)
    }
    pub fn put(&self, uri: &str) -> RequestBuilder<'_> {
        RequestBuilder::new(&self.app, Method::PUT, uri)
    }
    pub fn delete(&self, uri: &str) -> RequestBuilder<'_> {
        RequestBuilder::new(&self.app, Method::DELETE, uri)
    }
}

impl<'a> RequestBuilder<'a> {
    fn new(app: &'a axum::Router, method: Method, uri: &str) -> Self {
        Self { app, method, uri: uri.to_string(), body: Vec::new(), headers: Vec::new() }
    }

    /// Attach a JSON body.
    pub fn json<T: serde::Serialize>(mut self, v: &T) -> Self {
        self.body = serde_json::to_vec(v).expect("serialize body");
        self.headers.push(("content-type".into(), "application/json".into()));
        self
    }

    /// Attach a raw body.
    pub fn body(mut self, b: Vec<u8>) -> Self {
        self.body = b;
        self
    }

    /// Add an arbitrary header.
    pub fn header(mut self, k: &str, v: &str) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }

    /// Add a `Authorization: Bearer <token>` header.
    pub fn auth(self, token: &str) -> Self {
        self.header("authorization", &format!("Bearer {}", token))
    }

    /// Execute the request against the app (in-process, no socket).
    pub async fn send(self) -> Result<axum::response::Response, Box<dyn std::error::Error + Send + Sync>> {
        let mut b = Request::builder().method(self.method).uri(&self.uri);
        for (k, v) in &self.headers {
            b = b.header(k, v);
        }
        let req = b.body(Body::from(self.body))?;
        Ok(self.app.clone().oneshot(req).await?)
    }
}

/// Read the JSON body of a response as a `serde_json::Value`.
pub async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        panic!("failed to parse body as JSON: {}", String::from_utf8_lossy(&bytes))
    })
}
