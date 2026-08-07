use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use shared::{ConsultationType, CreateInvoice, CreatePayment, Invoice, InvoiceItem, Payment, ServiceItem};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

fn row_to_ct(r: &sqlx::sqlite::SqliteRow) -> ConsultationType {
    ConsultationType {
        id: r.get("id"), type_code: r.get("type_code"), type_name: r.get("type_name"),
        description: r.get("description"), default_price: r.get("default_price"),
        default_duration_minutes: r.get("default_duration_minutes"),
        medicare_item_number: r.get("medicare_item_number"), active: r.get("active"),
    }
}
fn row_to_svc(r: &sqlx::sqlite::SqliteRow) -> ServiceItem {
    ServiceItem {
        id: r.get("id"), service_code: r.get("service_code"), service_name: r.get("service_name"),
        category: r.get("category"), description: r.get("description"), unit_price: r.get("unit_price"),
        unit_type: r.get("unit_type"), tax_rate: r.get("tax_rate"), active: r.get("active"),
    }
}

// ---------- Catalog ----------

pub async fn consultation_types(State(state): State<AppState>) -> ApiResult<Json<Vec<ConsultationType>>> {
    let rows = sqlx::query("SELECT * FROM consultation_types WHERE active = 1 ORDER BY type_name")
        .fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(row_to_ct).collect()))
}

#[derive(Deserialize)]
pub struct SvcQuery { #[serde(default)] pub category: Option<String> }

pub async fn services(State(state): State<AppState>, Query(q): Query<SvcQuery>) -> ApiResult<Json<Vec<ServiceItem>>> {
    let rows = if let Some(cat) = q.category {
        sqlx::query("SELECT * FROM services WHERE active = 1 AND category = ? ORDER BY service_name")
            .bind(cat).fetch_all(&state.db).await?
    } else {
        sqlx::query("SELECT * FROM services WHERE active = 1 ORDER BY category, service_name")
            .fetch_all(&state.db).await?
    };
    Ok(Json(rows.iter().map(row_to_svc).collect()))
}

pub async fn service_categories(State(state): State<AppState>) -> ApiResult<Json<Vec<String>>> {
    let rows = sqlx::query("SELECT DISTINCT category FROM services WHERE active = 1 ORDER BY category")
        .fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(|r| r.get::<String, _>("category")).collect()))
}

// ---------- Invoices ----------

pub async fn invoices_by_patient(State(state): State<AppState>, Path(pid): Path<i64>) -> ApiResult<Json<Vec<Invoice>>> {
    let rows = sqlx::query("SELECT * FROM invoices WHERE patient_id = ? ORDER BY invoice_date DESC")
        .bind(pid).fetch_all(&state.db).await?;
    let mut out = Vec::new();
    for r in &rows {
        let inv_id: i64 = r.get("id");
        let items = sqlx::query("SELECT * FROM invoice_items WHERE invoice_id = ?").bind(inv_id)
            .fetch_all(&state.db).await?
            .iter().map(|r| InvoiceItem {
                id: r.get("id"), invoice_id: r.get("invoice_id"), item_type: r.get("item_type"),
                description: r.get("description"), quantity: r.get("quantity"), unit_price: r.get("unit_price"),
                discount_percent: r.get("discount_percent"), tax_rate: r.get("tax_rate"), total: r.get("total"),
            }).collect();
        out.push(Invoice {
            id: inv_id, invoice_number: r.get("invoice_number"), patient_id: r.get("patient_id"),
            appointment_id: r.get("appointment_id"), invoice_date: r.get("invoice_date"),
            due_date: r.get("due_date"), subtotal: r.get("subtotal"), tax_amount: r.get("tax_amount"),
            discount_amount: r.get("discount_amount"), total_amount: r.get("total_amount"),
            amount_paid: r.get("amount_paid"), balance_due: r.get("balance_due"), status: r.get("status"),
            payment_method: r.get("payment_method"), notes: r.get("notes"), created_at: r.get("created_at"),
            items,
        });
    }
    Ok(Json(out))
}

pub async fn create_invoice(State(state): State<AppState>, Json(b): Json<CreateInvoice>) -> ApiResult<Json<Invoice>> {
    // ---- Business-rule validation --------------------------------------
    //
    // An invoice represents a real financial obligation. Invalid line items
    // can invert or zero-out the grand total, producing nonsense data that
    // breaks every downstream report. We reject early (400) before any DB
    // write so partial state is never persisted.
    //
    // Rules (conservative, documented):
    //   * items must be non-empty (an invoice with no lines is meaningless)
    //   * quantity    > 0            (zero/negative qty is nonsensical)
    //   * unit_price  >= 0           (free items allowed; negative rejected)
    //   * discount_percent in [0, 100] (>100% would invert the line total)
    //   * tax_rate    in [0, 1]      (>100% tax is implausible)
    //   * no NaN/Infinity inputs      (serde_json parses "NaN"/"Infinity"
    //     as f64::NAN / f64::INFINITY for some clients; reject them)
    if b.items.is_empty() {
        return Err(ApiError::BadRequest("invoice must have at least one item".into()));
    }
    for (i, it) in b.items.iter().enumerate() {
        if !it.quantity.is_finite() {
            return Err(ApiError::BadRequest(format!("item {}: quantity must be a finite number", i)));
        }
        if !it.unit_price.is_finite() {
            return Err(ApiError::BadRequest(format!("item {}: unit_price must be a finite number", i)));
        }
        if it.quantity <= 0.0 {
            return Err(ApiError::BadRequest(format!("item {}: quantity must be > 0", i)));
        }
        if it.unit_price < 0.0 {
            return Err(ApiError::BadRequest(format!("item {}: unit_price must be >= 0", i)));
        }
        if !(0.0..=100.0).contains(&it.discount_percent) {
            return Err(ApiError::BadRequest(format!("item {}: discount_percent must be in [0, 100]", i)));
        }
        if !(0.0..=1.0).contains(&it.tax_rate) {
            return Err(ApiError::BadRequest(format!("item {}: tax_rate must be in [0, 1]", i)));
        }
    }

    // compute totals
    let mut subtotal = 0.0f64;
    let mut tax = 0.0f64;
    let computed: Vec<(String, String, f64, f64, f64, f64, f64)> = b.items.iter().map(|it| {
        let net = it.unit_price * it.quantity * (1.0 - it.discount_percent / 100.0);
        let t = net * it.tax_rate;
        let total = net + t;
        subtotal += net;
        tax += t;
        (it.item_type.clone(), it.description.clone(), it.quantity, it.unit_price, it.discount_percent, it.tax_rate, total)
    }).collect();
    let total_amount = subtotal + tax;

    // Overflow / NaN / Infinity guard on the computed totals. Even with
    // each individual input finite and in-range, summing many large line
    // items can overflow f64 to +Infinity (e.g. two f64::MAX values).
    // SQLite stores REAL as IEEE-754 double, so Inf/NaN *can* be persisted
    // — but they break every downstream SUM/comparison and serialize as
    // `null`/`Infinity` in JSON. Reject before any write.
    if !subtotal.is_finite() || !tax.is_finite() || !total_amount.is_finite() {
        return Err(ApiError::BadRequest("invoice total overflow (NaN/Infinity) — line items too large".into()));
    }

    // Generate the invoice number and insert the invoice inside a single
    // `BEGIN IMMEDIATE` transaction.
    //
    // RACE FIX: previously the invoice number was derived from
    // `SELECT COUNT(*) FROM invoices` executed *outside* any transaction.
    // Two concurrent `create_invoice` calls could both read the same count,
    // compute the same `INV-2026-{:04}` number, and the second INSERT then
    // violated the `invoice_number UNIQUE` constraint — surfacing as an HTTP
    // 409 to one caller even though both requests were legitimate.
    //
    // `BEGIN IMMEDIATE` acquires the SQLite write lock at transaction start
    // (rather than deferring it to the first write). SQLite only allows a
    // single writer at a time, so this serializes the count-then-insert
    // sequence: each transaction sees a row count that reflects all
    // previously-committed invoices. Concurrent writers simply block until
    // the lock is released (subject to the pool's busy timeout), then
    // proceed — they never see a stale count.
    //
    // (SQLite has no `SELECT ... FOR UPDATE`; `BEGIN IMMEDIATE` is the
    // standard SQLite idiom for serializing writers.)
    let mut conn = state.db.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

    // Generate invoice number inside the transaction. We keep the historical
    // `COUNT(*) + 7` offset so existing/seeded invoice numbers are unchanged;
    // the correctness gain comes from the serializing transaction, not from
    // the numbering scheme.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM invoices")
        .fetch_one(&mut *conn).await?;
    let invoice_number = format!("INV-2026-{:04}", count.0 + 7);

    let r = sqlx::query(
        "INSERT INTO invoices (invoice_number, patient_id, appointment_id, subtotal, tax_amount, discount_amount, total_amount, amount_paid, balance_due, status, payment_method, notes)
         VALUES (?, ?, ?, ?, ?, 0, ?, 0, ?, 'issued', ?, ?)")
        .bind(&invoice_number).bind(b.patient_id).bind(b.appointment_id)
        .bind(subtotal).bind(tax).bind(total_amount).bind(total_amount)
        .bind(&b.payment_method).bind(&b.notes)
        .execute(&mut *conn).await?;
    let inv_id = r.last_insert_rowid();

    for (item_type, desc, qty, price, disc, tr, total) in &computed {
        sqlx::query("INSERT INTO invoice_items (invoice_id, item_type, description, quantity, unit_price, discount_percent, tax_rate, total) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(inv_id).bind(item_type).bind(desc).bind(*qty).bind(*price).bind(*disc).bind(*tr).bind(*total)
            .execute(&mut *conn).await?;
    }

    // Read back the full invoice row + items within the same transaction so
    // the returned object reflects the committed state.
    let row = sqlx::query("SELECT * FROM invoices WHERE id = ?").bind(inv_id)
        .fetch_one(&mut *conn).await?;
    let items = sqlx::query("SELECT * FROM invoice_items WHERE invoice_id = ?").bind(inv_id)
        .fetch_all(&mut *conn).await?
        .iter().map(|r| InvoiceItem {
            id: r.get("id"), invoice_id: r.get("invoice_id"), item_type: r.get("item_type"),
            description: r.get("description"), quantity: r.get("quantity"), unit_price: r.get("unit_price"),
            discount_percent: r.get("discount_percent"), tax_rate: r.get("tax_rate"), total: r.get("total"),
        }).collect();

    // Commit the transaction. If any statement above returned an error, the
    // `?` operator would propagate it and `conn` would be dropped — sqlx
    // rolls back an un-committed transaction on drop, so partial writes are
    // never persisted.
    sqlx::query("COMMIT").execute(&mut *conn).await?;

    Ok(Json(Invoice {
        id: inv_id, invoice_number: row.get("invoice_number"), patient_id: row.get("patient_id"),
        appointment_id: row.get("appointment_id"), invoice_date: row.get("invoice_date"),
        due_date: row.get("due_date"), subtotal: row.get("subtotal"), tax_amount: row.get("tax_amount"),
        discount_amount: row.get("discount_amount"), total_amount: row.get("total_amount"),
        amount_paid: row.get("amount_paid"), balance_due: row.get("balance_due"), status: row.get("status"),
        payment_method: row.get("payment_method"), notes: row.get("notes"), created_at: row.get("created_at"),
        items,
    }))
}

// ---------- Payments ----------

pub async fn payments_by_invoice(State(state): State<AppState>, Path(inv): Path<i64>) -> ApiResult<Json<Vec<Payment>>> {
    let rows = sqlx::query("SELECT * FROM payments WHERE invoice_id = ? ORDER BY payment_date")
        .bind(inv).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(|r| Payment {
        id: r.get("id"), invoice_id: r.get("invoice_id"), payment_date: r.get("payment_date"),
        amount: r.get("amount"), payment_method: r.get("payment_method"),
        reference_number: r.get("reference_number"), notes: r.get("notes"), created_at: r.get("created_at"),
    }).collect()))
}

pub async fn add_payment(State(state): State<AppState>, Json(b): Json<CreatePayment>) -> ApiResult<Json<Payment>> {
    let r = sqlx::query("INSERT INTO payments (invoice_id, amount, payment_method, reference_number, notes) VALUES (?, ?, ?, ?, ?)")
        .bind(b.invoice_id).bind(b.amount).bind(&b.payment_method).bind(&b.reference_number).bind(&b.notes)
        .execute(&state.db).await?;
    let id = r.last_insert_rowid();

    // update invoice amount_paid + balance + status
    sqlx::query(
        "UPDATE invoices SET
           amount_paid = amount_paid + ?,
           balance_due = MAX(0, total_amount - (amount_paid + ?)),
           status = CASE WHEN (amount_paid + ?) >= total_amount THEN 'paid' ELSE 'partially_paid' END
         WHERE id = ?")
        .bind(b.amount).bind(b.amount).bind(b.amount).bind(b.invoice_id)
        .execute(&state.db).await?;

    let row = sqlx::query("SELECT * FROM payments WHERE id = ?").bind(id).fetch_one(&state.db).await?;
    Ok(Json(Payment {
        id: row.get("id"), invoice_id: row.get("invoice_id"), payment_date: row.get("payment_date"),
        amount: row.get("amount"), payment_method: row.get("payment_method"),
        reference_number: row.get("reference_number"), notes: row.get("notes"), created_at: row.get("created_at"),
    }))
}
