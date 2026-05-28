use serde::Serialize;
use vox_cli::command_registry_model::RegistryFile;

#[derive(Serialize)]
pub struct CommandMetadata {
    pub product_lane: Option<String>,
    pub feature_gate: Option<String>,
    pub catalog_group: Option<String>,
    pub status: String,
}

#[tauri::command]
pub fn get_command_metadata(path: Vec<String>) -> Result<Option<CommandMetadata>, String> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let registry_path = repo_root.join("contracts/cli/command-registry.yaml");
    
    if !registry_path.exists() {
        return Err(format!("Registry not found at {:?}", registry_path));
    }

    let content = std::fs::read_to_string(&registry_path)
        .map_err(|e| format!("Failed to read registry: {}", e))?;
    
    let registry: RegistryFile = serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse registry: {}", e))?;

    // Find operation matching the path
    for op in registry.operations {
        if op.path == path {
            return Ok(Some(CommandMetadata {
                product_lane: op.product_lane,
                feature_gate: op.feature_gate,
                catalog_group: op.catalog_group,
                status: op.status,
            }));
        }
    }

    Ok(None)
}

#[tauri::command]
pub fn get_full_registry() -> Result<serde_json::Value, String> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let registry_path = repo_root.join("contracts/cli/command-registry.yaml");
    
    let content = std::fs::read_to_string(&registry_path)
        .map_err(|e| format!("Failed to read registry: {}", e))?;
    
    let registry: RegistryFile = serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse registry: {}", e))?;

    serde_json::to_value(&registry).map_err(|e| e.to_string())
}
