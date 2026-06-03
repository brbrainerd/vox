//! Daemon [`ExtraDispatch`] impl (B5 path-c + B3 cross-process).
//!
//! Serves `orch.tool_call` / `orch.list_pending_approvals` / `orch.resolve_approval`
//! against the daemon's own MCP [`ServerState`], so callers (the GUI, peers) run
//! tools and resolve HITL approvals through the one shared orchestrator instead
//! of a second in-process one. Wired by `vox-orchestrator-d`. Lives here (not in
//! `vox-orchestrator`) to avoid a dependency cycle — the `ExtraDispatch` trait is
//! defined in `vox-orchestrator`, the heavy `ServerState` stays in this crate.

use async_trait::async_trait;

use vox_foundation::protocol::{
    DispatchPayload, DispatchRequest, DispatchResponse, dei_method, orch_daemon_method,
};
use vox_orchestrator::orch_daemon::ExtraDispatch;

use crate::server_state::ServerState;

/// [`ExtraDispatch`] backed by an MCP [`ServerState`].
pub struct McpExtraDispatch {
    state: ServerState,
}

impl McpExtraDispatch {
    // toestub-ignore(skeleton/untested-pub-api) — constructor wires ServerState; exercised via tests/daemon_extra_tests.rs
    #[must_use]
    pub fn new(state: ServerState) -> Self {
        Self { state }
    }
}

fn result(id: &str, value: serde_json::Value) -> DispatchResponse {
    DispatchResponse {
        id: id.to_string(),
        payload: DispatchPayload::Result { value },
    }
}

fn error(id: &str, message: impl Into<String>) -> DispatchResponse {
    DispatchResponse {
        id: id.to_string(),
        payload: DispatchPayload::Error {
            message: message.into(),
            code: 1,
        },
    }
}

#[async_trait]
impl ExtraDispatch for McpExtraDispatch {
    async fn try_handle(&self, req: &DispatchRequest) -> Option<DispatchResponse> {
        match req.method.as_str() {
            // Enqueue a SCIENTIA research run inside this persistent daemon. The
            // underlying MCP handler creates the session row, spawns the pipeline
            // (which advances the session through real stages to a terminal
            // `completed`/`failed`), and returns immediately — fire-and-forget.
            // This is the cross-process executor for `vox research run --async`,
            // which only inserts a `queued` row and never runs the pipeline.
            dei_method::RESEARCH_RUN => {
                let params: crate::memory::ResearchStartParams =
                    match serde_json::from_value(req.params.clone()) {
                        Ok(p) => p,
                        Err(e) => {
                            return Some(error(
                                &req.id,
                                format!("invalid research.run params: {e}"),
                            ));
                        }
                    };
                // `research_start` is the async fire-and-forget executor: it
                // creates the session, spawns the pipeline (which advances to a
                // terminal `completed`/`failed`), and returns a `running`
                // envelope immediately. Returns a JSON-string `ToolResult`.
                let json = crate::memory::research_start(&self.state, params).await;
                let envelope = serde_json::from_str::<serde_json::Value>(&json)
                    .unwrap_or_else(|_| serde_json::json!({ "raw": json }));
                // On failure, surface the ToolResult error as a daemon error.
                if envelope.get("success") == Some(&serde_json::Value::Bool(false)) {
                    let msg = envelope
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("research.run failed");
                    return Some(error(&req.id, msg.to_string()));
                }
                // `research_start` nests its `{session_id, task_id, status}`
                // object as a JSON *string* under `data`; unwrap it so callers
                // get a clean object rather than a stringified envelope.
                let value = envelope
                    .get("data")
                    .and_then(|d| d.as_str())
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                    .unwrap_or(envelope);
                Some(result(&req.id, value))
            }
            orch_daemon_method::TOOL_CALL => {
                let Some(name) = req.params.get("name").and_then(|v| v.as_str()) else {
                    return Some(error(&req.id, "params.name (string) required"));
                };
                let args = req
                    .params
                    .get("args")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                match crate::handle_tool_call(&self.state, name, args).await {
                    Ok(json) => {
                        let value = serde_json::from_str::<serde_json::Value>(&json)
                            .unwrap_or_else(|_| serde_json::json!({ "raw": json }));
                        Some(result(&req.id, value))
                    }
                    Err(e) => Some(error(&req.id, format!("tool '{name}' failed: {e}"))),
                }
            }
            orch_daemon_method::LIST_PENDING_APPROVALS => Some(result(
                &req.id,
                serde_json::json!({ "approvals": self.state.pending_approvals.list() }),
            )),
            orch_daemon_method::RESOLVE_APPROVAL => {
                let approval_id = req
                    .params
                    .get("approval_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let decision = req
                    .params
                    .get("outcome")
                    .or_else(|| req.params.get("decision"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let outcome = match decision {
                    "approve" | "approved" => vox_orchestrator::ApprovalOutcome::Approved,
                    "modify" | "modified" => vox_orchestrator::ApprovalOutcome::Modified,
                    "reject" | "rejected" => vox_orchestrator::ApprovalOutcome::Rejected,
                    other => {
                        return Some(error(
                            &req.id,
                            format!(
                                "unrecognized approval decision {other:?}; expected one of approve|approved|modify|modified|reject|rejected"
                            ),
                        ));
                    }
                };
                let resolved = self.state.pending_approvals.resolve(approval_id, outcome);
                Some(result(
                    &req.id,
                    serde_json::json!({ "resolved": resolved, "approval_id": approval_id }),
                ))
            }
            _ => None,
        }
    }
}
