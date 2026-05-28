#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::app_state::GuiState;
use std::sync::Mutex;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut initial_view = None;

    // Simple CLI arg parser for the Tauri process
    for i in 0..args.len() {
        if args[i] == "--command" && i + 1 < args.len() {
            initial_view = Some(args[i + 1].clone());
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(GuiState {
            initial_view: Mutex::new(initial_view),
        })
        .invoke_handler(tauri::generate_handler![
            commands::catalog::get_command_catalog,
            commands::execute::execute_command,
            commands::app_state::get_initial_view,
            commands::orchestrator::get_orchestrator_status,
            commands::orchestrator::get_orchestrator_status_bin,
            commands::orchestrator::set_orchestrator_config,
            commands::dynamic_mapping::get_command_metadata,
            commands::dynamic_mapping::get_full_registry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
