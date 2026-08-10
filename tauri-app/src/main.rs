// OptiCore PMS — Tauri desktop shell.
//
// On launch:
// 1. Starts the Rust HTTP server (axum + SQLite) in a background thread
// 2. Waits for the server to be ready (polls localhost:3000/api/health)
// 3. Opens the Tauri webview window
//
// The frontend talks to the embedded server on http://localhost:3000.

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use std::env;
use std::path::PathBuf;

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Restart the whole OptiCore app. Called by the frontend after the user links
/// to a different database or creates a new one — the embedded server rebinds
/// to the newly-configured database file on the next boot. This never deletes
/// any data; it just re-reads `opticore.config.json` and re-opens the DB.
#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    app.restart();
}

/// Where to store the database — a writable per-user data directory.
/// On Windows: %LOCALAPPDATA%\OptiCore
/// On macOS:   ~/Library/Application Support/OptiCore
/// On Linux:   ~/.local/share/OptiCore
fn data_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        env::var("LOCALAPPDATA").ok().map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        env::var("HOME").ok().map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        env::var("HOME").ok().map(|h| PathBuf::from(h).join(".local/share"))
    };
    base.unwrap_or_else(|| PathBuf::from(".")).join("OptiCore")
}

fn main() {
    // ---- Set default environment ----
    if env::var("PORT").is_err() {
        env::set_var("PORT", "3000");
    }

    // Default admin password to "admin" so the app works out of the box after
    // install. The user changes it in Settings after first login.
    if env::var("DEV_ADMIN_PASSWORD").is_err() {
        env::set_var("DEV_ADMIN_PASSWORD", "admin");
    }

    // ── DATABASE LOCATION (portable) ───────────────────────────────────────
    //
    // OptiCore stores the clinic database at a location the USER controls, kept
    // in a small portable config file: <data_dir>/opticore.config.json.
    //
    // WHY THIS PROTECTS DATA:
    //   * The default DB lives in the per-user data dir (%LOCALAPPDATA%\OptiCore
    //     on Windows), which is OUTSIDE Program Files — so the MSI/NSIS
    //     uninstaller never touches it. Data survives uninstall/reinstall.
    //   * The user can also point the DB at any path (e.g. C:/OptiCoreData/
    //     clinic.db, a Documents folder, or a network share) from Settings.
    //     They back up the clinic by copying that ONE file and re-open it after
    //     a reinstall by linking to it again — no data migration needed.
    //
    // There is now ONE configured database. "Demo vs live" is about the DATA in
    // it (see the Settings "Load demo data" button + the load-demo endpoint),
    // not about which file we open. OPTICORE_MODE is kept only as a FALLBACK for
    // when no config file exists yet.
    let mode = env::var("OPTICORE_MODE").unwrap_or_default().to_lowercase();
    let is_live = mode == "live" || mode == "production" || mode == "final";

    // Store the DB in a writable location (Program Files is read-only).
    if env::var("DATABASE_URL").is_err() {
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);

        // 1. Config file wins: if the user has chosen a db_path, use it.
        //    server::config reads <data_dir>/opticore.config.json and falls
        //    back to <data_dir>/opticore.db when unset/malformed. We only treat
        //    an EXPLICIT db_path as "configured" so the OPTICORE_MODE fallback
        //    below still applies on a brand-new install with no config yet.
        let cfg = server::config::read_config();
        let configured = cfg.db_path.as_deref().map(str::trim).filter(|s| !s.is_empty());

        let db_path = match configured {
            Some(p) => std::path::PathBuf::from(p),
            None => {
                // 2. Fallback: legacy OPTICORE_MODE demo-vs-live file name.
                let db_name = if is_live { "opticore.db" } else { "opticore-demo.db" };
                dir.join(db_name)
            }
        };

        // Decide whether this boot should start EMPTY (wipe the seed data the
        // migrations always insert). CLEAN_START only ever wipes ONCE, on a
        // brand-new file — db.rs records a marker and never wipes an existing
        // database again, so real clinic data is never touched on later boots.
        //
        // We start empty when the target DB file does NOT yet exist AND either:
        //   * a db_path is configured (the user picked "Start a new empty
        //     database…" — its file won't exist yet, so this is a fresh empty
        //     DB), OR
        //   * no config yet and we're in the legacy LIVE fallback.
        //
        // When a configured file ALREADY EXISTS (the user linked to existing
        // data, e.g. after reinstalling), we never set CLEAN_START — that data
        // is authoritative and must be preserved untouched.
        let file_is_new = !db_path.exists();
        let want_empty = file_is_new && (configured.is_some() || is_live);
        if want_empty && env::var("CLEAN_START").is_err() {
            env::set_var("CLEAN_START", "1");
        }

        // Make sure the parent dir of a configured path exists (SQLite creates
        // the file itself via mode=rwc, but not the directory).
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Forward slashes for the sqlite URL (valid on Windows too).
        let url = format!(
            "sqlite://{}?mode=rwc",
            db_path.display().to_string().replace('\\', "/")
        );
        env::set_var("DATABASE_URL", url);
    }

    // Live mode connects to the online booking gateway so website bookings sync in.
    if is_live {
        if env::var("WORKER_URL").is_err() {
            env::set_var("WORKER_URL", "https://opticore-booking.fivelidz.workers.dev");
        }
        if env::var("SYNC_SECRET").is_err() {
            env::set_var("SYNC_SECRET", "opticore-sync-2026");
        }
    }

    // ---- Start the embedded HTTP server in a background thread ----
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(async {
            if let Err(e) = server::run().await {
                eprintln!("Server error: {e}");
            }
        });
    });

    // ---- Wait for the server to be ready (max 15 seconds) ----
    let port = env::var("PORT").unwrap_or_else(|_| "3000".into());
    let mut ready = false;
    for _ in 0..75 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            ready = true;
            break;
        }
    }
    if ready {
        eprintln!("✅ Server ready, opening window...");
    } else {
        eprintln!("⚠️  Server not ready after 15s, opening window anyway...");
    }

    // ---- Open the Tauri window ----
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![app_version, restart_app])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
