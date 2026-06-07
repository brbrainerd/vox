#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::app_state::GuiState;
use std::sync::Mutex;
use tauri::Manager;

#[tokio::main]
async fn main() {
    // Backend log stream → stderr. `try_init` is a no-op if a subscriber is already installed by
    // a dependency, so this never panics. Tune verbosity with RUST_LOG (default: info + GUI debug).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,vox_gui=debug".into()),
        )
        .with_writer(std::io::stderr)
        .try_init();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--print-action-manifest-json") {
        match commands::action_manifest::build_action_manifest()
            .map_err(|e| e.to_string())
            .and_then(|manifest| serde_json::to_string(&manifest).map_err(|e| e.to_string()))
        {
            Ok(json) => {
                println!("{json}");
                return;
            }
            Err(err) => {
                eprintln!("failed to print action manifest: {err}");
                std::process::exit(1);
            }
        }
    }
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
        .manage(std::sync::Arc::new(
            commands::daemon::PersistentDaemon::default(),
        ))
        .setup(|app| {
            // Single persistent orchestrator daemon shared by tool calls,
            // approvals, and the status/event streams.
            let daemon = app
                .state::<std::sync::Arc<commands::daemon::PersistentDaemon>>()
                .inner()
                .clone();
            // B1: start the live orchestrator status stream, re-emitting each
            // snapshot as the "vox://orch-status" Tauri event.
            commands::orchestrator::spawn_orchestrator_status_stream(
                app.handle().clone(),
                daemon.clone(),
            );
            // B4: start the live agent-event stream, re-emitting each AgentEvent
            // as the "vox://agent-events" Tauri event.
            commands::orchestrator::spawn_agent_event_stream(app.handle().clone(), daemon.clone());
            // F2: start the live Scientia-queue watcher, emitting a
            // "vox://scientia-queue" ping when the DB-backed queue changes.
            commands::scientia::spawn_scientia_queue_stream(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::catalog::get_command_catalog,
            commands::action_manifest::get_action_manifest,
            commands::execute::execute_command,
            commands::devlog::log_frontend,
            commands::app_state::get_initial_view,
            commands::build_info::get_build_info,
            commands::control_plane::submit_orchestrator_task,
            commands::control_plane::pause_orchestrator_agent,
            commands::control_plane::resume_orchestrator_agent,
            commands::control_plane::doubt_orchestrator_task,
            commands::control_plane::overrule_orchestrator_task,
            commands::orchestrator::get_orchestrator_status,
            commands::orchestrator::get_orchestrator_status_bin,
            commands::orchestrator::set_orchestrator_config,
            commands::dynamic_mapping::get_command_metadata,
            commands::dynamic_mapping::get_full_registry,
            commands::models::list_model_cards,
            commands::models::get_active_model,
            commands::models::set_active_model,
            commands::models::get_routing_summary,
            commands::models::get_routing_summary_live,
            commands::models::set_routing_priority,
            commands::models::get_selection_policy,
            commands::models::set_selection_policy,
            commands::models::get_model_scoreboard,
            commands::models::explain_model_selection,
            commands::models::suggest_model_for_task,
            commands::memory::get_memory_status,
            commands::memory::mnemosyne_recall,
            commands::memory::mnemosyne_reindex,
            commands::preferences::get_gui_preference,
            commands::preferences::set_gui_preference,
            commands::runs::start_gui_run,
            commands::runs::finish_gui_run,
            commands::runs::list_gui_runs,
            commands::runs::get_gui_run,
            commands::mcp::invoke_mcp_tool,
            commands::secrets::list_secret_status,
            commands::secrets::set_secret,
            commands::secrets::remove_secret,
            commands::gamify::get_ludus_profile,
            commands::gamify::list_ludus_notifications,
            commands::gamify::ack_ludus_notification,
            commands::gamify::get_gamify_settings,
            commands::gamify::set_gamify_settings,
            commands::gamify::list_gamify_leaderboard,
            commands::gamify::list_gamify_companions,
            commands::gamify::list_gamify_quests,
            commands::scientia::list_research_sessions,
            commands::scientia::get_research_session_detail,
            commands::scientia::list_publication_manifests,
            commands::search::vox_search_query,
            commands::search::open_locator,
            commands::vcs_isolation::get_vcs_isolation,
            commands::vcs_isolation::set_vcs_isolation_strategy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
