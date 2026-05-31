use vox_db::{DbConfig, VoxDb};
use vox_telemetry::{METRIC_TYPE_MODEL_CALL_EVENT, ModelCallEvent};

#[tokio::test(flavor = "multi_thread")]
async fn model_call_event_serializes_and_persists() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();

    let event = ModelCallEvent {
        model: "claude-opus-4-7".to_string(),
        provider: "Anthropic".to_string(),
        route_profile: None,
        selection_rationale: None,
        prompt_tokens: 1024,
        completion_tokens: 256,
        cache_read_input_tokens: Some(800),
        cache_creation_input_tokens: None,
        latency_ms: 1234,
        cost_usd: 0.042,
        cost_source: "provider_reported".to_string(),
        error_class: None,
        retry_attempt: 0,
        task_id: Some(42),
        parent_task_id: None,
        trace_id: None,
        caller_agent_id: None,
    };

    let metadata_json = serde_json::to_string(&event).unwrap();
    let session_id = format!("model:{}", event.task_id.unwrap());

    db.append_research_metric(
        &session_id,
        METRIC_TYPE_MODEL_CALL_EVENT,
        Some(event.cost_usd),
        Some(&metadata_json),
    )
    .await
    .unwrap();

    // Verify the row round-trips via JSON
    let parsed: ModelCallEvent = serde_json::from_str(&metadata_json).unwrap();
    assert_eq!(parsed.model, "claude-opus-4-7");
    assert_eq!(parsed.prompt_tokens, 1024);
    assert_eq!(parsed.cache_read_input_tokens, Some(800));
    assert_eq!(parsed.cost_usd, 0.042);
    assert_eq!(parsed.task_id, Some(42));
}

#[test]
fn model_call_event_json_has_expected_fields() {
    let event = ModelCallEvent {
        model: "gpt-4o".to_string(),
        provider: "OpenAI".to_string(),
        route_profile: Some("cloud_fast".to_string()),
        selection_rationale: Some("Top-tier coding reasoning".to_string()),
        prompt_tokens: 500,
        completion_tokens: 100,
        cache_read_input_tokens: None,
        cache_creation_input_tokens: Some(50),
        latency_ms: 800,
        cost_usd: 0.01,
        cost_source: "estimated".to_string(),
        error_class: None,
        retry_attempt: 1,
        task_id: None,
        parent_task_id: None,
        trace_id: Some("abc-trace".to_string()),
        caller_agent_id: Some("orchestrator".to_string()),
    };

    let v: serde_json::Value = serde_json::to_value(&event).unwrap();
    assert_eq!(v["model"], "gpt-4o");
    assert_eq!(v["cache_creation_input_tokens"], 50);
    assert!(v["cache_read_input_tokens"].is_null());
    assert_eq!(v["retry_attempt"], 1);
    assert_eq!(v["caller_agent_id"], "orchestrator");
}
