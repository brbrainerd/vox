//! JSON-line orchestrator daemon (ADR 022 Phase B): newline-delimited [`vox_foundation::protocol::DispatchRequest`].
//!
//! **Transport (`vox-orchestrator-d` process):** set **`VOX_ORCHESTRATOR_DAEMON_SOCKET`** to
//! `127.0.0.1:9745`, optional `tcp://` prefix, or **`stdio`** / **`-`** / **`stdin`** for one line in, one line out on stdio.
//! **`vox-mcp`** uses the same variable as a **TCP peer address** for `orch.ping` health checks (stdio skipped).

mod client;
mod dei_dispatch;

pub use client::OrchDaemonClient;

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use vox_foundation::protocol::orch_daemon_method;
use vox_foundation::protocol::{DispatchPayload, DispatchRequest, DispatchResponse};

use crate::Orchestrator;
use crate::types::TaskId;
use crate::{CompletionAttestation, FileAffinity, TaskEnqueueHints, TaskPriority};

/// Strip optional `tcp://` prefix and whitespace.
#[must_use]
pub fn normalize_tcp_bind_addr(raw: &str) -> String {
    let s = raw.trim();
    s.strip_prefix("tcp://").unwrap_or(s).trim().to_string()
}

/// Stdio transport for `vox-orchestrator-d` (not a TCP bind address).
#[must_use]
pub fn is_stdio_transport(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "stdio" | "-" | "stdin"
    )
}

#[must_use]
pub fn response_result(id: &str, value: serde_json::Value) -> DispatchResponse {
    DispatchResponse {
        id: id.to_string(),
        payload: DispatchPayload::Result { value },
    }
}

#[must_use]
pub fn response_err(id: impl Into<String>, msg: impl Into<String>) -> DispatchResponse {
    DispatchResponse {
        id: id.into(),
        payload: DispatchPayload::Error {
            message: msg.into(),
            code: 1,
        },
    }
}

/// How often [`stream_status_events`] re-samples orchestrator status while a
/// subscriber is connected. Frames are only emitted when the snapshot changes.
const SUBSCRIBE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

/// Write one newline-delimited [`DispatchResponse`] frame and flush.
async fn write_frame<W: AsyncWriteExt + Unpin>(
    out: &mut W,
    resp: &DispatchResponse,
) -> anyhow::Result<()> {
    let mut line = serde_json::to_string(resp)?;
    line.push('\n');
    out.write_all(line.as_bytes()).await?;
    out.flush().await?;
    Ok(())
}

/// Push orchestrator status snapshots as [`DispatchPayload::Event`] frames until
/// the peer disconnects (a write error ends the stream). The daemon initiates
/// every frame; the subscriber never polls. An initial snapshot is sent
/// immediately, then a new frame is emitted on each change.
async fn stream_status_events<W: AsyncWriteExt + Unpin>(
    id: &str,
    orch: &Arc<Orchestrator>,
    out: &mut W,
) -> anyhow::Result<()> {
    let mut last = String::new();
    loop {
        let value = serde_json::to_value(orch.status()).unwrap_or(serde_json::Value::Null);
        let serialized = value.to_string();
        if serialized != last {
            last = serialized;
            let frame = DispatchResponse {
                id: id.to_string(),
                payload: DispatchPayload::Event { value },
            };
            // Propagates an error once the peer closes the connection, ending the stream.
            write_frame(out, &frame).await?;
        }
        tokio::time::sleep(SUBSCRIBE_POLL_INTERVAL).await;
    }
}

/// Push every `AgentEvent` from the orchestrator's broadcast event bus as a
/// [`DispatchPayload::Event`] frame until the peer disconnects (a write error
/// ends the stream). Fully push-driven — no polling. The bus has no replay, so
/// only events emitted after this subscription are delivered; broadcast lag
/// (slow consumer past the channel capacity) is skipped rather than fatal.
async fn stream_agent_events<W: AsyncWriteExt + Unpin>(
    id: &str,
    orch: &Arc<Orchestrator>,
    out: &mut W,
) -> anyhow::Result<()> {
    let mut rx = orch.event_bus().subscribe();
    loop {
        match rx.recv().await {
            Ok(event) => {
                let value = serde_json::to_value(&event).unwrap_or(serde_json::Value::Null);
                let frame = DispatchResponse {
                    id: id.to_string(),
                    payload: DispatchPayload::Event { value },
                };
                // Errors once the peer closes the connection, ending the stream.
                write_frame(out, &frame).await?;
            }
            // Slow consumer fell behind the broadcast capacity — skip the gap
            // and keep streaming rather than dropping the subscriber.
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            // Sender (orchestrator) dropped — nothing more will arrive.
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
        }
    }
}

/// Dispatch one parsed request against the live orchestrator.
pub async fn dispatch_request(
    repository_id: &str,
    orch: Arc<Orchestrator>,
    req: &DispatchRequest,
) -> DispatchResponse {
    if let Some(resp) = dei_dispatch::try_dispatch_dei(repository_id, Arc::clone(&orch), req).await
    {
        return resp;
    }
    match req.method.as_str() {
        orch_daemon_method::PING => response_result(
            &req.id,
            serde_json::json!({
                "ok": true,
                "repository_id": repository_id,
                "protocol": "vox.orchestrator_daemon/v1",
            }),
        ),
        orch_daemon_method::WORKSPACE_JOURNEY => {
            let hint = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let repo = vox_repository::discover_repository_or_fallback(&hint);
            let mut diag = vox_db::workspace_journey_diagnostics_json(&repo.root, repository_id);
            if let Some(obj) = diag.as_object_mut() {
                obj.insert(
                    "daemon_repository_id".to_string(),
                    serde_json::Value::String(repository_id.to_string()),
                );
                obj.insert(
                    "discovered_repository_id".to_string(),
                    serde_json::Value::String(repo.repository_id.clone()),
                );
            }
            response_result(&req.id, diag)
        }
        orch_daemon_method::RELOAD_CONFIG => {
            // Hot-reload orchestrator configuration from Vox.toml
            orch.reload_config();
            response_result(&req.id, serde_json::json!({ "ok": true }))
        }
        orch_daemon_method::UNDO_OPERATION => {
            let Some(op_id_str) = req.params.get("op_id").and_then(|x| x.as_str()) else {
                return response_err(&req.id, "params.op_id (string) required");
            };
            let num_str = op_id_str.trim_start_matches("OP-");
            let Ok(num) = num_str.parse::<u64>() else {
                return response_err(
                    &req.id,
                    "params.op_id must be a valid OperationId (e.g. OP-000007)",
                );
            };
            match orch.undo_operation(crate::oplog::OperationId(num)).await {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::REDO_OPERATION => {
            let Some(op_id_str) = req.params.get("op_id").and_then(|x| x.as_str()) else {
                return response_err(&req.id, "params.op_id (string) required");
            };
            let num_str = op_id_str.trim_start_matches("OP-");
            let Ok(num) = num_str.parse::<u64>() else {
                return response_err(
                    &req.id,
                    "params.op_id must be a valid OperationId (e.g. OP-000007)",
                );
            };
            match orch.redo_operation(crate::oplog::OperationId(num)).await {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::STATUS => match serde_json::to_value(orch.status()) {
            Ok(v) => response_result(&req.id, v),
            Err(e) => response_err(&req.id, e.to_string()),
        },
        orch_daemon_method::TASK_STATUS => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            match orch.task_lifecycle_status_label(TaskId(task_id)) {
                Some(label) => response_result(&req.id, serde_json::json!({ "status": label })),
                None => response_err(&req.id, format!("task {task_id} not found")),
            }
        }
        orch_daemon_method::SPAWN_AGENT => {
            let Some(name) = req.params.get("name").and_then(|x| x.as_str()) else {
                return response_err(&req.id, "params.name (string) required");
            };
            let name = name.trim();
            if name.is_empty() {
                return response_err(&req.id, "params.name must be non-empty");
            }
            match orch.spawn_agent(name) {
                Ok(id) => response_result(&req.id, serde_json::json!({ "agent_id": id.0 })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::AGENT_IDS => {
            let ids: Vec<u64> = orch.agent_ids().into_iter().map(|a| a.0).collect();
            response_result(&req.id, serde_json::json!({ "agent_ids": ids }))
        }
        orch_daemon_method::SUBMIT_TASK => {
            let Some(description) = req.params.get("description").and_then(|x| x.as_str()) else {
                return response_err(&req.id, "params.description (string) required");
            };
            let file_manifest = match req.params.get("file_manifest") {
                Some(v) => match serde_json::from_value::<Vec<FileAffinity>>(v.clone()) {
                    Ok(m) => m,
                    Err(e) => return response_err(&req.id, format!("invalid file_manifest: {e}")),
                },
                None => Vec::new(),
            };
            let priority = match req.params.get("priority") {
                Some(v) => match serde_json::from_value::<TaskPriority>(v.clone()) {
                    Ok(p) => Some(p),
                    Err(e) => return response_err(&req.id, format!("invalid priority: {e}")),
                },
                None => None,
            };
            let target_agent = req
                .params
                .get("target_agent")
                .and_then(|x| x.as_str())
                .map(ToString::to_string);
            let capability_requirements = match req.params.get("capability_requirements") {
                Some(v) => match serde_json::from_value::<crate::TaskCapabilityHints>(v.clone()) {
                    Ok(c) => Some(c),
                    Err(e) => {
                        return response_err(
                            &req.id,
                            format!("invalid capability_requirements: {e}"),
                        );
                    }
                },
                None => None,
            };
            let enqueue_hints = match req.params.get("enqueue_hints") {
                Some(v) => match serde_json::from_value::<TaskEnqueueHints>(v.clone()) {
                    Ok(h) => Some(h),
                    Err(e) => return response_err(&req.id, format!("invalid enqueue_hints: {e}")),
                },
                None => None,
            };
            let session_id = req
                .params
                .get("session_id")
                .and_then(|x| x.as_str())
                .map(ToString::to_string);
            let tenant_id = req
                .params
                .get("tenant_id")
                .and_then(|x| x.as_str())
                .map(ToString::to_string);
            match orch
                .submit_task_with_agent(
                    description.to_string(),
                    file_manifest,
                    priority,
                    target_agent,
                    capability_requirements,
                    enqueue_hints,
                    session_id,
                    tenant_id,
                )
                .await
            {
                Ok(task_id) => {
                    response_result(&req.id, serde_json::json!({ "task_id": task_id.0 }))
                }
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::COMPLETE_TASK => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            let attestation = match req.params.get("attestation") {
                Some(v) if v.is_null() => None,
                Some(v) => match serde_json::from_value::<CompletionAttestation>(v.clone()) {
                    Ok(a) => Some(a),
                    Err(e) => return response_err(&req.id, format!("invalid attestation: {e}")),
                },
                None => None,
            };
            match orch
                .complete_task_with_attestation(TaskId(task_id), attestation)
                .await
            {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::FAIL_TASK => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            let reason = req
                .params
                .get("reason")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            match orch.fail_task(TaskId(task_id), reason).await {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::CANCEL_TASK => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            match orch.cancel_task(TaskId(task_id)) {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::REORDER_TASK => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            let priority = match req.params.get("priority").and_then(|x| x.as_str()) {
                Some("urgent") => TaskPriority::Urgent,
                Some("background") => TaskPriority::Background,
                Some("normal") | None => TaskPriority::Normal,
                Some(other) => {
                    return response_err(
                        &req.id,
                        format!("invalid priority '{other}' (expected urgent|normal|background)"),
                    );
                }
            };
            match orch.reorder_task(TaskId(task_id), priority) {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::DRAIN_AGENT => {
            let Some(agent_id) = req.params.get("agent_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.agent_id (u64) required");
            };
            match orch.drain_agent(crate::AgentId(agent_id)) {
                Ok(drained) => response_result(
                    &req.id,
                    serde_json::json!({ "drained_count": drained.len() }),
                ),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::REBALANCE => {
            let rebalanced = orch.rebalance();
            response_result(&req.id, serde_json::json!({ "rebalanced": rebalanced }))
        }
        orch_daemon_method::SPAWN_AGENT_EXT => {
            let Some(name) = req.params.get("name").and_then(|x| x.as_str()) else {
                return response_err(&req.id, "params.name (string) required");
            };
            let dynamic = req
                .params
                .get("dynamic")
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            let parent_agent_id = req
                .params
                .get("parent_agent_id")
                .and_then(|x| x.as_u64())
                .map(crate::AgentId);
            let source_task_id = req
                .params
                .get("source_task_id")
                .and_then(|x| x.as_u64())
                .map(TaskId);
            let delegation_reason = req.params.get("delegation_reason").and_then(|x| x.as_str());
            let res = if dynamic {
                orch.spawn_dynamic_agent_with_parent(
                    name,
                    parent_agent_id,
                    delegation_reason,
                    source_task_id,
                    None,
                )
            } else {
                orch.spawn_agent(name)
            };
            match res {
                Ok(id) => response_result(&req.id, serde_json::json!({ "agent_id": id.0 })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::RETIRE_AGENT => {
            let Some(agent_id) = req.params.get("agent_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.agent_id (u64) required");
            };
            match orch.retire_agent(crate::AgentId(agent_id)).await {
                Ok(remaining) => response_result(
                    &req.id,
                    serde_json::json!({ "remaining_tasks": remaining.len() }),
                ),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::PAUSE_AGENT => {
            let Some(agent_id) = req.params.get("agent_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.agent_id (u64) required");
            };
            match orch.pause_agent(crate::AgentId(agent_id)) {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::RESUME_AGENT => {
            let Some(agent_id) = req.params.get("agent_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.agent_id (u64) required");
            };
            match orch.resume_agent(crate::AgentId(agent_id)) {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::DOUBT_TASK => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            let reason = req
                .params
                .get("reason")
                .and_then(|x| x.as_str())
                .map(ToString::to_string);
            match orch.doubt_task(TaskId(task_id), reason) {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::OVERRULE_TASK => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            let reason = req
                .params
                .get("reason")
                .and_then(|x| x.as_str())
                .map(ToString::to_string);
            match orch.overrule_task(TaskId(task_id), reason) {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::VCS_ISOLATION_STATUS => {
            let v = crate::json_vcs_facade::isolation_status_json(&orch);
            response_result(&req.id, v)
        }
        orch_daemon_method::VCS_ISOLATION_SET_STRATEGY => {
            handle_vcs_isolation_set_strategy(&req.id, &orch, &req.params)
        }
        other => response_err(&req.id, format!("unknown method: {other}")),
    }
}

/// Apply the default and/or a per-agent isolation override against the live
/// `IsolationPlan`, then return the fresh status. Parity with
/// `POST /api/v2/vcs/isolation/strategy`: `strategy_default` sets the baseline;
/// an `agent_id` with `strategy` present sets/clears that agent's override
/// (`null` clears); at least one of the two must be supplied.
fn handle_vcs_isolation_set_strategy(
    id: &str,
    orch: &Arc<Orchestrator>,
    params: &serde_json::Value,
) -> DispatchResponse {
    use crate::isolation::IsolationStrategy;

    let parse_strategy = |v: &serde_json::Value| -> Result<IsolationStrategy, String> {
        serde_json::from_value::<IsolationStrategy>(v.clone())
            .map_err(|e| format!("invalid strategy: {e}"))
    };

    let default = match params.get("strategy_default") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => match parse_strategy(v) {
            Ok(s) => Some(s),
            Err(e) => return response_err(id, e),
        },
    };
    let agent_id = params.get("agent_id").and_then(|x| x.as_u64());

    if default.is_none() && agent_id.is_none() {
        return response_err(id, "supply strategy_default and/or agent_id (+ strategy)");
    }

    // `strategy` field: absent => leave overrides untouched; present (value or
    // null) => set/clear the override for `agent_id`.
    let override_change: Option<Option<IsolationStrategy>> = match params.get("strategy") {
        None => None,
        Some(serde_json::Value::Null) => Some(None),
        Some(v) => match parse_strategy(v) {
            Ok(s) => Some(Some(s)),
            Err(e) => return response_err(id, e),
        },
    };

    {
        let handle = orch.isolation_policy_handle();
        let mut plan = crate::sync_lock::rw_write(&handle);
        if let Some(d) = default {
            plan.default = d;
        }
        if let Some(agent_id) = agent_id {
            if let Some(change) = override_change {
                plan.set_override(crate::AgentId(agent_id), change);
            }
        }
    }

    let v = crate::json_vcs_facade::isolation_status_json(orch);
    response_result(id, v)
}

/// Optional out-of-band dispatcher for methods that need state the core daemon
/// does not hold — e.g. an MCP `ServerState` for `orch.tool_call` /
/// `orch.resolve_approval` / `orch.list_pending_approvals`. The binary supplies
/// the impl; the library stays free of the heavy MCP layer (no dependency cycle).
#[async_trait::async_trait]
pub trait ExtraDispatch: Send + Sync {
    /// Return `Some(response)` to handle `req`; `None` to fall through to the
    /// built-in `orch.*` dispatch.
    async fn try_handle(&self, req: &DispatchRequest) -> Option<DispatchResponse>;
}

async fn handle_connection(
    mut socket: TcpStream,
    repository_id: String,
    orch: Arc<Orchestrator>,
    extra: Option<Arc<dyn ExtraDispatch>>,
) -> anyhow::Result<()> {
    let (read_half, mut write_half) = socket.split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req = match serde_json::from_str::<DispatchRequest>(trimmed) {
            Ok(req) => req,
            Err(e) => {
                let resp = response_err("0", format!("invalid DispatchRequest JSON: {e}"));
                write_frame(&mut write_half, &resp).await?;
                continue;
            }
        };
        if req.method == orch_daemon_method::SUBSCRIBE {
            // Long-lived push stream; returns when the peer disconnects.
            stream_status_events(&req.id, &orch, &mut write_half).await?;
            break;
        }
        if req.method == orch_daemon_method::SUBSCRIBE_EVENTS {
            stream_agent_events(&req.id, &orch, &mut write_half).await?;
            break;
        }
        if let Some(ex) = extra.as_ref() {
            if let Some(resp) = ex.try_handle(&req).await {
                write_frame(&mut write_half, &resp).await?;
                continue;
            }
        }
        let resp = dispatch_request(&repository_id, orch.clone(), &req).await;
        write_frame(&mut write_half, &resp).await?;
    }
    Ok(())
}

/// Accept connections until `listener` is dropped (runs forever on success).
pub async fn serve_listener(
    listener: TcpListener,
    bind_display: String,
    repository_id: String,
    orch: Arc<Orchestrator>,
) -> anyhow::Result<()> {
    serve_listener_with_extra(listener, bind_display, repository_id, orch, None).await
}

/// [`serve_listener`] with an optional [`ExtraDispatch`] hook (the daemon binary
/// wires one carrying its MCP `ServerState`).
pub async fn serve_listener_with_extra(
    listener: TcpListener,
    bind_display: String,
    repository_id: String,
    orch: Arc<Orchestrator>,
    extra: Option<Arc<dyn ExtraDispatch>>,
) -> anyhow::Result<()> {
    tracing::info!(bind = %bind_display, "vox-orchestrator-d listening");
    loop {
        let (socket, peer) = listener.accept().await?;
        tracing::debug!(%peer, "orch daemon accepted");
        let repo = repository_id.clone();
        let o = orch.clone();
        let ex = extra.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, repo, o, ex).await {
                tracing::debug!(error = %e, "orch daemon connection closed with error");
            }
        });
    }
}

/// Bind `bind` then [`serve_listener`].
pub async fn run_tcp_server(
    bind: &str,
    repository_id: String,
    orch: Arc<Orchestrator>,
) -> anyhow::Result<()> {
    run_tcp_server_with_extra(bind, repository_id, orch, None).await
}

/// [`run_tcp_server`] with an optional [`ExtraDispatch`] hook.
pub async fn run_tcp_server_with_extra(
    bind: &str,
    repository_id: String,
    orch: Arc<Orchestrator>,
    extra: Option<Arc<dyn ExtraDispatch>>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(bind).await?;
    serve_listener_with_extra(listener, bind.to_string(), repository_id, orch, extra).await
}

/// Read newline-delimited [`DispatchRequest`] from stdin; write [`DispatchResponse`] lines to stdout.
pub async fn run_stdio_server(
    repository_id: String,
    orch: Arc<Orchestrator>,
) -> anyhow::Result<()> {
    run_stdio_server_with_extra(repository_id, orch, None).await
}

/// [`run_stdio_server`] with an optional [`ExtraDispatch`] hook.
pub async fn run_stdio_server_with_extra(
    repository_id: String,
    orch: Arc<Orchestrator>,
    extra: Option<Arc<dyn ExtraDispatch>>,
) -> anyhow::Result<()> {
    tracing::info!("vox-orchestrator-d serving on stdio (line-delimited JSON)");
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req = match serde_json::from_str::<DispatchRequest>(trimmed) {
            Ok(req) => req,
            Err(e) => {
                let resp = response_err("0", format!("invalid DispatchRequest JSON: {e}"));
                write_frame(&mut stdout, &resp).await?;
                continue;
            }
        };
        if req.method == orch_daemon_method::SUBSCRIBE {
            stream_status_events(&req.id, &orch, &mut stdout).await?;
            break;
        }
        if req.method == orch_daemon_method::SUBSCRIBE_EVENTS {
            stream_agent_events(&req.id, &orch, &mut stdout).await?;
            break;
        }
        if let Some(ex) = extra.as_ref() {
            if let Some(resp) = ex.try_handle(&req).await {
                write_frame(&mut stdout, &resp).await?;
                continue;
            }
        }
        let resp = dispatch_request(&repository_id, orch.clone(), &req).await;
        write_frame(&mut stdout, &resp).await?;
    }
    Ok(())
}

#[cfg(test)]
mod isolation_dispatch_tests {
    use super::*;
    use crate::config::OrchestratorConfig;

    fn req(method: &str, params: serde_json::Value) -> DispatchRequest {
        DispatchRequest {
            id: "1".to_string(),
            method: method.to_string(),
            params,
        }
    }

    fn result_value(resp: &DispatchResponse) -> &serde_json::Value {
        match &resp.payload {
            DispatchPayload::Result { value } => value,
            other => panic!("expected Result payload, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn status_returns_default_and_empty_collections() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::default()));
        let resp = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::VCS_ISOLATION_STATUS,
                serde_json::json!({}),
            ),
        )
        .await;
        let v = result_value(&resp);
        assert_eq!(v["strategy_default"], "shared_branch");
        assert_eq!(v["per_agent"].as_object().map(|m| m.len()), Some(0));
        assert_eq!(v["active_conflicts"].as_array().map(|a| a.len()), Some(0));
    }

    #[tokio::test]
    async fn set_strategy_default_persists_in_shared_orchestrator() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::default()));
        let set = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::VCS_ISOLATION_SET_STRATEGY,
                serde_json::json!({ "strategy_default": "separate_branches" }),
            ),
        )
        .await;
        assert_eq!(result_value(&set)["strategy_default"], "separate_branches");

        // A subsequent status call against the SAME orchestrator must observe it.
        let status = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::VCS_ISOLATION_STATUS,
                serde_json::json!({}),
            ),
        )
        .await;
        assert_eq!(
            result_value(&status)["strategy_default"],
            "separate_branches"
        );
    }

    #[tokio::test]
    async fn set_then_clear_per_agent_override() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::default()));
        let set = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::VCS_ISOLATION_SET_STRATEGY,
                serde_json::json!({ "agent_id": 7, "strategy": "split_changes" }),
            ),
        )
        .await;
        assert_eq!(result_value(&set)["per_agent"]["7"], "split_changes");

        let clear = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::VCS_ISOLATION_SET_STRATEGY,
                serde_json::json!({ "agent_id": 7, "strategy": serde_json::Value::Null }),
            ),
        )
        .await;
        assert_eq!(
            result_value(&clear)["per_agent"]
                .as_object()
                .map(|m| m.len()),
            Some(0)
        );
    }

    #[tokio::test]
    async fn set_with_no_fields_is_error() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::default()));
        let resp = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::VCS_ISOLATION_SET_STRATEGY,
                serde_json::json!({}),
            ),
        )
        .await;
        assert!(matches!(resp.payload, DispatchPayload::Error { .. }));
    }

    #[tokio::test]
    async fn invalid_strategy_is_error() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::default()));
        let resp = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::VCS_ISOLATION_SET_STRATEGY,
                serde_json::json!({ "strategy_default": "bogus" }),
            ),
        )
        .await;
        assert!(matches!(resp.payload, DispatchPayload::Error { .. }));
    }
}
