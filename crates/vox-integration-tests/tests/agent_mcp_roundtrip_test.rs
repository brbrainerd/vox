#![allow(missing_docs)]

use vox_orchestrator::OrchestratorConfig;
use vox_orchestrator_mcp::{ServerState, handle_tool_call as tools};

#[tokio::test]
async fn test_agent_mcp_roundtrip() {
    let config = OrchestratorConfig::default();
    let state = ServerState::new_full(config);

    // 1. Submit a task — parse the real task_id from the response.
    let submit_req = serde_json::json!({
        "description": "Test task",
        "files": []
    });
    let resp: String = tools(&state, "vox_submit_task", submit_req).await.unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&resp).expect("submit response must be valid JSON");
    assert_eq!(
        parsed["success"].as_bool(),
        Some(true),
        "Task submission failed: {}",
        resp
    );

    // Extract the real task_id from the submit response JSON.
    let task_id = parsed["data"]["task_id"]
        .as_u64()
        .expect("submit response must contain numeric task_id under data");

    // 2. Check task status.
    let status_req = serde_json::json!({ "task_id": task_id });
    let status_resp: String = tools(&state, "vox_task_status", status_req).await.unwrap();
    let status_parsed: serde_json::Value =
        serde_json::from_str(&status_resp).expect("status response must be valid JSON");
    assert_eq!(
        status_parsed["success"].as_bool(),
        Some(true),
        "Task status check failed: {}",
        status_resp
    );

    // 3. Cancel the task.
    //
    // A freshly submitted task is in "Queued" state and has not been assigned
    // to an agent, so `vox_complete_task` (which requires "Running/Assigned"
    // state) cannot succeed.  `vox_cancel_task` works for any lifecycle state.
    let cancel_req = serde_json::json!({ "task_id": task_id });
    let cancel_resp: String = tools(&state, "vox_cancel_task", cancel_req).await.unwrap();
    let cancel_parsed: serde_json::Value =
        serde_json::from_str(&cancel_resp).expect("cancel response must be valid JSON");
    assert_eq!(
        cancel_parsed["success"].as_bool(),
        Some(true),
        "Task cancel failed: {}",
        cancel_resp
    );
}
