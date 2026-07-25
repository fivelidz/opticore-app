//! Staff user management: list, create, update, toggle active, delete.
//! Admin-only (enforced by route placement behind require_role in main.rs).

use axum::{
    extract::{Path, State},
    Json,
};
use shared::{CreateUser, StaffUser, UpdateUser};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

fn row_to_user(r: &sqlx::sqlite::SqliteRow) -> StaffUser {
    StaffUser {
        id: r.get("id"),
        username: r.get("username"),
        email: r.get("email"),
        role: r.get("role"),
        first_name: r.get("first_name"),
        last_name: r.get("last_name"),
        is_active: r.get("is_active"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<StaffUser>>> {
    let rows = sqlx::query("SELECT id, username, email, role, first_name, last_name, is_active, created_at, updated_at FROM users ORDER BY first_name, last_name")
        .fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(row_to_user).collect()))
}

pub async fn create(State(state): State<AppState>, Json(b): Json<CreateUser>) -> ApiResult<axum::response::Response> {
    // validate role
    let role = b.role.as_str();
    if !["admin", "doctor", "nurse", "receptionist", "readonly"].contains(&role) {
        return Err(ApiError::BadRequest("Invalid role".into()));
    }
    if b.password.len() < 4 {
        return Err(ApiError::BadRequest("Password must be at least 4 characters".into()));
    }
    let hash = crate::auth::hash_password(&b.password)?;
    let r = sqlx::query(
        "INSERT INTO users (username, email, password_hash, role, first_name, last_name, is_active)
         VALUES (?, ?, ?, ?, ?, ?, 1)")
        .bind(&b.username).bind(&b.email).bind(&hash).bind(role).bind(&b.first_name).bind(&b.last_name)
        .execute(&state.db).await
        .map_err(|e| match e {
            sqlx::Error::Database(ref d) if d.is_unique_violation() => ApiError::Conflict("Username or email already exists".into()),
            _ => ApiError::from(e),
        })?;
    let id = r.last_insert_rowid();
    let row = sqlx::query("SELECT id, username, email, role, first_name, last_name, is_active, created_at, updated_at FROM users WHERE id = ?")
        .bind(id).fetch_one(&state.db).await?;
    use axum::response::IntoResponse;
    Ok((axum::http::StatusCode::CREATED, Json(row_to_user(&row))).into_response())
}

pub async fn update(State(state): State<AppState>, Path(id): Path<i64>, Json(b): Json<UpdateUser>) -> ApiResult<Json<StaffUser>> {
    // Build a dynamic update — only set provided fields.
    if let Some(email) = b.email {
        sqlx::query("UPDATE users SET email = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(&email).bind(id).execute(&state.db).await?;
    }
    if let Some(role) = b.role {
        if !["admin", "doctor", "nurse", "receptionist", "readonly"].contains(&role.as_str()) {
            return Err(ApiError::BadRequest("Invalid role".into()));
        }
        sqlx::query("UPDATE users SET role = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(&role).bind(id).execute(&state.db).await?;
    }
    if let Some(first) = b.first_name {
        sqlx::query("UPDATE users SET first_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(&first).bind(id).execute(&state.db).await?;
    }
    if let Some(last) = b.last_name {
        sqlx::query("UPDATE users SET last_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(&last).bind(id).execute(&state.db).await?;
    }
    if let Some(active) = b.is_active {
        sqlx::query("UPDATE users SET is_active = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(active).bind(id).execute(&state.db).await?;
    }
    if let Some(ref pw) = b.password {
        if pw.len() < 4 { return Err(ApiError::BadRequest("Password too short".into())); }
        let hash = crate::auth::hash_password(pw)?;
        sqlx::query("UPDATE users SET password_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(&hash).bind(id).execute(&state.db).await?;
    }
    let row = sqlx::query("SELECT id, username, email, role, first_name, last_name, is_active, created_at, updated_at FROM users WHERE id = ?")
        .bind(id).fetch_optional(&state.db).await?.ok_or(ApiError::NotFound)?;
    Ok(Json(row_to_user(&row)))
}

pub async fn toggle_active(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<serde_json::Value>> {
    let row = sqlx::query("SELECT is_active FROM users WHERE id = ?").bind(id).fetch_optional(&state.db).await?.ok_or(ApiError::NotFound)?;
    let current: bool = row.get("is_active");
    sqlx::query("UPDATE users SET is_active = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(!current).bind(id).execute(&state.db).await?;
    Ok(Json(serde_json::json!({ "id": id, "is_active": !current })))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<shared::MessageResponse>> {
    // Prevent deleting the last admin.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1").fetch_one(&state.db).await?;
    if count.0 <= 1 {
        let user: Option<(String,)> = sqlx::query_as("SELECT role FROM users WHERE id = ?").bind(id).fetch_optional(&state.db).await?;
        if let Some((role,)) = user { if role == "admin" { return Err(ApiError::BadRequest("Cannot delete the last active admin".into())); } }
    }
    let r = sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(&state.db).await?;
    if r.rows_affected() == 0 { return Err(ApiError::NotFound); }
    Ok(Json(shared::MessageResponse { message: "User deleted".into() }))
}
