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

/// Load the single booking-settings row (id = 1).
///
/// **Lazy-init invariant:** `booking_settings` is a single-row config table
/// with `CHECK (id = 1)` (migration 0008). The schema guarantees that *if* a
/// row exists it has id=1, but it does NOT guarantee a row exists at all — a
/// `DELETE FROM booking_settings` (e.g. via the sqlite CLI, a bug, or a future
/// code path) leaves the table empty, after which every caller of this
/// function (GET, PUT, approve/decline/queue notification flows) would 404 or
/// 500. There is intentionally no DELETE route on this resource, but the DB
/// row can still be removed out-of-band.
///
/// To make the "id=1 always exists" invariant robust without trigger magic,
/// we lazy-init here: if the row is missing, re-seed it with the migration's
/// column defaults (`INSERT OR IGNORE INTO booking_settings (id) VALUES (1)`)
/// and then return it. This is idempotent and matches the seed in 0008 exactly.
/// The cost is one extra round-trip only when the row is absent (the rare
/// repair path); the happy path is a single SELECT.
async fn load_settings(state: &AppState) -> ApiResult<BookingSettings> {
    let row = sqlx::query("SELECT * FROM booking_settings WHERE id = 1")
        .fetch_optional(&state.db)
        .await?;
    let row = match row {
        Some(r) => r,
        // Row missing (deleted out-of-band, or seed somehow didn't run).
        // Re-seed the default row, then re-fetch. INSERT OR IGNORE keeps this
        // safe against a concurrent insert racing between our SELECT and INSERT.
        None => {
            sqlx::query("INSERT OR IGNORE INTO booking_settings (id) VALUES (1)")
                .execute(&state.db)
                .await?;
            sqlx::query("SELECT * FROM booking_settings WHERE id = 1")
                .fetch_one(&state.db)
                .await?
        }
    };
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
    // ---- Field-level validation -----------------------------------------
    //
    // booking_mode is an enum-like column ('automatic' | 'approval'); any
    // other string would be silently stored and later break the booking
    // flow's mode dispatch. Reject unknown modes early.
    //
    // reminder_hours_before is a lead-time in hours; a negative value is
    // nonsensical (a reminder sent after the appointment). Zero is allowed
    // (reminder at appointment time). NaN/Infinity are not possible here
    // (the field is i64), so we only bound the lower end.
    if let Some(ref mode) = b.booking_mode {
        if !matches!(mode.as_str(), "automatic" | "approval") {
            return Err(ApiError::BadRequest(
                "booking_mode must be 'automatic' or 'approval'".into(),
            ));
        }
    }
    if let Some(hours) = b.reminder_hours_before {
        if hours < 0 {
            return Err(ApiError::BadRequest(
                "reminder_hours_before must be >= 0".into(),
            ));
        }
    }

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

    // Ensure the id=1 row exists before updating. If it was deleted
    // out-of-band (see `load_settings`'s lazy-init note), a bare
    // `UPDATE ... WHERE id = 1` would silently affect 0 rows and the caller's
    // requested changes would be dropped. Re-seed the default first, then the
    // UPDATE below applies the caller's fields on top of it. INSERT OR IGNORE
    // makes this a no-op on the happy path (row already present).
    sqlx::query("INSERT OR IGNORE INTO booking_settings (id) VALUES (1)")
        .execute(&state.db)
        .await?;

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
    //
    // These are pure reads (no mutation), so they run outside the write
    // transaction below. They do not affect correctness of the guard.
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

    // --- create patient + appointment + mark imported (atomic) ---
    //
    // Wrap the multi-step write in `BEGIN IMMEDIATE` with a status guard to
    // close two concurrency hazards (same fix as `intake::import_one`):
    //
    // 1. **Double-approve race (TOCTOU):** two concurrent approve calls on the
    //    same intake id both SELECT the intake, both see status='new', both
    //    INSERT a patient, both mark it imported → duplicate patients created
    //    from one submission. The fix acquires the write lock up-front and
    //    re-checks the status inside the transaction; the second caller sees
    //    status='imported' and is rejected with 409.
    // 2. **Partial-failure inconsistency:** INSERT patient → INSERT appointment
    //    → UPDATE intake previously ran as separate statements. If any step
    //    failed, earlier inserts were left behind but the intake stayed 'new'.
    //    The transaction makes all steps atomic.
    let mut conn = state.db.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

    // Re-fetch the intake row inside the transaction and check its status.
    let row = sqlx::query("SELECT * FROM intake_submissions WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    let row = match row {
        Some(r) => r,
        None => {
            sqlx::query("ROLLBACK").execute(&mut *conn).await?;
            return Err(ApiError::NotFound);
        }
    };
    let status: Option<String> = row.get("status");
    if status.as_deref().unwrap_or("new") != "new" {
        sqlx::query("ROLLBACK").execute(&mut *conn).await?;
        return Err(ApiError::Conflict(format!(
            "intake submission {} has already been processed (status: {})",
            id,
            status.as_deref().unwrap_or("?")
        )));
    }

    // --- create patient ---
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
    .execute(&mut *conn)
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
        .execute(&mut *conn)
        .await?;
        appointment_id = Some(ar.last_insert_rowid());
    }

    // --- mark submission imported ---
    sqlx::query("UPDATE intake_submissions SET status = 'imported', matched_patient_id = ? WHERE id = ?")
        .bind(pid)
        .bind(id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("COMMIT").execute(&mut *conn).await?;

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
    // Wrap the read-guard-write sequence in `BEGIN IMMEDIATE` with a status
    // guard — same double-processing race fix as `approve_intake`,
    // `intake::import_one`, and `intake::merge_into`.
    //
    // RACE FIX: previously the handler did
    //   SELECT * FROM intake_submissions WHERE id = ?   -- (1) read
    //   UPDATE intake_submissions SET status = 'declined' WHERE id = ?  -- (2)
    //   queue_notification(...)                        -- (3) INSERT notification
    //
    // with NO transaction wrapping steps 1–3. Two concurrent `decline_intake`
    // calls on the same id both SELECT the row, both see status='new', both
    // run the UPDATE, and both queue a decline notification → the patient
    // receives TWO decline messages. A concurrent `approve_intake` +
    // `decline_intake` on the same id could also race: both read status='new',
    // both proceed — one creates a patient+appointment, the other marks it
    // declined, leaving the submission in an inconsistent state
    // (status='declined' but matched_patient_id set).
    //
    // `BEGIN IMMEDIATE` acquires the SQLite write lock at transaction start
    // and serializes writers. The second caller's status re-check sees the
    // row already processed and is rejected with 409. (Matches the
    // `approve_intake` / `import_one` pattern exactly.)
    let mut conn = state.db.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

    // Re-fetch the intake row inside the transaction and check its status.
    let row = sqlx::query("SELECT * FROM intake_submissions WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    let row = match row {
        Some(r) => r,
        None => {
            sqlx::query("ROLLBACK").execute(&mut *conn).await?;
            return Err(ApiError::NotFound);
        }
    };
    let info = row_to_intake_info(&row);

    // Status guard: only 'new' submissions can be declined. If another
    // concurrent call already processed (imported/approved/declined/archived)
    // this submission, reject cleanly. (Without the transaction this would be
    // a TOCTOU.) We read the status from the row we just fetched inside the
    // transaction rather than re-binding, since `row_to_intake_info` does not
    // capture status — re-query it explicitly.
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM intake_submissions WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *conn)
        .await?;
    if status.as_deref().unwrap_or("new") != "new" {
        sqlx::query("ROLLBACK").execute(&mut *conn).await?;
        return Err(ApiError::Conflict(format!(
            "intake submission {} has already been processed (status: {})",
            id,
            status.as_deref().unwrap_or("?")
        )));
    }

    sqlx::query("UPDATE intake_submissions SET status = 'declined' WHERE id = ?")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("COMMIT").execute(&mut *conn).await?;

    // Queue the decline notification OUTSIDE the transaction. The status
    // guard above guarantees only one decline call reaches this point per
    // submission, so at most one notification is queued. (queue_notification
    // does its own INSERT; running it after COMMIT avoids holding the write
    // lock across the settings load + template fill.)
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

    // Snapshot the pending notification ids. We do NOT hold a transaction
    // across the HTTP sends below (providers can be slow; holding the SQLite
    // write lock across network I/O would block every other write in the app).
    // Instead we claim each row individually before sending — see the
    // per-row claim step inside the loop.
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

        // ---- TOCTOU race fix: claim the row before sending ----------------
        //
        // Two concurrent `send_pending` calls both snapshot the same pending
        // rows (the SELECT above runs outside a transaction). Without a
        // claim step, both would send each notification and both would
        // UPDATE the row — the patient receives TWO emails/SMS.
        //
        // Claim atomically: flip status 'pending' -> 'sending' inside a
        // `BEGIN IMMEDIATE` transaction. If `rows_affected() == 0`, another
        // caller already claimed (or finalized) this row — skip it. The
        // `status = 'pending'` predicate in the WHERE clause makes the
        // claim conditional on the row still being pending, so only one
        // caller can ever win the claim for a given row. (SQLite serializes
        // writers via BEGIN IMMEDIATE, so the two UPDATEs cannot both see
        // status='pending'.)
        //
        // We use an intermediate 'sending' status (the column is free-form
        // VARCHAR with no CHECK constraint) so that a crash mid-send leaves
        // the row visibly in-progress rather than silently re-sendable. A
        // future retry pass can pick up 'sending' rows that are stale.
        let mut conn = state.db.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;
        let claim = sqlx::query(
            "UPDATE booking_notifications SET status = 'sending' WHERE id = ? AND status = 'pending'",
        )
        .bind(n.id)
        .execute(&mut *conn)
        .await?;
        let claimed = claim.rows_affected() == 1;
        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);

        if !claimed {
            // Another concurrent send_pending already claimed this row.
            // It is responsible for delivering (or has already delivered) it.
            continue;
        }

        // ---- deliver via the configured provider (no DB lock held) --------
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

        // ---- finalize: record the terminal status -------------------------
        // A plain UPDATE suffices here — the row is already claimed as
        // 'sending' by us, so no other caller will touch it. (Even if a
        // future retry pass picked up stale 'sending' rows, the id-keyed
        // UPDATE here is idempotent for our own row.)
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
