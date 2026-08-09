use axum::{
    extract::{Path, Query, State},
    Json,
};
use shared::{
    Appointment, AppointmentList, AppointmentQuery, CreateAppointment, UpdateAppointment,
};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

fn row_to_appt(row: &sqlx::sqlite::SqliteRow) -> Appointment {
    Appointment {
        id: row.get("id"),
        patient_id: row.get("patient_id"),
        appointment_type: row.get("appointment_type"),
        appointment_date: row.get("appointment_date"),
        duration_minutes: row.get("duration_minutes"),
        practitioner: row.get("practitioner"),
        status: row.get("status"),
        notes: row.get("notes"),
        created_at: row.get("created_at"),
        first_name: row.try_get("first_name").ok(),
        last_name: row.try_get("last_name").ok(),
        phone: row.try_get("phone").ok(),
        mrn: row.try_get("mrn").ok(),
    }
}

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<AppointmentQuery>,
) -> ApiResult<Json<AppointmentList>> {
    let rows = if let Some(date) = q.date {
        sqlx::query(
            "SELECT a.*, p.first_name, p.last_name, p.phone, p.mrn
             FROM appointments a JOIN patients p ON a.patient_id = p.id
             WHERE DATE(a.appointment_date) = ?
             ORDER BY a.appointment_date ASC",
        )
        .bind(date)
        .fetch_all(&state.db)
        .await?
    } else if let Some(pid) = q.patient_id {
        sqlx::query(
            "SELECT a.*, p.first_name, p.last_name, p.phone, p.mrn
             FROM appointments a JOIN patients p ON a.patient_id = p.id
             WHERE a.patient_id = ?
             ORDER BY a.appointment_date ASC",
        )
        .bind(pid)
        .fetch_all(&state.db)
        .await?
    } else if let (Some(from), Some(to)) = (q.from, q.to) {
        sqlx::query(
            "SELECT a.*, p.first_name, p.last_name, p.phone, p.mrn
             FROM appointments a JOIN patients p ON a.patient_id = p.id
             WHERE a.appointment_date BETWEEN ? AND ?
             ORDER BY a.appointment_date ASC",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query(
            "SELECT a.*, p.first_name, p.last_name, p.phone, p.mrn
             FROM appointments a JOIN patients p ON a.patient_id = p.id
             ORDER BY a.appointment_date ASC LIMIT 200",
        )
        .fetch_all(&state.db)
        .await?
    };

    let appointments = rows.iter().map(row_to_appt).collect::<Vec<_>>();
    let count = appointments.len();
    Ok(Json(AppointmentList { appointments, count }))
}

pub async fn today(State(state): State<AppState>) -> ApiResult<Json<AppointmentList>> {
    let rows = sqlx::query(
        "SELECT a.*, p.first_name, p.last_name, p.phone, p.mrn
         FROM appointments a JOIN patients p ON a.patient_id = p.id
         WHERE DATE(a.appointment_date) = DATE('now','localtime')
         ORDER BY a.appointment_date ASC",
    )
    .fetch_all(&state.db)
    .await?;

    let appointments = rows.iter().map(row_to_appt).collect::<Vec<_>>();
    let count = appointments.len();
    Ok(Json(AppointmentList { appointments, count }))
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<Appointment>> {
    let row = sqlx::query(
        "SELECT a.*, p.first_name, p.last_name, p.phone, p.mrn
         FROM appointments a JOIN patients p ON a.patient_id = p.id
         WHERE a.id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(Json(row_to_appt(&row)))
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateAppointment>,
) -> ApiResult<axum::response::Response> {
    // ---- Date validation ------------------------------------------------
    //
    // A practice-management system should not allow booking appointments in
    // the past, nor should it store unparseable date strings. Previously
    // `normalize_dt` returned the raw string verbatim on parse failure,
    // so "not-a-date" was silently stored as the appointment_date — and
    // any past date was accepted, letting users "book" appointments that
    // could never be kept.
    //
    // Rules:
    //   * appointment_date must parse as RFC3339 or "YYYY-MM-DD"
    //     (malformed -> 400)
    //   * the parsed instant must be >= now (past -> 400)
    //
    // We do NOT constrain far-future dates (booking a year ahead is
    // legitimate) and we do NOT enforce business hours here (that belongs
    // in a scheduling-conflict layer, not date validation).
    let dt = shared::parse_dt(&body.appointment_date).ok_or_else(|| {
        ApiError::BadRequest(format!(
            "appointment_date '{}' is not a valid date (expected RFC3339 or YYYY-MM-DD)",
            body.appointment_date
        ))
    })?;
    if dt < chrono::Utc::now() {
        return Err(ApiError::BadRequest(
            "appointment_date cannot be in the past".into(),
        ));
    }
    // A zero/negative duration is nonsensical (a 0-minute or negative-length
    // appointment). Must be >= 1 minute.
    if body.duration_minutes < 1 {
        return Err(ApiError::BadRequest(
            "duration_minutes must be >= 1".into(),
        ));
    }
    let dt = shared::normalize_dt(&body.appointment_date);
    let r = sqlx::query(
        "INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes, practitioner, status, notes)
         VALUES (?, ?, ?, ?, ?, 'scheduled', ?)",
    )
    .bind(body.patient_id)
    .bind(&body.appointment_type)
    .bind(&dt)
    .bind(body.duration_minutes)
    .bind(&body.practitioner)
    .bind(&body.notes)
    .execute(&state.db)
    .await?;

    let id = r.last_insert_rowid();
    let row = sqlx::query(
        "SELECT a.*, p.first_name, p.last_name, p.phone, p.mrn
         FROM appointments a JOIN patients p ON a.patient_id = p.id
         WHERE a.id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    use axum::response::IntoResponse;
    Ok((axum::http::StatusCode::CREATED, Json(row_to_appt(&row))).into_response())
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateAppointment>,
) -> ApiResult<Json<Appointment>> {
    // Date validation on UPDATE: reject malformed dates, but DO NOT reject past
    // dates. Updates are how the UI marks a past appointment "completed" /
    // "cancelled" and edits its notes — those re-send the original (past)
    // appointment_date, which must be allowed. (Past-date rejection remains in
    // `create` — you can't book a new appointment in the past.)
    let parsed = shared::parse_dt(&body.appointment_date).ok_or_else(|| {
        ApiError::BadRequest(format!(
            "appointment_date '{}' is not a valid date (expected RFC3339 or YYYY-MM-DD)",
            body.appointment_date
        ))
    })?;
    let _ = parsed; // (parsed only to validate shape; normalize_dt re-parses below)
    // Same duration rule as `create`.
    if body.duration_minutes < 1 {
        return Err(ApiError::BadRequest(
            "duration_minutes must be >= 1".into(),
        ));
    }
    let dt = shared::normalize_dt(&body.appointment_date);
    sqlx::query(
        "UPDATE appointments SET appointment_type = ?, appointment_date = ?, duration_minutes = ?,
            practitioner = ?, status = ?, notes = ? WHERE id = ?",
    )
    .bind(&body.appointment_type)
    .bind(&dt)
    .bind(body.duration_minutes)
    .bind(&body.practitioner)
    .bind(&body.status)
    .bind(&body.notes)
    .bind(id)
    .execute(&state.db)
    .await?;

    let row = sqlx::query(
        "SELECT a.*, p.first_name, p.last_name, p.phone, p.mrn
         FROM appointments a JOIN patients p ON a.patient_id = p.id
         WHERE a.id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(Json(row_to_appt(&row)))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<shared::MessageResponse>> {
    let r = sqlx::query("DELETE FROM appointments WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(shared::MessageResponse { message: "Appointment deleted successfully".into() }))
}
