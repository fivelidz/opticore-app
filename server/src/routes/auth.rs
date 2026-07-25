use axum::{extract::State, Json};
use shared::{LoginRequest, LoginResponse, User};
use sqlx::Row;

use crate::auth::{hash_password, issue_token, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::AppState;

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Json<LoginResponse>> {
    let row = sqlx::query(
        "SELECT id, username, email, password_hash, role, first_name, last_name, is_active
         FROM users WHERE username = ?",
    )
    .bind(&req.username)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::Unauthorized)?;

    let is_active: bool = row.try_get("is_active").map_err(ApiError::from)?;
    if !is_active {
        return Err(ApiError::Forbidden);
    }
    let hash: String = row.try_get("password_hash").map_err(ApiError::from)?;
    if !crate::auth::verify_password(&req.password, &hash) {
        return Err(ApiError::Unauthorized);
    }

    let user = User {
        id: row.try_get("id").map_err(ApiError::from)?,
        username: row.try_get("username").map_err(ApiError::from)?,
        email: row.try_get("email").map_err(ApiError::from)?,
        role: row.try_get("role").map_err(ApiError::from)?,
        first_name: row.try_get("first_name").map_err(ApiError::from)?,
        last_name: row.try_get("last_name").map_err(ApiError::from)?,
    };

    let token = issue_token(&state.jwt, user.id, &user.username, &user.role);
    let refresh = issue_token(&state.jwt, user.id, &user.username, &user.role); // simplified

    Ok(Json(LoginResponse { token, refresh_token: refresh, user }))
}

pub async fn me(
    State(state): State<AppState>,
    axum::Extension(AuthUser { id, .. }): axum::Extension<AuthUser>,
) -> ApiResult<Json<User>> {
    let row = sqlx::query(
        "SELECT id, username, email, role, first_name, last_name FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)?;

    Ok(Json(User {
        id: row.try_get("id").map_err(ApiError::from)?,
        username: row.try_get("username").map_err(ApiError::from)?,
        email: row.try_get("email").map_err(ApiError::from)?,
        role: row.try_get("role").map_err(ApiError::from)?,
        first_name: row.try_get("first_name").map_err(ApiError::from)?,
        last_name: row.try_get("last_name").map_err(ApiError::from)?,
    }))
}

// keep hash_password referenced so it isn't dead-code warned in some builds
#[allow(dead_code)]
fn _keep() {
    let _ = hash_password("x");
}
