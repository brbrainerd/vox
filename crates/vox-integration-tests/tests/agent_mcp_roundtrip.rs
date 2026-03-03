use vox_mcp::{tools, ServerState};
use vox_orchestrator::OrchestratorConfig;

#[tokio::test]
async fn test_agent_mcp_roundtrip() {
    let config = OrchestratorConfig::default();
    let state = ServerState::new(config);

    // 1. Submit a task
    let submit_req = serde_json::json!({
        "description": "Test task",
        "files": []
    });
    let resp: String = tools::handle_tool_call(&state, "vox_submit_task", submit_req)
        .await
        .unwrap();
    assert!(
        resp.contains("\"success\": true"),
        "Task submission failed: {}",
        resp
    );

    // Extract task_id (we assume it's 1 since it's the first task)
    let task_id = 1;

    // 2. Check task status
    let status_req = serde_json::json!({
        "task_id": task_id
    });
    let status_resp: String =
        tools::handle_tool_call(&state, "vox_task_status", status_req.clone())
            .await
            .unwrap();
    assert!(status_resp.contains("\"success\": true"));

    // 3. Complete task
    let complete_req = serde_json::json!({
        "task_id": task_id,
        "files_modified": [],
        "agent_name": "Agent_1"
    });
    let complete_resp: String = tools::handle_tool_call(&state, "vox_complete_task", complete_req)
        .await
        .unwrap();
    assert!(
        complete_resp.contains("\"success\": true"),
        "Task complete failed: {}",
        complete_resp
    );
}
