//! Unified messages inbox: email, WhatsApp, website messages.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use shared::{CreateMessage, Message};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

fn row_to_msg(r: &sqlx::sqlite::SqliteRow) -> Message {
    Message {
        id: r.get("id"),
        received_at: r.get("received_at"),
        channel: r.get("channel"),
        from_name: r.get("from_name"),
        from_contact: r.get("from_contact"),
        subject: r.get("subject"),
        body: r.get("body"),
        status: r.get("status"),
        linked_patient_id: r.get("linked_patient_id"),
        thread_id: r.get("thread_id"),
        created_at: r.get("created_at"),
    }
}

#[derive(Deserialize)]
pub struct MsgQuery {
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

pub async fn list(State(state): State<AppState>, Query(q): Query<MsgQuery>) -> ApiResult<Json<Vec<Message>>> {
    let rows = match (q.channel.as_deref(), q.status.as_deref()) {
        (Some(c), Some(s)) => sqlx::query("SELECT * FROM messages WHERE channel = ? AND status = ? ORDER BY received_at DESC")
            .bind(c).bind(s).fetch_all(&state.db).await?,
        (Some(c), None) => sqlx::query("SELECT * FROM messages WHERE channel = ? ORDER BY received_at DESC")
            .bind(c).fetch_all(&state.db).await?,
        (None, Some(s)) => sqlx::query("SELECT * FROM messages WHERE status = ? ORDER BY received_at DESC")
            .bind(s).fetch_all(&state.db).await?,
        (None, None) => sqlx::query("SELECT * FROM messages ORDER BY received_at DESC LIMIT 200")
            .fetch_all(&state.db).await?,
    };
    Ok(Json(rows.iter().map(row_to_msg).collect()))
}

/// PUBLIC: receive a message (from website contact form, webhook, etc.)
pub async fn receive(State(state): State<AppState>, Json(b): Json<CreateMessage>) -> ApiResult<axum::response::Response> {
    let r = sqlx::query(
        "INSERT INTO messages (channel, from_name, from_contact, subject, body, thread_id)
         VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&b.channel).bind(&b.from_name).bind(&b.from_contact)
        .bind(&b.subject).bind(&b.body).bind(&b.thread_id)
        .execute(&state.db).await?;
    let id = r.last_insert_rowid();
    let row = sqlx::query("SELECT * FROM messages WHERE id = ?").bind(id).fetch_one(&state.db).await?;
    use axum::response::IntoResponse;
    Ok((axum::http::StatusCode::CREATED, Json(row_to_msg(&row))).into_response())
}

pub async fn mark_read(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<shared::MessageResponse>> {
    sqlx::query("UPDATE messages SET status = 'read' WHERE id = ?").bind(id).execute(&state.db).await?;
    Ok(Json(shared::MessageResponse { message: "Marked read".into() }))
}

pub async fn archive(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<shared::MessageResponse>> {
    sqlx::query("UPDATE messages SET status = 'archived' WHERE id = ?").bind(id).execute(&state.db).await?;
    Ok(Json(shared::MessageResponse { message: "Archived".into() }))
}

pub async fn link_patient(
    State(state): State<AppState>,
    Path((id, pid)): Path<(i64, i64)>,
) -> ApiResult<Json<shared::MessageResponse>> {
    // The `messages` table has NO foreign-key constraint on
    // `linked_patient_id` (it was added in 0005_messages.sql without one).
    // Without this check, linking a message to a nonexistent patient would
    // silently store a dangling reference. Verify the patient exists first.
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM patients WHERE id = ?)")
        .bind(pid)
        .fetch_one(&state.db)
        .await?;
    if !exists {
        return Err(ApiError::BadRequest(
            "referenced patient does not exist".into(),
        ));
    }
    sqlx::query("UPDATE messages SET linked_patient_id = ? WHERE id = ?")
        .bind(pid).bind(id).execute(&state.db).await?;
    Ok(Json(shared::MessageResponse { message: "Linked to patient".into() }))
}
