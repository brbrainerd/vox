//! Bridges `vox-orchestrator`'s autonomous task loop (`AiTaskProcessor`) into
//! real MCP tool dispatch (T1.5 follow-up, harness reliability spec,
//! `docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md`).
//!
//! Before this module existed, an agent's own `@tool` intent line — emitted
//! while the orchestrator autonomously executed a task — was only logged as a
//! tracing breadcrumb. It never reached [`crate::dispatch::handle_tool_call_with_mode`],
//! so dangerous-tool approvals never got `task_id` correlation for genuinely
//! autonomous calls (only GUI-invoked `orch.tool_call` calls did). See the
//! historical audit in `crates/vox-orchestrator-queue/src/oplog/mod.rs`'s
//! `OperationKind::ApprovalRequested` doc comment.
//!
//! [`McpToolDispatcher`] implements `vox_orchestrator::runtime::ToolDispatcher`
//! (defined in `vox-orchestrator`, not here — same dependency-direction
//! constraint as `daemon_extra.rs`'s `ExtraDispatch`: `vox-orchestrator` cannot
//! depend on this crate) against this crate's [`ServerState`], the same state
//! GUI-driven `orch.tool_call` calls run against.

use std::sync::Arc;

use async_trait::async_trait;
use vox_orchestrator::runtime::ToolDispatcher;
use vox_orchestrator::types::{AgentId, TaskId};

use crate::server_state::ServerState;

/// [`ToolDispatcher`] backed by an MCP [`ServerState`].
pub struct McpToolDispatcher {
    state: ServerState,
}

impl McpToolDispatcher {
    #[must_use]
    pub fn new(state: ServerState) -> Self {
        Self { state }
    }

    /// Wraps `Self::new` in an `Arc` for direct use with
    /// `AiTaskProcessor::with_tool_dispatcher`.
    #[must_use]
    pub fn new_arc(state: ServerState) -> Arc<dyn ToolDispatcher> {
        Arc::new(Self::new(state))
    }
}

#[async_trait]
impl ToolDispatcher for McpToolDispatcher {
    async fn dispatch(
        &self,
        task_id: TaskId,
        agent_id: AgentId,
        tool_name: &str,
        mut args: serde_json::Value,
        permission_mode: Option<&str>,
    ) -> anyhow::Result<String> {
        // `task_id` is threaded in here — the one place this bridge writes
        // it — rather than trusting an `args["task_id"]` the LLM's own
        // narration might (or might not) have set. `handle_tool_call_with_mode`
        // reads `args.get("task_id")` purely as a best-effort `run_id`
        // correlation fallback (see `dispatch.rs`'s `run_id_for_approval`); it
        // is never consulted for approval-bypass decisions (T0.1), so
        // overwriting it here with the caller-verified real task id is safe
        // and strictly more trustworthy than whatever the model wrote, if
        // anything.
        if let serde_json::Value::Object(ref mut map) = args {
            map.insert("task_id".to_string(), serde_json::json!(task_id.0));
            // Best-effort: also identify the calling agent, mirroring what
            // GUI-driven calls set explicitly.
            map.entry("agent_id".to_string())
                .or_insert_with(|| serde_json::json!(agent_id.0.to_string()));
        } else {
            args = serde_json::json!({
                "task_id": task_id.0,
                "agent_id": agent_id.0.to_string(),
            });
        }

        crate::dispatch::handle_tool_call_with_mode(&self.state, tool_name, args, permission_mode)
            .await
    }
}
