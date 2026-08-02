//! T1.5 follow-up: `AiTaskProcessor`'s autonomous `@tool` intent lines must
//! actually dispatch through the real MCP gate (`handle_tool_call_with_mode`),
//! not just log a tracing breadcrumb — see
//! `docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md`
//! T1.5's "known gap" note and the doc comment on
//! `OperationKind::ApprovalRequested::run_id`
//! (`crates/vox-orchestrator-queue/src/oplog/mod.rs`).
//!
//! These tests exercise the bridge at two levels:
//! - `vox-orchestrator`'s pure `parse_tool_intent_line` line-parser (no MCP
//!   dependency available there — see its own unit tests in `runtime.rs`).
//! - `McpToolDispatcher::dispatch` here: proves a `TaskId` passed as an
//!   explicit parameter (not caller-supplied `args`) reaches
//!   `dispatch.rs`'s dangerous-tool approval gate and is recorded as the
//!   durable `ApprovalRequested.run_id`, and that a model-supplied
//!   `args["task_id"]` cannot override it.

use std::sync::Arc;

use vox_orchestrator::ApprovalOutcome;
use vox_orchestrator::runtime::ToolDispatcher;
use vox_orchestrator::types::{AgentId, TaskId};
use vox_orchestrator_mcp::autonomous_tool_dispatch::McpToolDispatcher;
use vox_orchestrator_mcp::server::tool_json_envelope_is_error;
use vox_orchestrator_mcp::{ServerState, load_config};

const D_15S: std::time::Duration = std::time::Duration::from_secs(15);

/// A dangerous tool dispatched via `McpToolDispatcher::dispatch` (the bridge
/// `AiTaskProcessor` calls for a detected `@tool` intent line) must actually
/// reach the approval gate and park a pending approval — proving real
/// dispatch happened, not just a tracing breadcrumb.
#[tokio::test]
async fn dispatch_actually_parks_a_pending_approval() {
    let state = Arc::new(ServerState::new_full(load_config()));
    let dispatcher = McpToolDispatcher::new((*state).clone());

    let call = tokio::spawn(async move {
        dispatcher
            .dispatch(
                TaskId(777),
                AgentId(3),
                "vox_run_shell",
                serde_json::json!({ "command": "echo hi" }),
                None,
            )
            .await
    });

    let deadline = tokio::time::Instant::now() + D_15S;
    loop {
        if !state.pending_approvals.list().is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "autonomous dispatch never registered a pending approval — bridge did not reach the gate"
        );
        tokio::time::sleep(vox_config::timeouts::D_20MS).await;
    }

    let pending = state.pending_approvals.list();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tool, "vox_run_shell");
    let approval_id = pending[0].approval_id.clone();

    assert!(
        state
            .pending_approvals
            .resolve(&approval_id, ApprovalOutcome::Rejected)
    );
    let raw = call
        .await
        .expect("join")
        .expect("dispatch returns Ok even for a rejected approval envelope");
    assert!(
        tool_json_envelope_is_error(&raw),
        "rejected approval must yield an error envelope, got: {raw}"
    );
}

/// The `TaskId` explicit parameter — not any `args["task_id"]` the caller
/// composes — is what reaches `dispatch.rs`'s `run_id_for_approval` fallback.
/// `McpToolDispatcher::dispatch` overwrites `args["task_id"]` with the real
/// task id immediately before calling `handle_tool_call_with_mode`, so the
/// durable `ApprovalRequested.run_id` always reflects the caller-verified
/// value even if the model's own narration tried to set a different one.
#[tokio::test]
async fn task_id_parameter_wins_over_model_supplied_args_task_id() {
    let state = Arc::new(ServerState::new_full(load_config()));
    let dispatcher = McpToolDispatcher::new((*state).clone());

    let call = tokio::spawn(async move {
        dispatcher
            .dispatch(
                TaskId(9001),
                AgentId(1),
                "vox_run_shell",
                // A model could (accidentally or adversarially) narrate a
                // different task_id in its own JSON args; the bridge must not
                // trust it.
                serde_json::json!({ "command": "echo hi", "task_id": 1 }),
                None,
            )
            .await
    });

    let deadline = tokio::time::Instant::now() + D_15S;
    loop {
        if !state.pending_approvals.list().is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "autonomous dispatch never registered a pending approval"
        );
        tokio::time::sleep(vox_config::timeouts::D_20MS).await;
    }
    let pending = state.pending_approvals.list();
    assert_eq!(pending.len(), 1);
    let approval_id = pending[0].approval_id.clone();

    assert!(
        state
            .pending_approvals
            .resolve(&approval_id, ApprovalOutcome::Rejected)
    );
    let raw = call.await.expect("join").expect("dispatch ok");
    assert!(tool_json_envelope_is_error(&raw));

    let deadline = tokio::time::Instant::now() + D_15S;
    let mut saw_run_id;
    loop {
        let entries = state.orchestrator.list_recent_operations(None, 256).await;
        saw_run_id = entries.iter().any(|e| {
            matches!(
                &e.kind,
                vox_orchestrator::oplog::OperationKind::ApprovalRequested { approval_id: aid, run_id, .. }
                    if aid == &approval_id && run_id.as_deref() == Some("9001")
            )
        });
        if saw_run_id {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "ApprovalRequested.run_id never reflected the explicit task_id parameter (9001)"
        );
        tokio::time::sleep(vox_config::timeouts::D_20MS).await;
    }
    assert!(
        saw_run_id,
        "ApprovalRequested.run_id must equal the explicit TaskId parameter, not a model-supplied args[\"task_id\"]"
    );
}

/// A non-dangerous (unclassified) tool dispatched via the bridge with no
/// `args` at all still completes without panicking and returns a usable
/// envelope — the bridge must gracefully synthesize an `args` object even
/// when the parsed `@tool` line carried none.
#[tokio::test]
async fn dispatch_with_empty_args_object_succeeds_for_a_readonly_tool() {
    let state = ServerState::new_full(load_config());
    let dispatcher = McpToolDispatcher::new(state.clone());

    let raw = dispatcher
        .dispatch(
            TaskId(55),
            AgentId(2),
            "vox_git_status",
            serde_json::json!({}),
            None,
        )
        .await
        .expect("dispatch ok");

    assert_ne!(
        serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| v.get("success").cloned()),
        Some(serde_json::Value::Bool(false)),
        "vox_git_status via the autonomous bridge should succeed, got: {raw}"
    );
}
