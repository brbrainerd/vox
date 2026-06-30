#![allow(missing_docs)]

use vox_orchestrator::OrchestratorConfig;
use vox_orchestrator_mcp::{ServerState, handle_tool_call as tools};

#[tokio::test]
async fn propose_skill_surfaces_in_feedback_list() {
    let state = ServerState::new_full(OrchestratorConfig::default());

    let req = serde_json::json!({
        "name": "read-edit-run",
        "description": "read → edit → run (seen 4×)"
    });
    let resp = tools(&state, "vox_propose_skill", req).await.unwrap();
    assert!(resp.contains("feedback_id"), "got: {resp}");

    let list = tools(&state, "vox_feedback_list", serde_json::json!({}))
        .await
        .unwrap();
    assert!(
        list.contains("skill_proposal"),
        "proposal must appear in needs_you: {list}"
    );
}
