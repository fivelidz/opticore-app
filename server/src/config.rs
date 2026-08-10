//! Database-portability config.
//!
//! OptiCore stores the *location* of the clinic database in a small JSON file
//! that lives NEXT TO the app's per-user data dir — NOT inside the database and
//! NOT inside the installed program folder. This is what makes the clinic data
//! portable and survivable across uninstall/reinstall:
//!
//!   * On Windows the data dir is `%LOCALAPPDATA%\OptiCore`, which the MSI/NSIS
//!     uninstaller does NOT touch (it only removes files under Program Files).
//!     So both the config file and a default-located database survive a
//!     reinstall automatically.
//!   * The user can also point `db_path` at ANY location — e.g.
//!     `C:/OptiCoreData/clinic.db`, a Documents folder, or a network share.
//!     They can then back up the clinic by copying that one file, and re-open
//!     it after reinstalling by linking to it again.
//!
//! The config is intentionally tiny and forgiving: if the file is missing or
//! malformed we fall back to the built-in default database path. We NEVER
//! delete or move a database as a side effect of reading/writing this config.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The on-disk shape of `opticore.config.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    /// Absolute path to the SQLite database the user has chosen, stored with
    /// forward slashes for cross-platform safety. `None`/absent => use the
    /// built-in default (see [`default_db_path`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_path: Option<String>,
}

/// Where OptiCore keeps per-user, writable data.
///   Windows: `%LOCALAPPDATA%\OptiCore`
///   macOS:   `~/Library/Application Support/OptiCore`
///   Linux:   `~/.local/share/OptiCore`
///
/// This mirrors the `data_dir()` helper in the Tauri shell so the server and
/// the shell always agree on where the config file lives.
pub fn data_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var("LOCALAPPDATA").ok().map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".local/share"))
    };
    base.unwrap_or_else(|| PathBuf::from(".")).join("OptiCore")
}

/// Absolute path of the config file: `<data_dir>/opticore.config.json`.
pub fn config_path() -> PathBuf {
    data_dir().join("opticore.config.json")
}

/// The built-in default database path (`<data_dir>/opticore.db`).
///
/// Used when no config file exists or it does not specify a `db_path`. Note the
/// Tauri shell may additionally consult `OPTICORE_MODE` for a demo-vs-live file
/// name as a *fallback*; the config file always wins over that fallback.
pub fn default_db_path() -> PathBuf {
    data_dir().join("opticore.db")
}

/// Normalize a filesystem path to a forward-slash string for portable storage.
pub fn to_forward_slashes(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// Read and parse the config file. Missing or malformed files yield a default
/// (empty) config — we never error out reading it, so a corrupt config can
/// never brick startup or the Settings screen. Data is never lost by this.
pub fn read_config() -> AppConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str::<AppConfig>(&s).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

/// Persist the chosen database path into `opticore.config.json`, creating the
/// data dir if needed. The path is stored with forward slashes.
pub fn write_db_path(db_path: &Path) -> std::io::Result<()> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    let cfg = AppConfig {
        db_path: Some(to_forward_slashes(db_path)),
    };
    let json = serde_json::to_string_pretty(&cfg)
        .unwrap_or_else(|_| "{}".to_string());
    std::fs::write(config_path(), json)
}

/// Resolve the effective database file path: the configured `db_path` if set,
/// else the built-in default. Returns a real `PathBuf` (config value is stored
/// with forward slashes, which is a valid path form on every OS SQLite runs on).
pub fn resolved_db_path() -> PathBuf {
    match read_config().db_path {
        Some(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => default_db_path(),
    }
}

/// Build a `sqlite://<path>?mode=rwc` URL from a filesystem path, using forward
/// slashes so it is valid on Windows too. `mode=rwc` means "read-write-create":
/// SQLite creates the file if it does not exist yet (and migrations then run on
/// it), so pointing at a not-yet-existing path is safe and never loses data.
pub fn sqlite_url_for(db_path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", to_forward_slashes(db_path))
}
