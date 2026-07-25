//! Combined calendar view: appointments + blocked times for a date range.
//! Used by the frontend calendar and (later) pushed up to the Worker so the
//! website can show live availability.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use sqlx::Row;

use crate::error::ApiResult;
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct CalendarEvent {
    pub id: i64,
    pub kind: String, // "appointment" | "blocked"
    pub start_at: String,
    pub end_at: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub practitioner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Payment state for appointments: "paid" | "partial" | "unpaid" | None (blocked/n/a)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid: Option<String>,
    /// Amount still outstanding (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<f64>,
}

pub async fn range(
    State(state): State<AppState>,
    Path((from, to)): Path<(String, String)>,
) -> ApiResult<Json<Vec<CalendarEvent>>> {
    // Join appointments with their invoice payment status (if any invoice linked
    // to the patient for this appointment, or the patient's overall balance).
    let appts = sqlx::query(
        "SELECT a.id, a.appointment_date, a.duration_minutes, a.status, a.practitioner,
                a.patient_id, p.first_name, p.last_name,
                (SELECT CAST(COALESCE(SUM(i.balance_due),0) AS REAL) FROM invoices i
                 WHERE i.patient_id = a.patient_id AND i.appointment_id = a.id) AS appt_balance,
                (SELECT COUNT(*) FROM invoices i
                 WHERE i.patient_id = a.patient_id AND i.appointment_id = a.id) AS has_invoice
         FROM appointments a JOIN patients p ON a.patient_id = p.id
         WHERE a.appointment_date BETWEEN ? AND ?
         ORDER BY a.appointment_date",
    )
    .bind(&from)
    .bind(&to)
    .fetch_all(&state.db)
    .await?;

    let mut events: Vec<CalendarEvent> = appts
        .iter()
        .map(|r| {
            let start: String = r.get("appointment_date");
            let dur: i64 = r.get("duration_minutes");
            let first: String = r.get("first_name");
            let last: String = r.get("last_name");
            let end = shift_minutes(&start, dur);
            let has_invoice: i64 = r.get("has_invoice");
            let appt_balance: f64 = r.get("appt_balance");
            let paid = if has_invoice == 0 {
                None // no invoice yet — not billed
            } else if appt_balance <= 0.01 {
                Some("paid".into())
            } else {
                Some("partial".into())
            };
            let balance = if appt_balance > 0.01 { Some(appt_balance) } else { None };
            CalendarEvent {
                id: r.get("id"),
                kind: "appointment".into(),
                start_at: start,
                end_at: end,
                title: format!("{} {}", first, last),
                patient_id: Some(r.get("patient_id")),
                practitioner: r.try_get("practitioner").ok(),
                status: Some(r.get("status")),
                reason: None,
                paid,
                balance,
            }
        })
        .collect();

    let blocked = sqlx::query(
        "SELECT id, start_at, end_at, reason, practitioner FROM blocked_times
         WHERE start_at BETWEEN ? AND ? ORDER BY start_at",
    )
    .bind(&from)
    .bind(&to)
    .fetch_all(&state.db)
    .await?;

    for r in &blocked {
        events.push(CalendarEvent {
            id: r.get("id"),
            kind: "blocked".into(),
            start_at: r.get("start_at"),
            end_at: r.get("end_at"),
            title: r.try_get::<String, _>("reason").ok().unwrap_or_else(|| "Blocked".into()),
            patient_id: None,
            practitioner: r.try_get("practitioner").ok(),
            status: None,
            reason: r.try_get("reason").ok(),
            paid: None,
            balance: None,
        });
    }

    Ok(Json(events))
}

fn shift_minutes(dt: &str, mins: i64) -> String {
    if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(dt, "%Y-%m-%d %H:%M:%S") {
        return (parsed + chrono::Duration::minutes(mins)).format("%Y-%m-%d %H:%M:%S").to_string();
    }
    dt.to_string()
}
