//! Passive Ludus event recording.

/// Fire-and-forget passive ludus event recording after a CLI command completes.
pub fn record_cli_event_fire_and_forget(
    event_type: &'static str,
    success: bool,
    capability_id: Option<&'static str>,
    command_path: Option<&'static str>,
) {
    tokio::spawn(async move {
        let _ = record_cli_event_inner(event_type, success, capability_id, command_path).await;
    });
}

async fn record_cli_event_inner(
    event_type: &str,
    success: bool,
    capability_id: Option<&str>,
    command_path: Option<&str>,
) -> anyhow::Result<()> {
    // Only proceed if DB is available
    let Ok(db) = vox_db::VoxDb::connect_canonical().await else {
        return Ok(());
    };
    let user_id = vox_db::paths::local_user_id();

    // Codex: log CLI command event for unified capability telemetry
    if let (Some(cap_id), Some(cmd_path)) = (capability_id, command_path) {
        let metadata = serde_json::json!({
            "capability_id": cap_id,
            "success": success,
        })
        .to_string();
        let _ = db
            .record_behavior_event(&user_id, "cli_command", Some(cmd_path), Some(&metadata))
            .await;
    }

    // Ludus recording (only if enabled)
    if vox_gamify::config_gate::is_enabled() {
        let ludus_uid = vox_gamify::db::canonical_user_id();
        let event_json = serde_json::json!({
            "type": event_type,
            "success": success,
            "agent_id": 0u64,
        });
        let _ = vox_gamify::event_router::route_event(&db, &ludus_uid, &event_json).await;
    }

    Ok(())
}
