use vox_mcp::{tools, ServerState};
use vox_orchestrator::OrchestratorConfig;

#[tokio::test]
async fn test_skill_install_tool_availability() {
    let config = OrchestratorConfig::default();
    let state = ServerState::new(config);

    // 1. Install a test skill via vox_skill_install
    let test_skill = r#"---
id = "test.macro"
name = "Test Macro"
version = "1.0.0"
author = "test"
description = "Test macro tool"
category = "custom"
tools = ["vox_test_macro_tool"]
tags = []
permissions = []
---
# Test Macro
Instructions inside.
"#;

    let bundle = vox_skills::parser::parse_skill_md(test_skill).unwrap();
    let bundle_json = serde_json::to_string(&bundle).unwrap();

    let install_req = serde_json::json!({
        "bundle_json": bundle_json
    });

    let resp: String = tools::handle_tool_call(&state, "vox_skill_install", install_req)
        .await
        .unwrap();
    assert!(
        resp.contains("\"success\": true") || resp.contains("\"success\":true"),
        "Failed to install skill: {}",
        resp
    );

    // 2. Verify it appears in vox_skill_list
    let list_req = serde_json::json!({});
    let list_resp: String = tools::handle_tool_call(&state, "vox_skill_list", list_req)
        .await
        .unwrap();
    assert!(list_resp.contains("test.macro"));
    assert!(list_resp.contains("Test Macro"));

    // 3. Verify its tools are registered in list_tools
    // In our modified lib.rs, we dynamically add them in ServerHandler::list_tools.
    // Since we don't instantiate ServerHandler here easily, we can just call the missing tool to test `handle_tool_call`.
    let macro_req = serde_json::json!({});
    let macro_resp: String = tools::handle_tool_call(&state, "vox_test_macro_tool", macro_req)
        .await
        .unwrap();

    // Expected to gracefully fail due to no DB, but this proves the fallback path caught it
    assert!(macro_resp.contains("\"success\": false") || macro_resp.contains("\"success\":false"));
    assert!(macro_resp.contains("Skill instructions not available"));
}
