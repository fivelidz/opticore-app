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

/// Minimum password length. OWASP / NIST SP 800-63B recommend a minimum of 8
/// characters. The old code only required 4, which is trivially brute-forced
/// and far below any modern baseline — unacceptable for a medical PMS handling
/// patient health data.
pub const MIN_PASSWORD_LEN: usize = 8;

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
