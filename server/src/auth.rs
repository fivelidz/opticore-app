//! Authentication: argon2 password hashing, JWT issue/verify, axum middleware.

use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::AppState;

pub struct JwtCfg {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl JwtCfg {
    pub fn from_env() -> Self {
        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            // Dev-only fallback. Production MUST set JWT_SECRET (>=32 chars).
            tracing::warn!("⚠️  JWT_SECRET not set — using insecure dev fallback. DO NOT use in production.");
            "dev-insecure-secret-change-me-aaaaaaaaaaaaaaaa".to_string()
        });
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64, // user id
    pub username: String,
    pub role: String,
    pub exp: usize,
}

static VALIDATION: Lazy<Validation> = Lazy::new(|| {
    let mut v = Validation::new(jsonwebtoken::Algorithm::HS256);
    v.leeway = 60;
    v
});

pub fn issue_token(cfg: &JwtCfg, user_id: i64, username: &str, role: &str) -> String {
    let exp = (Utc::now() + Duration::hours(8)).timestamp() as usize;
    let claims = Claims { sub: user_id, username: username.to_string(), role: role.to_string(), exp };
    encode(&Header::default(), &claims, &cfg.encoding).expect("jwt encode")
}

pub fn verify_token(cfg: &JwtCfg, token: &str) -> Option<Claims> {
    decode::<Claims>(token, &cfg.decoding, &VALIDATION).ok().map(|d| d.claims)
}

pub fn hash_password(plain: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ApiError::Internal(format!("hash: {e}")))
}

pub fn verify_password(plain: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .and_then(|parsed| Argon2::default().verify_password(plain.as_bytes(), &parsed))
        .is_ok()
}

// ---------------------------------------------------------------------------
// Password policy
// ---------------------------------------------------------------------------

/// Minimum password length. Kept lenient (4) so the default `admin`/`admin`
/// first-login flow is frictionless for clinic staff on a fresh install.
/// The app runs on the local network behind a VPN/firewall; raise this if
/// the threat model changes (e.g. internet-exposed deployment).
pub const MIN_PASSWORD_LEN: usize = 4;

/// Validate a password against the policy. Returns `Ok(())` if it passes, or
/// a user-facing error message if it fails.
///
/// Centralized here so every password-setting surface (user creation, user
/// update, change-password) enforces the SAME rule — no drift. Each call site
/// returns the message inside an `ApiError::BadRequest`.
pub fn validate_password(pw: &str) -> Result<(), String> {
    if pw.len() < MIN_PASSWORD_LEN {
        return Err(format!(
            "Password must be at least {} characters",
            MIN_PASSWORD_LEN
        ));
    }
    Ok(())
}

/// A pre-computed argon2 hash used as a dummy in timing-attack mitigation.
///
/// When a login attempt targets a username that does not exist, we still run
/// an argon2 verification against this hash so the not-found path takes
/// roughly the same time as the wrong-password path. Without this, an attacker
/// can enumerate valid usernames by timing: a fast 401 = user doesn't exist, a
/// slow 401 = user exists (argon2 ran). See `login` in `routes/auth.rs`.
///
/// This is a real argon2 hash of a random value; the plaintext is irrelevant
/// — we only need the verification to run and fail in comparable time.
static DUMMY_HASH: Lazy<String> = Lazy::new(|| {
    // Hash a random 32-byte password once, at first use. The result is cached
    // so subsequent not-found logins reuse the same hash (no per-request
    // hashing cost — only the verify cost, which is the point).
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(b"dummy-password-for-timing-mitigation-do-not-match", &salt)
        .expect("hash dummy password");
    hash.to_string()
});

/// Run a dummy password verification to consume time comparable to a real
/// `verify_password` call. Used on login paths where the user is not found,
/// to prevent timing-based username enumeration.
pub fn verify_password_dummy(plain: &str) {
    // Result intentionally ignored — we only care about the CPU time spent.
    let _ = verify_password(plain, &DUMMY_HASH);
}

/// Claims attached to the request after successful auth.
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub role: String,
}

/// Axum extractor for the authenticated user.
#[axum::async_trait]
impl axum::extract::FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer ").map(|t| t.trim().to_string()))
            .ok_or(ApiError::Unauthorized)?;
        let claims = verify_token(&state.jwt, &token).ok_or(ApiError::Unauthorized)?;
        Ok(AuthUser { id: claims.sub, username: claims.username, role: claims.role })
    }
}

/// Middleware form (for the protected router).
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(|t| t.trim().to_string()))
        .ok_or(ApiError::Unauthorized)?;
    let claims = verify_token(&state.jwt, &token).ok_or(ApiError::Unauthorized)?;
    // Insert claims so handlers/extractors can read them.
    req.extensions_mut().insert(AuthUser { id: claims.sub, username: claims.username, role: claims.role });
    Ok(next.run(req).await)
}

/// Middleware that requires the authenticated user to be an admin.
pub async fn require_admin(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(|t| t.trim().to_string()))
        .ok_or(ApiError::Unauthorized)?;
    let claims = verify_token(&state.jwt, &token).ok_or(ApiError::Unauthorized)?;
    if claims.role != "admin" {
        return Err(ApiError::Forbidden);
    }
    req.extensions_mut().insert(AuthUser { id: claims.sub, username: claims.username, role: claims.role });
    Ok(next.run(req).await)
}
