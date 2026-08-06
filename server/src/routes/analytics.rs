//! Analytics aggregates for the dashboard + analytics page.

use axum::{
    extract::{Path, State},
    Json,
};
use shared::{
    AgeBracket, AnalyticsOverview, HourCount, NoShowRate, OutstandingPatient, RevenueByType,
    SourceBreakdown, TimeSeriesPoint, WebsiteTrafficPoint,
};
use sqlx::Row;

use crate::error::ApiResult;
use crate::AppState;

pub async fn overview(State(state): State<AppState>) -> ApiResult<Json<AnalyticsOverview>> {
    let p: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM patients").fetch_one(&state.db).await?;
    let a: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM appointments").fetch_one(&state.db).await?;
    // NOTE: CAST(... AS REAL) is required — COALESCE(SUM(...),0) returns SQL type
    // INTEGER when the table is empty (the literal 0 has INTEGER affinity), which
    // sqlx cannot decode as f64, causing a 500 on fresh/empty databases.
    let rev: (f64,) = sqlx::query_as("SELECT CAST(COALESCE(SUM(amount_paid),0) AS REAL) FROM invoices").fetch_one(&state.db).await?;
    let out: (f64,) = sqlx::query_as("SELECT CAST(COALESCE(SUM(balance_due),0) AS REAL) FROM invoices WHERE status IN ('issued','partially_paid','overdue')").fetch_one(&state.db).await?;
    let am: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM appointments WHERE appointment_date >= date('now','start of month')").fetch_one(&state.db).await?;
    let rm: (f64,) = sqlx::query_as("SELECT CAST(COALESCE(SUM(amount_paid),0) AS REAL) FROM invoices WHERE invoice_date >= date('now','start of month')").fetch_one(&state.db).await?;
    let avg = if a.0 > 0 { rev.0 / a.0 as f64 } else { 0.0 };
    Ok(Json(AnalyticsOverview {
        total_patients: p.0,
        total_appointments: a.0,
        total_revenue: rev.0,
        outstanding_balance: out.0,
        appointments_this_month: am.0,
        revenue_this_month: rm.0,
        avg_appt_value: avg,
    }))
}

/// Revenue per day for the last N days (default 30).
pub async fn revenue_series(State(state): State<AppState>, Path(days): Path<i64>) -> ApiResult<Json<Vec<TimeSeriesPoint>>> {
    let rows = sqlx::query(
        "SELECT DATE(invoice_date) AS d, CAST(COALESCE(SUM(amount_paid),0) AS REAL) AS v
         FROM invoices
         WHERE invoice_date >= date('now', ?)
         GROUP BY d ORDER BY d")
        .bind(format!("-{days} days"))
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(|r| TimeSeriesPoint {
        date: r.get("d"), value: r.get("v"),
    }).collect()))
}

/// Appointments per day for the last N days.
pub async fn appointment_series(State(state): State<AppState>, Path(days): Path<i64>) -> ApiResult<Json<Vec<TimeSeriesPoint>>> {
    let rows = sqlx::query(
        "SELECT DATE(appointment_date) AS d, COUNT(*) AS v
         FROM appointments
         WHERE appointment_date >= date('now', ?)
         GROUP BY d ORDER BY d")
        .bind(format!("-{days} days"))
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(|r| TimeSeriesPoint {
        date: r.get("d"), value: r.get::<i64, _>("v") as f64,
    }).collect()))
}

/// Website traffic for the last N days.
pub async fn traffic_series(State(state): State<AppState>, Path(days): Path<i64>) -> ApiResult<Json<Vec<WebsiteTrafficPoint>>> {
    let rows = sqlx::query(
        "SELECT event_date, visitors, page_views, bookings, source
         FROM website_events
         WHERE event_date >= date('now', ?)
         ORDER BY event_date")
        .bind(format!("-{days} days"))
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(|r| WebsiteTrafficPoint {
        date: r.get("event_date"), visitors: r.get("visitors"), page_views: r.get("page_views"),
        bookings: r.get("bookings"), source: r.get("source"),
    }).collect()))
}

/// Traffic aggregated by source.
pub async fn traffic_by_source(State(state): State<AppState>) -> ApiResult<Json<Vec<SourceBreakdown>>> {
    let rows = sqlx::query(
        "SELECT source, COALESCE(SUM(visitors),0) AS visitors, COALESCE(SUM(bookings),0) AS bookings
         FROM website_events GROUP BY source ORDER BY visitors DESC")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(|r| SourceBreakdown {
        source: r.get("source"), visitors: r.get("visitors"), bookings: r.get("bookings"),
    }).collect()))
}

/// New patients per week for the last N days (grouped by ISO week start).
pub async fn patient_growth(State(state): State<AppState>, Path(days): Path<i64>) -> ApiResult<Json<Vec<TimeSeriesPoint>>> {
    // SQLite: bucket each created_at into the Monday of its week via date(created_at,'weekday 0','-6 days').
    let rows = sqlx::query(
        "SELECT date(created_at, 'weekday 0', '-6 days') AS wk, COUNT(*) AS v
         FROM patients
         WHERE created_at >= date('now', ?)
         GROUP BY wk ORDER BY wk")
        .bind(format!("-{days} days"))
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(|r| TimeSeriesPoint {
        date: r.get("wk"), value: r.get::<i64, _>("v") as f64,
    }).collect()))
}

/// Revenue (amount paid) broken down by appointment_type.
/// Joins invoices -> appointments; invoices without a linked appointment are grouped as 'Unlinked'.
pub async fn revenue_by_type(State(state): State<AppState>) -> ApiResult<Json<Vec<RevenueByType>>> {
    let rows = sqlx::query(
        "SELECT COALESCE(a.appointment_type, 'Unlinked') AS t,
                CAST(COALESCE(SUM(i.amount_paid), 0) AS REAL) AS rev,
                COUNT(i.id) AS cnt
         FROM invoices i
         LEFT JOIN appointments a ON a.id = i.appointment_id
         GROUP BY t
         ORDER BY rev DESC")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(|r| RevenueByType {
        appointment_type: r.get("t"),
        revenue: r.get("rev"),
        count: r.get("cnt"),
    }).collect()))
}

/// No-show / cancellation rate across all appointments.
pub async fn no_show_rate(State(state): State<AppState>) -> ApiResult<Json<NoShowRate>> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM appointments").fetch_one(&state.db).await?;
    let ns: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM appointments WHERE status = 'noshow'").fetch_one(&state.db).await?;
    let cx: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM appointments WHERE status = 'cancelled'").fetch_one(&state.db).await?;
    let comp: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM appointments WHERE status = 'completed'").fetch_one(&state.db).await?;
    let t = total.0 as f64;
    Ok(Json(NoShowRate {
        total: total.0,
        no_show: ns.0,
        cancelled: cx.0,
        completed: comp.0,
        no_show_rate: if t > 0.0 { ns.0 as f64 / t * 100.0 } else { 0.0 },
        cancellation_rate: if t > 0.0 { cx.0 as f64 / t * 100.0 } else { 0.0 },
    }))
}

/// Appointment count by hour of day (0-23).
pub async fn hour_distribution(State(state): State<AppState>) -> ApiResult<Json<Vec<HourCount>>> {
    let rows = sqlx::query(
        "SELECT CAST(strftime('%H', appointment_date) AS INTEGER) AS h, COUNT(*) AS v
         FROM appointments
         GROUP BY h ORDER BY h")
        .fetch_all(&state.db)
        .await?;
    // Build a dense 0-23 series so every hour appears (zeros for empty slots).
    let mut counts = [0i64; 24];
    for r in &rows {
        let h: i64 = r.get("h");
        if (0..24).contains(&h) {
            counts[h as usize] = r.get::<i64, _>("v");
        }
    }
    Ok(Json((0..24).map(|h| HourCount { hour: h, count: counts[h as usize] }).collect()))
}

/// Patient counts by age bracket, computed from date_of_birth.
pub async fn age_demographics(State(state): State<AppState>) -> ApiResult<Json<Vec<AgeBracket>>> {
    let rows = sqlx::query(
        "SELECT
           CASE
             WHEN age <= 18 THEN '0-18'
             WHEN age <= 35 THEN '19-35'
             WHEN age <= 50 THEN '36-50'
             WHEN age <= 65 THEN '51-65'
             ELSE '65+'
           END AS bracket,
           COUNT(*) AS v
         FROM (
           SELECT CAST((julianday('now') - julianday(date_of_birth)) / 365.25 AS INTEGER) AS age
           FROM patients
           WHERE date_of_birth IS NOT NULL AND date_of_birth != ''
         )
         GROUP BY bracket")
        .fetch_all(&state.db)
        .await?;
    // Preserve a fixed bracket order regardless of which buckets have rows.
    let order = ["0-18", "19-35", "36-50", "51-65", "65+"];
    let mut map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for r in &rows {
        map.insert(r.get::<String, _>("bracket"), r.get::<i64, _>("v"));
    }
    Ok(Json(order.iter().map(|b| AgeBracket {
        bracket: b.to_string(),
        count: *map.get(*b).unwrap_or(&0),
    }).collect()))
}

/// Top 10 patients ranked by outstanding balance.
pub async fn outstanding_by_patient(State(state): State<AppState>) -> ApiResult<Json<Vec<OutstandingPatient>>> {
    let rows = sqlx::query(
        "SELECT p.id AS pid,
                p.first_name || ' ' || p.last_name AS name,
                p.mrn AS mrn,
                CAST(COALESCE(SUM(i.balance_due), 0) AS REAL) AS outstanding,
                COUNT(i.id) AS invoice_count
         FROM patients p
         JOIN invoices i ON i.patient_id = p.id
         WHERE i.status IN ('issued','partially_paid','overdue')
         GROUP BY p.id
         HAVING outstanding > 0
         ORDER BY outstanding DESC
         LIMIT 10")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(|r| OutstandingPatient {
        patient_id: r.get("pid"),
        name: r.get("name"),
        mrn: r.get("mrn"),
        outstanding: r.get("outstanding"),
        invoice_count: r.get("invoice_count"),
    }).collect()))
}
