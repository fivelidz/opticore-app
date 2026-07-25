//! OptiCore PMS — Rust LAN server.
//!
//! Single static binary. axum + sqlx (SQLite). Runs on the clinic's always-on
//! server PC and serves every Tauri client on the LAN.
//!
//! Ported route-by-route from opticore/backend (TypeScript), with the async
//! bugs fixed (sqlx compile-time-checked queries) and authentication always on.

mod auth;
mod db;
mod error;
mod routes;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use shared::HealthResponse;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub jwt: Arc<auth::JwtCfg>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=info,tower_http=info".into()),
        )
        .init();

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        // Default: a local SQLite file next to the binary / in the workspace.
        "sqlite://pms.db?mode=rwc".to_string()
    });

    let pool = db::init_pool(&db_url).await?;
    db::run_migrations(&pool).await?;
    db::ensure_admin(&pool).await?;

    let jwt = Arc::new(auth::JwtCfg::from_env());
    let state = AppState { db: pool.clone(), jwt };

    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    // Static input page (public, served at /input)
    let static_input = tower_http::services::ServeFile::new("server/static/input.html");
    let static_showcase = tower_http::services::ServeFile::new("server/static/showcase.html");

    // Public routes (no auth)
    let public = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/login", post(routes::auth::login))
        .route("/api/intake/submit", post(routes::intake::submit))
        .route("/api/messages/receive", post(routes::messages::receive))
        .route("/api/public/availability/:days", get(routes::public_api::availability))
        .route("/api/public/appointment-types", get(routes::public_api::appointment_types))
        .route("/api/public/match-patient", post(routes::public_api::match_patient))
        .route_service("/input", static_input)
        .route_service("/showcase", static_showcase);

    // Protected routes (require valid JWT)
    let protected = Router::new()
        .route("/api/auth/me", get(routes::auth::me))
        .route("/api/auth/change-password", post(routes::change_password::change_password))
        .route(
            "/api/patients",
            get(routes::patients::list).post(routes::patients::create),
        )
        .route(
            "/api/patients/:id",
            get(routes::patients::get_one)
                .put(routes::patients::update)
                .delete(routes::patients::delete),
        )
        .route("/api/patients/:id/detail", get(routes::patient_detail::detail))
        .route("/api/patients/enriched/list", get(routes::patients::list_enriched))
        // patient photos
        .route(
            "/api/patients/:id/photos",
            get(routes::photos::list).post(routes::photos::upload),
        )
        .route(
            "/api/patients/:id/photos/:photo",
            axum::routing::get(routes::photos::get_data).delete(routes::photos::delete),
        )
        .route("/api/patients/:id/photos/:photo/make-profile", post(routes::photos::make_profile))
        .route(
            "/api/patients/:id/notes",
            get(routes::clinical::list_notes).post(routes::clinical::add_note),
        )
        .route("/api/patients/:id/notes/:nid", axum::routing::delete(routes::clinical::del_note))
        .route(
            "/api/patients/:id/allergies",
            get(routes::clinical::list_allergies),
        )
        .route("/api/allergies", post(routes::clinical::add_allergy))
        .route("/api/allergies/:id", axum::routing::delete(routes::clinical::del_allergy))
        .route(
            "/api/patients/:id/osdi",
            get(routes::clinical::list_osdi).post(routes::clinical::add_osdi),
        )
        .route(
            "/api/patients/:id/ipl",
            get(routes::clinical::list_ipl).post(routes::clinical::add_ipl),
        )
        .route(
            "/api/appointments",
            get(routes::appointments::list).post(routes::appointments::create),
        )
        .route("/api/appointments/today", get(routes::appointments::today))
        .route(
            "/api/appointments/:id",
            get(routes::appointments::get_one)
                .put(routes::appointments::update)
                .delete(routes::appointments::delete),
        )
        .route(
            "/api/blocked-times",
            get(routes::blocked::list).post(routes::blocked::create),
        )
        .route(
            "/api/blocked-times/:id",
            axum::routing::put(routes::blocked::update).delete(routes::blocked::delete),
        )
        .route("/api/calendar/:from/:to", get(routes::calendar::range))
        // billing catalog
        .route("/api/billing/consultation-types", get(routes::billing::consultation_types))
        .route("/api/billing/services", get(routes::billing::services))
        .route("/api/billing/service-categories", get(routes::billing::service_categories))
        // invoices + payments
        .route("/api/billing/invoices/patient/:pid", get(routes::billing::invoices_by_patient))
        .route("/api/billing/invoices", post(routes::billing::create_invoice))
        .route("/api/billing/payments/invoice/:inv", get(routes::billing::payments_by_invoice))
        .route("/api/billing/payments", post(routes::billing::add_payment))
        // analytics
        .route("/api/analytics/overview", get(routes::analytics::overview))
        .route("/api/analytics/revenue/:days", get(routes::analytics::revenue_series))
        .route("/api/analytics/appointments/:days", get(routes::analytics::appointment_series))
        .route("/api/analytics/traffic/:days", get(routes::analytics::traffic_series))
        .route("/api/analytics/traffic-by-source", get(routes::analytics::traffic_by_source))
        // intake management (staff)
        .route("/api/intake", get(routes::intake::list))
        .route("/api/intake/:id/import", post(routes::intake::import))
        .route("/api/intake/:id/archive", post(routes::intake::archive))
        .route("/api/intake/auto-import", post(routes::intake::auto_import))
        // messages inbox
        .route(
            "/api/messages",
            get(routes::messages::list).post(routes::messages::receive),
        )
        .route("/api/messages/:id/read", post(routes::messages::mark_read))
        .route("/api/messages/:id/archive", post(routes::messages::archive))
        .route("/api/messages/:id/link/:pid", post(routes::messages::link_patient))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    // Admin-only routes (require admin role on top of auth)
    let admin = Router::new()
        .route(
            "/api/users",
            get(routes::users::list).post(routes::users::create),
        )
        .route(
            "/api/users/:id",
            axum::routing::put(routes::users::update).delete(routes::users::delete),
        )
        .route("/api/users/:id/toggle", post(routes::users::toggle_active))
        // data export/import (admin-only)
        .route("/api/data/export", post(routes::data_io::export_data))
        .route("/api/data/import", post(routes::data_io::import_data))
        .route("/api/data/version", get(routes::data_io::version_info))
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_admin));

    let app = Router::new()
        .merge(public)
        .merge(protected)
        .merge(admin)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("🩺 PMS server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health(
    axum::extract::State(_s): axum::extract::State<AppState>,
) -> axum::Json<HealthResponse> {
    axum::Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        clinic: "OptiCore".into(),
    })
}
