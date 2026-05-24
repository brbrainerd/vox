use vox_orchestrator::{OrchestratorConfig, build_repo_scoped_orchestrator};
use vox_gamify::db::acknowledge_message;
use std::path::Path;

#[tauri::command]
pub async fn ack_ludus_alert(note_id: i64) -> Result<(), String> {
    let config = OrchestratorConfig::default(); 
    let build = build_repo_scoped_orchestrator(config, None::<&Path>);
    
    let db = {
        let db_guard = build.orchestrator.db.read().map_err(|e| e.to_string())?;
        db_guard.as_ref().ok_or("Database not initialized")?.clone()
    };
    
    acknowledge_message(&db, note_id).await.map_err(|e: anyhow::Error| e.to_string())
}
