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
    // Validate role up-front (no DB access needed; fail fast on bad input).
    if let Some(ref role) = b.role {
        if !["admin", "doctor", "nurse", "receptionist", "readonly"].contains(&role.as_str()) {
            return Err(ApiError::BadRequest("Invalid role".into()));
        }
    }
    if let Some(ref pw) = b.password {
        if pw.len() < 4 {
            return Err(ApiError::BadRequest("Password too short".into()));
        }
    }

    // Wrap the read-guard-write sequence in a single `BEGIN IMMEDIATE`
    // transaction.
    //
    // RACE FIX: previously the handler did
    //   SELECT role, is_active FROM users WHERE id = ?   -- (1) read current
    //   ... if role/is_active change would drop the last active admin:
    //       SELECT COUNT(*) FROM users WHERE role='admin' AND is_active=1  -- (2) guard
    //       if count <= 1 { return 400 }                              -- (3) reject
    //   UPDATE users SET ... WHERE id = ?                            -- (4) write
    //
    // with NO transaction wrapping steps 1–4. Two concurrent `update`
    // requests that each demote/deactivate one of two active admins could
    // BOTH read `count = 2` (each sees both admins still active), BOTH pass
    // the `count <= 1` guard, and BOTH run their UPDATE — leaving zero
    // active admins and bricking login. Confirmed by test
    // `concurrent_demote_cannot_zero_admins`: 2 concurrent demotions of the
    // two active admins both succeeded pre-fix.
    //
    // `BEGIN IMMEDIATE` acquires the SQLite write lock at transaction start.
    // SQLite only allows a single writer at a time, so this serializes the
    // read-check-write sequence: the second demotion's `SELECT COUNT(*)`
    // only runs after the first has committed, so it sees `count = 1` and
    // is correctly rejected. (Matches the `create_invoice` / `add_payment`
    // pattern from prior sessions. SQLite has no `SELECT ... FOR UPDATE`;
    // `BEGIN IMMEDIATE` is the standard idiom for serializing writers.)
    let mut conn = state.db.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

    // Fetch the user's current role/is_active so we can guard the last-active-
    // admin invariant on role-change and deactivation. (Same invariant `delete`
    // and `toggle_active` guard — without it, an admin could demote or
    // deactivate themselves and brick the system: no active admin can log in.)
    let current: Option<(String, bool)> = sqlx::query_as(
        "SELECT role, is_active FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    let (cur_role, cur_active) = match current {
        Some(t) => t,
        None => {
            sqlx::query("ROLLBACK").execute(&mut *conn).await?;
            return Err(ApiError::NotFound);
        }
    };

    // Build a dynamic update — only set provided fields.
    if let Some(email) = b.email {
        sqlx::query("UPDATE users SET email = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(&email).bind(id).execute(&mut *conn).await?;
    }
    if let Some(role) = b.role {
        // Guard: demoting the last active admin to a non-admin role would
        // leave zero active admins. Reject it.
        if cur_role == "admin" && role != "admin" && cur_active {
            let active_admins: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1")
                    .fetch_one(&mut *conn)
                    .await?;
            if active_admins <= 1 {
                sqlx::query("ROLLBACK").execute(&mut *conn).await?;
                return Err(ApiError::BadRequest(
                    "Cannot demote the last active admin".into(),
                ));
            }
        }
        sqlx::query("UPDATE users SET role = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(&role).bind(id).execute(&mut *conn).await?;
    }
    if let Some(first) = b.first_name {
        sqlx::query("UPDATE users SET first_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(&first).bind(id).execute(&mut *conn).await?;
    }
    if let Some(last) = b.last_name {
        sqlx::query("UPDATE users SET last_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(&last).bind(id).execute(&mut *conn).await?;
    }
    if let Some(active) = b.is_active {
        // Guard: deactivating the last active admin would brick the system.
        if !active && cur_role == "admin" && cur_active {
            let active_admins: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1")
                    .fetch_one(&mut *conn)
                    .await?;
            if active_admins <= 1 {
                sqlx::query("ROLLBACK").execute(&mut *conn).await?;
                return Err(ApiError::BadRequest(
                    "Cannot deactivate the last active admin".into(),
                ));
            }
        }
        sqlx::query("UPDATE users SET is_active = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(active).bind(id).execute(&mut *conn).await?;
    }
    if let Some(ref pw) = b.password {
        let hash = crate::auth::hash_password(pw)?;
        sqlx::query("UPDATE users SET password_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(&hash).bind(id).execute(&mut *conn).await?;
    }

    // Read back the updated row within the same transaction so the returned
    // object reflects the committed state.
    let row = sqlx::query("SELECT id, username, email, role, first_name, last_name, is_active, created_at, updated_at FROM users WHERE id = ?")
        .bind(id).fetch_optional(&mut *conn).await?;
    sqlx::query("COMMIT").execute(&mut *conn).await?;
    let row = row.ok_or(ApiError::NotFound)?;
    Ok(Json(row_to_user(&row)))
}

pub async fn toggle_active(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<serde_json::Value>> {
    // Wrap the read-guard-write sequence in a single `BEGIN IMMEDIATE`
    // transaction — same race fix as `update` and `delete`. Without it, two
    // concurrent `toggle_active` requests that each deactivate one of two
    // active admins could BOTH read `count = 2`, BOTH pass the guard, and
    // BOTH run their UPDATE — leaving zero active admins. See the comment
    // in `update` for the full rationale.
    let mut conn = state.db.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

    let row = sqlx::query("SELECT is_active, role FROM users WHERE id = ?").bind(id).fetch_optional(&mut *conn).await?;
    let row = match row {
        Some(r) => r,
        None => {
            sqlx::query("ROLLBACK").execute(&mut *conn).await?;
            return Err(ApiError::NotFound);
        }
    };
    let current: bool = row.get("is_active");
    let role: String = row.get("role");

    // Guard: never deactivate the last active admin (would brick the system —
    // no active admin can log in to recover it). Mirrors the guard in `delete`.
    // Toggling an already-inactive user ON is always safe (current=false).
    if current && role == "admin" {
        let active_admins: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1")
                .fetch_one(&mut *conn)
                .await?;
        if active_admins <= 1 {
            sqlx::query("ROLLBACK").execute(&mut *conn).await?;
            return Err(ApiError::BadRequest(
                "Cannot deactivate the last active admin".into(),
            ));
        }
    }

    sqlx::query("UPDATE users SET is_active = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(!current).bind(id).execute(&mut *conn).await?;
    sqlx::query("COMMIT").execute(&mut *conn).await?;
    Ok(Json(serde_json::json!({ "id": id, "is_active": !current })))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<shared::MessageResponse>> {
    // Wrap the read-guard-write sequence in a single `BEGIN IMMEDIATE`
    // transaction — same race fix as `update` and `toggle_active`. Without
    // it, two concurrent `delete` requests that each delete one of two
    // active admins could BOTH read `count = 2`, BOTH pass the guard, and
    // BOTH run their DELETE — leaving zero active admins. See the comment
    // in `update` for the full rationale.
    let mut conn = state.db.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

    // Prevent deleting the last active admin.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1").fetch_one(&mut *conn).await?;
    if count.0 <= 1 {
        let user: Option<(String,)> = sqlx::query_as("SELECT role FROM users WHERE id = ?").bind(id).fetch_optional(&mut *conn).await?;
        if let Some((role,)) = user {
            if role == "admin" {
                sqlx::query("ROLLBACK").execute(&mut *conn).await?;
                return Err(ApiError::BadRequest("Cannot delete the last active admin".into()));
            }
        }
    }
    let r = sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(&mut *conn).await?;
    let rows_affected = r.rows_affected();
    sqlx::query("COMMIT").execute(&mut *conn).await?;
    if rows_affected == 0 { return Err(ApiError::NotFound); }
    Ok(Json(shared::MessageResponse { message: "User deleted".into() }))
}
