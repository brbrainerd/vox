use vox_mcp::{tools, ServerState};
use vox_orchestrator::OrchestratorConfig;

#[tokio::test]
async fn test_a2a_mcp_roundtrip() {
    let config = OrchestratorConfig::default();
    let state = ServerState::new(config);

    // 1. Send A2A message
    let send_req = serde_json::json!({
        "sender_id": 1,
        "receiver_id": 2,
        "msg_type": "help_request",
        "payload": "I need help with parser."
    });

    let send_resp: String = tools::handle_tool_call(&state, "vox_a2a_send", send_req)
        .await
        .unwrap();
    assert!(
        send_resp.contains("\"success\": true") || send_resp.contains("\"success\":true"),
        "Send failed: {}",
        send_resp
    );

    // 2. Check inbox for receiver
    let inbox_req = serde_json::json!({
        "agent_id": 2
    });

    let inbox_resp: String = tools::handle_tool_call(&state, "vox_a2a_inbox", inbox_req)
        .await
        .unwrap();
    assert!(inbox_resp.contains("\"success\": true") || inbox_resp.contains("\"success\":true"));
    assert!(inbox_resp.contains("I need help with parser."));

    // Parse to get message_id. We know it should be 1 since it's the first message
    let msg_id = 1;

    // 3. Ack the message
    let ack_req = serde_json::json!({
        "agent_id": 2,
        "message_id": msg_id
    });

    let ack_resp: String = tools::handle_tool_call(&state, "vox_a2a_ack", ack_req)
        .await
        .unwrap();
    assert!(ack_resp.contains("\"success\": true") || ack_resp.contains("\"success\":true"));

    // 4. Verify inbox is now empty
    let inbox_req2 = serde_json::json!({
        "agent_id": 2
    });
    let inbox_resp2: String = tools::handle_tool_call(&state, "vox_a2a_inbox", inbox_req2)
        .await
        .unwrap();
    // It shouldn't contain the payload anymore or the array length is 0, let's just check it doesn't contain the payload
    assert!(!inbox_resp2.contains("I need help with parser."));

    // 5. Verify history contains it
    let history_req = serde_json::json!({
        "since_ms": 0,
        "limit": 10
    });

    let history_resp: String = tools::handle_tool_call(&state, "vox_a2a_history", history_req)
        .await
        .unwrap();
    assert!(
        history_resp.contains("\"success\": true") || history_resp.contains("\"success\":true")
    );
    assert!(history_resp.contains("I need help with parser."));
}
