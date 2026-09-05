#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;

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
        .plugin(tauri_plugin_dialog::init())
        .manage(GuiState {
            initial_view: Mutex::new(initial_view),
        })
        .manage(commands::mic::MicCaptureState::default())
        .manage(commands::pty::PtyManager::default())
        .manage(commands::terminal_core::TerminalSessionManager::default())
        .manage(std::sync::Arc::new(
            commands::daemon::PersistentDaemon::default(),
        ))
        .manage(std::sync::Arc::new(
            commands::browser::BrowserState::default(),
        ))
        .setup(|app| {
            // `.setup()` runs synchronously on a worker thread already inside
            // the #[tokio::main] runtime (needed below so scientia's
            // tokio::spawn calls have an ambient reactor) — block_on alone
            // panics with "Cannot start a runtime from within a runtime".
            // block_in_place is the sanctioned way to block synchronously
            // from within a multi-thread runtime's worker thread.
            let pool = tokio::task::block_in_place(|| {
                tauri::async_runtime::block_on(
                    commands::gui_db_pool::GuiDbPool::connect_workspace(),
                )
            });
            if let Ok(db) = pool.handle() {
                commands::scientia::spawn_scientia_queue_stream(app.handle().clone(), db.clone());
                commands::scientia::spawn_discovery_surfaced_stream(app.handle().clone(), db);
            }
            app.manage(pool);

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
            // T3.1: background supervision task — periodically pings the
            // cached orchestrator daemon and clears/re-resolves it if it has
            // gone unreachable, so liveness is actively monitored rather than
            // only checked reactively the next time a stream/command touches
            // the daemon.
            daemon.clone().spawn_supervisor();
            let daemon = app
                .state::<std::sync::Arc<commands::daemon::PersistentDaemon>>()
                .inner()
                .clone();
            let browser_state = app
                .state::<std::sync::Arc<commands::browser::BrowserState>>()
                .inner()
                .clone();
            commands::browser::spawn_browser_frame_stream(
                app.handle().clone(),
                daemon,
                browser_state,
            );
            let browser_state = app
                .state::<std::sync::Arc<commands::browser::BrowserState>>()
                .inner()
                .clone();
            commands::browser::emit_preview_available_from_env(app.handle().clone(), browser_state);
            // Reactive Runtime settings: forward vox-config change bumps to the webview
            // as "vox://llm-config-changed" so the settings surface refreshes live.
            commands::user_config::spawn_llm_config_bridge(app.handle().clone());
            // B2: forward vox-config snapshot bumps to the webview as
            // "vox://orchestrator-config-changed" so the Orchestrator settings
            // surface refreshes live after set_orchestrator_config writes.
            commands::orchestrator::spawn_orchestrator_config_watch(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::activity::activity_query,
            commands::catalog::get_command_catalog,
            commands::discovery::discovery_suggest,
            commands::discovery::discovery_help,
            commands::discovery::discovery_record,
            commands::pty::pty_spawn,
            commands::pty::pty_write,
            commands::pty::pty_close,
            commands::terminal_core::term_pty_bytes,
            commands::terminal_core::term_get_blocks,
            commands::terminal_core::term_submit,
            commands::console_a2a::send_to_agent,
            commands::action_manifest::get_action_manifest,
            commands::execute::execute_command,
            commands::coderabbit::coderabbit_plan,
            commands::coderabbit::coderabbit_run_async,
            commands::coderabbit::coderabbit_report,
            commands::coderabbit::coderabbit_token_present,
            commands::devlog::log_frontend,
            commands::app_state::get_initial_view,
            commands::build_info::get_build_info,
            commands::chat::chat_create_session,
            commands::chat::chat_list_sessions,
            commands::harness_eval::harness_eval_history,
            commands::harness_eval::harness_eval_regressions,
            commands::harness_issues::scan_training_corpus,
            commands::harness_issues::propose_harness_issue_fix,
            commands::harness_issues::list_harness_fix_proposals,
            commands::harness_issues::resolve_harness_fix_proposal,
            commands::harness_issues::list_harness_issues,
            commands::harness_issues::list_harness_issues_for_session,
            commands::harness_issues::record_harness_issue_decision,
            commands::chat::chat_get_messages,
            commands::chat::chat_append_message,
            commands::chat::chat_send_message,
            commands::chat_turn::chat_turn,
            commands::chat::secretary_confirm_task,
            commands::chat::chat_rename_session,
            commands::chat::chat_archive_session,
            commands::chat::chat_unarchive_session,
            commands::identity::get_identity_summary,
            commands::harness::get_task_diff,
            commands::graphify::vox_graphify_status,
            commands::harness::list_repo_files,
            commands::control_plane::submit_orchestrator_task,
            commands::control_plane::pause_orchestrator_agent,
            commands::control_plane::resume_orchestrator_agent,
            commands::control_plane::doubt_orchestrator_task,
            commands::control_plane::overrule_orchestrator_task,
            commands::control_plane::list_orchestrator_tasks,
            commands::control_plane::edit_orchestrator_task,
            commands::control_plane::cancel_orchestrator_task,
            commands::control_plane::interrupt_orchestrator_task,
            commands::control_plane::reorder_orchestrator_task,
            commands::control_plane::approve_orchestrator_task_plan,
            commands::control_plane::skip_orchestrator_verify,
            commands::control_plane::force_orchestrator_verify,
            commands::orchestrator::get_orchestrator_config,
            commands::orchestrator::get_orchestrator_config_catalog,
            commands::orchestrator::get_task_policy_overrides,
            commands::orchestrator::resolve_default_task_policy,
            commands::orchestrator::set_task_policy_override,
            commands::orchestrator::clear_task_policy_override,
            commands::llm_settings::get_llm_config,
            commands::llm_settings::set_llm_config,
            commands::llm_settings::openrouter_key_status,
            commands::llm_settings::inference_provider_status,
            commands::docs_index::vox_docs_index,
            commands::docs_index::read_doc_markdown,
            commands::orchestrator::get_orchestrator_status,
            commands::orchestrator::get_orchestrator_status_bin,
            commands::orchestrator::set_orchestrator_config,
            commands::orchestrator::get_orchestrator_config,
            commands::orchestrator::get_context_budget,
            commands::orchestrator::hopper_list,
            commands::orchestrator::hopper_submit,
            commands::orchestrator::hopper_reprioritize,
            commands::orchestrator::hopper_cancel,
            commands::orchestrator::hopper_mark_done,
            commands::dynamic_mapping::get_command_metadata,
            commands::dynamic_mapping::get_full_registry,
            commands::models::list_model_cards,
            commands::models::get_active_model,
            commands::models::set_active_model,
            commands::models::get_routing_summary_live,
            commands::models::set_routing_priority,
            commands::models::get_routing_intentions,
            commands::models::nudge_routing_intention,
            commands::models::get_selection_policy,
            commands::models::set_selection_policy,
            commands::models::get_model_scoreboard,
            commands::models::explain_model_selection,
            commands::models::suggest_model_for_task,
            commands::memory::get_memory_status,
            commands::memory::mnemosyne_recall,
            commands::memory::mnemosyne_reindex,
            commands::mercatus::mercatus_load_config,
            commands::mercatus::mercatus_save_config,
            commands::preferences::get_gui_preference,
            commands::preferences::set_gui_preference,
            commands::runs::start_gui_run,
            commands::runs::finish_gui_run,
            commands::runs::list_gui_runs,
            commands::runs::get_gui_run,
            commands::mcp::invoke_mcp_tool,
            commands::plan_panel::update_plan_node,
            commands::plan_panel::insert_plan_node,
            commands::plan_panel::approve_plan_nodes,
            commands::plan_panel::list_plan_nodes,
            commands::plan_panel::plan_open_task_counts,
            commands::plan_panel::latest_plan_session_for_chat,
            commands::secrets::list_secret_status,
            commands::secrets::set_secret,
            commands::secrets::remove_secret,
            commands::secrets::secrets_backend_status,
            commands::secrets::import_env,
            commands::secrets::migrate_auth_store,
            commands::oauth::oauth_login_openrouter,
            commands::oauth::verify_openrouter_key,
            commands::user_config::get_user_config,
            commands::user_config::set_user_config,
            commands::user_config::reset_user_config,
            commands::user_config::get_llm_spend,
            commands::stt_config::get_stt_config,
            commands::stt_config::set_stt_config,
            commands::gamify::get_ludus_profile,
            commands::gamify::list_ludus_notifications,
            commands::gamify::ack_ludus_notification,
            commands::gamify::get_gamify_settings,
            commands::gamify::set_gamify_settings,
            commands::gamify::record_gui_event,
            commands::gamify::list_gamify_leaderboard,
            commands::gamify::list_gamify_companions,
            commands::gamify::list_gamify_quests,
            commands::gamify::gamify_due_actions,
            commands::gamify::gamify_kpi_summary,
            commands::workspace_town::workspace_town_scan,
            commands::harness_town::harness_ci_fleet_status,
            commands::harness_town::vcs_town_status,
            commands::research::start_research_async,
            commands::scientia::list_research_sessions,
            commands::scientia::get_research_session_detail,
            commands::scientia::list_publication_manifests,
            commands::scientia::scientia_dashboard_snapshot,
            commands::scientia::scientia_cost_rollup,
            commands::oratio::oratio_transcribe,
            commands::scientia_review::list_publication_review_queue,
            commands::scientia_review::record_publication_claim_review,
            commands::scientia_review::nanopublish_approved_claim,
            commands::scientia_review::suggest_evidence_improvements,
            commands::scientia_review::get_novelty_assessment,
            commands::scientia_review::list_discovery_inbox,
            commands::scientia_review::acknowledge_discovery,
            commands::scientia_review::get_completion_report,
            commands::scientia_review::run_autofill,
            commands::scientia_review::get_archive_status,
            commands::search::vox_search_query,
            commands::search::open_locator,
            commands::policy::policy_list,
            commands::policy::policy_show,
            commands::policy::policy_status,
            commands::policy::list_branches,
            commands::policy::policy_set_enabled,
            commands::policy::policy_edit,
            commands::vcs_isolation::get_vcs_isolation,
            commands::vcs_isolation::set_vcs_isolation_strategy,
            commands::browser::preview_status,
            commands::browser::preview_start,
            commands::browser::preview_stop,
            commands::browser::browser_open_session,
            commands::browser::browser_close_session,
            commands::browser::browser_close_page,
            commands::browser::browser_list_pages,
            commands::browser::browser_attach_session,
            commands::browser::browser_page_info,
            commands::browser::browser_navigate,
            commands::browser::browser_goto_url,
            commands::browser::browser_scroll,
            commands::browser::browser_click_xy,
            commands::browser::browser_type_text,
            commands::browser::browser_input_key,
            commands::browser::browser_set_control_mode,
            commands::browser::browser_screenshot_frame,
            commands::browser::browser_session_status,
            commands::browser::browser_validate_playwright,
            commands::signing::signing_key_status,
            commands::signing::rotate_signing_key,
            commands::mesh::list_trusted_nodes,
            commands::mesh::trust_mesh_node,
            commands::mesh::untrust_mesh_node,
            commands::mic::start_mic_capture,
            commands::mic::stop_mic_capture_and_transcribe,
            commands::mission_control::list_subagent_tree,
            commands::mission_control::list_mc_approvals,
            commands::mission_control::set_task_mesh_policy,
            commands::daemon::orchestrator_version_mismatch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
