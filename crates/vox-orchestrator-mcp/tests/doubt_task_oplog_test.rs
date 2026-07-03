//! T1.1 RED test: the MCP `vox_doubt_task` tool records a durable `TaskDoubted`
//! op-log entry (crates/vox-orchestrator-mcp/src/task_tools/lifecycle.rs), not just
//! the event-bus emit + FeedbackStore registration performed inside the sync
//! `Orchestrator::doubt_task`.

use std::sync::Arc;

use vox_orchestrator_mcp::{ServerState, handle_tool_call, load_config};

#[tokio::test]
async fn doubt_task_records_durable_oplog_entry() {
    let state = Arc::new(ServerState::new_full(load_config()));

    // Submit a task directly on the orchestrator to get a real assigned task_id.
    let task_id = state
        .orchestrator
        .submit_task(
            "doubt oplog test task",
            vec![vox_orchestrator::FileAffinity::write("src/doubt_oplog.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");

    let raw = handle_tool_call(
        &state,
        "vox_doubt_task",
        serde_json::json!({ "task_id": task_id.0, "reason": "T1.1 durability check" }),
    )
    .await
    .expect("dispatch ok");
    assert!(
        !vox_orchestrator_mcp::server::tool_json_envelope_is_error(&raw),
        "doubt_task call failed: {raw}"
    );

    let entries = state.orchestrator.list_recent_operations(None, 256).await;
    let saw_doubted = entries.iter().any(|e| {
        matches!(
            &e.kind,
            vox_orchestrator::oplog::OperationKind::TaskDoubted { task_id: tid, reason }
                if *tid == task_id.0 && reason.as_deref() == Some("T1.1 durability check")
        )
    });
    assert!(
        saw_doubted,
        "TaskDoubted for task {task_id} must be queryable from the durable op-log; entries: {entries:?}"
    );

    // T1.1 follow-up: the doubt-triggered FeedbackRequested{kind:"doubt"} must
    // ALSO be durably recorded, not just TaskDoubted — the feedback-registration
    // half of `doubt_task` is a separate write from the bus-emit half, and the
    // `dispatch-events.v1.schema.json` contract's FeedbackRequested.kind enum
    // explicitly includes "doubt" as a valid value.
    let saw_feedback_requested = entries.iter().any(|e| {
        matches!(
            &e.kind,
            vox_orchestrator::oplog::OperationKind::FeedbackRequested { task_id: tid, kind, .. }
                if *tid == Some(task_id.0) && kind == "doubt"
        )
    });
    assert!(
        saw_feedback_requested,
        "FeedbackRequested{{kind:\"doubt\"}} for task {task_id} must be queryable from the \
         durable op-log; entries: {entries:?}"
    );
}

/// T1.2 follow-up RED test: overruling a doubted task via the MCP
/// `vox_resolve_feedback` "Overrule" action (the primary production HITL
/// overrule path) records a durable `TaskComplete` op-log entry BEFORE the
/// `TaskCompleted` bus event is broadcast — `Orchestrator::overrule_task`
/// itself must not perform the broadcast internally (Tier-A
/// durable-before-broadcast; see `vox_orchestrator::events::is_tier_a`).
/// Zero subscribers are attached to the event bus in this test — if the
/// durability write depended on (or lived downstream of) the broadcast, this
/// test would show nothing recorded.
#[tokio::test]
async fn overrule_task_records_durable_oplog_entry() {
    let state = Arc::new(ServerState::new_full(load_config()));

    let task_id = state
        .orchestrator
        .submit_task(
            "overrule oplog test task",
            vec![vox_orchestrator::FileAffinity::write("src/overrule_oplog.rs")],
            None,
            None,
            None,
        )
        .await
        .expect("submit should succeed");

    let raw = handle_tool_call(
        &state,
        "vox_doubt_task",
        serde_json::json!({ "task_id": task_id.0, "reason": "T1.2 overrule durability check" }),
    )
    .await
    .expect("dispatch ok");
    assert!(
        !vox_orchestrator_mcp::server::tool_json_envelope_is_error(&raw),
        "doubt_task call failed: {raw}"
    );

    let doubt_feedback = state
        .feedback()
        .open_needs_you()
        .into_iter()
        .find(|item| item.doubted_task_id == Some(task_id))
        .expect("doubt_task must register a Needs-You feedback item carrying doubted_task_id");

    let raw = handle_tool_call(
        &state,
        "vox_resolve_feedback",
        serde_json::json!({
            "feedback_id": doubt_feedback.id.0,
            "action": { "action": "overrule" },
        }),
    )
    .await
    .expect("dispatch ok");
    assert!(
        !vox_orchestrator_mcp::server::tool_json_envelope_is_error(&raw),
        "resolve_feedback (overrule) call failed: {raw}"
    );

    let entries = state.orchestrator.list_recent_operations(None, 256).await;
    let saw_complete = entries.iter().any(|e| {
        matches!(
            &e.kind,
            vox_orchestrator::oplog::OperationKind::TaskComplete { task_id: tid }
                if *tid == task_id.0
        )
    });
    assert!(
        saw_complete,
        "TaskComplete for overruled task {task_id} must be queryable from the durable \
         op-log; entries: {entries:?}"
    );
}
