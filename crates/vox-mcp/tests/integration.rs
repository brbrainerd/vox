//! Integration tests for vox-mcp

use serde_json::json;
use vox_mcp::{tools, ServerState};
use vox_orchestrator::OrchestratorConfig;

#[tokio::test]
async fn test_mcp_status_tool() {
    let config = OrchestratorConfig::default();
    let state = ServerState::new(config);

    // Call the vox_status tool which is actually named "vox_orchestrator_status"
    let result = tools::handle_tool_call(&state, "vox_orchestrator_status", json!({}))
        .await
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    // We wrapped it in ToolResult previously wait wait
    // Actually orchestrator_status returns ToolResult ok directly?
    // Yes, but looking at handle_tool_call, it just does Ok(orchestrator_status(state).await)
    assert_eq!(parsed["success"], true);
    assert!(parsed["data"].get("agent_count").is_some());
}

#[tokio::test]
async fn test_mcp_a2a_tools() {
    let config = OrchestratorConfig::default();
    let state = ServerState::new(config);

    // Test that vox_a2a_inbox exists but may be empty initially
    let result = tools::handle_tool_call(
        &state,
        "vox_a2a_inbox",
        json!({
            "agent_id": 1
        }),
    )
    .await
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed["success"], true);
    // Unread count should be 0
    assert_eq!(parsed["data"]["unread_count"].as_i64().unwrap(), 0);
}
