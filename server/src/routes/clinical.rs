use axum::{
    extract::{Path, State},
    Json,
};
use chrono::NaiveDate;
use shared::{
    Allergy, ClinicalNote, CreateAllergy, CreateNote, CreateOsdi, CreateIpl, IplTreatment, OsdiScore,
};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

// ---------- Clinical notes ----------

pub async fn list_notes(State(state): State<AppState>, Path(pid): Path<i64>) -> ApiResult<Json<Vec<ClinicalNote>>> {
    let rows = sqlx::query("SELECT * FROM clinical_notes WHERE patient_id = ? ORDER BY created_at DESC")
        .bind(pid)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(|r| ClinicalNote {
        id: r.get("id"), patient_id: r.get("patient_id"), author: r.get("author"),
        category: r.get("category"), note: r.get("note"), created_at: r.get("created_at"),
    }).collect()))
}

pub async fn add_note(State(state): State<AppState>, Json(b): Json<CreateNote>) -> ApiResult<axum::response::Response> {
    // The `note` column is TEXT NOT NULL, but that still accepts the empty
    // string. An empty (or whitespace-only) clinical note is meaningless, so
    // reject it before insert.
    if b.note.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "note must not be empty".into(),
        ));
    }
    let r = sqlx::query("INSERT INTO clinical_notes (patient_id, author, category, note) VALUES (?, ?, ?, ?)")
        .bind(b.patient_id).bind(&b.author).bind(&b.category).bind(&b.note)
        .execute(&state.db).await?;
    let id = r.last_insert_rowid();
    let row = sqlx::query("SELECT * FROM clinical_notes WHERE id = ?").bind(id).fetch_one(&state.db).await?;
    use axum::response::IntoResponse;
    Ok((axum::http::StatusCode::CREATED, Json(ClinicalNote {
        id: row.get("id"), patient_id: row.get("patient_id"), author: row.get("author"),
        category: row.get("category"), note: row.get("note"), created_at: row.get("created_at"),
    })).into_response())
}

pub async fn del_note(State(state): State<AppState>, Path((_pid, nid)): Path<(i64, i64)>) -> ApiResult<Json<shared::MessageResponse>> {
    sqlx::query("DELETE FROM clinical_notes WHERE id = ?").bind(nid).execute(&state.db).await?;
    Ok(Json(shared::MessageResponse { message: "Note deleted".into() }))
}

// ---------- Allergies ----------

pub async fn list_allergies(State(state): State<AppState>, Path(pid): Path<i64>) -> ApiResult<Json<Vec<Allergy>>> {
    let rows = sqlx::query("SELECT * FROM allergies WHERE patient_id = ? ORDER BY noted_at DESC")
        .bind(pid).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(|r| Allergy {
        id: r.get("id"), patient_id: r.get("patient_id"), substance: r.get("substance"),
        severity: r.get("severity"), noted_at: r.get("noted_at"),
    }).collect()))
}

pub async fn add_allergy(State(state): State<AppState>, Json(b): Json<CreateAllergy>) -> ApiResult<Json<Allergy>> {
    // `substance` is VARCHAR(200) NOT NULL, but that accepts the empty string.
    // An allergy with no substance is meaningless.
    if b.substance.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "substance must not be empty".into(),
        ));
    }
    let r = sqlx::query("INSERT INTO allergies (patient_id, substance, severity) VALUES (?, ?, ?)")
        .bind(b.patient_id).bind(&b.substance).bind(&b.severity).execute(&state.db).await?;
    let id = r.last_insert_rowid();
    let row = sqlx::query("SELECT * FROM allergies WHERE id = ?").bind(id).fetch_one(&state.db).await?;
    Ok(Json(Allergy {
        id: row.get("id"), patient_id: row.get("patient_id"), substance: row.get("substance"),
        severity: row.get("severity"), noted_at: row.get("noted_at"),
    }))
}

pub async fn del_allergy(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<shared::MessageResponse>> {
    sqlx::query("DELETE FROM allergies WHERE id = ?").bind(id).execute(&state.db).await?;
    Ok(Json(shared::MessageResponse { message: "Allergy removed".into() }))
}

// ---------- OSDI ----------

pub async fn list_osdi(State(state): State<AppState>, Path(pid): Path<i64>) -> ApiResult<Json<Vec<OsdiScore>>> {
    let rows = sqlx::query("SELECT * FROM osdi_scores WHERE patient_id = ? ORDER BY score_date DESC")
        .bind(pid).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(|r| OsdiScore {
        id: r.get("id"), patient_id: r.get("patient_id"), score_date: r.get("score_date"),
        total_score: r.get("total_score"), ocular_symptoms: r.get("ocular_symptoms"),
        vision_function: r.get("vision_function"), environmental_triggers: r.get("environmental_triggers"),
        created_at: r.get("created_at"),
    }).collect()))
}

pub async fn add_osdi(State(state): State<AppState>, Json(b): Json<CreateOsdi>) -> ApiResult<Json<OsdiScore>> {
    // OSDI total_score is a severity score in the range [0, 100]. Negative
    // values are meaningless. (We do not upper-bound at 100 here because the
    // raw-sum subscores can legitimately exceed 100 before normalization; the
    // total itself is the practitioner's responsibility. The lower bound is
    // unambiguous.)
    if b.total_score < 0.0 || b.total_score.is_nan() {
        return Err(ApiError::BadRequest(
            "total_score must be a non-negative number".into(),
        ));
    }
    // Subscores (ocular_symptoms, vision_function, environmental_triggers) are
    // also severity scores. A negative subscore is meaningless and would
    // distort the total. Validate each provided subscore's lower bound.
    for (label, val) in [
        ("ocular_symptoms", b.ocular_symptoms),
        ("vision_function", b.vision_function),
        ("environmental_triggers", b.environmental_triggers),
    ] {
        if let Some(v) = val {
            if v < 0.0 || v.is_nan() {
                return Err(ApiError::BadRequest(format!(
                    "{label} must be a non-negative number"
                )));
            }
        }
    }
    // Normalize score_date so it is stored in a consistent format. Previously
    // the raw string was bound verbatim, so a garbage value like "not-a-date"
    // was silently stored. normalize_dt returns the input unchanged on parse
    // failure, so we validate first: reject anything that doesn't parse as
    // RFC3339 or YYYY-MM-DD.
    if shared::parse_dt(&b.score_date).is_none()
        && NaiveDate::parse_from_str(&b.score_date, "%Y-%m-%d").is_err()
    {
        return Err(ApiError::BadRequest(format!(
            "score_date '{}' is not a valid date (expected RFC3339 or YYYY-MM-DD)",
            b.score_date
        )));
    }
    let dt = shared::normalize_dt(&b.score_date);
    let r = sqlx::query("INSERT INTO osdi_scores (patient_id, score_date, total_score, ocular_symptoms, vision_function, environmental_triggers) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(b.patient_id).bind(&dt).bind(b.total_score)
        .bind(b.ocular_symptoms).bind(b.vision_function).bind(b.environmental_triggers)
        .execute(&state.db).await?;
    let id = r.last_insert_rowid();
    let row = sqlx::query("SELECT * FROM osdi_scores WHERE id = ?").bind(id).fetch_one(&state.db).await?;
    Ok(Json(OsdiScore {
        id: row.get("id"), patient_id: row.get("patient_id"), score_date: row.get("score_date"),
        total_score: row.get("total_score"), ocular_symptoms: row.get("ocular_symptoms"),
        vision_function: row.get("vision_function"), environmental_triggers: row.get("environmental_triggers"),
        created_at: row.get("created_at"),
    }))
}

// ---------- IPL ----------

pub async fn list_ipl(State(state): State<AppState>, Path(pid): Path<i64>) -> ApiResult<Json<Vec<IplTreatment>>> {
    let rows = sqlx::query("SELECT * FROM ipl_treatments WHERE patient_id = ? ORDER BY treatment_date DESC")
        .bind(pid).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(|r| IplTreatment {
        id: r.get("id"), patient_id: r.get("patient_id"), treatment_date: r.get("treatment_date"),
        session_number: r.get("session_number"), fluence_j_cm2: r.get("fluence_j_cm2"),
        number_of_pulses: r.get("number_of_pulses"), operator_name: r.get("operator_name"),
        clinical_notes: r.get("clinical_notes"), created_at: r.get("created_at"),
    }).collect()))
}

pub async fn add_ipl(State(state): State<AppState>, Json(b): Json<CreateIpl>) -> ApiResult<Json<IplTreatment>> {
    // Treatment sessions are 1-indexed; session_number < 1 is nonsensical.
    if b.session_number < 1 {
        return Err(ApiError::BadRequest(
            "session_number must be >= 1".into(),
        ));
    }
    // fluence_j_cm2 (energy fluence in J/cm²) must be non-negative. A negative
    // energy value is physically meaningless and would corrupt treatment
    // records / analytics. NaN is also rejected (Infinity cannot reach here
    // via standard JSON, but we guard for defense-in-depth).
    if let Some(f) = b.fluence_j_cm2 {
        if f < 0.0 || f.is_nan() || f.is_infinite() {
            return Err(ApiError::BadRequest(
                "fluence_j_cm2 must be a non-negative finite number".into(),
            ));
        }
    }
    // number_of_pulses must be a positive integer when provided. Zero or
    // negative pulses is nonsensical for a treatment record (a treatment with
    // no pulses did not happen).
    if let Some(p) = b.number_of_pulses {
        if p < 1 {
            return Err(ApiError::BadRequest(
                "number_of_pulses must be >= 1".into(),
            ));
        }
    }
    // Validate treatment_date: normalize_dt returns the raw string verbatim on
    // parse failure, so a garbage value like "not-a-date" would be silently
    // stored. Reject anything that doesn't parse as RFC3339 or YYYY-MM-DD.
    // (We do NOT reject past dates here — unlike appointments, IPL treatment
    // records are historical: staff routinely record a treatment that already
    // happened.)
    if shared::parse_dt(&b.treatment_date).is_none()
        && NaiveDate::parse_from_str(&b.treatment_date, "%Y-%m-%d").is_err()
    {
        return Err(ApiError::BadRequest(format!(
            "treatment_date '{}' is not a valid date (expected RFC3339 or YYYY-MM-DD)",
            b.treatment_date
        )));
    }
    let dt = shared::normalize_dt(&b.treatment_date);
    let r = sqlx::query("INSERT INTO ipl_treatments (patient_id, treatment_date, session_number, fluence_j_cm2, number_of_pulses, operator_name, clinical_notes) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(b.patient_id).bind(&dt).bind(b.session_number).bind(b.fluence_j_cm2)
        .bind(b.number_of_pulses).bind(&b.operator_name).bind(&b.clinical_notes)
        .execute(&state.db).await?;
    let id = r.last_insert_rowid();
    let row = sqlx::query("SELECT * FROM ipl_treatments WHERE id = ?").bind(id).fetch_one(&state.db).await?;
    Ok(Json(IplTreatment {
        id: row.get("id"), patient_id: row.get("patient_id"), treatment_date: row.get("treatment_date"),
        session_number: row.get("session_number"), fluence_j_cm2: row.get("fluence_j_cm2"),
        number_of_pulses: row.get("number_of_pulses"), operator_name: row.get("operator_name"),
        clinical_notes: row.get("clinical_notes"), created_at: row.get("created_at"),
    }))
}
