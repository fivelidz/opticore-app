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

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn main() {
    // ---- Set default environment ----
    if env::var("PORT").is_err() {
        env::set_var("PORT", "3000");
    }
    if env::var("DATABASE_URL").is_err() {
        let db_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("opticore.db");
        env::set_var("DATABASE_URL", format!("sqlite://{}?mode=rwc", db_path.display()));
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

    // ---- Wait for the server to be ready (max 10 seconds) ----
    let port = env::var("PORT").unwrap_or_else(|_| "3000".into());
    let health_url = format!("http://localhost:{}/api/health", port);
    let mut ready = false;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if std::net::TcpStream::connect(format!("localhost:{}", port)).is_ok() {
            ready = true;
            break;
        }
    }
    if ready {
        eprintln!("✅ Server ready, opening window...");
    } else {
        eprintln!("⚠️  Server not ready after 10s, opening window anyway...");
    }

    // ---- Open the Tauri window ----
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
