use axum::{
    extract::{Path, Query, State},
    Json,
};
use rand::Rng;
use serde::Serialize;
use shared::{
    CreatePatient, Patient, PatientList, PatientQuery, UpdatePatient,
};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

fn row_to_patient(row: &sqlx::sqlite::SqliteRow) -> Patient {
    Patient {
        id: row.get("id"),
        mrn: row.get("mrn"),
        first_name: row.get("first_name"),
        last_name: row.get("last_name"),
        date_of_birth: row.get("date_of_birth"),
        gender: row.get("gender"),
        phone: row.get("phone"),
        email: row.get("email"),
        address: row.get("address"),
        medicare_number: row.get("medicare_number"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn gen_mrn() -> String {
    let mut rng = rand::thread_rng();
    let n: u32 = rng.gen_range(1..1_000_000);
    let year = chrono::Utc::now().format("%Y");
    format!("MOS-{}{:07}", year, n)
}

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<PatientQuery>,
) -> ApiResult<Json<PatientList>> {
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);

    let rows = if let Some(ref s) = q.search {
        let pat = format!("%{s}%");
        sqlx::query(
            "SELECT * FROM patients
             WHERE first_name LIKE ? OR last_name LIKE ? OR mrn LIKE ? OR phone LIKE ? OR email LIKE ?
             ORDER BY last_name, first_name LIMIT ? OFFSET ?",
        )
        .bind(&pat)
        .bind(&pat)
        .bind(&pat)
        .bind(&pat)
        .bind(&pat)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query("SELECT * FROM patients ORDER BY last_name, first_name LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.db)
            .await?
    };

    let patients = rows.iter().map(row_to_patient).collect::<Vec<_>>();
    let count = patients.len();
    Ok(Json(PatientList { patients, count }))
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<Patient>> {
    let row = sqlx::query("SELECT * FROM patients WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(row_to_patient(&row)))
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreatePatient>,
) -> ApiResult<axum::response::Response> {
    // first_name / last_name are required (non-Option) but the empty string
    // still deserializes. A patient record with no name is useless and breaks
    // patient-matching/listing. Reject empty/whitespace-only names.
    if body.first_name.trim().is_empty() || body.last_name.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "first_name and last_name must not be empty".into(),
        ));
    }
    let mrn = body.mrn.clone().unwrap_or_else(gen_mrn);

    // Retry on MRN collision (up to 3 attempts).
    let mut last_err: Option<sqlx::Error> = None;
    for _ in 0..3 {
        let res = sqlx::query(
            "INSERT INTO patients (mrn, first_name, last_name, date_of_birth, gender, phone, email, address, medicare_number)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&mrn)
        .bind(&body.first_name)
        .bind(&body.last_name)
        .bind(&body.date_of_birth)
        .bind(&body.gender)
        .bind(&body.phone)
        .bind(&body.email)
        .bind(&body.address)
        .bind(&body.medicare_number)
        .execute(&state.db)
        .await;
        match res {
            Ok(r) => {
                let id = r.last_insert_rowid();
                let row = sqlx::query("SELECT * FROM patients WHERE id = ?")
                    .bind(id)
                    .fetch_one(&state.db)
                    .await?;
                return Ok((axum::http::StatusCode::CREATED, Json(row_to_patient(&row)))
                    .into_response());
            }
            Err(e) => {
                if matches!(e, sqlx::Error::Database(ref d) if d.is_unique_violation()) {
                    last_err = Some(e);
                    continue;
                }
                return Err(ApiError::from(e));
            }
        }
    }
    Err(ApiError::Conflict(
        last_err.map(|e| e.to_string()).unwrap_or_else(|| "mrn collision".into()),
    ))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdatePatient>,
) -> ApiResult<Json<Patient>> {
    sqlx::query(
        "UPDATE patients SET first_name = ?, last_name = ?, date_of_birth = ?, gender = ?,
            phone = ?, email = ?, address = ?, medicare_number = ?, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(&body.first_name)
    .bind(&body.last_name)
    .bind(&body.date_of_birth)
    .bind(&body.gender)
    .bind(&body.phone)
    .bind(&body.email)
    .bind(&body.address)
    .bind(&body.medicare_number)
    .bind(id)
    .execute(&state.db)
    .await?;

    let row = sqlx::query("SELECT * FROM patients WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(row_to_patient(&row)))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<shared::MessageResponse>> {
    // ---- Referential-integrity guard -----------------------------------
    //
    // The dependent tables (appointments, invoices, clinical_notes, etc.)
    // declare `FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE
    // CASCADE`, but SQLite does NOT enforce foreign keys unless
    // `PRAGMA foreign_keys = ON` is set per-connection — and this app does
    // not set it for normal request handling (only the data-import path
    // toggles it). So a bare `DELETE FROM patients WHERE id = ?` would
    // silently SUCCEED and leave every dependent row orphaned (pointing at
    // a nonexistent patient_id): lost appointment history, broken joins,
    // dangling invoices/payments/clinical notes.
    //
    // For a medical PMS, silently orphaning clinical/financial history is
    // dangerous. The conservative fix: REFUSE the deletion (409) when any
    // dependent record exists, and tell the caller which tables block it.
    // A proper hard-delete / anonymize / merge workflow is a separate
    // feature; until then we never destroy a patient that has history.
    //
    // We check the tables that represent real patient history. (Photos and
    // messages are intentionally omitted: photos can be re-uploaded and
    // messages are an inbox, not a clinical record.)
    let blockers: [(&str, &str); 5] = [
        ("appointments",  "SELECT COUNT(*) FROM appointments WHERE patient_id = ?"),
        ("invoices",      "SELECT COUNT(*) FROM invoices WHERE patient_id = ?"),
        ("clinical_notes","SELECT COUNT(*) FROM clinical_notes WHERE patient_id = ?"),
        ("allergies",     "SELECT COUNT(*) FROM allergies WHERE patient_id = ?"),
        ("osdi_scores",   "SELECT COUNT(*) FROM osdi_scores WHERE patient_id = ?"),
    ];
    let mut blocking_tables = Vec::new();
    for (name, sql) in &blockers {
        let n: i64 = sqlx::query_scalar(sql).bind(id).fetch_one(&state.db).await?;
        if n > 0 {
            blocking_tables.push(*name);
        }
    }
    if !blocking_tables.is_empty() {
        return Err(ApiError::Conflict(format!(
            "cannot delete patient {}: dependent record(s) exist in [{}]; \
             remove or reassign them first (a proper merge/anonymize workflow is TBD)",
            id,
            blocking_tables.join(", ")
        )));
    }

    let r = sqlx::query("DELETE FROM patients WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(shared::MessageResponse { message: "Patient deleted successfully".into() }))
}

use axum::response::IntoResponse;

// ---------- Enriched patient list (with next/last appointment + spend) ----------

#[derive(Debug, Serialize)]
pub struct PatientRow {
    pub id: i64,
    pub mrn: String,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: String,
    pub gender: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub medicare_number: Option<String>,
    pub created_at: String,
    pub next_appointment: Option<String>,
    pub last_appointment: Option<String>,
    pub total_visits: i64,
    pub total_spent: f64,
    pub outstanding: f64,
}

/// GET /api/patients/enriched/list — patients with next/last appt + billing summary.
pub async fn list_enriched(
    State(state): State<AppState>,
    Query(q): Query<PatientQuery>,
) -> ApiResult<Json<Vec<PatientRow>>> {
    let limit = q.limit.unwrap_or(500).clamp(1, 1000);
    let rows = if let Some(ref s) = q.search {
        let pat = format!("%{s}%");
        sqlx::query(
            "SELECT p.id, p.mrn, p.first_name, p.last_name, p.date_of_birth, p.gender,
                    p.phone, p.email, p.medicare_number, p.created_at,
                    (SELECT MIN(appointment_date) FROM appointments a
                     WHERE a.patient_id = p.id AND a.appointment_date > datetime('now')
                       AND a.status NOT IN ('cancelled','noshow')) AS next_appointment,
                    (SELECT MAX(appointment_date) FROM appointments a
                     WHERE a.patient_id = p.id AND a.appointment_date <= datetime('now')) AS last_appointment,
                    (SELECT COUNT(*) FROM appointments a WHERE a.patient_id = p.id) AS total_visits,
                    (SELECT CAST(COALESCE(SUM(amount_paid),0) AS REAL) FROM invoices i WHERE i.patient_id = p.id) AS total_spent,
                    (SELECT CAST(COALESCE(SUM(balance_due),0) AS REAL) FROM invoices i WHERE i.patient_id = p.id) AS outstanding
             FROM patients p
             WHERE p.first_name LIKE ? OR p.last_name LIKE ? OR p.mrn LIKE ? OR p.phone LIKE ? OR p.email LIKE ?
             ORDER BY p.last_name, p.first_name LIMIT ?")
            .bind(&pat).bind(&pat).bind(&pat).bind(&pat).bind(&pat).bind(limit)
            .fetch_all(&state.db).await?
    } else {
        sqlx::query(
            "SELECT p.id, p.mrn, p.first_name, p.last_name, p.date_of_birth, p.gender,
                    p.phone, p.email, p.medicare_number, p.created_at,
                    (SELECT MIN(appointment_date) FROM appointments a
                     WHERE a.patient_id = p.id AND a.appointment_date > datetime('now')
                       AND a.status NOT IN ('cancelled','noshow')) AS next_appointment,
                    (SELECT MAX(appointment_date) FROM appointments a
                     WHERE a.patient_id = p.id AND a.appointment_date <= datetime('now')) AS last_appointment,
                    (SELECT COUNT(*) FROM appointments a WHERE a.patient_id = p.id) AS total_visits,
                    (SELECT CAST(COALESCE(SUM(amount_paid),0) AS REAL) FROM invoices i WHERE i.patient_id = p.id) AS total_spent,
                    (SELECT CAST(COALESCE(SUM(balance_due),0) AS REAL) FROM invoices i WHERE i.patient_id = p.id) AS outstanding
             FROM patients p
             ORDER BY p.last_name, p.first_name LIMIT ?")
            .bind(limit)
            .fetch_all(&state.db).await?
    };
    let out: Vec<PatientRow> = rows.iter().map(|r| PatientRow {
        id: r.get("id"),
        mrn: r.get("mrn"),
        first_name: r.get("first_name"),
        last_name: r.get("last_name"),
        date_of_birth: r.get("date_of_birth"),
        gender: r.get("gender"),
        phone: r.get("phone"),
        email: r.get("email"),
        medicare_number: r.get("medicare_number"),
        created_at: r.get("created_at"),
        next_appointment: r.get("next_appointment"),
        last_appointment: r.get("last_appointment"),
        total_visits: r.get("total_visits"),
        total_spent: r.get("total_spent"),
        outstanding: r.get("outstanding"),
    }).collect();
    Ok(Json(out))
}
