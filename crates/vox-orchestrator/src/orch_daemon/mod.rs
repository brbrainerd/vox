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
                return response_err(&req.id, "params.op_id must be a valid OperationId (e.g. OP-000007)");
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
                return response_err(&req.id, "params.op_id must be a valid OperationId (e.g. OP-000007)");
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
        other => response_err(&req.id, format!("unknown method: {other}")),
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    repository_id: String,
    orch: Arc<Orchestrator>,
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
        let resp = match serde_json::from_str::<DispatchRequest>(trimmed) {
            Ok(req) => dispatch_request(&repository_id, orch.clone(), &req).await,
            Err(e) => response_err("0", format!("invalid DispatchRequest JSON: {e}")),
        };
        let mut out = serde_json::to_string(&resp)?;
        out.push('\n');
        write_half.write_all(out.as_bytes()).await?;
        write_half.flush().await?;
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
    tracing::info!(bind = %bind_display, "vox-orchestrator-d listening");
    loop {
        let (socket, peer) = listener.accept().await?;
        tracing::debug!(%peer, "orch daemon accepted");
        let repo = repository_id.clone();
        let o = orch.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, repo, o).await {
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
    let listener = TcpListener::bind(bind).await?;
    serve_listener(listener, bind.to_string(), repository_id, orch).await
}

/// Read newline-delimited [`DispatchRequest`] from stdin; write [`DispatchResponse`] lines to stdout.
pub async fn run_stdio_server(
    repository_id: String,
    orch: Arc<Orchestrator>,
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
        let resp = match serde_json::from_str::<DispatchRequest>(trimmed) {
            Ok(req) => dispatch_request(&repository_id, orch.clone(), &req).await,
            Err(e) => response_err("0", format!("invalid DispatchRequest JSON: {e}")),
        };
        let mut out = serde_json::to_string(&resp)?;
        out.push('\n');
        stdout.write_all(out.as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}
