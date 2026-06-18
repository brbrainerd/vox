//! Integration tests for per-skill MCP tool allowlist enforcement.

use vox_orchestrator::OrchestratorConfig;
use vox_orchestrator_mcp::{ServerState, handle_tool_call, skills_tools::skill_use};
use vox_plugin_api::skill::LoadedSkill;
use vox_skills::SkillManifest;

fn install_restricted_git_skill(state: &ServerState) -> &'static str {
    const SKILL_ID: &str = "integration-test-git-skill";
    state.skill_registry.install(LoadedSkill {
        plugin_id: SKILL_ID.to_string(),
        format_version: 1,
        manifest: SkillManifest {
            id: SKILL_ID.to_string(),
            name: "integration-test-git-skill".to_string(),
            version: "1.0.0".to_string(),
            description: "git only".to_string(),
            tools: vec!["vox_git_status".to_string()],
            ..Default::default()
        },
        body: "---\nname: integration-test-git-skill\ndescription: git\n---\n".to_string(),
        exposed_tools: vec!["vox_git_status".to_string()],
    });
    SKILL_ID
}

#[tokio::test]
async fn skill_use_blocks_domain_tool_but_allows_skill_run() {
    let state = ServerState::new_full(OrchestratorConfig::default());
    let skill_id = install_restricted_git_skill(&state);

    let use_resp = skill_use(
        &state,
        vox_orchestrator_mcp::skills_tools::SkillIdParams {
            id: skill_id.to_string(),
        },
    );
    let use_v: serde_json::Value = serde_json::from_str(&use_resp).unwrap();
    assert_eq!(use_v["success"], true);
    assert_eq!(
        state.active_skill_id.read().as_deref(),
        Some(skill_id),
        "skill_use must pin active_skill_id"
    );
    let manifest = state.skill_registry.get(skill_id).expect("installed skill");
    assert_eq!(manifest.tools, vec!["vox_git_status"]);

    // Use a non-HITL tool so denial is from the skill allowlist, not approval timeout.
    let denied = handle_tool_call(&state, "vox_git_diff", serde_json::json!({}))
        .await
        .unwrap();
    let denied_v: serde_json::Value = serde_json::from_str(&denied).unwrap();
    assert_eq!(denied_v["success"], false);
    assert!(
        denied_v["error"]
            .as_str()
            .unwrap_or("")
            .contains("allowlist"),
        "expected allowlist denial, got {denied_v}"
    );

    let allowed = handle_tool_call(
        &state,
        "vox_skill_run",
        serde_json::json!({"id": skill_id, "command": "echo ok"}),
    )
    .await
    .unwrap();
    let allowed_v: serde_json::Value = serde_json::from_str(&allowed).unwrap();
    let allowed_err = allowed_v["error"].as_str().unwrap_or("");
    assert!(
        !allowed_err.contains("allowlist"),
        "vox_skill_run must not be blocked by skill allowlist: {allowed_v}"
    );
}

#[tokio::test]
async fn activate_skill_for_id_or_name_sets_active_id() {
    let state = ServerState::new_full(OrchestratorConfig::default());
    let skill_id = install_restricted_git_skill(&state);
    assert!(vox_orchestrator_mcp::skills_tools::activate_skill_for_id_or_name(&state, skill_id));
    assert_eq!(state.active_skill_id.read().as_deref(), Some(skill_id));
}
