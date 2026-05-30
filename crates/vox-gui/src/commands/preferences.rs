use tauri::command;
use vox_db::DbConnectSurface;
use vox_db::connect_workspace_journey_optional;

#[command]
pub async fn get_gui_preference(key: String) -> Result<Option<String>, String> {
    let db: vox_db::VoxDb = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "No workspace db found".to_string())?;

    db.get_user_preference("local_user", &key)
        .await
        .map_err(|e: vox_db::StoreError| e.to_string())
}

#[command]
pub async fn set_gui_preference(key: String, value: String) -> Result<(), String> {
    let db: vox_db::VoxDb = connect_workspace_journey_optional(DbConnectSurface::Runtime, true)
        .await
        .ok_or_else(|| "No workspace db found".to_string())?;

    db.set_user_preference("local_user", &key, &value)
        .await
        .map_err(|e: vox_db::StoreError| e.to_string())
}
