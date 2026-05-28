#[tauri::command]
pub fn get_command_catalog() -> Result<serde_json::Value, String> {
    let catalog = vox_cli::command_catalog::build_catalog();
    serde_json::to_value(&catalog).map_err(|e| e.to_string())
}
