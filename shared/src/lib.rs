//! Shared types between the Rust server, the Tauri shell, and (via JSON) the frontend.
//!
//! These mirror the opticore SQLite schema (COLLABORATION/database/schema.sql)
//! and are the canonical API contract.

use chrono::{DateTime, NaiveDate};
use serde::{Deserialize, Serialize};

// ---------- Auth ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub role: String,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub refresh_token: String,
    pub user: User,
}

#[derive(Debug, Serialize)]
pub struct AuthError {
    pub error: String,
}

// ---------- Patients ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patient {
    pub id: i64,
    pub mrn: String,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: String,
    pub gender: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub medicare_number: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePatient {
    pub mrn: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: String,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub medicare_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePatient {
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: String,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub medicare_number: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PatientQuery {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PatientList {
    pub patients: Vec<Patient>,
    pub count: usize,
}

// ---------- Appointments ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appointment {
    pub id: i64,
    pub patient_id: i64,
    pub appointment_type: String,
    pub appointment_date: String,
    pub duration_minutes: i64,
    pub practitioner: Option<String>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: String,
    // joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mrn: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAppointment {
    pub patient_id: i64,
    pub appointment_type: String,
    pub appointment_date: String,
    pub duration_minutes: i64,
    #[serde(default)]
    pub practitioner: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAppointment {
    pub appointment_type: String,
    pub appointment_date: String,
    pub duration_minutes: i64,
    #[serde(default)]
    pub practitioner: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AppointmentQuery {
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub patient_id: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AppointmentList {
    pub appointments: Vec<Appointment>,
    pub count: usize,
}

// ---------- Blocked times (calendar) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedTime {
    pub id: i64,
    pub start_at: String,
    pub end_at: String,
    pub reason: Option<String>,
    pub practitioner: Option<String>,
    pub all_day: bool,
    pub is_recurring: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateBlockedTime {
    pub start_at: String,
    pub end_at: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub practitioner: Option<String>,
    #[serde(default)]
    pub all_day: Option<bool>,
    #[serde(default)]
    pub is_recurring: Option<bool>,
}

// ---------- Generic ----------

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub clinic: String,
}

/// Convenience: parse a flexible datetime string to a normalized one.
/// Accepts ISO 8601 or "YYYY-MM-DD HH:MM". Returns the input unchanged on failure.
pub fn normalize_dt(s: &str) -> String {
    // Try parsing as RFC3339; if ok, re-serialize in a SQLite-friendly form.
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return dt.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.format("%Y-%m-%d 00:00:00").to_string();
    }
    s.to_string()
}

// ---------- Clinical notes ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClinicalNote {
    pub id: i64,
    pub patient_id: i64,
    pub author: Option<String>,
    pub category: String,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateNote {
    pub patient_id: i64,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default = "default_category")]
    pub category: String,
    pub note: String,
}
fn default_category() -> String { "general".into() }

// ---------- Allergies ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allergy {
    pub id: i64,
    pub patient_id: i64,
    pub substance: String,
    pub severity: String,
    pub noted_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAllergy {
    pub patient_id: i64,
    pub substance: String,
    #[serde(default = "default_severity")]
    pub severity: String,
}
fn default_severity() -> String { "mild".into() }

// ---------- OSDI ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsdiScore {
    pub id: i64,
    pub patient_id: i64,
    pub score_date: String,
    pub total_score: f64,
    pub ocular_symptoms: Option<f64>,
    pub vision_function: Option<f64>,
    pub environmental_triggers: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateOsdi {
    pub patient_id: i64,
    pub score_date: String,
    pub total_score: f64,
    #[serde(default)]
    pub ocular_symptoms: Option<f64>,
    #[serde(default)]
    pub vision_function: Option<f64>,
    #[serde(default)]
    pub environmental_triggers: Option<f64>,
}

// ---------- IPL ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IplTreatment {
    pub id: i64,
    pub patient_id: i64,
    pub treatment_date: String,
    pub session_number: i64,
    pub fluence_j_cm2: Option<f64>,
    pub number_of_pulses: Option<i64>,
    pub operator_name: Option<String>,
    pub clinical_notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateIpl {
    pub patient_id: i64,
    pub treatment_date: String,
    pub session_number: i64,
    #[serde(default)]
    pub fluence_j_cm2: Option<f64>,
    #[serde(default)]
    pub number_of_pulses: Option<i64>,
    #[serde(default)]
    pub operator_name: Option<String>,
    #[serde(default)]
    pub clinical_notes: Option<String>,
}

// ---------- Billing catalog ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsultationType {
    pub id: i64,
    pub type_code: String,
    pub type_name: String,
    pub description: Option<String>,
    pub default_price: f64,
    pub default_duration_minutes: i64,
    pub medicare_item_number: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceItem {
    pub id: i64,
    pub service_code: String,
    pub service_name: String,
    pub category: String,
    pub description: Option<String>,
    pub unit_price: f64,
    pub unit_type: String,
    pub tax_rate: f64,
    pub active: bool,
}

// ---------- Invoices ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceItem {
    pub id: i64,
    pub invoice_id: i64,
    pub item_type: String,
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    pub discount_percent: f64,
    pub tax_rate: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: i64,
    pub invoice_number: String,
    pub patient_id: i64,
    pub appointment_id: Option<i64>,
    pub invoice_date: String,
    pub due_date: Option<String>,
    pub subtotal: f64,
    pub tax_amount: f64,
    pub discount_amount: f64,
    pub total_amount: f64,
    pub amount_paid: f64,
    pub balance_due: f64,
    pub status: String,
    pub payment_method: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub items: Vec<InvoiceItem>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvoiceItem {
    pub item_type: String,
    pub description: String,
    #[serde(default = "one")]
    pub quantity: f64,
    pub unit_price: f64,
    #[serde(default)]
    pub discount_percent: f64,
    #[serde(default = "default_tax")]
    pub tax_rate: f64,
}
fn one() -> f64 { 1.0 }
fn default_tax() -> f64 { 0.10 }

#[derive(Debug, Deserialize)]
pub struct CreateInvoice {
    pub patient_id: i64,
    #[serde(default)]
    pub appointment_id: Option<i64>,
    #[serde(default)]
    pub payment_method: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub items: Vec<CreateInvoiceItem>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePayment {
    pub invoice_id: i64,
    pub amount: f64,
    pub payment_method: String,
    #[serde(default)]
    pub reference_number: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    pub id: i64,
    pub invoice_id: i64,
    pub payment_date: String,
    pub amount: f64,
    pub payment_method: String,
    pub reference_number: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

// ---------- Analytics ----------

#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsOverview {
    pub total_patients: i64,
    pub total_appointments: i64,
    pub total_revenue: f64,
    pub outstanding_balance: f64,
    pub appointments_this_month: i64,
    pub revenue_this_month: f64,
    pub avg_appt_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeSeriesPoint {
    pub date: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebsiteTrafficPoint {
    pub date: String,
    pub visitors: i64,
    pub page_views: i64,
    pub bookings: i64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceBreakdown {
    pub source: String,
    pub visitors: i64,
    pub bookings: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RevenueByType {
    pub appointment_type: String,
    pub revenue: f64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NoShowRate {
    pub total: i64,
    pub no_show: i64,
    pub cancelled: i64,
    pub completed: i64,
    pub no_show_rate: f64,
    pub cancellation_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HourCount {
    pub hour: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgeBracket {
    pub bracket: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutstandingPatient {
    pub patient_id: i64,
    pub name: String,
    pub mrn: String,
    pub outstanding: f64,
    pub invoice_count: i64,
}

// ---------- Intake submissions (public input page) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntakeSubmission {
    pub id: i64,
    pub submitted_at: String,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub medicare_number: Option<String>,
    pub preferred_date: Option<String>,
    pub preferred_time: Option<String>,
    pub appointment_type: Option<String>,
    pub symptoms: Option<String>,
    pub source: String,
    pub status: String,
    pub matched_patient_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIntake {
    pub first_name: String,
    pub last_name: String,
    #[serde(default)]
    pub date_of_birth: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub medicare_number: Option<String>,
    #[serde(default)]
    pub preferred_date: Option<String>,
    #[serde(default)]
    pub preferred_time: Option<String>,
    #[serde(default)]
    pub appointment_type: Option<String>,
    #[serde(default)]
    pub symptoms: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

// ---------- Messages (unified inbox) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    pub received_at: String,
    pub channel: String,
    pub from_name: Option<String>,
    pub from_contact: Option<String>,
    pub subject: Option<String>,
    pub body: String,
    pub status: String,
    pub linked_patient_id: Option<i64>,
    pub thread_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateMessage {
    pub channel: String,
    #[serde(default)]
    pub from_name: Option<String>,
    #[serde(default)]
    pub from_contact: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    pub body: String,
    #[serde(default)]
    pub thread_id: Option<String>,
}

// ---------- Users (staff management) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffUser {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub role: String,
    pub first_name: String,
    pub last_name: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: String,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUser {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub password: Option<String>,
}

// ---------- Patient photos / files ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientPhoto {
    pub id: i64,
    pub patient_id: i64,
    pub category: String, // profile | medical | document
    pub filename: String,
    pub mime_type: String,
    pub caption: Option<String>,
    pub file_size: Option<i64>,
    pub captured_at: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadPhoto {
    pub patient_id: i64,
    #[serde(default = "default_photo_category")]
    pub category: String,
    pub filename: String,
    #[serde(default = "default_mime")]
    pub mime_type: String,
    #[serde(default)]
    pub caption: Option<String>,
    pub data_base64: String,
}
fn default_photo_category() -> String { "document".into() }
fn default_mime() -> String { "image/jpeg".into() }

// ---------- Booking settings ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingSettings {
    pub booking_mode: String,
    pub auto_confirm_message: bool,
    pub auto_reminder_message: bool,
    pub reminder_hours_before: i64,
    pub email_provider: String,
    pub email_api_key: Option<String>,
    pub email_from: String,
    pub sms_provider: String,
    pub sms_api_key: Option<String>,
    pub sms_sender: String,
    pub template_booking_received: String,
    pub template_booking_confirmed: String,
    pub template_booking_declined: String,
    pub template_reminder: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBookingSettings {
    #[serde(default)]
    pub booking_mode: Option<String>,
    #[serde(default)]
    pub auto_confirm_message: Option<bool>,
    #[serde(default)]
    pub auto_reminder_message: Option<bool>,
    #[serde(default)]
    pub reminder_hours_before: Option<i64>,
    #[serde(default)]
    pub email_provider: Option<String>,
    #[serde(default)]
    pub email_api_key: Option<String>,
    #[serde(default)]
    pub email_from: Option<String>,
    #[serde(default)]
    pub sms_provider: Option<String>,
    #[serde(default)]
    pub sms_api_key: Option<String>,
    #[serde(default)]
    pub sms_sender: Option<String>,
    #[serde(default)]
    pub template_booking_received: Option<String>,
    #[serde(default)]
    pub template_booking_confirmed: Option<String>,
    #[serde(default)]
    pub template_booking_declined: Option<String>,
    #[serde(default)]
    pub template_reminder: Option<String>,
}

// ---------- Booking notifications ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingNotification {
    pub id: i64,
    pub booking_id: Option<i64>,
    pub intake_submission_id: Option<i64>,
    pub channel: String,
    pub recipient: String,
    pub template_used: Option<String>,
    pub body: String,
    pub status: String,
    pub provider_response: Option<String>,
    pub sent_at: Option<String>,
    pub created_at: String,
}
