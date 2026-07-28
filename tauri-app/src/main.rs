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

    // OptiCore has two independent modes, each with its OWN database file so the
    // demo and the real clinic data never touch each other:
    //   * LIVE (production): OPTICORE_MODE=live  -> opticore.db, starts empty,
    //       connects to online booking. This is the real clinic database.
    //   * DEMO (default):    (unset / demo)      -> opticore-demo.db, sample data.
    // Because they are separate files, switching between demo and live — or
    // installing an update — never overwrites or loses the other one's data.
    let mode = env::var("OPTICORE_MODE").unwrap_or_default().to_lowercase();
    let is_live = mode == "live" || mode == "production" || mode == "final";

    // Store the DB in a writable per-user data dir (Program Files is read-only).
    if env::var("DATABASE_URL").is_err() {
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let db_name = if is_live { "opticore.db" } else { "opticore-demo.db" };
        let db_path = dir.join(db_name);

        // For the LIVE database, only clear seed data on the VERY FIRST creation
        // (when the file does not yet exist). This gives a clean empty start
        // without ever wiping real data on subsequent launches.
        if is_live && !db_path.exists() && env::var("CLEAN_START").is_err() {
            env::set_var("CLEAN_START", "1");
        }

        // Use forward slashes for the sqlite URL (works on Windows too).
        let url = format!("sqlite://{}?mode=rwc", db_path.display().to_string().replace('\\', "/"));
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
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
