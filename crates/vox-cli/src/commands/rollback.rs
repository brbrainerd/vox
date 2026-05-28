use anyhow::Result;
use vox_cli_core::daemon_ipc::dispatch;
use vox_foundation::protocol::orch_daemon_method;

pub async fn run(id: Option<String>) -> Result<()> {
    let target = match id {
        Some(t) => t,
        None => {
            anyhow::bail!("No target ID specified. Please specify an operation ID to rollback.");
        }
    };
    
    println!("Rolling back operation {}...", target);
    
    let res = dispatch::call_daemon(
        "vox-orchestrator-d",
        orch_daemon_method::UNDO_OPERATION,
        serde_json::json!({ "op_id": target }),
        false,
    ).await?;
    
    println!("Rollback complete: {:?}", res);
    Ok(())
}
