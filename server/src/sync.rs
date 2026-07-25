//! Sync engine — connects the desktop app to the Cloudflare Worker.
//!
//! Every 30 seconds (configurable), this:
//! 1. PUSHES the clinic's availability + appointment types to the Worker
//!    (so the public intake form shows correct slots).
//! 2. PULLS pending bookings from the Worker (so staff see new bookings).
//!
//! The Worker URL + shared secret are set via environment variables:
//!   WORKER_URL=https://opticore-booking.workers.dev
//!   SYNC_SECRET=<the shared secret>

use std::time::Duration;
use sqlx::Row;
use tokio::time::interval;

use crate::error::ApiResult;
use crate::AppState;

const SYNC_INTERVAL_SECS: u64 = 30;

/// Start the background sync loop. Runs forever (until the server stops).
pub fn start(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(SYNC_INTERVAL_SECS));
        loop {
            ticker.tick().await;
            if let Err(e) = run_sync_cycle(&state).await {
                tracing::warn!("sync cycle failed: {e}");
            }
        }
    });
}

pub async fn run_sync_cycle(state: &AppState) -> ApiResult<()> {
    let worker_url = match std::env::var("WORKER_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => return Ok(()), // no worker configured — skip
    };
    let secret = std::env::var("SYNC_SECRET").unwrap_or_else(|_| "dev-sync-secret".into());

    // 1. PUSH availability + types
    if let Err(e) = push_to_worker(state, &worker_url, &secret).await {
        tracing::debug!("push to worker failed (non-fatal): {e}");
    }

    // 2. PULL pending bookings
    if let Err(e) = pull_from_worker(state, &worker_url, &secret).await {
        tracing::debug!("pull from worker failed (non-fatal): {e}");
    }

    Ok(())
}

async fn push_to_worker(state: &AppState, worker_url: &str, secret: &str) -> Result<(), String> {
    let client = reqwest::Client::new();

    // gather availability (next 30 days of weekday slots, minus booked/blocked)
    let slots = gather_availability(state).await.map_err(|e| e.to_string())?;
    let appt_types = gather_types(state).await.map_err(|e| e.to_string())?;

    let body = serde_json::json!({
        "slots": slots,
        "appointment_types": appt_types,
    });

    client
        .post(format!("{}/api/sync/push", worker_url))
        .header("Authorization", format!("Bearer {}", secret))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("sync: pushed {} slots + {} types to worker", slots.len(), appt_types.len());
    Ok(())
}

async fn pull_from_worker(state: &AppState, worker_url: &str, secret: &str) -> Result<(), String> {
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/sync/pull", worker_url))
        .header("Authorization", format!("Bearer {}", secret))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let bookings = data["bookings"].as_array().ok_or("invalid response")?;

    if bookings.is_empty() {
        return Ok(());
    }

    // Insert each booking as an intake submission
    for b in bookings {
        let first: String = b["first_name"].as_str().unwrap_or("").into();
        let last: String = b["last_name"].as_str().unwrap_or("").into();
        if first.is_empty() && last.is_empty() { continue; }

        sqlx::query(
            "INSERT OR IGNORE INTO intake_submissions
             (first_name, last_name, date_of_birth, phone, email, address,
              preferred_date, preferred_time, appointment_type, symptoms, source, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'worker-sync', 'new')")
            .bind(first)
            .bind(last)
            .bind(b["date_of_birth"].as_str())
            .bind(b["phone"].as_str())
            .bind(b["email"].as_str())
            .bind(b["address"].as_str())
            .bind(b["preferred_date"].as_str())
            .bind(b["preferred_time"].as_str())
            .bind(b["appointment_type"].as_str())
            .bind(b["symptoms"].as_str())
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
    }

    tracing::info!("sync: pulled {} bookings from worker", bookings.len());
    Ok(())
}

/// Generate the availability snapshot: next 30 days, weekdays, 9am-5pm,
/// minus existing appointments and blocked times.
async fn gather_availability(state: &AppState) -> ApiResult<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        "SELECT appointment_date FROM appointments WHERE appointment_date >= datetime('now')")
        .fetch_all(&state.db).await?;
    let booked: Vec<String> = rows.iter().map(|r| r.get::<String, _>("appointment_date")).collect();

    let blocked_rows = sqlx::query("SELECT start_at, end_at FROM blocked_times WHERE start_at >= datetime('now')")
        .fetch_all(&state.db).await?;
    let blocked: Vec<(i32, i32)> = blocked_rows.iter().filter_map(|r| {
        let s: String = r.get("start_at");
        let e: String = r.get("end_at");
        let sh: i32 = s.split(' ').nth(1)?.split(':').next()?.parse().ok()?;
        let sm: i32 = s.split(' ').nth(1)?.split(':').nth(1)?.parse().ok()?;
        let eh: i32 = e.split(' ').nth(1)?.split(':').next()?.parse().ok()?;
        let em: i32 = e.split(' ').nth(1)?.split(':').nth(1)?.parse().ok()?;
        Some((sh * 60 + sm, eh * 60 + em))
    }).collect();

    let mut slots = Vec::new();
    for day_offset in 1..=30 {
        let date_str = sqlx::query_scalar::<_, String>(
            &format!("SELECT date('now', '+{} days')", day_offset))
            .fetch_one(&state.db).await?;
        let dow: i64 = sqlx::query_scalar(
            &format!("SELECT CAST(strftime('%w','{}') AS INTEGER)", date_str))
            .fetch_one(&state.db).await?;
        if dow == 0 || dow == 6 { continue; } // skip weekends

        for hour in 9..=16 {
            let time_str = format!("{:02}:00", hour);
            let slot_min = hour * 60;
            let is_blocked = blocked.iter().any(|(s, e)| slot_min >= *s && slot_min < *e);
            let is_booked = booked.iter().any(|b| b.contains(&time_str) && b.contains(&date_str));
            slots.push(serde_json::json!({
                "date": date_str,
                "time": time_str,
                "available": !is_blocked && !is_booked,
            }));
        }
    }
    Ok(slots)
}

async fn gather_types(state: &AppState) -> ApiResult<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        "SELECT type_code, type_name, default_price, default_duration_minutes, description FROM consultation_types WHERE active = 1")
        .fetch_all(&state.db).await?;
    let types = rows.iter().map(|r| serde_json::json!({
        "code": r.get::<String, _>("type_code"),
        "name": r.get::<String, _>("type_name"),
        "price": r.get::<f64, _>("default_price"),
        "duration": r.get::<i64, _>("default_duration_minutes"),
        "description": r.try_get::<String, _>("description").ok(),
    })).collect();
    Ok(types)
}
