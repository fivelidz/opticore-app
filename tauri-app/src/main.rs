// OptiCore PMS — Tauri desktop shell.
//
// This does TWO things on launch:
// 1. Starts the Rust HTTP server (axum + SQLite) in a background thread
// 2. Opens the Tauri webview window pointing at the frontend
//
// The frontend talks to the embedded server on http://localhost:3000.
// When the window closes, the server stops too.

#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use std::env;
use std::sync::Arc;

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn main() {
    // ---- Start the embedded HTTP server in a background thread ----
    // Set defaults if not already set by the environment.
    if env::var("PORT").is_err() {
        env::set_var("PORT", "3000");
    }

    // Use a local SQLite DB next to the executable (or in the current dir).
    if env::var("DATABASE_URL").is_err() {
        let db_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("opticore.db");
        env::set_var("DATABASE_URL", format!("sqlite://{}?mode=rwc", db_path.display()));
    }

    // Spawn the server in a separate thread.
    // The server code is the same `server` crate's main logic.
    std::thread::spawn(|| {
        // We call the server's run function via the crate dependency.
        // If the server crate exposes a `run()` we use it; otherwise we
        // start a subprocess. For now, we use the inline approach below.
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(async {
            if let Err(e) = server::run().await {
                eprintln!("Server error: {e}");
            }
        });
    });

    // Give the server a moment to start before opening the window.
    std::thread::sleep(std::time::Duration::from_millis(1500));

    // ---- Open the Tauri window ----
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
