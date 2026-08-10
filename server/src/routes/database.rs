//! Database portability endpoints.
//!
//! These let an admin, from the Settings screen, choose WHICH SQLite file
//! OptiCore uses as the clinic database — link to an existing one, start a new
//! empty one, or load sample demo data into the current one. The chosen path is
//! persisted to `opticore.config.json` (see [`crate::config`]) and takes effect
//! on the NEXT app restart; the running server is never hot-swapped (that would
//! be unsafe with open connections mid-request). The frontend triggers the
//! restart via a Tauri command after a successful link/new.
//!
//! SAFETY: none of these endpoints ever delete, move, or overwrite a database
//! file. "Switching" the database only rewrites a path in a JSON config file —
//! the previous database stays exactly where it is and can be re-linked later.
//!
//! Auth: mounted under the admin router (see `lib.rs`), so `require_admin`
//! guards every call — only an authenticated admin can change the DB location.

use std::path::{Path, PathBuf};

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

/// The demo "sentinel" patient inserted by migration 0017. Its presence means
/// the rich demo dataset has been seeded into the current database.
const DEMO_SENTINEL_MRN: &str = "MOS-2026000006";

#[derive(Debug, Serialize)]
pub struct DatabaseInfo {
    /// The effective database file path (forward slashes), as currently
    /// configured / defaulted.
    pub current_path: String,
    pub file_exists: bool,
    pub file_size_bytes: i64,
    pub patient_count: i64,
    pub is_demo_seeded: bool,
    /// Where the config file lives (forward slashes) — shown for transparency.
    pub config_path: String,
}

/// GET /api/database — report the current database location and status.
pub async fn info(State(state): State<AppState>) -> ApiResult<Json<DatabaseInfo>> {
    let db_path = config::resolved_db_path();
    let (file_exists, file_size_bytes) = match std::fs::metadata(&db_path) {
        Ok(m) => (true, m.len() as i64),
        Err(_) => (false, 0),
    };

    // Patient count comes from the CURRENTLY running pool (which is the
    // configured DB — startup resolves the same path), not from opening the
    // file again. This is accurate for the live server.
    let patient_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM patients")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let is_demo_seeded: bool =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM patients WHERE mrn = ?")
            .bind(DEMO_SENTINEL_MRN)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0)
            > 0;

    Ok(Json(DatabaseInfo {
        current_path: config::to_forward_slashes(&db_path),
        file_exists,
        file_size_bytes,
        patient_count,
        is_demo_seeded,
        config_path: config::to_forward_slashes(&config::config_path()),
    }))
}

#[derive(Debug, Deserialize)]
pub struct PathBody {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct RestartResponse {
    pub ok: bool,
    pub restart_required: bool,
}

/// Validate that `path` names a file we can use as the clinic database.
///
///   * The parent directory must exist OR be creatable (we create it here for
///     `new`; for `link` we require it to already exist).
///   * If the file already exists it must look like a real SQLite database
///     (first 16 bytes are the `SQLite format 3\0` magic header). This is a
///     cheap, dependency-free integrity check that avoids opening a full pool.
///
/// The path is NEVER interpolated into SQL — it only ever becomes part of the
/// `DATABASE_URL` and the JSON config file.
fn validate_target(path: &Path, must_be_sqlite_if_exists: bool) -> Result<(), ApiError> {
    if path.as_os_str().is_empty() {
        return Err(ApiError::BadRequest("Path must not be empty".into()));
    }
    // Reject a path that is an existing directory (can't be a DB file).
    if path.is_dir() {
        return Err(ApiError::BadRequest(
            "That path is a folder, not a database file".into(),
        ));
    }

    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    match parent {
        Some(dir) => {
            if !dir.exists() {
                return Err(ApiError::BadRequest(format!(
                    "The folder '{}' does not exist",
                    config::to_forward_slashes(dir)
                )));
            }
        }
        None => {
            // No parent means a bare filename in the cwd — allowed.
        }
    }

    if must_be_sqlite_if_exists && path.exists() {
        if !looks_like_sqlite(path) {
            return Err(ApiError::BadRequest(
                "That file does not look like a SQLite database".into(),
            ));
        }
    }
    Ok(())
}

/// Cheap header sniff: a valid SQLite database file begins with the 16-byte
/// magic string "SQLite format 3\0". An empty (zero-byte) file is also accepted
/// because SQLite will initialise it on first open.
fn looks_like_sqlite(path: &Path) -> bool {
    use std::io::Read;
    const MAGIC: &[u8; 16] = b"SQLite format 3\0";
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    // A brand-new/empty file is valid (SQLite will create the schema).
    if f.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return true;
    }
    let mut header = [0u8; 16];
    match f.read_exact(&mut header) {
        Ok(_) => &header == MAGIC,
        Err(_) => false,
    }
}

/// POST /api/database/link — point OptiCore at an EXISTING database file.
///
/// Validates the path (parent must exist; if the file exists it must be a
/// SQLite file), writes it to the config, and asks the app to restart. Does
/// NOT switch the running server.
pub async fn link(
    State(_state): State<AppState>,
    Json(body): Json<PathBody>,
) -> ApiResult<Json<RestartResponse>> {
    let path = PathBuf::from(body.path.trim());
    validate_target(&path, true)?;

    config::write_db_path(&path)
        .map_err(|e| ApiError::Internal(format!("Could not save config: {e}")))?;

    Ok(Json(RestartResponse {
        ok: true,
        restart_required: true,
    }))
}

/// POST /api/database/new — start a BRAND-NEW empty database at `path`.
///
/// Creates the parent directories, writes the config, and asks for a restart.
/// The file itself is created by SQLite on the next boot (mode=rwc) and
/// migrations run on it; no demo data is seeded (the demo-seed migrations guard
/// on an existing demo patient, which a fresh DB has none of).
pub async fn new_database(
    State(_state): State<AppState>,
    Json(body): Json<PathBody>,
) -> ApiResult<Json<RestartResponse>> {
    let path = PathBuf::from(body.path.trim());
    if path.as_os_str().is_empty() {
        return Err(ApiError::BadRequest("Path must not be empty".into()));
    }
    if path.is_dir() {
        return Err(ApiError::BadRequest(
            "That path is a folder, not a database file".into(),
        ));
    }

    // Create parent dirs so the new DB can be created there on next boot.
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .map_err(|e| ApiError::BadRequest(format!("Could not create folder: {e}")))?;
    }

    // If a file already exists at this path, refuse rather than risk pointing a
    // "new empty database" at real data (we never overwrite it — refuse and let
    // the user pick "Link" instead).
    if path.exists() {
        return Err(ApiError::Conflict(
            "A file already exists there. Use \"Link to an existing database\" to open it, or choose a different name.".into(),
        ));
    }

    config::write_db_path(&path)
        .map_err(|e| ApiError::Internal(format!("Could not save config: {e}")))?;

    Ok(Json(RestartResponse {
        ok: true,
        restart_required: true,
    }))
}

/// POST /api/database/load-demo — load the sample demo dataset into the CURRENT
/// database.
///
/// We do NOT seed inline here (that would race with in-flight requests and
/// duplicate the delicate guarded migration logic). Instead we set a one-shot
/// flag in `app_meta` and restart: on the next boot, if `force_demo_seed` is
/// set AND the database currently has 0 patients, the bundled seed SQL is
/// applied and the flag cleared (see [`crate::db::maybe_force_demo_seed`]).
///
/// Guard: only allowed when the current database is empty (0 patients), so this
/// can never inject demo rows into a real clinic database.
pub async fn load_demo(State(state): State<AppState>) -> ApiResult<Json<RestartResponse>> {
    let patient_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM patients")
        .fetch_one(&state.db)
        .await?;
    if patient_count > 0 {
        return Err(ApiError::Conflict(
            "This database already contains patients — demo data can only be loaded into an empty database.".into(),
        ));
    }

    sqlx::query("INSERT OR REPLACE INTO app_meta (key, value) VALUES ('force_demo_seed', datetime('now'))")
        .execute(&state.db)
        .await?;

    Ok(Json(RestartResponse {
        ok: true,
        restart_required: true,
    }))
}
