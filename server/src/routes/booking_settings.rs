//! Booking settings, booking notifications, and intake approve/decline flow.
//!
//! - `booking_settings` is a single-row config table (id = 1).
//! - `booking_notifications` is an outgoing-message log queued when an intake
//!   submission is approved/declined, and later actually sent by the provider
//!   send routine (Postmark for email, ClickSend for SMS).

use axum::{
    extract::{Path, State},
    Json,
};
use shared::{BookingNotification, BookingSettings, UpdateBookingSettings};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

// ---------- row extractors ----------

fn row_to_settings(r: &sqlx::sqlite::SqliteRow) -> BookingSettings {
    BookingSettings {
        booking_mode: r.get("booking_mode"),
        auto_confirm_message: r.get::<i64, _>("auto_confirm_message") != 0,
        auto_reminder_message: r.get::<i64, _>("auto_reminder_message") != 0,
        reminder_hours_before: r.get("reminder_hours_before"),
        email_provider: r.get("email_provider"),
        email_api_key: r.get("email_api_key"),
        email_from: r.get("email_from"),
        sms_provider: r.get("sms_provider"),
        sms_api_key: r.get("sms_api_key"),
        sms_sender: r.get("sms_sender"),
        sms_username: r.get("sms_username"),
        template_booking_received: r.get("template_booking_received"),
        template_booking_confirmed: r.get("template_booking_confirmed"),
        template_booking_declined: r.get("template_booking_declined"),
        template_reminder: r.get("template_reminder"),
        updated_at: r.get("updated_at"),
    }
}

fn row_to_notification(r: &sqlx::sqlite::SqliteRow) -> BookingNotification {
    BookingNotification {
        id: r.get("id"),
        booking_id: r.get("booking_id"),
        intake_submission_id: r.get("intake_submission_id"),
        channel: r.get("channel"),
        recipient: r.get("recipient"),
        template_used: r.get("template_used"),
        body: r.get("body"),
        status: r.get("status"),
        provider_response: r.get("provider_response"),
        sent_at: r.get("sent_at"),
        created_at: r.get("created_at"),
    }
}

/// Simple placeholder substitution for notification templates.
fn fill_template(tmpl: &str, name: &str, date: &str, time: &str, appt_type: &str) -> String {
    tmpl.replace("{{name}}", name)
        .replace("{{date}}", date)
        .replace("{{time}}", time)
        .replace("{{type}}", appt_type)
}

async fn load_settings(state: &AppState) -> ApiResult<BookingSettings> {
    let row = sqlx::query("SELECT * FROM booking_settings WHERE id = 1")
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(row_to_settings(&row))
}

// ---------- settings ----------

/// GET /api/booking-settings — the single booking-settings row (id = 1).
pub async fn get_settings(State(state): State<AppState>) -> ApiResult<Json<BookingSettings>> {
    Ok(Json(load_settings(&state).await?))
}

/// PUT /api/booking-settings — update any provided fields, return the updated row.
pub async fn update_settings(
    State(state): State<AppState>,
    Json(b): Json<UpdateBookingSettings>,
) -> ApiResult<Json<BookingSettings>> {
    // Build a dynamic SET clause so only provided fields are touched.
    let mut sets: Vec<&str> = Vec::new();
    if b.booking_mode.is_some() { sets.push("booking_mode = ?"); }
    if b.auto_confirm_message.is_some() { sets.push("auto_confirm_message = ?"); }
    if b.auto_reminder_message.is_some() { sets.push("auto_reminder_message = ?"); }
    if b.reminder_hours_before.is_some() { sets.push("reminder_hours_before = ?"); }
    if b.email_provider.is_some() { sets.push("email_provider = ?"); }
    if b.email_api_key.is_some() { sets.push("email_api_key = ?"); }
    if b.email_from.is_some() { sets.push("email_from = ?"); }
    if b.sms_provider.is_some() { sets.push("sms_provider = ?"); }
    if b.sms_api_key.is_some() { sets.push("sms_api_key = ?"); }
    if b.sms_sender.is_some() { sets.push("sms_sender = ?"); }
    if b.sms_username.is_some() { sets.push("sms_username = ?"); }
    if b.template_booking_received.is_some() { sets.push("template_booking_received = ?"); }
    if b.template_booking_confirmed.is_some() { sets.push("template_booking_confirmed = ?"); }
    if b.template_booking_declined.is_some() { sets.push("template_booking_declined = ?"); }
    if b.template_reminder.is_some() { sets.push("template_reminder = ?"); }

    if !sets.is_empty() {
        sets.push("updated_at = CURRENT_TIMESTAMP");
        let sql = format!("UPDATE booking_settings SET {} WHERE id = 1", sets.join(", "));
        let mut q = sqlx::query(&sql);
        if let Some(v) = &b.booking_mode { q = q.bind(v); }
        if let Some(v) = b.auto_confirm_message { q = q.bind(v as i64); }
        if let Some(v) = b.auto_reminder_message { q = q.bind(v as i64); }
        if let Some(v) = b.reminder_hours_before { q = q.bind(v); }
        if let Some(v) = &b.email_provider { q = q.bind(v); }
        if let Some(v) = &b.email_api_key { q = q.bind(v); }
        if let Some(v) = &b.email_from { q = q.bind(v); }
        if let Some(v) = &b.sms_provider { q = q.bind(v); }
        if let Some(v) = &b.sms_api_key { q = q.bind(v); }
        if let Some(v) = &b.sms_sender { q = q.bind(v); }
        if let Some(v) = &b.sms_username { q = q.bind(v); }
        if let Some(v) = &b.template_booking_received { q = q.bind(v); }
        if let Some(v) = &b.template_booking_confirmed { q = q.bind(v); }
        if let Some(v) = &b.template_booking_declined { q = q.bind(v); }
        if let Some(v) = &b.template_reminder { q = q.bind(v); }
        q.execute(&state.db).await?;
    }

    Ok(Json(load_settings(&state).await?))
}

// ---------- notifications ----------

/// GET /api/booking-notifications — newest first, limit 100.
pub async fn list_notifications(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<BookingNotification>>> {
    let rows = sqlx::query(
        "SELECT * FROM booking_notifications ORDER BY created_at DESC, id DESC LIMIT 100",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.iter().map(row_to_notification).collect()))
}

// ---------- intake approve / decline ----------

/// A distilled view of the fields we need from an intake submission.
struct IntakeInfo {
    first_name: String,
    last_name: String,
    phone: Option<String>,
    email: Option<String>,
    address: Option<String>,
    medicare_number: Option<String>,
    date_of_birth: Option<String>,
    preferred_date: Option<String>,
    preferred_time: Option<String>,
    appointment_type: Option<String>,
    symptoms: Option<String>,
}

fn row_to_intake_info(r: &sqlx::sqlite::SqliteRow) -> IntakeInfo {
    IntakeInfo {
        first_name: r.get("first_name"),
        last_name: r.get("last_name"),
        phone: r.get("phone"),
        email: r.get("email"),
        address: r.get("address"),
        medicare_number: r.get("medicare_number"),
        date_of_birth: r.get("date_of_birth"),
        preferred_date: r.get("preferred_date"),
        preferred_time: r.get("preferred_time"),
        appointment_type: r.get("appointment_type"),
        symptoms: r.get("symptoms"),
    }
}

/// Queue a booking notification. Channel is chosen from the contact info the
/// patient gave: email if they have one, otherwise SMS if they have a phone.
/// Returns Ok(false) if no usable contact channel exists (nothing queued).
async fn queue_notification(
    state: &AppState,
    intake_id: i64,
    info: &IntakeInfo,
    template: &str,
    template_name: &str,
) -> ApiResult<bool> {
    let name = format!("{} {}", info.first_name, info.last_name);
    let date = info.preferred_date.clone().unwrap_or_default();
    let time = info.preferred_time.clone().unwrap_or_default();
    let appt_type = info
        .appointment_type
        .clone()
        .unwrap_or_else(|| "appointment".into());
    let body = fill_template(template, &name, &date, &time, &appt_type);

    // Prefer email, fall back to SMS.
    let (channel, recipient) = if let Some(email) = info.email.as_ref().filter(|e| !e.is_empty()) {
        ("email", email.clone())
    } else if let Some(phone) = info.phone.as_ref().filter(|p| !p.is_empty()) {
        ("sms", phone.clone())
    } else {
        return Ok(false);
    };

    sqlx::query(
        "INSERT INTO booking_notifications
         (intake_submission_id, channel, recipient, template_used, body, status)
         VALUES (?, ?, ?, ?, ?, 'pending')",
    )
    .bind(intake_id)
    .bind(channel)
    .bind(&recipient)
    .bind(template_name)
    .bind(&body)
    .execute(&state.db)
    .await?;
    Ok(true)
}

/// POST /api/intake/:id/approve — staff approves an intake submission:
/// create the patient + appointment, mark the submission imported, and queue a
/// booking-confirmed notification.
pub async fn approve_intake(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let row = sqlx::query("SELECT * FROM intake_submissions WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let info = row_to_intake_info(&row);

    // --- near-match / possible-duplicate detection (advisory) ---
    // Before creating a patient, surface existing patients with SIMILAR but not
    // identical details so staff are warned about a possible duplicate. This is
    // ADVISORY: approval still proceeds (creating a new patient). To merge
    // instead, staff use the /match-check + /merge-into flow.
    let exact_id = crate::routes::intake::exact_match_patient(
        &state,
        &info.first_name,
        &info.last_name,
        info.date_of_birth.as_deref(),
        info.phone.as_deref(),
        info.email.as_deref(),
    )
    .await?;
    let near_matches = crate::routes::intake::near_match_patients(
        &state,
        exact_id,
        &info.first_name,
        &info.last_name,
        info.date_of_birth.as_deref(),
        info.phone.as_deref(),
        info.email.as_deref(),
    )
    .await?;

    // --- create patient (inlined from intake::import_one) ---
    let year = chrono::Utc::now().format("%Y");
    let mrn = format!("MOS-{}{:07}", year, rand::random::<u32>() % 1_000_000);
    let pr = sqlx::query(
        "INSERT INTO patients (mrn, first_name, last_name, date_of_birth, phone, email, address, medicare_number)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&mrn)
    .bind(&info.first_name)
    .bind(&info.last_name)
    .bind(info.date_of_birth.as_deref().unwrap_or("1900-01-01"))
    .bind(&info.phone)
    .bind(&info.email)
    .bind(&info.address)
    .bind(&info.medicare_number)
    .execute(&state.db)
    .await?;
    let pid = pr.last_insert_rowid();

    // --- create appointment if a preferred date was given ---
    let mut appointment_id: Option<i64> = None;
    if let Some(date) = info.preferred_date.as_ref().filter(|d| !d.is_empty()) {
        let time = info
            .preferred_time
            .clone()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "09:00".into());
        let dt = format!("{} {}:00", date, time);
        let atype = info
            .appointment_type
            .clone()
            .unwrap_or_else(|| "Dry Eye Consultation".into());
        let ar = sqlx::query(
            "INSERT INTO appointments (patient_id, appointment_type, appointment_date, duration_minutes, status, notes)
             VALUES (?, ?, ?, 60, 'scheduled', ?)",
        )
        .bind(pid)
        .bind(&atype)
        .bind(&dt)
        .bind(&info.symptoms)
        .execute(&state.db)
        .await?;
        appointment_id = Some(ar.last_insert_rowid());
    }

    // --- mark submission imported ---
    sqlx::query("UPDATE intake_submissions SET status = 'imported', matched_patient_id = ? WHERE id = ?")
        .bind(pid)
        .bind(id)
        .execute(&state.db)
        .await?;

    // --- queue confirmation notification ---
    let settings = load_settings(&state).await?;
    let queued = queue_notification(
        &state,
        id,
        &info,
        &settings.template_booking_confirmed,
        "template_booking_confirmed",
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "patient_id": pid,
        "mrn": mrn,
        "appointment_id": appointment_id,
        "notification_queued": queued,
        // advisory: possible duplicates detected at approval time. A non-empty
        // list means staff may have wanted to merge instead of create-new.
        "exact_match_existed": exact_id.is_some(),
        "possible_matches": near_matches,
    })))
}

/// POST /api/intake/:id/decline — mark declined and queue a decline notification.
pub async fn decline_intake(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let row = sqlx::query("SELECT * FROM intake_submissions WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)?;
    let info = row_to_intake_info(&row);

    sqlx::query("UPDATE intake_submissions SET status = 'declined' WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;

    let settings = load_settings(&state).await?;
    let queued = queue_notification(
        &state,
        id,
        &info,
        &settings.template_booking_declined,
        "template_booking_declined",
    )
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "notification_queued": queued,
    })))
}

// ---------- provider send ----------

/// POST /api/booking-notifications/send — attempt to deliver all pending
/// notifications via the configured providers. No API key => 'skipped'.
pub async fn send_pending(
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let settings = load_settings(&state).await?;

    let rows = sqlx::query(
        "SELECT * FROM booking_notifications WHERE status = 'pending' ORDER BY id ASC",
    )
    .fetch_all(&state.db)
    .await?;

    let client = reqwest::Client::new();
    let mut sent = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for r in &rows {
        let n = row_to_notification(r);
        let (status, response): (&str, String) = match n.channel.as_str() {
            "email" => {
                match settings.email_api_key.as_ref().filter(|k| !k.is_empty()) {
                    None => ("skipped", "no api key configured".into()),
                    Some(key) => {
                        send_email(&client, &settings, key, &n.recipient, &n.body).await
                    }
                }
            }
            "sms" => {
                match settings.sms_api_key.as_ref().filter(|k| !k.is_empty()) {
                    None => ("skipped", "no api key configured".into()),
                    Some(key) => {
                        send_sms(&client, &settings, key, &n.recipient, &n.body).await
                    }
                }
            }
            other => ("failed", format!("unknown channel: {}", other)),
        };

        match status {
            "sent" => sent += 1,
            "failed" => failed += 1,
            _ => skipped += 1,
        }

        sqlx::query(
            "UPDATE booking_notifications
             SET status = ?, provider_response = ?,
                 sent_at = CASE WHEN ? = 'sent' THEN CURRENT_TIMESTAMP ELSE sent_at END
             WHERE id = ?",
        )
        .bind(status)
        .bind(&response)
        .bind(status)
        .bind(n.id)
        .execute(&state.db)
        .await?;
    }

    Ok(Json(serde_json::json!({
        "processed": rows.len(),
        "sent": sent,
        "failed": failed,
        "skipped": skipped,
    })))
}

/// Send one email via Postmark. Returns (status, provider_response).
async fn send_email(
    client: &reqwest::Client,
    settings: &BookingSettings,
    api_key: &str,
    recipient: &str,
    body: &str,
) -> (&'static str, String) {
    let payload = serde_json::json!({
        "From": settings.email_from,
        "To": recipient,
        "Subject": "Your appointment — OptiCore",
        "TextBody": body,
    });

    let res = client
        .post("https://api.postmarkapp.com/email")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("X-Postmark-Server-Token", api_key)
        .json(&payload)
        .send()
        .await;

    match res {
        Ok(resp) => {
            let ok = resp.status().is_success();
            let text = resp.text().await.unwrap_or_default();
            if ok {
                ("sent", text)
            } else {
                ("failed", text)
            }
        }
        Err(e) => ("failed", e.to_string()),
    }
}

/// Send one SMS via ClickSend. Returns (status, provider_response).
async fn send_sms(
    client: &reqwest::Client,
    settings: &BookingSettings,
    api_key: &str,
    recipient: &str,
    body: &str,
) -> (&'static str, String) {
    // ClickSend uses HTTP Basic auth with (account_username, api_key).
    // The username is the ClickSend account login (stored in sms_username).
    let username = settings.sms_username.as_deref().unwrap_or("");
    let payload = serde_json::json!({
        "messages": [{
            "source": "opticore",
            "from": settings.sms_sender,
            "to": normalise_phone(recipient),
            "body": body,
        }]
    });

    let res = client
        .post("https://rest.clicksend.com/v3/sms/send")
        .basic_auth(username, Some(api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    match res {
        Ok(resp) => {
            let ok = resp.status().is_success();
            let text = resp.text().await.unwrap_or_default();
            if ok {
                ("sent", text)
            } else {
                ("failed", text)
            }
        }
        Err(e) => ("failed", e.to_string()),
    }
}

/// Normalise an Australian phone number to E.164 format for ClickSend.
/// "0412 345 678" → "+61412345678", "+61 412 345 678" → "+61412345678"
fn normalise_phone(phone: &str) -> String {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.starts_with("0") {
        format!("+61{}", &digits[1..])
    } else if digits.starts_with("61") {
        format!("+{}", digits)
    } else if phone.starts_with("+") {
        phone.to_string()
    } else {
        format!("+61{}", digits)
    }
}
