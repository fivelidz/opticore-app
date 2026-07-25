use axum::{
    extract::{Path, State},
    Json,
};
use axum::http::StatusCode;
use shared::{BlockedTime, CreateBlockedTime};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

fn row(row: &sqlx::sqlite::SqliteRow) -> BlockedTime {
    BlockedTime {
        id: row.get("id"),
        start_at: row.get("start_at"),
        end_at: row.get("end_at"),
        reason: row.get("reason"),
        practitioner: row.get("practitioner"),
        all_day: row.get("all_day"),
        is_recurring: row.get("is_recurring"),
        created_at: row.get("created_at"),
    }
}

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<BlockedTime>>> {
    let rows = sqlx::query("SELECT * FROM blocked_times ORDER BY start_at")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(row).collect()))
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateBlockedTime>,
) -> ApiResult<axum::response::Response> {
    let start = shared::normalize_dt(&body.start_at);
    let end = shared::normalize_dt(&body.end_at);
    let all_day = body.all_day.unwrap_or(false);
    let recurring = body.is_recurring.unwrap_or(false);
    let r = sqlx::query(
        "INSERT INTO blocked_times (start_at, end_at, reason, practitioner, all_day, is_recurring) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&start)
    .bind(&end)
    .bind(&body.reason)
    .bind(&body.practitioner)
    .bind(all_day)
    .bind(recurring)
    .execute(&state.db)
    .await?;
    let id = r.last_insert_rowid();
    let rrow = sqlx::query("SELECT * FROM blocked_times WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    use axum::response::IntoResponse;
    Ok((axum::http::StatusCode::CREATED, Json(row(&rrow))).into_response())
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<shared::MessageResponse>> {
    let r = sqlx::query("DELETE FROM blocked_times WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(shared::MessageResponse { message: "Blocked time removed".into() }))
}

/// PUT /api/blocked-times/:id — update an existing blocked time atomically
/// (replaces the lossy delete+create pattern).
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<CreateBlockedTime>,
) -> ApiResult<axum::response::Response> {
    let start = shared::normalize_dt(&body.start_at);
    let end = shared::normalize_dt(&body.end_at);
    let all_day = body.all_day.unwrap_or(false);
    let recurring = body.is_recurring.unwrap_or(false);
    let r = sqlx::query(
        "UPDATE blocked_times SET start_at = ?, end_at = ?, reason = ?, practitioner = ?, all_day = ?, is_recurring = ? WHERE id = ?")
        .bind(&start).bind(&end).bind(&body.reason).bind(&body.practitioner).bind(all_day).bind(recurring).bind(id)
        .execute(&state.db).await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    let rrow = sqlx::query("SELECT * FROM blocked_times WHERE id = ?").bind(id).fetch_one(&state.db).await?;
    use axum::response::IntoResponse;
    Ok((StatusCode::OK, Json(row(&rrow))).into_response())
}
