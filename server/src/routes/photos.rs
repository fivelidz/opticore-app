//! Patient photos/files: profile pictures, medical documentation, other documents.

use axum::{
    extract::{Path, State},
    Json,
};
use axum::http::StatusCode;
use shared::{PatientPhoto, UploadPhoto};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

fn row_to_photo(r: &sqlx::sqlite::SqliteRow) -> PatientPhoto {
    PatientPhoto {
        id: r.get("id"),
        patient_id: r.get("patient_id"),
        category: r.get("category"),
        filename: r.get("filename"),
        mime_type: r.get("mime_type"),
        caption: r.get("caption"),
        file_size: r.get("file_size"),
        captured_at: r.get("captured_at"),
        created_at: r.get("created_at"),
    }
}

/// GET /api/patients/:id/photos — list a patient's photos (metadata only, no data).
pub async fn list(State(state): State<AppState>, Path(pid): Path<i64>) -> ApiResult<Json<Vec<PatientPhoto>>> {
    let rows = sqlx::query("SELECT id, patient_id, category, filename, mime_type, caption, file_size, captured_at, created_at FROM patient_photos WHERE patient_id = ? ORDER BY category, created_at DESC")
        .bind(pid).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(row_to_photo).collect()))
}

/// GET /api/patients/:id/photos/:photo — return the raw base64 data for display.
pub async fn get_data(State(state): State<AppState>, Path((pid, photo)): Path<(i64, i64)>) -> ApiResult<Json<serde_json::Value>> {
    let row = sqlx::query("SELECT data_base64, mime_type FROM patient_photos WHERE id = ? AND patient_id = ?")
        .bind(photo).bind(pid).fetch_optional(&state.db).await?.ok_or(ApiError::NotFound)?;
    let data: String = row.get("data_base64");
    let mime: String = row.get("mime_type");
    Ok(Json(serde_json::json!({ "data": data, "mime": mime })))
}

/// POST /api/patients/:id/photos — upload a photo (base64 body).
pub async fn upload(State(state): State<AppState>, Path(pid): Path<i64>, Json(body): Json<UploadPhoto>) -> ApiResult<axum::response::Response> {
    // validate category
    if !["profile", "medical", "document"].contains(&body.category.as_str()) {
        return Err(ApiError::BadRequest("Invalid category".into()));
    }
    // rough size guard (base64 ~1.3x raw; cap at ~10MB raw)
    if body.data_base64.len() > 14_000_000 {
        return Err(ApiError::BadRequest("File too large (max ~10MB)".into()));
    }
    let file_size = (body.data_base64.len() as f64 / 1.33) as i64;
    let r = sqlx::query(
        "INSERT INTO patient_photos (patient_id, category, filename, mime_type, caption, data_base64, file_size)
         VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(pid).bind(&body.category).bind(&body.filename).bind(&body.mime_type)
        .bind(&body.caption).bind(&body.data_base64).bind(file_size)
        .execute(&state.db).await?;
    let id = r.last_insert_rowid();

    // if profile, set as the patient's profile photo
    if body.category == "profile" {
        sqlx::query("UPDATE patients SET profile_photo_id = ? WHERE id = ?")
            .bind(id).bind(pid).execute(&state.db).await?;
    }

    let row = sqlx::query("SELECT id, patient_id, category, filename, mime_type, caption, file_size, captured_at, created_at FROM patient_photos WHERE id = ?")
        .bind(id).fetch_one(&state.db).await?;
    use axum::response::IntoResponse;
    Ok((StatusCode::CREATED, Json(row_to_photo(&row))).into_response())
}

/// DELETE /api/patients/:id/photos/:photo
pub async fn delete(State(state): State<AppState>, Path((pid, photo)): Path<(i64, i64)>) -> ApiResult<Json<shared::MessageResponse>> {
    // clear profile pointer if needed
    sqlx::query("UPDATE patients SET profile_photo_id = NULL WHERE profile_photo_id = ?").bind(photo).execute(&state.db).await?;
    let r = sqlx::query("DELETE FROM patient_photos WHERE id = ? AND patient_id = ?").bind(photo).bind(pid).execute(&state.db).await?;
    if r.rows_affected() == 0 { return Err(ApiError::NotFound); }
    Ok(Json(shared::MessageResponse { message: "Photo deleted".into() }))
}

/// POST /api/patients/:id/photos/:photo/make-profile — set an existing photo as the profile pic.
pub async fn make_profile(State(state): State<AppState>, Path((pid, photo)): Path<(i64, i64)>) -> ApiResult<Json<shared::MessageResponse>> {
    sqlx::query("UPDATE patients SET profile_photo_id = ? WHERE id = ?").bind(photo).bind(pid).execute(&state.db).await?;
    Ok(Json(shared::MessageResponse { message: "Set as profile photo".into() }))
}
