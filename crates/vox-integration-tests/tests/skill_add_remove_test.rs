#![allow(missing_docs)]

use vox_orchestrator::OrchestratorConfig;
use vox_orchestrator_mcp::{ServerState, handle_tool_call as tools};

// Adds a skill from a LOCAL PATH source (no network), confirms it lists, then
// removes it (ownership-scoped) and confirms the directory is gone.
#[tokio::test]
async fn add_then_remove_local_skill() {
    // Fixture source: <tmp_src>/skills/fixture-skill/SKILL.md
    let src = tempfile::tempdir().unwrap();
    let skill_dir = src.path().join("skills/fixture-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: fixture-skill\ndescription: A fixture skill for the add/remove test\n---\n\n# body\n",
    )
    .unwrap();

    // Isolated workspace so `.vox/skills` resolves under a temp dir.
    let ws = tempfile::tempdir().unwrap();
    let config = OrchestratorConfig::default();
    let mut state = ServerState::new_full(config);
    state.workspace_root = Some(ws.path().to_path_buf());

    // 1. Add from the local path.
    let add_req = serde_json::json!({ "source": src.path().to_string_lossy() });
    let add_resp = tools(&state, "vox_skill_add", add_req).await.unwrap();
    assert!(
        add_resp.contains("fixture-skill") && !add_resp.contains("\"is_error\":true"),
        "add failed: {add_resp}"
    );

    // 2. It now lists.
    let list_resp = tools(&state, "vox_skill_list", serde_json::json!({}))
        .await
        .unwrap();
    assert!(
        list_resp.contains("fixture-skill"),
        "not listed: {list_resp}"
    );

    // 3. The directory landed under the isolated workspace's .vox/skills.
    let installed_dir = ws.path().join(".vox/skills/fixture-skill");
    assert!(installed_dir.join("SKILL.md").is_file());

    // 4. Remove it (ownership-scoped — it is under .vox/skills, so removable).
    let rm_resp = tools(
        &state,
        "vox_skill_remove",
        serde_json::json!({ "id": "fixture-skill" }),
    )
    .await
    .unwrap();
    assert!(rm_resp.contains("Removed"), "remove failed: {rm_resp}");

    // 5. Directory is gone.
    assert!(!installed_dir.exists(), "dir still present after remove");
}
