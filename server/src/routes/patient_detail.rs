//! Patient detail aggregate: everything about one patient in one call.
//! Used by the deep patient detail view.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;
use shared::{Allergy, Appointment, ClinicalNote, Invoice, IplTreatment, OsdiScore, Patient};

#[derive(Debug, Serialize)]
pub struct PatientDetail {
    pub patient: Patient,
    pub appointments: Vec<Appointment>,
    pub notes: Vec<ClinicalNote>,
    pub allergies: Vec<Allergy>,
    pub osdi_scores: Vec<OsdiScore>,
    pub ipl_treatments: Vec<IplTreatment>,
    pub invoices: Vec<Invoice>,
    pub stats: PatientStats,
}

#[derive(Debug, Serialize)]
pub struct PatientStats {
    pub total_visits: i64,
    pub last_visit: Option<String>,
    pub total_spent: f64,
    pub outstanding: f64,
    pub first_visit: Option<String>,
}

pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<PatientDetail>> {
    // patient
    let prow = sqlx::query("SELECT * FROM patients WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let patient = shared::Patient {
        id: prow.get("id"),
        mrn: prow.get("mrn"),
        first_name: prow.get("first_name"),
        last_name: prow.get("last_name"),
        date_of_birth: prow.get("date_of_birth"),
        gender: prow.get("gender"),
        phone: prow.get("phone"),
        email: prow.get("email"),
        address: prow.get("address"),
        medicare_number: prow.get("medicare_number"),
        created_at: prow.get("created_at"),
        updated_at: prow.get("updated_at"),
    };

    // appointments
    let arows = sqlx::query(
        "SELECT a.*, p.first_name, p.last_name, p.phone, p.mrn
         FROM appointments a JOIN patients p ON a.patient_id = p.id
         WHERE a.patient_id = ? ORDER BY a.appointment_date DESC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let appointments = arows
        .iter()
        .map(|r| Appointment {
            id: r.get("id"),
            patient_id: r.get("patient_id"),
            appointment_type: r.get("appointment_type"),
            appointment_date: r.get("appointment_date"),
            duration_minutes: r.get("duration_minutes"),
            practitioner: r.get("practitioner"),
            status: r.get("status"),
            notes: r.get("notes"),
            created_at: r.get("created_at"),
            first_name: r.try_get("first_name").ok(),
            last_name: r.try_get("last_name").ok(),
            phone: r.try_get("phone").ok(),
            mrn: r.try_get("mrn").ok(),
        })
        .collect();

    // notes
    let nrows = sqlx::query("SELECT * FROM clinical_notes WHERE patient_id = ? ORDER BY created_at DESC")
        .bind(id)
        .fetch_all(&state.db)
        .await?;
    let notes = nrows
        .iter()
        .map(|r| ClinicalNote {
            id: r.get("id"),
            patient_id: r.get("patient_id"),
            author: r.get("author"),
            category: r.get("category"),
            note: r.get("note"),
            created_at: r.get("created_at"),
        })
        .collect();

    // allergies
    let alrows = sqlx::query("SELECT * FROM allergies WHERE patient_id = ? ORDER BY noted_at DESC")
        .bind(id)
        .fetch_all(&state.db)
        .await?;
    let allergies = alrows
        .iter()
        .map(|r| Allergy {
            id: r.get("id"),
            patient_id: r.get("patient_id"),
            substance: r.get("substance"),
            severity: r.get("severity"),
            noted_at: r.get("noted_at"),
        })
        .collect();

    // osdi
    let orows = sqlx::query("SELECT * FROM osdi_scores WHERE patient_id = ? ORDER BY score_date DESC")
        .bind(id)
        .fetch_all(&state.db)
        .await?;
    let osdi_scores = orows
        .iter()
        .map(|r| OsdiScore {
            id: r.get("id"),
            patient_id: r.get("patient_id"),
            score_date: r.get("score_date"),
            total_score: r.get("total_score"),
            ocular_symptoms: r.get("ocular_symptoms"),
            vision_function: r.get("vision_function"),
            environmental_triggers: r.get("environmental_triggers"),
            created_at: r.get("created_at"),
        })
        .collect();

    // ipl
    let irows = sqlx::query("SELECT * FROM ipl_treatments WHERE patient_id = ? ORDER BY treatment_date DESC")
        .bind(id)
        .fetch_all(&state.db)
        .await?;
    let ipl_treatments = irows
        .iter()
        .map(|r| IplTreatment {
            id: r.get("id"),
            patient_id: r.get("patient_id"),
            treatment_date: r.get("treatment_date"),
            session_number: r.get("session_number"),
            fluence_j_cm2: r.get("fluence_j_cm2"),
            number_of_pulses: r.get("number_of_pulses"),
            operator_name: r.get("operator_name"),
            clinical_notes: r.get("clinical_notes"),
            created_at: r.get("created_at"),
        })
        .collect();

    // invoices + items
    let invrows = sqlx::query("SELECT * FROM invoices WHERE patient_id = ? ORDER BY invoice_date DESC")
        .bind(id)
        .fetch_all(&state.db)
        .await?;
    let mut invoices: Vec<Invoice> = Vec::new();
    for ir in &invrows {
        let inv_id: i64 = ir.get("id");
        let item_rows = sqlx::query("SELECT * FROM invoice_items WHERE invoice_id = ?")
            .bind(inv_id)
            .fetch_all(&state.db)
            .await?;
        let items = item_rows
            .iter()
            .map(|r| shared::InvoiceItem {
                id: r.get("id"),
                invoice_id: r.get("invoice_id"),
                item_type: r.get("item_type"),
                description: r.get("description"),
                quantity: r.get("quantity"),
                unit_price: r.get("unit_price"),
                discount_percent: r.get("discount_percent"),
                tax_rate: r.get("tax_rate"),
                total: r.get("total"),
            })
            .collect();
        invoices.push(Invoice {
            id: inv_id,
            invoice_number: ir.get("invoice_number"),
            patient_id: ir.get("patient_id"),
            appointment_id: ir.get("appointment_id"),
            invoice_date: ir.get("invoice_date"),
            due_date: ir.get("due_date"),
            subtotal: ir.get("subtotal"),
            tax_amount: ir.get("tax_amount"),
            discount_amount: ir.get("discount_amount"),
            total_amount: ir.get("total_amount"),
            amount_paid: ir.get("amount_paid"),
            balance_due: ir.get("balance_due"),
            status: ir.get("status"),
            payment_method: ir.get("payment_method"),
            notes: ir.get("notes"),
            created_at: ir.get("created_at"),
            items,
        });
    }

    // stats
    let srow = sqlx::query(
        "SELECT
           COUNT(*) AS total_visits,
           MIN(appointment_date) AS first_visit,
           MAX(appointment_date) AS last_visit
         FROM appointments WHERE patient_id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    let total_visits: i64 = srow.get("total_visits");
    let first_visit: Option<String> = srow.get("first_visit");
    let last_visit: Option<String> = srow.get("last_visit");

    // NOTE: CAST(... AS REAL) — COALESCE(SUM(...),0) returns SQL type INTEGER
    // when no rows match (the literal 0 has INTEGER affinity), which sqlx
    // cannot decode as f64. Same bug class as analytics::overview.
    let mrow = sqlx::query("SELECT CAST(COALESCE(SUM(amount_paid),0) AS REAL) AS spent, CAST(COALESCE(SUM(balance_due),0) AS REAL) AS outstanding FROM invoices WHERE patient_id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    let total_spent: f64 = mrow.get("spent");
    let outstanding: f64 = mrow.get("outstanding");

    Ok(Json(PatientDetail {
        patient,
        appointments,
        notes,
        allergies,
        osdi_scores,
        ipl_treatments,
        invoices,
        stats: PatientStats {
            total_visits,
            last_visit,
            total_spent,
            outstanding,
            first_visit,
        },
    }))
}
