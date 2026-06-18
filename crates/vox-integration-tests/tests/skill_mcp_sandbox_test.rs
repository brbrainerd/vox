//! Sandbox MCP parity for `vox_skill_run`.

use vox_orchestrator::OrchestratorConfig;
use vox_orchestrator_mcp::{ServerState, handle_tool_call};
use vox_skills::{SkillManifest, VoxSkillBundle};

#[tokio::test]
#[ignore = "slow; needs docker"]
async fn skill_run_mcp_matches_sandbox_exit_semantics() {
    let state = ServerState::new_full(OrchestratorConfig::default());
    let bundle = VoxSkillBundle::new(
        SkillManifest {
            id: "sandbox-echo".to_string(),
            name: "sandbox-echo".to_string(),
            version: "1.0.0".to_string(),
            description: "echo test".to_string(),
            ..Default::default()
        },
        "---\nname: sandbox-echo\ndescription: echo\n---\n",
    );
    state.skill_registry.install_bundle(&bundle).await.unwrap();

    let resp = handle_tool_call(
        &state,
        "vox_skill_run",
        serde_json::json!({
            "id": "sandbox-echo",
            "command": "echo hello"
        }),
    )
    .await
    .unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["success"], true);
}
