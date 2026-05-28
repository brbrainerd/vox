use vox_orchestrator::{OrchestratorConfig, build_repo_scoped_orchestrator};
use std::path::Path;
use vox_package_types::manifest::VoxManifest;

#[tauri::command]
pub async fn get_orchestrator_status() -> Result<serde_json::Value, String> {
    // This is a heavy operation as it rebuilds the orchestrator state from disk/db
    // In a production app, we would have a long-running sidecar or shared memory.
    // For the preview, we'll poll the local repo state.
    
    // Note: We use a default config here; ideally we'd load it from the repo.
    let config = OrchestratorConfig::default(); 
    
    // We pass None to discover from CWD
    let build = build_repo_scoped_orchestrator(config, None::<&Path>);
    let status = build.orchestrator.status();
    
    serde_json::to_value(&status).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_orchestrator_status_bin() -> Result<tauri::ipc::Response, String> {
    let config = OrchestratorConfig::default(); 
    let build = build_repo_scoped_orchestrator(config, None::<&Path>);
    let status = build.orchestrator.status();
    
    let bytes = rmp_serde::to_vec_named(&status).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn set_orchestrator_config(config: serde_json::Value) -> Result<(), String> {
    // 1. Discover Vox.toml
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let (mut manifest, path) = VoxManifest::discover(&current_dir).map_err(|e| e.to_string())?;
    
    // 2. Parse incoming JSON overrides into the orchestrator table
    let mut orch_table = manifest.orchestrator.unwrap_or_default();
    
    if let Some(c) = config.get("concurrency").and_then(|v| v.as_u64()) {
        orch_table.insert("max_agents".to_string(), toml::Value::Integer(c as i64));
    }
    if let Some(cap) = config.get("capUsd").and_then(|v| v.as_f64()) {
        // Convert USD to micros
        orch_table.insert("financial_cost_budget_micros".to_string(), toml::Value::Integer((cap * 1_000_000.0) as i64));
    }
    if let Some(doubt) = config.get("doubtThresh").and_then(|v| v.as_f64()) {
        orch_table.insert("trust_auto_approve_min".to_string(), toml::Value::Float(doubt));
    }
    if let Some(iso) = config.get("isolation").and_then(|v| v.as_str()) {
        let val = if iso == "wasm" { "Wasm" } else if iso == "ctr" { "Container" } else { "Native" };
        orch_table.insert("scope_enforcement".to_string(), toml::Value::String(val.to_string()));
    }
    if let Some(auto) = config.get("autobudget").and_then(|v| v.as_bool()) {
        orch_table.insert("exec_time_budget_enabled".to_string(), toml::Value::Boolean(auto));
    }
    if let Some(shadow) = config.get("doubt").and_then(|v| v.as_bool()) {
        orch_table.insert("socrates_gate_shadow".to_string(), toml::Value::Boolean(!shadow));
        orch_table.insert("socrates_gate_enforce".to_string(), toml::Value::Boolean(shadow));
    }
    
    // 3. Save it back
    manifest.orchestrator = Some(orch_table);
    let toml_str = manifest.to_toml_string().map_err(|e| e.to_string())?;
    std::fs::write(&path, toml_str).map_err(|e| e.to_string())?;
    
    // 4. Try to signal vox-orchestrator-d to hot-reload if it is running
    // We do this in a fire-and-forget manner to not block or fail the UI update.
    tokio::spawn(async move {
        let _ = vox_cli_core::daemon_ipc::dispatch::call_daemon(
            "vox-orchestrator-d",
            vox_foundation::protocol::orch_daemon_method::RELOAD_CONFIG,
            serde_json::json!({}),
            false,
        ).await;
    });
    
    Ok(())
}
