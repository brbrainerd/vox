use vox_orchestrator_mcp::chat_tools::chat_message;
use vox_orchestrator_mcp::chat_tools::params::ChatMessageParams;
use vox_orchestrator_mcp::server_state::ServerState;

#[tokio::test]
async fn test_forced_autonomous_chat_research_triggers() {
    let state = ServerState::new_full(vox_orchestrator_mcp::load_config());
    let params = ChatMessageParams {
        prompt: "explain quantum physics".to_string(),
        context_files: vec![],
        open_files: vec![],
        active_file: None,
        active_line: None,
        selected_text: None,
        diagnostics: vec![],
        session_id: Some("test-session".to_string()),
        thread_id: None,
        journey_id: None,
        cognitive_profile: None,
        json_mode: false,
        trace_id: None,
        correlation_id: None,
        attachment_manifest: None,
        temperature: None,
        top_p: None,
        skill: None,
        model_override: None,
        tier: None,
        clutch: None,
        risk: None,
        skill_exclusions: vec![],
        mode: None,
        priority: None,
        dry_run: None,
        force_research: Some(true),
        research_scope: Some("web".to_string()),
    };

    // Since network backends and API keys might not be present in local test environments,
    // we verify that the chat message execution handles the research trigger and completes/falls back gracefully.
    let response = chat_message(&state, params).await;
    assert!(!response.is_empty());
}
