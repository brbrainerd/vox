//! Receives webview console output forwarded by the frontend `consoleBridge` and prints it to
//! the backend's stderr, so a single `cargo run -p vox-gui` log stream carries both frontend
//! and backend diagnostics.

use tauri::command;

#[command]
pub fn log_frontend(level: String, message: String) {
    match level.as_str() {
        "error" => eprintln!("[frontend:error] {message}"),
        "warn" => eprintln!("[frontend:warn] {message}"),
        other => eprintln!("[frontend:{other}] {message}"),
    }
}
