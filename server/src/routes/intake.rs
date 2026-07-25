//! Public intake submissions (from the localhost input page / future website).
//! These are PUBLIC endpoints (no auth) — the input form anyone can fill in.

use axum::{
    extract::{Path, State},
    Json,
};
use shared::{CreateIntake, IntakeSubmission};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

fn row_to_intake(r: &sqlx::sqlite::SqliteRow) -> IntakeSubmission {
    IntakeSubmission {
        id: r.get("id"),
        submitted_at: r.get("submitted_at"),
        first_name: r.get("first_name"),
        last_name: r.get("last_name"),
        date_of_birth: r.get("date_of_birth"),
        phone: r.get("phone"),
        email: r.get("email"),
        address: r.get("address"),
        medicare_number: r.get("medicare_number"),
        preferred_date: r.get("preferred_date"),
        preferred_time: r.get("preferred_time"),
        appointment_type: r.get("appointment_type"),
        symptoms: r.get("symptoms"),
        source: r.get("source"),
        status: r.get("status"),
        matched_patient_id: r.get("matched_patient_id"),
    }
}

/// PUBLIC: submit an intake form. No auth required.
pub async fn submit(State(state): State<AppState>, Json(b): Json<CreateIntake>) -> ApiResult<axum::response::Response> {
    let r = sqlx::query(
        "INSERT INTO intake_submissions
         (first_name, last_name, date_of_birth, phone, email, address, medicare_number,
          preferred_date, preferred_time, appointment_type, symptoms, source)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&b.first_name).bind(&b.last_name).bind(&b.date_of_birth)
        .bind(&b.phone).bind(&b.email).bind(&b.address).bind(&b.medicare_number)
        .bind(&b.preferred_date).bind(&b.preferred_time).bind(&b.appointment_type)
        .bind(&b.symptoms).bind(b.source.unwrap_or_else(|| "input-page".into()))
        .execute(&state.db).await?;
    let id = r.last_insert_rowid();
    let row = sqlx::query("SELECT * FROM intake_submissions WHERE id = ?").bind(id).fetch_one(&state.db).await?;
    use axum::response::IntoResponse;
    Ok((axum::http::StatusCode::CREATED, Json(row_to_intake(&row))).into_response())
}

/// PROTECTED: list intake submissions (for staff to review/import).
pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<IntakeSubmission>>> {
    let rows = sqlx::query("SELECT * FROM intake_submissions ORDER BY submitted_at DESC LIMIT 200")
        .fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(row_to_intake).collect()))
}

/// PROTECTED: import an intake submission as a patient + optional appointment.
pub async fn import(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<shared::MessageResponse>> {
    import_one(&state, id).await?;
    // fetch the MRN we just created for the message
    let row = sqlx::query("SELECT matched_patient_id FROM intake_submissions WHERE id = ?")
        .bind(id).fetch_one(&state.db).await?;
    let pid: Option<i64> = row.get("matched_patient_id");
    let mrn = if let Some(pid) = pid {
        let prow = sqlx::query("SELECT mrn FROM patients WHERE id = ?").bind(pid).fetch_one(&state.db).await?;
        prow.get::<String, _>("mrn")
    } else { "unknown".into() };
    Ok(Json(shared::MessageResponse { message: format!("Imported as patient (MRN {})", mrn) }))
}

/// PROTECTED: archive a submission without importing.
pub async fn archive(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<shared::MessageResponse>> {
    sqlx::query("UPDATE intake_submissions SET status = 'archived' WHERE id = ?")
        .bind(id).execute(&state.db).await?;
    Ok(Json(shared::MessageResponse { message: "Archived".into() }))
}

/// PROTECTED: auto-import ALL new submissions in one go. Returns count.
pub async fn auto_import(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let rows = sqlx::query("SELECT id FROM intake_submissions WHERE status = 'new'")
        .fetch_all(&state.db).await?;
    let total = rows.len();
    let mut imported = 0;
    for r in &rows {
        let id: i64 = r.get("id");
        // reuse the single-import logic
        if import_one(&state, id).await.is_ok() {
            imported += 1;
        }
    }
    Ok(Json(serde_json::json!({ "imported": imported, "total_new": total })))
}

/// Shared import logic used by both single-import and auto-import.
async fn import_one(state: &AppState, id: i64) -> ApiResult<()> {
    let row = sqlx::query("SELECT * FROM intake_submissions WHERE id = ?")
        .bind(id).fetch_optional(&state.db).await?.ok_or(ApiError::NotFound)?;
    let sub = row_to_intake(&row);

    let year = chrono::Utc::now().format("%Y");
    let mrn = format!("MOS-{}{:07}", year, rand::random::<u32>() % 1_000_000);
    let pr = sqlx::query(
        "INSERT INTO patients (mrn, first_name, last_name, date_of_birth, phone, email, address, medicare_number)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&mrn).bind(&sub.first_name).bind(&sub.last_name).bind(&sub.date_of_birth)
        .bind(&sub.phone).bind(&sub.email).bind(&sub.address).bind(&sub.medicare_number)
        .execute(&state.db).await?;
    let pid = pr.last_insert_rowid();

    if let Some(date) = sub.preferred_date {
        let dt = format!("{} {}:00", date, sub.preferred_time.unwrap_or_else(|| "09:00".into()));
        let atype = sub.appointment_type.unwrap_or_else(|| "Dry Eye Consultation".into());
        sqlx::query(
            "INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes, status, notes)
             VALUES (?, ?, ?, 60, 'scheduled', ?)")
            .bind(pid).bind(&atype).bind(&dt).bind(&sub.symptoms)
            .execute(&state.db).await?;
    }

    sqlx::query("UPDATE intake_submissions SET status = 'imported', matched_patient_id = ? WHERE id = ?")
        .bind(pid).bind(id).execute(&state.db).await?;
    Ok(())
}
