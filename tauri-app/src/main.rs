// Mosman Eye PMS — Tauri desktop shell.
// Wraps the React frontend (../frontend) in a native window.
// The frontend talks to the Rust LAN server on http://localhost:3000.
//
// In release builds on Windows, hide the console window.
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
