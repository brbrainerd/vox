use vox_orchestrator::mcp_tools::code_validator::{validate_file, vox_check};
use vox_orchestrator::mcp_tools::params::{ValidateFileParams, VoxCheckParams};
use vox_orchestrator::mcp_tools::server_state::ServerState;

#[tokio::test]
#[ignore = "pending MCP code_validator implementation — owner: orchestrator sunset: 2026-12-31"]
async fn test_orchestrator_rejects_syntactic_configurability() {
    let config_opts = vox_orchestrator::OrchestratorConfig::default();
    let state = ServerState::new_full(config_opts);
    // Create a temporary file
    let test_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("vox_tests");
    std::fs::create_dir_all(&test_dir).unwrap();
    let file_path = test_dir.join("macro_test.vox");
    let source = "macro_rules! hello_world { () => {} }";
    std::fs::write(&file_path, source).unwrap();
    let path_str = file_path.to_string_lossy().to_string();

    let check_params = VoxCheckParams {
        path: path_str.clone(),
    };

    // Note: vox_check currently might fail if path doesn't exist in a repository
    // Let's just make sure it parses the response string.
    let response = vox_check(&state, check_params).await;
    // We expect "E091" inside the json
    assert!(
        response.contains("E091"),
        "Expected E091 in response: {}",
        response
    );

    let val_params = ValidateFileParams { path: path_str };
    let val_response = validate_file(&state, val_params).await;
    assert!(
        val_response.contains("UNSUPPORTED_SYNTAX"),
        "Expected UNSUPPORTED_SYNTAX in response: {}",
        val_response
    );
}
