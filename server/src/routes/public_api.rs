//! Public availability: which slots are free for booking.
//! Used by the intake form to show patients available times.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use sqlx::Row;

use crate::error::ApiResult;
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct AvailableSlot {
    pub date: String,
    pub time: String,
    pub label: String,
    pub available: bool,
}

/// GET /api/public/availability/:days — public (no auth).
/// Returns the next N days of bookable slots (9am–5pm), marking booked/blocked ones.
pub async fn availability(State(state): State<AppState>, Path(days): Path<i64>) -> ApiResult<Json<Vec<AvailableSlot>>> {
    let days = days.clamp(1, 30);
    let mut out = Vec::new();

    for day_offset in 1..=days {
        let date = format!("date('now','+{} days')", day_offset);
        // fetch appointments + blocked times for this day
        let appts: Vec<(String,)> = sqlx::query_as(
            &format!("SELECT appointment_date FROM appointments WHERE DATE(appointment_date) = {} ORDER BY appointment_date", date))
            .fetch_all(&state.db).await?;
        let blocked: Vec<(String, String)> = sqlx::query_as(
            &format!("SELECT start_at, end_at FROM blocked_times WHERE DATE(start_at) = {}", date))
            .fetch_all(&state.db).await?;

        let booked_times: Vec<String> = appts.iter().map(|(d,)| d.split(' ').nth(1).unwrap_or("").to_string()).collect();
        let blocked_ranges: Vec<(i32, i32)> = blocked.iter().filter_map(|(s, e)| {
            let sh: i32 = s.split(' ').nth(1)?.split(':').next()?.parse().ok()?;
            let sm: i32 = s.split(' ').nth(1)?.split(':').nth(1)?.parse().ok()?;
            let eh: i32 = e.split(' ').nth(1)?.split(':').next()?.parse().ok()?;
            let em: i32 = e.split(' ').nth(1)?.split(':').nth(1)?.parse().ok()?;
            Some((sh * 60 + sm, eh * 60 + em))
        }).collect();

        // generate slots 9am–5pm, every 60 min
        let date_str = sqlx::query_scalar::<_, String>(&format!("SELECT DATE({})", date))
            .fetch_one(&state.db).await?;
        let dow = sqlx::query_scalar::<_, i64>(&format!("SELECT CAST(strftime('%w',{}) AS INTEGER)", date))
            .fetch_one(&state.db).await?;
        // skip Sundays (0) and Saturdays (6) for a clinic
        if dow == 0 || dow == 6 { continue; }

        for hour in 9..=16 {
            let time = format!("{:02}:00", hour);
            let slot_min = hour * 60;
            let is_blocked = blocked_ranges.iter().any(|(s, e)| slot_min >= *s && slot_min < *e);
            let is_booked = booked_times.contains(&time) || booked_times.iter().any(|t| t.starts_with(&format!("{:02}:", hour)));
            out.push(AvailableSlot {
                date: date_str.clone(),
                time: time.clone(),
                label: format!("{} {}", date_str, time),
                available: !is_blocked && !is_booked,
            });
        }
    }
    Ok(Json(out))
}

/// GET /api/public/appointment-types — public list of bookable appointment types + prices.
pub async fn appointment_types(State(state): State<AppState>) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let rows = sqlx::query("SELECT type_code, type_name, description, default_price, default_duration_minutes FROM consultation_types WHERE active = 1 ORDER BY default_price")
        .fetch_all(&state.db).await?;
    let out: Vec<serde_json::Value> = rows.iter().map(|r| serde_json::json!({
        "code": r.get::<String, _>("type_code"),
        "name": r.get::<String, _>("type_name"),
        "description": r.try_get::<String, _>("description").ok(),
        "price": r.get::<f64, _>("default_price"),
        "duration": r.get::<i64, _>("default_duration_minutes"),
    })).collect();
    Ok(Json(out))
}

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct MatchRequest {
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub date_of_birth: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

/// POST /api/public/match-patient — public.
/// Checks if the given details match an existing patient (by name + DOB, or phone, or email).
/// Returns whether a match was found (without revealing the patient record).
pub async fn match_patient(State(state): State<AppState>, Json(req): Json<MatchRequest>) -> ApiResult<Json<serde_json::Value>> {
    let mut matched = false;
    let mut confidence = "none".to_string();

    // strongest: name + DOB
    if let Some(dob) = &req.date_of_birth {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM patients WHERE LOWER(first_name) = LOWER(?) AND LOWER(last_name) = LOWER(?) AND date_of_birth = ?")
            .bind(&req.first_name).bind(&req.last_name).bind(dob)
            .fetch_optional(&state.db).await?;
        if row.is_some() { matched = true; confidence = "high".into(); }
    }
    // phone match
    if !matched {
        if let Some(phone) = &req.phone {
            let clean = phone.replace(" ", "").replace("-", "");
            let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM patients WHERE REPLACE(REPLACE(phone,' ',''),'-','') = ?")
                .bind(&clean).fetch_optional(&state.db).await?;
            if row.is_some() { matched = true; confidence = "medium".into(); }
        }
    }
    // email match
    if !matched {
        if let Some(email) = &req.email {
            let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM patients WHERE LOWER(email) = LOWER(?)")
                .bind(email).fetch_optional(&state.db).await?;
            if row.is_some() { matched = true; confidence = "medium".into(); }
        }
    }
    // name-only match (low confidence)
    if !matched {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM patients WHERE LOWER(first_name) = LOWER(?) AND LOWER(last_name) = LOWER(?)")
            .bind(&req.first_name).bind(&req.last_name)
            .fetch_optional(&state.db).await?;
        if row.is_some() { matched = true; confidence = "low".into(); }
    }

    Ok(Json(serde_json::json!({ "matched": matched, "confidence": confidence })))
}
