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

/// Validate that `s` is well-formed standard base64 (RFC 4648) and decodes to
/// at least one byte.
///
/// The photo store keeps `data_base64` as TEXT (the schema's design), so we
/// don't need the decoded bytes — but we MUST reject malformed input up front.
/// Without this check, a client can upload `data_base64: "!!!garbage!!!"` and
/// it is persisted verbatim; every downstream consumer that decodes the stored
/// value then gets garbage or a decode error (silent data corruption).
///
/// Rules checked:
///   - non-empty (an empty photo is meaningless)
///   - only legal base64 alphabet chars `[A-Za-z0-9+/]` plus `=` padding and
///     internal whitespace (some transports wrap lines)
///   - length is a multiple of 4 (after stripping whitespace)
///   - padding is only at the tail and at most 2 `=` chars
fn validate_base64(s: &str) -> Result<(), &'static str> {
    if s.is_empty() {
        return Err("data_base64 must not be empty");
    }
    // Strip internal whitespace/newlines (some transports line-wrap base64).
    let filtered: Vec<u8> = s.bytes().filter(|b| *b != b'\n' && *b != b'\r' && *b != b' ').collect();
    if filtered.is_empty() {
        return Err("data_base64 must not be empty");
    }
    // Length must be a multiple of 4 (padding included).
    if filtered.len() % 4 != 0 {
        return Err("data_base64 has invalid length (must be a multiple of 4)");
    }
    // Validate every byte. Padding (`=`) is only legal in the final two
    // positions; everything else must be in the base64 alphabet.
    let n = filtered.len();
    for (i, &b) in filtered.iter().enumerate() {
        let is_alpha = b.is_ascii_alphanumeric() || b == b'+' || b == b'/';
        if is_alpha {
            continue;
        }
        if b == b'=' {
            // Padding only allowed at the tail (last 1 or 2 positions).
            if i < n - 2 {
                return Err("data_base64 has padding in a non-terminal position");
            }
            // The very last char before padding must not itself be padding
            // unless it's the second-to-last (i.e. "==" is fine, "===" is not).
            if i == n - 2 && filtered[n - 1] != b'=' {
                return Err("data_base64 has invalid padding");
            }
            continue;
        }
        return Err("data_base64 contains illegal characters");
    }
    Ok(())
}

fn row_to_photo(r: &sqlx::sqlite::SqliteRow) -> PatientPhoto {
    PatientPhoto {
        id: r.get("id"),
        patient_id: r.get("patient_id"),
        appointment_id: r.get("appointment_id"),
        category: r.get("category"),
        filename: r.get("filename"),
        mime_type: r.get("mime_type"),
        caption: r.get("caption"),
        file_size: r.get("file_size"),
        captured_at: r.get("captured_at"),
        created_at: r.get("created_at"),
    }
}

// Columns selected for the metadata (no data_base64) list responses.
const PHOTO_COLS: &str = "id, patient_id, appointment_id, category, filename, mime_type, caption, file_size, captured_at, created_at";

/// GET /api/patients/:id/photos — list a patient's photos (metadata only, no data).
pub async fn list(State(state): State<AppState>, Path(pid): Path<i64>) -> ApiResult<Json<Vec<PatientPhoto>>> {
    let sql = format!("SELECT {PHOTO_COLS} FROM patient_photos WHERE patient_id = ? ORDER BY category, created_at DESC");
    let rows = sqlx::query(&sql).bind(pid).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(row_to_photo).collect()))
}

/// GET /api/appointments/:id/attachments — files attached to a specific appointment.
pub async fn list_by_appointment(State(state): State<AppState>, Path(aid): Path<i64>) -> ApiResult<Json<Vec<PatientPhoto>>> {
    let sql = format!("SELECT {PHOTO_COLS} FROM patient_photos WHERE appointment_id = ? ORDER BY created_at DESC");
    let rows = sqlx::query(&sql).bind(aid).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(row_to_photo).collect()))
}

/// GET /api/appointments/:id/attachments/:photo — raw base64 data for display/download.
pub async fn get_appointment_data(State(state): State<AppState>, Path((aid, photo)): Path<(i64, i64)>) -> ApiResult<Json<serde_json::Value>> {
    let row = sqlx::query("SELECT data_base64, mime_type FROM patient_photos WHERE id = ? AND appointment_id = ?")
        .bind(photo).bind(aid).fetch_optional(&state.db).await?.ok_or(ApiError::NotFound)?;
    let data: String = row.get("data_base64");
    let mime: String = row.get("mime_type");
    Ok(Json(serde_json::json!({ "data": data, "mime": mime })))
}

/// POST /api/appointments/:id/attachments — upload a file attached to this appointment.
/// The patient_id is resolved from the appointment so the file also shows on the
/// patient's file, and the appointment_id links it to this visit.
pub async fn upload_to_appointment(State(state): State<AppState>, Path(aid): Path<i64>, Json(mut body): Json<UploadPhoto>) -> ApiResult<axum::response::Response> {
    // Resolve the owning patient from the appointment.
    let arow = sqlx::query("SELECT patient_id FROM appointments WHERE id = ?")
        .bind(aid).fetch_optional(&state.db).await?.ok_or(ApiError::NotFound)?;
    let pid: i64 = arow.get("patient_id");
    body.patient_id = pid;
    body.appointment_id = Some(aid);
    // Attachments to an appointment are 'document' unless explicitly medical.
    if body.category == "profile" { body.category = "document".into(); }
    insert_photo(&state, body).await
}

/// DELETE /api/appointments/:id/attachments/:photo
pub async fn delete_appointment_attachment(State(state): State<AppState>, Path((aid, photo)): Path<(i64, i64)>) -> ApiResult<Json<shared::MessageResponse>> {
    let r = sqlx::query("DELETE FROM patient_photos WHERE id = ? AND appointment_id = ?").bind(photo).bind(aid).execute(&state.db).await?;
    if r.rows_affected() == 0 { return Err(ApiError::NotFound); }
    Ok(Json(shared::MessageResponse { message: "Attachment removed".into() }))
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
pub async fn upload(State(state): State<AppState>, Path(pid): Path<i64>, Json(mut body): Json<UploadPhoto>) -> ApiResult<axum::response::Response> {
    body.patient_id = pid;
    insert_photo(&state, body).await
}

/// Shared insert used by both patient-level and appointment-level uploads.
async fn insert_photo(state: &AppState, body: UploadPhoto) -> ApiResult<axum::response::Response> {
    // validate category
    if !["profile", "medical", "document"].contains(&body.category.as_str()) {
        return Err(ApiError::BadRequest("Invalid category".into()));
    }
    // Validate the base64 payload BEFORE storing it. Without this, malformed
    // base64 (illegal chars, bad padding, empty) is persisted verbatim and
    // surfaces as garbage or decode errors on every downstream read.
    if let Err(msg) = validate_base64(&body.data_base64) {
        return Err(ApiError::BadRequest(msg.into()));
    }
    // rough size guard (base64 ~1.3x raw; cap at ~10MB raw)
    if body.data_base64.len() > 14_000_000 {
        return Err(ApiError::BadRequest("File too large (max ~10MB)".into()));
    }
    let file_size = (body.data_base64.len() as f64 / 1.33) as i64;
    let r = sqlx::query(
        "INSERT INTO patient_photos (patient_id, appointment_id, category, filename, mime_type, caption, data_base64, file_size)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(body.patient_id).bind(body.appointment_id).bind(&body.category).bind(&body.filename).bind(&body.mime_type)
        .bind(&body.caption).bind(&body.data_base64).bind(file_size)
        .execute(&state.db).await?;
    let id = r.last_insert_rowid();

    // if profile, set as the patient's profile photo
    if body.category == "profile" {
        sqlx::query("UPDATE patients SET profile_photo_id = ? WHERE id = ?")
            .bind(id).bind(body.patient_id).execute(&state.db).await?;
    }

    let sql = format!("SELECT {PHOTO_COLS} FROM patient_photos WHERE id = ?");
    let row = sqlx::query(&sql).bind(id).fetch_one(&state.db).await?;
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
    // Verify the photo exists AND belongs to this patient before linking it.
    // Without this check, a caller could set `profile_photo_id` to any integer
    // — including a nonexistent id (dangling pointer; the column has no FK
    // constraint so the DB won't reject it) or another patient's photo id
    // (cross-patient data reference).
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM patient_photos WHERE id = ? AND patient_id = ?")
        .bind(photo).bind(pid).fetch_optional(&state.db).await?;
    if exists.is_none() {
        return Err(ApiError::NotFound);
    }
    sqlx::query("UPDATE patients SET profile_photo_id = ? WHERE id = ?").bind(photo).bind(pid).execute(&state.db).await?;
    Ok(Json(shared::MessageResponse { message: "Set as profile photo".into() }))
}
