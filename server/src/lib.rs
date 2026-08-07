//! OptiCore server library — exposes run() so the Tauri app can embed the server.

pub mod auth;
pub mod db;
pub mod error;
pub mod routes;
pub mod sync;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::error::ApiResult;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub jwt: Arc<auth::JwtCfg>,
}

/// Build the public (unauthenticated) sub-router. Does NOT include static file
/// routes — those are added by `run()` since they depend on the runtime cwd.
fn public_router() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/login", post(routes::auth::login))
        .route("/api/intake/submit", post(routes::intake::submit))
        .route("/api/messages/receive", post(routes::messages::receive))
        .route("/api/public/availability/:days", get(routes::public_api::availability))
        .route("/api/public/appointment-types", get(routes::public_api::appointment_types))
        .route("/api/public/match-patient", post(routes::public_api::match_patient))
}

/// Build the protected (auth-required) sub-router.
fn protected_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/auth/me", get(routes::auth::me))
        .route("/api/auth/change-password", post(routes::change_password::change_password))
        .route("/api/patients", get(routes::patients::list).post(routes::patients::create))
        .route("/api/patients/:id", get(routes::patients::get_one).put(routes::patients::update).delete(routes::patients::delete))
        .route("/api/patients/:id/detail", get(routes::patient_detail::detail))
        .route("/api/patients/enriched/list", get(routes::patients::list_enriched))
        .route("/api/patients/:id/photos", get(routes::photos::list).post(routes::photos::upload))
        .route("/api/patients/:id/photos/:photo", axum::routing::get(routes::photos::get_data).delete(routes::photos::delete))
        .route("/api/patients/:id/photos/:photo/make-profile", post(routes::photos::make_profile))
        .route("/api/patients/:id/notes", get(routes::clinical::list_notes).post(routes::clinical::add_note))
        .route("/api/patients/:id/notes/:nid", axum::routing::delete(routes::clinical::del_note))
        .route("/api/patients/:id/allergies", get(routes::clinical::list_allergies))
        .route("/api/allergies", post(routes::clinical::add_allergy))
        .route("/api/allergies/:id", axum::routing::delete(routes::clinical::del_allergy))
        .route("/api/patients/:id/osdi", get(routes::clinical::list_osdi).post(routes::clinical::add_osdi))
        .route("/api/patients/:id/ipl", get(routes::clinical::list_ipl).post(routes::clinical::add_ipl))
        .route("/api/appointments", get(routes::appointments::list).post(routes::appointments::create))
        .route("/api/appointments/today", get(routes::appointments::today))
        .route("/api/appointments/:id", get(routes::appointments::get_one).put(routes::appointments::update).delete(routes::appointments::delete))
        .route("/api/appointments/:id/attachments", get(routes::photos::list_by_appointment).post(routes::photos::upload_to_appointment))
        .route("/api/appointments/:id/attachments/:photo", axum::routing::get(routes::photos::get_appointment_data).delete(routes::photos::delete_appointment_attachment))
        .route("/api/blocked-times", get(routes::blocked::list).post(routes::blocked::create))
        .route("/api/blocked-times/:id", axum::routing::put(routes::blocked::update).delete(routes::blocked::delete))
        .route("/api/calendar/:from/:to", get(routes::calendar::range))
        .route("/api/billing/consultation-types", get(routes::billing::consultation_types))
        .route("/api/billing/services", get(routes::billing::services))
        .route("/api/billing/service-categories", get(routes::billing::service_categories))
        .route("/api/billing/invoices/patient/:pid", get(routes::billing::invoices_by_patient))
        .route("/api/billing/invoices", post(routes::billing::create_invoice))
        .route("/api/billing/payments/invoice/:inv", get(routes::billing::payments_by_invoice))
        .route("/api/billing/payments", post(routes::billing::add_payment))
        .route("/api/analytics/overview", get(routes::analytics::overview))
        .route("/api/analytics/revenue/:days", get(routes::analytics::revenue_series))
        .route("/api/analytics/appointments/:days", get(routes::analytics::appointment_series))
        .route("/api/analytics/traffic/:days", get(routes::analytics::traffic_series))
        .route("/api/analytics/traffic-by-source", get(routes::analytics::traffic_by_source))
        .route("/api/analytics/patient-growth/:days", get(routes::analytics::patient_growth))
        .route("/api/analytics/revenue-by-type", get(routes::analytics::revenue_by_type))
        .route("/api/analytics/no-show-rate", get(routes::analytics::no_show_rate))
        .route("/api/analytics/hour-distribution", get(routes::analytics::hour_distribution))
        .route("/api/analytics/age-demographics", get(routes::analytics::age_demographics))
        .route("/api/analytics/outstanding-by-patient", get(routes::analytics::outstanding_by_patient))
        .route("/api/intake", get(routes::intake::list))
        .route("/api/intake/:id/import", post(routes::intake::import))
        .route("/api/intake/:id/archive", post(routes::intake::archive))
        .route("/api/intake/auto-import", post(routes::intake::auto_import))
        .route("/api/intake/:id/match-check", post(routes::intake::match_check))
        .route("/api/intake/:id/merge-into/:patient_id", post(routes::intake::merge_into))
        .route("/api/messages", get(routes::messages::list).post(routes::messages::receive))
        .route("/api/messages/:id/read", post(routes::messages::mark_read))
        .route("/api/messages/:id/archive", post(routes::messages::archive))
        .route("/api/messages/:id/link/:pid", post(routes::messages::link_patient))
        .route("/api/booking-settings", get(routes::booking_settings::get_settings).put(routes::booking_settings::update_settings))
        .route("/api/booking-notifications", get(routes::booking_settings::list_notifications).post(routes::booking_settings::send_pending))
        .route("/api/intake/:id/approve", post(routes::booking_settings::approve_intake))
        .route("/api/intake/:id/decline", post(routes::booking_settings::decline_intake))
        .route("/api/sync/status", get(sync_status))
        .route("/api/sync/now", post(sync_now))
        .layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware))
}

/// Build the admin (admin-role-required) sub-router.
fn admin_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/users", get(routes::users::list).post(routes::users::create))
        .route("/api/users/:id", axum::routing::put(routes::users::update).delete(routes::users::delete))
        .route("/api/users/:id/toggle", post(routes::users::toggle_active))
        .route("/api/data/export", post(routes::data_io::export_data))
        .route("/api/data/import", post(routes::data_io::import_data))
        .route("/api/data/version", get(routes::data_io::version_info))
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_admin))
}

/// Build the full application Router from an AppState, including CORS + tracing
/// layers but NOT the static-file routes (those are cwd-dependent and only
/// relevant to the running server, not to tests).
///
/// Exposed publicly so integration tests can construct the app without binding
/// a TCP socket — they drive it with `tower::ServiceExt::oneshot` or a
/// `reqwest` client against a bound ephemeral port.
pub fn build_app(state: AppState) -> Router {
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);
    Router::new()
        .merge(public_router())
        .merge(protected_router(state.clone()))
        .merge(admin_router(state.clone()))
        .layer(middleware::from_fn(normalize_error_response))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Middleware that normalizes error responses to the app's JSON error shape.
///
/// axum's built-in extractors (`Json`, `Path`, `Query`) produce their own
/// rejection responses on bad input — plain text bodies with a 400/404/422
/// status. This breaks API consistency: every handler-driven error path
/// returns `{"error": "..."}` via `ApiError::into_response()`, but extractor
/// rejections return raw text. API clients that parse the `error` field then
/// fail on these responses.
///
/// This middleware inspects every response: if the status is 4xx/5xx AND the
/// content-type is NOT already `application/json`, it re-wraps the body as
/// `{"error": "<original body text>"}` with `application/json`. Responses that
/// are already JSON (the normal `ApiError` path) pass through unchanged.
async fn normalize_error_response(
    req: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let resp = next.run(req).await;
    let status = resp.status();

    // Only re-wrap client/server error responses (4xx/5xx). Success responses
    // (2xx/3xx) pass through untouched.
    if !status.is_client_error() && !status.is_server_error() {
        return resp;
    }

    // If the response is already JSON, leave it alone (the normal ApiError
    // path already produces the right shape).
    let already_json = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.starts_with("application/json"))
        .unwrap_or(false);
    if already_json {
        return resp;
    }

    // Extract the original body text (axum extractor rejections put a
    // human-readable message in the body, e.g. "Failed to deserialize the JSON
    // body into the target type: ...").
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap_or_default();
    let msg = String::from_utf8_lossy(&bytes);
    let body = serde_json::json!({ "error": msg });
    (
        status,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        Json(body),
    )
        .into_response()
}

/// Start the HTTP server. Blocks until the server stops.
/// Called by either the standalone server binary or the Tauri app.
pub async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=info,tower_http=info".into()),
        )
        .init();

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "sqlite://opticore.db?mode=rwc".to_string()
    });

    let pool = db::init_pool(&db_url).await?;
    db::run_migrations(&pool).await?;
    db::ensure_admin(&pool).await?;

    let jwt = Arc::new(auth::JwtCfg::from_env());
    let state = AppState { db: pool.clone(), jwt };

    sync::start(state.clone());

    // Static HTML pages served from disk (cwd-dependent; not part of build_app
    // so tests don't need the static files to exist).
    let static_input = tower_http::services::ServeFile::new("server/static/input.html");
    let static_showcase = tower_http::services::ServeFile::new("server/static/showcase.html");
    let static_online = tower_http::services::ServeFile::new("server/static/online-booking.html");

    // Serve the built frontend SPA (frontend/dist/) so that other devices on
    // the LAN can open the full PMS UI in a browser by navigating to
    // http://<this-machine-IP>:3000/ — no Tauri desktop app needed.
    //
    // The ServeDir is a fallback: any request that doesn't match an /api/*
    // route or a named static page falls through to the SPA. If the file
    // exists (e.g. /assets/index.js) it's served; otherwise index.html is
    // returned so client-side routing works (e.g. /patients/42).
    //
    // The path is configurable via FRONTEND_DIST (defaults to the repo-relative
    // location). Not part of build_app so tests don't need the dist to exist.
    let frontend_dist =
        std::env::var("FRONTEND_DIST").unwrap_or_else(|_| "frontend/dist".to_string());
    let spa = tower_http::services::ServeDir::new(&frontend_dist)
        .fallback(tower_http::services::ServeFile::new(format!("{}/index.html", frontend_dist)));

    let app = build_app(state)
        .route_service("/input", static_input)
        .route_service("/showcase", static_showcase)
        .route_service("/book", static_online)
        .fallback_service(spa);

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("🩺 OptiCore server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health(axum::extract::State(_s): axum::extract::State<AppState>) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "clinic": "OptiCore",
        "mode": if std::env::var("CLEAN_START").is_ok() { "production" } else { "demo" }
    }))
}

async fn sync_status(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> ApiResult<axum::Json<serde_json::Value>> {
    let worker_url = std::env::var("WORKER_URL").unwrap_or_default();
    let configured = !worker_url.is_empty();
    // Propagate DB errors as 500 — previously these used `.ok().flatten()`,
    // silently returning pending_intake=0 / null last_worker_intake during a DB
    // outage (indistinguishable from "no pending intake").
    let pending: Option<(i64,)> = sqlx::query_as("SELECT COUNT(*) FROM intake_submissions WHERE status='new'")
        .fetch_optional(&state.db).await?;
    let last_sync: Option<(String,)> = sqlx::query_as("SELECT created_at FROM intake_submissions WHERE source='worker-sync' ORDER BY created_at DESC LIMIT 1")
        .fetch_optional(&state.db).await?;
    Ok(axum::Json(serde_json::json!({
        "configured": configured,
        "worker_url": worker_url,
        "sync_interval_secs": 30,
        "pending_intake": pending.map(|(c,)| c).unwrap_or(0),
        "last_worker_intake": last_sync.map(|(s,)| s),
    })))
}

async fn sync_now(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> ApiResult<axum::Json<serde_json::Value>> {
    // Propagate DB errors as 500 — previously this caught ALL errors and
    // returned 200 with {"ok": false, "error": "..."}, masking DB outages.
    sync::run_sync_cycle(&state).await?;
    Ok(axum::Json(serde_json::json!({"ok": true, "message": "Sync cycle complete"})))
}
