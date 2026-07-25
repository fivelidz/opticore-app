//! Analytics aggregates for the dashboard + analytics page.

use axum::{
    extract::{Path, State},
    Json,
};
use shared::{AnalyticsOverview, SourceBreakdown, TimeSeriesPoint, WebsiteTrafficPoint};
use sqlx::Row;

use crate::error::ApiResult;
use crate::AppState;

pub async fn overview(State(state): State<AppState>) -> ApiResult<Json<AnalyticsOverview>> {
    let p: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM patients").fetch_one(&state.db).await?;
    let a: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM appointments").fetch_one(&state.db).await?;
    let rev: (f64,) = sqlx::query_as("SELECT COALESCE(SUM(amount_paid),0) FROM invoices").fetch_one(&state.db).await?;
    let out: (f64,) = sqlx::query_as("SELECT COALESCE(SUM(balance_due),0) FROM invoices WHERE status IN ('issued','partially_paid','overdue')").fetch_one(&state.db).await?;
    let am: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM appointments WHERE appointment_date >= date('now','start of month')").fetch_one(&state.db).await?;
    let rm: (f64,) = sqlx::query_as("SELECT COALESCE(SUM(amount_paid),0) FROM invoices WHERE invoice_date >= date('now','start of month')").fetch_one(&state.db).await?;
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
        "SELECT DATE(invoice_date) AS d, COALESCE(SUM(amount_paid),0) AS v
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
