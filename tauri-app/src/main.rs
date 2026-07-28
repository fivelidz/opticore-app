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

    // Store the DB in a writable per-user data dir (Program Files is read-only).
    if env::var("DATABASE_URL").is_err() {
        let dir = data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let db_path = dir.join("opticore.db");
        // Use forward slashes for the sqlite URL (works on Windows too).
        let url = format!("sqlite://{}?mode=rwc", db_path.display().to_string().replace('\\', "/"));
        env::set_var("DATABASE_URL", url);
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
