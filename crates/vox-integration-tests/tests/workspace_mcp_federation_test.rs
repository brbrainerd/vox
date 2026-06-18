//! E2E tests for workspace MCP federation into vox-mcp.

use vox_orchestrator::OrchestratorConfig;
use vox_orchestrator_mcp::{
    ServerState, handle_tool_call,
    registry::merged_tool_registry,
    workspace_mcp::{WorkspaceMcpLoader, dispatch_workspace_resource, load_scan_config},
};

#[tokio::test]
async fn federated_tool_appears_in_list_and_dispatches() {
    let state = ServerState::new_full(OrchestratorConfig::default());
    let tools = merged_tool_registry(&state);
    assert!(tools.iter().any(|t| t.name == "read_file"));
    let resp = handle_tool_call(
        &state,
        "read_file",
        serde_json::json!({"path": "README.md"}),
    )
    .await
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["success"], true);
}

#[tokio::test]
async fn federated_resource_appears_in_surface_and_reads() {
    let state = ServerState::new_full(OrchestratorConfig::default());
    let ws = state.workspace_mcp.read();
    assert!(
        ws.resource_by_uri("vox://golden/mcp-status").is_some(),
        "golden resource should be federated"
    );
    let text = dispatch_workspace_resource(&ws, "vox://golden/mcp-status").unwrap();
    assert_eq!(text, "ok");
}

#[tokio::test]
async fn workspace_mcp_refresh_returns_diagnostics() {
    let state = ServerState::new_full(OrchestratorConfig::default());
    let resp = handle_tool_call(&state, "vox_workspace_mcp_refresh", serde_json::json!({}))
        .await
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["success"], true);
    assert!(v["data"]["tool_count"].as_u64().unwrap_or(0) > 0);
    assert!(v["data"]["errors"].as_array().unwrap().is_empty());
}

#[test]
fn loader_finds_golden_resource_in_repo() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    let load = WorkspaceMcpLoader::load_repo(repo, &load_scan_config(repo)).unwrap();
    assert!(load.errors.is_empty());
    assert!(
        load.surface
            .resource_by_uri("vox://golden/mcp-status")
            .is_some()
    );
}
