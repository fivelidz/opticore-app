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
        claimed_returning: r
            .try_get::<Option<i64>, _>("claimed_returning")
            .ok()
            .flatten()
            .map(|v| v != 0),
        claimed_no_match: r
            .try_get::<Option<i64>, _>("claimed_no_match")
            .ok()
            .flatten()
            .map(|v| v != 0)
            .unwrap_or(false),
    }
}

/// PUBLIC: submit an intake form. No auth required.
pub async fn submit(State(state): State<AppState>, Json(b): Json<CreateIntake>) -> ApiResult<axum::response::Response> {
    // If the patient claims to be a returning/existing patient, check whether an
    // exact record actually exists. If not, flag the submission so staff see
    // "claims to be existing patient — no record found".
    let claimed_returning_int: Option<i64> = b.claimed_returning.map(|v| if v { 1 } else { 0 });
    let mut claimed_no_match = 0i64;
    if b.claimed_returning == Some(true) {
        let exact = exact_match_patient(
            &state,
            &b.first_name,
            &b.last_name,
            b.date_of_birth.as_deref(),
            b.phone.as_deref(),
            b.email.as_deref(),
        )
        .await?;
        if exact.is_none() {
            claimed_no_match = 1;
        }
    }

    let r = sqlx::query(
        "INSERT INTO intake_submissions
         (first_name, last_name, date_of_birth, phone, email, address, medicare_number,
          preferred_date, preferred_time, appointment_type, symptoms, source,
          claimed_returning, claimed_no_match)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&b.first_name).bind(&b.last_name).bind(&b.date_of_birth)
        .bind(&b.phone).bind(&b.email).bind(&b.address).bind(&b.medicare_number)
        .bind(&b.preferred_date).bind(&b.preferred_time).bind(&b.appointment_type)
        .bind(&b.symptoms).bind(b.source.unwrap_or_else(|| "input-page".into()))
        .bind(claimed_returning_int).bind(claimed_no_match)
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

// ============================================================================
// Patient matching: exact match + near-match / possible-duplicate detection.
// ============================================================================

/// Normalise a phone number for comparison (strip spaces, dashes, parens, +).
fn norm_phone(p: &str) -> String {
    p.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// A lightweight patient record used in match results.
#[derive(Debug, serde::Serialize)]
pub struct MatchCandidate {
    pub patient_id: i64,
    pub mrn: String,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    /// Which fields differ from the intake submission (e.g. "date_of_birth").
    pub differing_fields: Vec<String>,
    /// Human-readable reason this is flagged as a possible match.
    pub reason: String,
}

/// Find an EXACT match for the given details. Same logic priority as the public
/// match-patient endpoint (name+DOB strongest, then phone, then email).
/// Returns the patient id if a confident exact match exists.
pub(crate) async fn exact_match_patient(
    state: &AppState,
    first_name: &str,
    last_name: &str,
    dob: Option<&str>,
    phone: Option<&str>,
    email: Option<&str>,
) -> ApiResult<Option<i64>> {
    // name + DOB
    if let Some(dob) = dob.filter(|d| !d.is_empty()) {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM patients WHERE LOWER(first_name) = LOWER(?) AND LOWER(last_name) = LOWER(?) AND date_of_birth = ?")
            .bind(first_name).bind(last_name).bind(dob)
            .fetch_optional(&state.db).await?;
        if let Some((id,)) = row { return Ok(Some(id)); }
    }
    // exact phone
    if let Some(phone) = phone.filter(|p| !p.is_empty()) {
        let clean = norm_phone(phone);
        if !clean.is_empty() {
            let row: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM patients WHERE REPLACE(REPLACE(REPLACE(REPLACE(phone,' ',''),'-',''),'(',''),')','') = ?")
                .bind(&clean).fetch_optional(&state.db).await?;
            if let Some((id,)) = row { return Ok(Some(id)); }
        }
    }
    // exact email
    if let Some(email) = email.filter(|e| !e.is_empty()) {
        let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM patients WHERE LOWER(email) = LOWER(?)")
            .bind(email).fetch_optional(&state.db).await?;
        if let Some((id,)) = row { return Ok(Some(id)); }
    }
    Ok(None)
}

/// Find NEAR matches (possible duplicates): patients that are SIMILAR but not an
/// identical match, so a human can decide MERGE vs create-new. Detects:
///   - same first+last name but a DIFFERENT date_of_birth
///   - same phone number but a DIFFERENT name
///   - same email but a DIFFERENT name
pub(crate) async fn near_match_patients(
    state: &AppState,
    exact_id: Option<i64>,
    first_name: &str,
    last_name: &str,
    dob: Option<&str>,
    phone: Option<&str>,
    email: Option<&str>,
) -> ApiResult<Vec<MatchCandidate>> {
    use std::collections::HashMap;
    // patient_id -> candidate (dedup, merge reasons/differing fields)
    let mut found: HashMap<i64, MatchCandidate> = HashMap::new();

    let dob = dob.filter(|d| !d.is_empty());
    let phone_clean = phone.filter(|p| !p.is_empty()).map(norm_phone).filter(|p| !p.is_empty());
    let email = email.filter(|e| !e.is_empty());

    let mut push = |cand: MatchCandidate| {
        let entry = found.entry(cand.patient_id).or_insert_with(|| MatchCandidate {
            patient_id: cand.patient_id,
            mrn: cand.mrn.clone(),
            first_name: cand.first_name.clone(),
            last_name: cand.last_name.clone(),
            date_of_birth: cand.date_of_birth.clone(),
            phone: cand.phone.clone(),
            email: cand.email.clone(),
            differing_fields: Vec::new(),
            reason: String::new(),
        });
        for f in cand.differing_fields {
            if !entry.differing_fields.contains(&f) {
                entry.differing_fields.push(f);
            }
        }
        if entry.reason.is_empty() {
            entry.reason = cand.reason;
        } else if !cand.reason.is_empty() {
            entry.reason = format!("{}; {}", entry.reason, cand.reason);
        }
    };

    // ---- same name, different DOB ----
    {
        let rows = sqlx::query(
            "SELECT id, mrn, first_name, last_name, date_of_birth, phone, email
             FROM patients
             WHERE LOWER(first_name) = LOWER(?) AND LOWER(last_name) = LOWER(?)")
            .bind(first_name).bind(last_name)
            .fetch_all(&state.db).await?;
        for r in &rows {
            let pid: i64 = r.get("id");
            if Some(pid) == exact_id { continue; }
            let pdob: Option<String> = r.get("date_of_birth");
            // Only a near-match if DOB differs (or intake gave no DOB to compare).
            let dob_differs = match (dob, pdob.as_deref()) {
                (Some(a), Some(b)) => a != b,
                _ => true, // one side missing => treat as "needs review"
            };
            if dob_differs {
                push(MatchCandidate {
                    patient_id: pid,
                    mrn: r.get("mrn"),
                    first_name: r.get("first_name"),
                    last_name: r.get("last_name"),
                    date_of_birth: pdob.clone(),
                    phone: r.get("phone"),
                    email: r.get("email"),
                    differing_fields: vec!["date_of_birth".into()],
                    reason: format!(
                        "Same name but different date of birth (form: {}, record: {})",
                        dob.unwrap_or("—"),
                        pdob.as_deref().unwrap_or("—"),
                    ),
                });
            }
        }
    }

    // ---- same phone, different name ----
    if let Some(clean) = &phone_clean {
        let rows = sqlx::query(
            "SELECT id, mrn, first_name, last_name, date_of_birth, phone, email
             FROM patients
             WHERE REPLACE(REPLACE(REPLACE(REPLACE(phone,' ',''),'-',''),'(',''),')','') = ?")
            .bind(clean)
            .fetch_all(&state.db).await?;
        for r in &rows {
            let pid: i64 = r.get("id");
            if Some(pid) == exact_id { continue; }
            let pf: String = r.get("first_name");
            let pl: String = r.get("last_name");
            let name_differs = !pf.eq_ignore_ascii_case(first_name) || !pl.eq_ignore_ascii_case(last_name);
            if name_differs {
                push(MatchCandidate {
                    patient_id: pid,
                    mrn: r.get("mrn"),
                    first_name: pf.clone(),
                    last_name: pl.clone(),
                    date_of_birth: r.get("date_of_birth"),
                    phone: r.get("phone"),
                    email: r.get("email"),
                    differing_fields: vec!["first_name".into(), "last_name".into()],
                    reason: format!(
                        "Same phone but different name (form: {} {}, record: {} {})",
                        first_name, last_name, pf, pl,
                    ),
                });
            }
        }
    }

    // ---- same email, different name ----
    if let Some(email) = email {
        let rows = sqlx::query(
            "SELECT id, mrn, first_name, last_name, date_of_birth, phone, email
             FROM patients
             WHERE LOWER(email) = LOWER(?)")
            .bind(email)
            .fetch_all(&state.db).await?;
        for r in &rows {
            let pid: i64 = r.get("id");
            if Some(pid) == exact_id { continue; }
            let pf: String = r.get("first_name");
            let pl: String = r.get("last_name");
            let name_differs = !pf.eq_ignore_ascii_case(first_name) || !pl.eq_ignore_ascii_case(last_name);
            if name_differs {
                push(MatchCandidate {
                    patient_id: pid,
                    mrn: r.get("mrn"),
                    first_name: pf.clone(),
                    last_name: pl.clone(),
                    date_of_birth: r.get("date_of_birth"),
                    phone: r.get("phone"),
                    email: r.get("email"),
                    differing_fields: vec!["first_name".into(), "last_name".into()],
                    reason: format!(
                        "Same email but different name (form: {} {}, record: {} {})",
                        first_name, last_name, pf, pl,
                    ),
                });
            }
        }
    }

    Ok(found.into_values().collect())
}

/// PROTECTED: POST /api/intake/:id/match-check
/// Returns whether the intake exactly matches an existing patient, plus any
/// near-matches (possible duplicates) so the frontend can show a merge/flag UI
/// BEFORE approving.
pub async fn match_check(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let row = sqlx::query("SELECT * FROM intake_submissions WHERE id = ?")
        .bind(id).fetch_optional(&state.db).await?.ok_or(ApiError::NotFound)?;
    let sub = row_to_intake(&row);

    let exact_id = exact_match_patient(
        &state,
        &sub.first_name,
        &sub.last_name,
        sub.date_of_birth.as_deref(),
        sub.phone.as_deref(),
        sub.email.as_deref(),
    )
    .await?;

    let near = near_match_patients(
        &state,
        exact_id,
        &sub.first_name,
        &sub.last_name,
        sub.date_of_birth.as_deref(),
        sub.phone.as_deref(),
        sub.email.as_deref(),
    )
    .await?;

    // Keep the "claims existing but no record found" flag current.
    let claimed_no_match = sub.claimed_returning == Some(true) && exact_id.is_none();
    sqlx::query("UPDATE intake_submissions SET claimed_no_match = ? WHERE id = ?")
        .bind(if claimed_no_match { 1i64 } else { 0i64 })
        .bind(id)
        .execute(&state.db).await?;

    Ok(Json(serde_json::json!({
        "intake_id": id,
        "exact_match": exact_id.is_some(),
        "exact_patient_id": exact_id,
        "claimed_returning": sub.claimed_returning,
        "claimed_no_match": claimed_no_match,
        "near_matches": near,
    })))
}

/// PROTECTED: POST /api/intake/:id/merge-into/:patient_id
/// Instead of creating a NEW patient, attach this intake to an EXISTING patient:
///   - update the existing patient's contact details where the intake has a
///     value the record is missing (never blindly overwrite existing data),
///   - create the appointment (if a preferred date was given) for that patient,
///   - mark the intake as imported and link matched_patient_id.
pub async fn merge_into(
    State(state): State<AppState>,
    Path((id, patient_id)): Path<(i64, i64)>,
) -> ApiResult<Json<serde_json::Value>> {
    let row = sqlx::query("SELECT * FROM intake_submissions WHERE id = ?")
        .bind(id).fetch_optional(&state.db).await?.ok_or(ApiError::NotFound)?;
    let sub = row_to_intake(&row);

    // ensure target patient exists
    let prow = sqlx::query("SELECT * FROM patients WHERE id = ?")
        .bind(patient_id).fetch_optional(&state.db).await?.ok_or(ApiError::NotFound)?;

    // --- update contact details where the record is missing info ---
    let mut updated_fields: Vec<String> = Vec::new();
    macro_rules! fill_if_missing {
        ($col:literal, $intake:expr) => {{
            let cur: Option<String> = prow.get($col);
            let has_new = $intake.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
            let record_empty = cur.as_ref().map(|s| s.is_empty()).unwrap_or(true);
            if has_new && record_empty {
                sqlx::query(concat!("UPDATE patients SET ", $col, " = ? WHERE id = ?"))
                    .bind($intake.as_deref())
                    .bind(patient_id)
                    .execute(&state.db).await?;
                updated_fields.push($col.to_string());
            }
        }};
    }
    fill_if_missing!("phone", sub.phone);
    fill_if_missing!("email", sub.email);
    fill_if_missing!("address", sub.address);
    fill_if_missing!("medicare_number", sub.medicare_number);

    // --- create the appointment for the existing patient ---
    let mut appointment_id: Option<i64> = None;
    if let Some(date) = sub.preferred_date.as_ref().filter(|d| !d.is_empty()) {
        let time = sub.preferred_time.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "09:00".into());
        let dt = format!("{} {}:00", date, time);
        let atype = sub.appointment_type.clone().unwrap_or_else(|| "Dry Eye Consultation".into());
        let ar = sqlx::query(
            "INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes, status, notes)
             VALUES (?, ?, ?, 60, 'scheduled', ?)")
            .bind(patient_id).bind(&atype).bind(&dt).bind(&sub.symptoms)
            .execute(&state.db).await?;
        appointment_id = Some(ar.last_insert_rowid());
    }

    // --- mark intake imported + linked, clear the no-match flag ---
    sqlx::query("UPDATE intake_submissions SET status = 'imported', matched_patient_id = ?, claimed_no_match = 0 WHERE id = ?")
        .bind(patient_id).bind(id).execute(&state.db).await?;

    let mrn: String = prow.get("mrn");
    Ok(Json(serde_json::json!({
        "success": true,
        "merged_into_patient_id": patient_id,
        "mrn": mrn,
        "appointment_id": appointment_id,
        "updated_fields": updated_fields,
    })))
}
