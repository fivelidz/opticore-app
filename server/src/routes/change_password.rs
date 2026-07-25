use axum::{
    extract::State,
    Json,
};
use serde::Deserialize;
use sqlx::Row;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// POST /api/auth/change-password
/// Requires the current password; sets a new one. Enforces a minimum length.
pub async fn change_password(
    State(state): State<AppState>,
    axum::Extension(AuthUser { id, .. }): axum::Extension<AuthUser>,
    Json(body): Json<ChangePasswordRequest>,
) -> ApiResult<Json<shared::MessageResponse>> {
    if body.new_password.len() < 4 {
        return Err(ApiError::BadRequest("New password must be at least 4 characters".into()));
    }
    if body.current_password == body.new_password {
        return Err(ApiError::BadRequest("New password must be different".into()));
    }

    let row = sqlx::query("SELECT password_hash FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let hash: String = row.try_get("password_hash").map_err(ApiError::from)?;

    if !crate::auth::verify_password(&body.current_password, &hash) {
        return Err(ApiError::BadRequest("Current password is incorrect".into()));
    }

    let new_hash = crate::auth::hash_password(&body.new_password)?;
    sqlx::query("UPDATE users SET password_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(&new_hash)
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(shared::MessageResponse { message: "Password changed successfully".into() }))
}
