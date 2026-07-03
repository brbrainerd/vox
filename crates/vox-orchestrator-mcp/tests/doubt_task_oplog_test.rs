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
}
