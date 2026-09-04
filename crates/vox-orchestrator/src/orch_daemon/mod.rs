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

/// Is `addr` (a normalized `host:port` bind address) a loopback address?
/// Recognizes `127.0.0.1`, `localhost`, and `::1` (with or without a trailing
/// `:port`; `::1` may also appear bracketed as `[::1]:port`). Used to decide
/// whether `vox-orchestrator-d` may bind without an explicitly operator-set
/// `VOX_ORCHESTRATOR_DAEMON_TOKEN` (T0.2).
#[must_use]
pub fn is_loopback_bind_addr(addr: &str) -> bool {
    let s = addr.trim();
    // Bare unbracketed IPv6 loopback, with no `:port` suffix (ambiguous to
    // split on ':' otherwise, since the host itself contains colons).
    if s == "::1" {
        return true;
    }
    // Bracketed IPv6 host (`[::1]:9745` or bare `[::1]`).
    if let Some(rest) = s.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            let host = &rest[..end];
            return host == "::1";
        }
        return false;
    }
    // Host portion is everything before the last `:port`, if present and the
    // remainder after it parses as a port number; otherwise treat the whole
    // string as the host (covers bare "localhost" / "127.0.0.1").
    let host = match s.rsplit_once(':') {
        Some((h, port)) if !h.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => h,
        _ => s,
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// Constant-time byte comparison to avoid a timing side channel on the daemon
/// auth token (T0.2). Mirrors the existing helper of the same name/shape in
/// `vox-orchestrator-mcp::http_gateway` and `vox-actor-runtime::auth` — kept
/// as a small local copy here rather than a shared dependency, matching how
/// those other crates each already define their own.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() != b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        let ai = *a.get(i).unwrap_or(&0);
        let bi = *b.get(i).unwrap_or(&0);
        diff |= ai ^ bi;
    }
    diff == 0
}

#[cfg(test)]
mod constant_time_eq_tests {
    use super::*;

    #[test]
    fn equal_bytes_match() {
        assert!(constant_time_eq(b"same-token", b"same-token"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn different_bytes_do_not_match() {
        assert!(!constant_time_eq(b"correct-token", b"wrong-token-value"));
        assert!(!constant_time_eq(b"short", b"shorter-or-longer"));
        assert!(!constant_time_eq(b"a", b""));
        assert!(!constant_time_eq(b"", b"a"));
    }
}

#[cfg(test)]
mod loopback_bind_addr_tests {
    use super::*;

    #[test]
    fn loopback_addresses_recognized() {
        assert!(is_loopback_bind_addr("127.0.0.1:9745"));
        assert!(is_loopback_bind_addr("127.0.0.1"));
        assert!(is_loopback_bind_addr("localhost:9745"));
        assert!(is_loopback_bind_addr("localhost"));
        assert!(is_loopback_bind_addr("::1"));
        assert!(is_loopback_bind_addr("[::1]:9745"));
        assert!(is_loopback_bind_addr("[::1]"));
    }

    #[test]
    fn non_loopback_addresses_rejected() {
        assert!(!is_loopback_bind_addr("0.0.0.0:9745"));
        assert!(!is_loopback_bind_addr("0.0.0.0"));
        assert!(!is_loopback_bind_addr("192.168.1.5:9745"));
        assert!(!is_loopback_bind_addr("example.com:9745"));
        assert!(!is_loopback_bind_addr("[::]:9745"));
    }
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
///
/// T1.3 note: `orch.subscribe` carries whole-status *snapshots*, not discrete
/// durably-ided operations — there is no `OperationId`-keyed history to replay
/// here the way `stream_agent_events` replays durable Tier-A ops. A
/// `from_offset` param would have nothing meaningful to do, so this function
/// intentionally does not accept one; a reconnecting client just gets the
/// current snapshot immediately (which is already always-fresh, unlike a
/// discrete event log).
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
///
/// T1.2 note on Tier-B batching/coalescing: the plan language calls for the
/// daemon relay to batch/coalesce Tier-B frames (`TokenStreamed` etc.) so a
/// slow client lags on tokens, never on lifecycle. This was evaluated and
/// deliberately deferred rather than implemented here: the wire contract for
/// this stream (`OrchDaemonClient::subscribe_with_method`, see
/// `orch_daemon/client.rs`) forwards each `DispatchPayload::Event.value` to
/// callers as **one serialized `AgentEvent`** (`{id, timestamp_ms, kind}`);
/// existing consumers (dashboard SSE bridges, CLI subscribers) deserialize
/// each pushed value as a single event, not an array. Coalescing multiple
/// Tier-B events into one frame would require either (a) changing that wire
/// contract (a breaking change to every consumer, out of scope for a T1.2
/// reliability fix) or (b) a non-standard sentinel/array-vs-object framing
/// distinguished per-message (meaningfully more complex, and risks
/// destabilizing this already-working streaming path for a nice-to-have).
/// What *is* already true, and was verified rather than assumed: this loop
/// never blocks indefinitely on a slow client — `write_frame` bounds each
/// write to the size of one JSON line, and a genuinely stuck TCP write
/// eventually errors (peer closed / OS buffer semantics) rather than hanging
/// forever; and `RecvError::Lagged` already means a slow consumer skips
/// stale broadcast entries instead of blocking the sender. So the "never
/// lags on lifecycle" property holds today at the *broadcast* layer (Tier-A
/// events are never the ones silently dropped by `Lagged`, because Tier-A
/// callers durably record them independently of bus delivery — see
/// `events::is_tier_a`); it just isn't reinforced by frame-level batching in
/// this relay loop. Follow-up: introduce a versioned array/batch frame kind
/// in `vox_foundation::protocol::DispatchPayload` and update
/// `subscribe_with_method` to unwrap it, then reintroduce coalescing here.
/// One durable Tier-A op re-shaped as a replay frame value (T1.3). Deliberately
/// **not** disguised as an `AgentEvent` — reconstructing the original
/// `AgentEventKind` from a durable `OperationKind` would need a fragile
/// reverse-mapping (the two enums are not 1:1; several `AgentEventKind`
/// variants carry fields `OperationKind` never recorded). Instead this is an
/// honest, distinctly-shaped envelope a caller can tell apart from a live
/// `AgentEvent` frame (`replay: true`, `op_id` instead of live `id`). Existing
/// consumers (dashboard/GUI bridges) only forward `Value` verbatim today — see
/// `crates/vox-gui/src/commands/orchestrator.rs`'s `spawn_agent_event_stream`
/// — so this new shape does not break them; a caller that cares distinguishes
/// replay frames from live ones by checking for the `replay` key.
fn replay_frame_value(entry: &vox_orchestrator_queue::oplog::OperationEntry) -> serde_json::Value {
    serde_json::json!({
        "replay": true,
        "op_id": entry.id.0,
        "agent_id": entry.agent_id.0,
        "timestamp_ms": entry.timestamp_ms,
        "description": entry.description,
        "kind": serde_json::to_value(&entry.kind).unwrap_or(serde_json::Value::Null),
    })
}

/// [`stream_agent_events`], optionally preceded by a **replay phase** (T1.3)
/// when `from_offset` is `Some`: every durable Tier-A op with `op_id >
/// from_offset` for this daemon's repository is pushed (oldest-first, each as
/// a [`replay_frame_value`] envelope) *before* the live-tail loop begins.
/// `from_offset` absent reproduces today's behavior exactly (live-tail only,
/// no replay, no back-compat break for existing callers).
///
/// Known limitation (documented per plan rather than built out): there is a
/// narrow window between "replay query executed" and "live subscription
/// established" in which a new Tier-A op could land and be missed by both —
/// the replay already ran, and the live `rx.recv()` subscription is created
/// strictly after it. Closing this gap fully would mean subscribing to the
/// live bus (buffering) *before* running the replay query, then de-duplicating
/// against whatever replay returns by `op_id`. That is a bigger redesign than
/// this task's scope; the accepted fallback is a best-effort narrow gap. A
/// client that needs gapless delivery can mitigate by reconnecting with
/// `from_offset` set to the last op_id/event id it saw, same as the
/// Lagged-reconnect story below.
async fn stream_agent_events_from<W: AsyncWriteExt + Unpin>(
    id: &str,
    orch: &Arc<Orchestrator>,
    out: &mut W,
    from_offset: Option<u64>,
) -> anyhow::Result<()> {
    if let Some(since) = from_offset {
        if let Some(db) = orch.db() {
            let repo = crate::lineage::repository_id();
            match vox_orchestrator_queue::oplog::list_from_db_since(&db, repo.as_str(), since).await
            {
                Ok(entries) => {
                    for entry in &entries {
                        let frame = DispatchResponse {
                            id: id.to_string(),
                            payload: DispatchPayload::Event {
                                value: replay_frame_value(entry),
                            },
                        };
                        write_frame(out, &frame).await?;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        from_offset = since,
                        "orch.subscribe_events: replay query failed; proceeding to live-tail \
                         without replay (client may be missing durable history)"
                    );
                }
            }
        } else {
            tracing::warn!(
                from_offset = since,
                "orch.subscribe_events: from_offset requested but no DB attached; \
                 no durable history to replay, proceeding straight to live-tail"
            );
        }
    }

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
            // Slow consumer fell behind the broadcast capacity. Always log the
            // skip count (T1.3) — previously silent. Behavior then forks on
            // whether this subscriber is offset-aware:
            //   * offset-aware (`from_offset.is_some()`): end the stream with a
            //     structured error naming a reconnect offset, rather than
            //     silently continuing to lose events under an API the client
            //     has already opted into being able to recover via.
            //   * legacy (no `from_offset`): keep the existing `continue`
            //     behavior unchanged — an existing non-offset-aware caller has
            //     no way to ask for a resumable reconnect, so ending the
            //     stream here would just be a regression (stream dies with no
            //     recovery path) rather than an improvement.
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(
                    skipped,
                    offset_aware = from_offset.is_some(),
                    "orch.subscribe_events: broadcast receiver lagged, skipped {skipped} events"
                );
                if from_offset.is_some() {
                    let frame = DispatchResponse {
                        id: id.to_string(),
                        payload: DispatchPayload::Error {
                            message: format!(
                                "lagged: skipped {skipped} live events; reconnect with a \
                                 from_offset at or after the last op_id/event id you \
                                 successfully received"
                            ),
                            code: 2,
                        },
                    };
                    let _ = write_frame(out, &frame).await;
                    return Ok(());
                }
                continue;
            }
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
                "version": env!("CARGO_PKG_VERSION"),
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
        orch_daemon_method::SUBAGENT_TREE => {
            // Phase D Task D2: the daemon RPC counterpart of `vox-gui`'s
            // `list_subagent_tree` Tauri command (crates/vox-gui/src/commands/mission_control.rs),
            // which has been calling this method since it was added but had no
            // handler here — every call previously fell through to the
            // catch-all `Method not found` arm below, so the GUI's SubAgents
            // surface always rendered empty. Field names/shapes here must match
            // `SubagentTreeNode`/`SubagentTreeEdge` on the Tauri and TS sides:
            // `task_id` mirrors those callers' existing 0-as-unset convention
            // for an absent `source_task_id` (see `record_lineage_event`'s own
            // `task_id.unwrap_or(0)` in orchestrator/core/lineage.rs).
            // `chat_session_id`/`origin_turn_id` (Phase D Task D1) ride along
            // as additional fields for D3 correlation surfaces to consume.
            let snapshot = orch.topology_snapshot();
            let tree: Vec<serde_json::Value> = snapshot
                .delegation_edges
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "task_id": e.source_task_id.map(|t| t.0).unwrap_or(0),
                        "agent_id": e.child_agent_id.0,
                        "parent_agent_id": e.parent_agent_id.0,
                        "source_task_id": e.source_task_id.map(|t| t.0),
                        "reason": e.reason,
                        "chat_session_id": e.chat_session_id,
                        "origin_turn_id": e.origin_turn_id,
                    })
                })
                .collect();
            response_result(&req.id, serde_json::json!({ "tree": tree }))
        }
        orch_daemon_method::SUBMIT_TASK => {
            let Some(description) = req.params.get("description").and_then(|x| x.as_str()) else {
                return response_err(&req.id, "params.description (string) required");
            };
            // `.filter(|v| !v.is_null())` treats an explicit JSON `null` the
            // same as an omitted key — callers that always serialize an
            // `Option<T>` field (rather than skip it when `None`) send
            // `"field": null`, which is `Some(&Value::Null)` from `.get()`,
            // not `None`; without this, deserializing null into a non-Option
            // target type fails with a confusing "invalid type: null,
            // expected ..." error instead of using the default.
            let file_manifest = match req.params.get("file_manifest").filter(|v| !v.is_null()) {
                Some(v) => match serde_json::from_value::<Vec<FileAffinity>>(v.clone()) {
                    Ok(m) => m,
                    Err(e) => return response_err(&req.id, format!("invalid file_manifest: {e}")),
                },
                None => Vec::new(),
            };
            let priority = match req.params.get("priority").filter(|v| !v.is_null()) {
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
            let mut enqueue_hints = match req.params.get("enqueue_hints") {
                Some(v) => match serde_json::from_value::<TaskEnqueueHints>(v.clone()) {
                    Ok(h) => Some(h),
                    Err(e) => return response_err(&req.id, format!("invalid enqueue_hints: {e}")),
                },
                None => None,
            };
            // Explicit top-level `task_category` hint from the chat composer (or
            // any other caller). A typo/unknown value silently falls back to
            // `TaskCategory::General` via the generated `FromStr` impl rather
            // than erroring - safe-by-default here since it only affects
            // routing, never crashes or produces a wrong-but-plausible category.
            let task_category = req
                .params
                .get("task_category")
                .filter(|v| !v.is_null())
                .and_then(|x| x.as_str())
                .and_then(|s| s.parse::<crate::TaskCategory>().ok());
            if let Some(category) = task_category {
                enqueue_hints
                    .get_or_insert_with(TaskEnqueueHints::default)
                    .task_category = Some(category);
            }
            // Opt-in, per-session grounding-check toggle from the chat composer
            // (see docs/superpowers/plans/2026-07-20-chat-flow-docking-redesign.md
            // Phase D). Same null-safe idiom as `task_category` above.
            let grounding_check_enabled = req
                .params
                .get("grounding_check_enabled")
                .filter(|v| !v.is_null())
                .and_then(|v| v.as_bool());
            if let Some(enabled) = grounding_check_enabled {
                enqueue_hints
                    .get_or_insert_with(TaskEnqueueHints::default)
                    .grounding_check_enabled = Some(enabled);
            }
            // Phase D Task D1/D3: `vox-gui`'s `submit_task_params`
            // (crates/vox-gui/src/commands/control_plane.rs) has been sending
            // this top-level `chat_session_id` key since it was added, but
            // nothing here ever read it — every background `/spawn`/task-mode
            // dispatch silently lost its originating chat session before this
            // fix. Same null-safe idiom as `task_category`/`grounding_check_enabled`
            // above.
            let chat_session_id = req
                .params
                .get("chat_session_id")
                .filter(|v| !v.is_null())
                .and_then(|x| x.as_str())
                .map(str::to_string)
                .filter(|s| !s.trim().is_empty());
            if let Some(chat_session_id) = chat_session_id {
                enqueue_hints
                    .get_or_insert_with(TaskEnqueueHints::default)
                    .chat_session_id = Some(chat_session_id);
            }
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
            // Near-duplicate scan over live (queued + in-progress) tasks. When a
            // near-duplicate exists and the caller did not opt in, refuse and
            // report which task it matched so the GUI can offer merge/skip.
            let allow_duplicate = req
                .params
                .get("allow_duplicate")
                .and_then(|x| x.as_bool())
                .unwrap_or(true);
            let duplicate_of = orch
                .all_tasks()
                .iter()
                .filter(|t| {
                    crate::services::similarity::jaccard(&t.description, description)
                        >= crate::services::similarity::NEAR_DUPLICATE_THRESHOLD
                })
                .map(|t| t.id.0)
                .next();
            if let Some(dup) = duplicate_of {
                if !allow_duplicate {
                    return response_result(
                        &req.id,
                        serde_json::json!({ "task_id": null, "duplicate_of": dup }),
                    );
                }
            }
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
                Ok(task_id) => response_result(
                    &req.id,
                    serde_json::json!({ "task_id": task_id.0, "duplicate_of": duplicate_of }),
                ),
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
        orch_daemon_method::INTERRUPT_TASK => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            match orch.interrupt_task(TaskId(task_id)) {
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
        orch_daemon_method::LIST_TASKS => {
            let assignments = orch.task_assignments_copy();
            let tasks: Vec<serde_json::Value> = orch
                .all_tasks()
                .into_iter()
                .map(|t| {
                    let agent_id = assignments.get(&t.id).map(|a| a.0);
                    let lifecycle = orch
                        .task_lifecycle_status_label(t.id)
                        .unwrap_or_else(|| "unknown".to_string());
                    let write_files: Vec<String> = t
                        .file_manifest
                        .iter()
                        .filter(|f| matches!(f.access, crate::AccessKind::Write))
                        .map(|f| f.path.to_string_lossy().to_string())
                        .collect();
                    serde_json::json!({
                        "id": t.id.0,
                        "description": t.description,
                        "priority": t.priority,            // raw: "Urgent"|"Normal"|"Background"
                        "status": t.status,
                        "lifecycle": lifecycle,            // raw: "Completed"|"InProgress"|"Blocked"|"Queued"
                        "category": t.task_category,       // raw: "General"|"Chat"|... (TaskCategory)
                        "agent_id": agent_id,
                        "session_id": t.session_id,
                        "estimated_complexity": t.estimated_complexity,
                        "depends_on": t.depends_on.iter().map(|d| d.0).collect::<Vec<u64>>(),
                        "write_files": write_files,
                        // A2A remote delegation: the mesh node that claimed this
                        // task (null when executing locally).
                        "remote_node": t
                            .populi_remote_delegate
                            .as_ref()
                            .and_then(|d| d.claimer_node_id.clone()),
                    })
                })
                .collect();
            response_result(&req.id, serde_json::json!({ "tasks": tasks }))
        }
        orch_daemon_method::EDIT_TASK => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            let Some(description) = req.params.get("description").and_then(|x| x.as_str()) else {
                return response_err(&req.id, "params.description (string) required");
            };
            match orch.edit_task_description(TaskId(task_id), description.to_string()) {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, e),
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
            let chat_session_id = req
                .params
                .get("chat_session_id")
                .and_then(|x| x.as_str())
                .map(str::to_string);
            let origin_turn_id = req
                .params
                .get("origin_turn_id")
                .and_then(|x| x.as_str())
                .map(str::to_string);
            let res = if dynamic {
                orch.spawn_dynamic_agent_with_parent(
                    name,
                    parent_agent_id,
                    delegation_reason,
                    source_task_id,
                    None,
                    chat_session_id,
                    origin_turn_id,
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
            let assigned = orch.agent_assigned_to_task(TaskId(task_id));
            let reason_for_oplog = reason.clone();
            match orch.doubt_task(TaskId(task_id), reason) {
                Ok(outcome) => {
                    // T1.2: durable TaskDoubted BEFORE the bus broadcast, mirroring the MCP
                    // `doubt_task` tool's wiring (task_tools/lifecycle.rs) for this second
                    // (daemon RPC) call path into `Orchestrator::doubt_task`. `doubt_task`
                    // no longer emits on the event bus itself — we record durably first,
                    // then broadcast via `emit_doubt_events`.
                    orch.record_operation(
                        assigned.unwrap_or(crate::AgentId(0)),
                        crate::oplog::OperationKind::TaskDoubted {
                            task_id,
                            reason: reason_for_oplog,
                        },
                        format!("Task {task_id} doubted"),
                        None,
                        None,
                        None,
                        None,
                    )
                    .await;
                    // T1.1 follow-up: durable FeedbackRequested{kind:"doubt"} for the
                    // Doubt-kind feedback item, mirroring the MCP doubt_task tool's
                    // wiring (task_tools/lifecycle.rs) for this call path too.
                    orch.record_operation(
                        assigned.unwrap_or(crate::AgentId(0)),
                        crate::oplog::OperationKind::FeedbackRequested {
                            request_id: outcome.feedback_id.0.clone(),
                            task_id: Some(task_id),
                            kind: "doubt".into(),
                        },
                        format!("Feedback requested (doubt): {}", outcome.feedback_id.0),
                        None,
                        None,
                        None,
                        None,
                    )
                    .await;
                    orch.emit_doubt_events(TaskId(task_id), &outcome);
                    response_result(&req.id, serde_json::json!({ "ok": true }))
                }
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
                Ok(outcome) => {
                    // T1.2 follow-up: durable TaskComplete BEFORE the bus broadcast,
                    // mirroring the MCP resolve-feedback overrule wiring in
                    // feedback_tools.rs for this call path too.
                    orch.record_operation(
                        outcome.agent_id,
                        crate::oplog::OperationKind::TaskComplete { task_id },
                        format!("Task {} overruled", task_id),
                        None,
                        None,
                        None,
                        None,
                    )
                    .await;
                    orch.emit_overrule_events(&outcome);
                    response_result(&req.id, serde_json::json!({ "ok": true }))
                }
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::APPROVE_PLAN => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            match orch.pav_advance_to_acting(TaskId(task_id)) {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::SKIP_VERIFY => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            match orch.pav_skip_verify(TaskId(task_id)) {
                Ok(()) => response_result(&req.id, serde_json::json!({ "ok": true })),
                Err(e) => response_err(&req.id, format!("{e}")),
            }
        }
        orch_daemon_method::FORCE_VERIFY => {
            let Some(task_id) = req.params.get("task_id").and_then(|x| x.as_u64()) else {
                return response_err(&req.id, "params.task_id (u64) required");
            };
            match orch.pav_force_verify(TaskId(task_id)) {
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
        orch_daemon_method::SAFETY_BUDGET_SIGNALS => {
            let budget_manager = orch.budget_manager_handle();
            let bm = crate::sync_lock::rw_read(&*budget_manager);
            let status = orch.status();
            let agents: Vec<serde_json::Value> = status
                .agents
                .iter()
                .map(|a| {
                    let signal = bm.agent_budget_signal(a.id);
                    serde_json::json!({
                        "id": a.id.0,
                        "name": a.name,
                        "signal": signal,
                    })
                })
                .collect();
            response_result(&req.id, serde_json::json!({ "agents": agents }))
        }
        orch_daemon_method::SAFETY_LEDGER => {
            let filter_agent = req.params.get("agent_id").and_then(|x| x.as_u64());
            let ledger_handle = orch.tool_ledger_handle();
            let ledger = ledger_handle.read().unwrap();
            let snapshot = ledger.snapshot();
            let receipts: Vec<serde_json::Value> = snapshot
                .iter()
                .filter(|(_, (aid, _))| filter_agent.is_none_or(|target| aid.0 == target))
                .map(|(id, (aid, tool))| {
                    serde_json::json!({
                        "receipt_id": id,
                        "agent_id": aid.0,
                        "tool_name": tool,
                    })
                })
                .collect();
            response_result(&req.id, serde_json::json!({ "receipts": receipts }))
        }
        orch_daemon_method::SAFETY_LOCKS => {
            let snapshot = orch.resource_locks().snapshot();
            let locks: Vec<serde_json::Value> = snapshot
                .iter()
                .map(|lock| {
                    serde_json::json!({
                        "resource_id": lock.resource_id,
                        "kind": lock.kind,
                        "holder": lock.holder.0,
                        "expires_ms": lock.expires_ms,
                    })
                })
                .collect();
            response_result(&req.id, serde_json::json!({ "locks": locks }))
        }
        orch_daemon_method::ATTENTION_SNAPSHOT => {
            let budget_manager = orch.budget_manager_handle();
            let bm = crate::sync_lock::rw_read(&*budget_manager);
            let snap = bm.attention_snapshot();

            let config_handle = orch.config_handle();
            let config = crate::sync_lock::rw_read(&*config_handle);

            response_result(
                &req.id,
                serde_json::json!({
                    "snapshot": snap,
                    "config": {
                        "attention_enabled": config.attention_enabled,
                        "attention_budget_ms": config.attention_budget_ms,
                        "attention_alert_threshold": config.attention_alert_threshold,
                    },
                }),
            )
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

#[cfg(test)]
mod isolation_dispatch_tests {
    use super::*;
    use crate::config::OrchestratorConfig;

    fn req(method: &str, params: serde_json::Value) -> DispatchRequest {
        DispatchRequest {
            id: "1".to_string(),
            method: method.to_string(),
            params,
            auth_token: None,
            permission_mode: None,
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

    /// Phase D Task D2: `orch.subagent_tree` had no handler at all — every real
    /// call fell through to the catch-all "Method not found" arm, so
    /// `vox-gui`'s `list_subagent_tree` Tauri command always resolved an empty
    /// tree. This exercises the real `dispatch_request` RPC path (not the
    /// underlying topology-building logic in isolation): spawn a parent agent,
    /// spawn a dynamic child delegated from it (carrying the Phase D Task D1
    /// `chat_session_id`/`origin_turn_id` fields), then call
    /// `orch.subagent_tree` and confirm the edge comes back non-empty with
    /// those fields intact.
    #[tokio::test]
    async fn subagent_tree_returns_edge_after_spawn() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::default()));

        let empty = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(orch_daemon_method::SUBAGENT_TREE, serde_json::json!({})),
        )
        .await;
        assert_eq!(
            result_value(&empty)["tree"].as_array().map(|a| a.len()),
            Some(0),
            "no spawns yet: tree must be empty, not an error or missing field"
        );

        let spawn_parent = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SPAWN_AGENT_EXT,
                serde_json::json!({ "name": "parent-agent", "dynamic": false }),
            ),
        )
        .await;
        let parent_id = result_value(&spawn_parent)["agent_id"]
            .as_u64()
            .expect("parent agent_id");

        let spawn_child = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SPAWN_AGENT_EXT,
                serde_json::json!({
                    "name": "delegated-child",
                    "dynamic": true,
                    "parent_agent_id": parent_id,
                    "delegation_reason": "test delegation",
                    "chat_session_id": "chat-session-d2",
                    "origin_turn_id": "call_d2_test",
                }),
            ),
        )
        .await;
        let child_id = result_value(&spawn_child)["agent_id"]
            .as_u64()
            .expect("child agent_id");

        let tree_resp = dispatch_request(
            "rid",
            orch,
            &req(orch_daemon_method::SUBAGENT_TREE, serde_json::json!({})),
        )
        .await;
        let tree = result_value(&tree_resp)["tree"]
            .as_array()
            .expect("tree is an array");
        assert_eq!(tree.len(), 1, "exactly one delegation edge after one spawn");
        let edge = &tree[0];
        assert_eq!(edge["agent_id"], child_id);
        assert_eq!(edge["parent_agent_id"], parent_id);
        assert_eq!(edge["reason"], "test delegation");
        assert_eq!(edge["chat_session_id"], "chat-session-d2");
        assert_eq!(edge["origin_turn_id"], "call_d2_test");
    }

    /// Phase D Task D1/D3: `vox-gui`'s `submit_task_params`
    /// (crates/vox-gui/src/commands/control_plane.rs) sends a top-level
    /// `chat_session_id` on every `orch.submit_task` call, but this RPC arm
    /// never read it before this fix — the value was silently discarded, so
    /// no submitted task (e.g. a `/spawn` background dispatch) ever carried
    /// its originating chat session for correlation. Confirms it now lands on
    /// `AgentTask::chat_session_id` via `TaskEnqueueHints`.
    #[tokio::test]
    async fn submit_task_chat_session_id_reaches_the_task() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::default()));
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({
                    "description": "fix the login bug",
                    "chat_session_id": "chat-session-d1d3",
                }),
            ),
        )
        .await;
        let task_id = result_value(&resp)["task_id"].as_u64().expect("task_id");
        let task = orch
            .all_tasks()
            .into_iter()
            .find(|t| t.id.0 == task_id)
            .expect("submitted task is findable");
        assert_eq!(task.chat_session_id.as_deref(), Some("chat-session-d1d3"));
    }

    #[tokio::test]
    async fn ping_response_includes_the_running_binary_version() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::default()));
        let resp = dispatch_request(
            "rid",
            orch,
            &req(orch_daemon_method::PING, serde_json::json!({})),
        )
        .await;
        let value = result_value(&resp);
        assert_eq!(
            value.get("version").and_then(|v| v.as_str()),
            Some(env!("CARGO_PKG_VERSION")),
            "ping response must report the running daemon's own workspace version"
        );
    }
}

#[cfg(test)]
mod task_dispatch_tests {
    use super::*;
    use crate::config::OrchestratorConfig;

    fn req(method: &str, params: serde_json::Value) -> DispatchRequest {
        DispatchRequest {
            id: "1".to_string(),
            method: method.to_string(),
            params,
            auth_token: None,
            permission_mode: None,
        }
    }

    fn result_value(resp: &DispatchResponse) -> &serde_json::Value {
        match &resp.payload {
            DispatchPayload::Result { value } => value,
            other => panic!("expected Result payload, got {other:?}"),
        }
    }

    async fn orch_with_one_task() -> (Arc<Orchestrator>, u64) {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
        orch.spawn_agent("a1").unwrap();
        // NOTE: SUBMIT_TASK parses `priority` as the TaskPriority serde enum
        // (Capitalized "Normal"); omit it to default to Normal. Lowercase
        // priority strings are only accepted by REORDER_TASK.
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({ "description": "first task" }),
            ),
        )
        .await;
        let task_id = result_value(&resp)["task_id"].as_u64().unwrap();
        (orch, task_id)
    }

    #[tokio::test]
    async fn submit_task_treats_explicit_null_priority_and_file_manifest_as_omitted() {
        // Reproduces a live bug: a caller that always serializes optional
        // fields sends `"priority": null` / `"file_manifest": null` rather
        // than omitting the keys. Before the fix this returned "invalid
        // priority: invalid type: null, expected string or map" instead of
        // defaulting to Normal priority / an empty manifest.
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
        orch.spawn_agent("a1").unwrap();
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({
                    "description": "explicit null optional fields",
                    "priority": null,
                    "file_manifest": null,
                }),
            ),
        )
        .await;
        let task_id = result_value(&resp)["task_id"]
            .as_u64()
            .expect("submit succeeds instead of erroring on null priority/file_manifest");

        let list_resp = dispatch_request(
            "rid",
            orch,
            &req(orch_daemon_method::LIST_TASKS, serde_json::json!({})),
        )
        .await;
        let tasks = result_value(&list_resp)["tasks"].as_array().unwrap();
        let t = tasks
            .iter()
            .find(|t| t["id"].as_u64() == Some(task_id))
            .unwrap();
        assert_eq!(t["priority"].as_str(), Some("Normal"));
    }

    #[tokio::test]
    async fn submit_task_with_explicit_chat_category_routes_to_chat_processor_category() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
        orch.spawn_agent("a1").unwrap();
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({
                    "description": "hi there",
                    "task_category": "chat",
                }),
            ),
        )
        .await;
        let task_id = result_value(&resp)["task_id"].as_u64().unwrap();
        let list_resp = dispatch_request(
            "rid",
            orch,
            &req(orch_daemon_method::LIST_TASKS, serde_json::json!({})),
        )
        .await;
        let tasks = result_value(&list_resp)["tasks"].as_array().unwrap();
        let t = tasks
            .iter()
            .find(|t| t["id"].as_u64() == Some(task_id))
            .unwrap();
        assert_eq!(t["category"].as_str(), Some("Chat"));
    }

    #[tokio::test]
    async fn list_tasks_returns_submitted_task_with_fields() {
        let (orch, task_id) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            orch,
            &req(orch_daemon_method::LIST_TASKS, serde_json::json!({})),
        )
        .await;
        let v = result_value(&resp);
        let tasks = v["tasks"].as_array().expect("tasks array");
        assert_eq!(tasks.len(), 1);
        let t = &tasks[0];
        assert_eq!(t["id"].as_u64(), Some(task_id));
        assert_eq!(t["description"].as_str(), Some("first task"));
        assert!(t["priority"].is_string());
        assert!(t["lifecycle"].is_string());
        assert!(t.get("agent_id").is_some());
        assert!(t["write_files"].is_array());
        // remote_node key is always present (null for a locally-executing task).
        assert!(t.get("remote_node").is_some());
        assert!(t["remote_node"].is_null());
    }

    #[tokio::test]
    async fn edit_task_rewrites_description_of_queued_task() {
        let (orch, task_id) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::EDIT_TASK,
                serde_json::json!({ "task_id": task_id, "description": "rewritten" }),
            ),
        )
        .await;
        assert_eq!(result_value(&resp)["ok"], true);

        let list = dispatch_request(
            "rid",
            orch,
            &req(orch_daemon_method::LIST_TASKS, serde_json::json!({})),
        )
        .await;
        let tasks = result_value(&list)["tasks"].as_array().unwrap();
        assert_eq!(tasks[0]["description"].as_str(), Some("rewritten"));
    }

    #[tokio::test]
    async fn edit_task_unknown_id_is_error() {
        let (orch, _) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::EDIT_TASK,
                serde_json::json!({ "task_id": 999_999, "description": "x" }),
            ),
        )
        .await;
        assert!(matches!(resp.payload, DispatchPayload::Error { .. }));
    }

    #[tokio::test]
    async fn edit_task_empty_description_is_error() {
        let (orch, task_id) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::EDIT_TASK,
                serde_json::json!({ "task_id": task_id, "description": "  " }),
            ),
        )
        .await;
        assert!(matches!(resp.payload, DispatchPayload::Error { .. }));
    }

    #[tokio::test]
    async fn near_duplicate_blocked_when_not_allowed() {
        let (orch, first_id) = orch_with_one_task().await; // "first task"
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({
                    "description": "first task",
                    "allow_duplicate": false,
                }),
            ),
        )
        .await;
        let v = result_value(&resp);
        assert_eq!(v["duplicate_of"].as_u64(), Some(first_id));
        assert!(v["task_id"].is_null());
        assert_eq!(orch.all_tasks().len(), 1);
    }

    #[tokio::test]
    async fn near_duplicate_enqueued_but_flagged_when_allowed() {
        let (orch, first_id) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({ "description": "first task" }), // allow_duplicate defaults true
            ),
        )
        .await;
        let v = result_value(&resp);
        assert!(v["task_id"].as_u64().is_some());
        assert_eq!(v["duplicate_of"].as_u64(), Some(first_id));
        assert_eq!(orch.all_tasks().len(), 2);
    }

    #[tokio::test]
    async fn distinct_task_has_no_duplicate_flag() {
        let (orch, _) = orch_with_one_task().await;
        let resp = dispatch_request(
            "rid",
            orch,
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({ "description": "completely unrelated migration work" }),
            ),
        )
        .await;
        assert!(result_value(&resp)["duplicate_of"].is_null());
    }

    #[tokio::test]
    async fn submit_with_enqueue_hints_carries_tier_and_mode() {
        let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
        orch.spawn_agent("a1").unwrap();
        let resp = dispatch_request(
            "rid",
            Arc::clone(&orch),
            &req(
                orch_daemon_method::SUBMIT_TASK,
                serde_json::json!({
                    "description": "tiered task",
                    "enqueue_hints": { "model_preference": "mesh", "mode": "plan" }
                }),
            ),
        )
        .await;
        let task_id = result_value(&resp)["task_id"].as_u64().unwrap();
        let task = orch
            .all_tasks()
            .into_iter()
            .find(|t| t.id.0 == task_id)
            .unwrap();
        assert_eq!(task.model_preference.as_deref(), Some("mesh"));
        assert_eq!(task.mode.as_deref(), Some("plan"));
    }
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
    token: Option<Arc<str>>,
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
        // Auth gate (T0.2): every method — including SUBSCRIBE/SUBSCRIBE_EVENTS
        // and the `extra` dispatch — must pass this check before being handled.
        // Only enforced when the daemon has a configured token; a wrong or
        // missing token gets a clear error response and the connection is left
        // open (matching the invalid-JSON-parse path) so a client can retry
        // with a corrected token on the same connection. The byte comparison
        // itself is constant-time (mirroring the HTTP gateway's bearer-token
        // check) to avoid a timing side channel on the secret; the `is_some()`
        // branch above is not secret-dependent, so it does not need to be.
        if let Some(expected) = token.as_deref() {
            let provided = req.auth_token.as_deref().unwrap_or("");
            if !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
                let resp = response_err(
                    &req.id,
                    "unauthorized: missing or invalid daemon auth token",
                );
                write_frame(&mut write_half, &resp).await?;
                continue;
            }
        }
        if req.method == orch_daemon_method::SUBSCRIBE {
            // Long-lived push stream; returns when the peer disconnects.
            stream_status_events(&req.id, &orch, &mut write_half).await?;
            break;
        }
        if req.method == orch_daemon_method::SUBSCRIBE_EVENTS {
            let from_offset = parse_from_offset(&req.params);
            stream_agent_events_from(&req.id, &orch, &mut write_half, from_offset).await?;
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

/// Parse the optional `params.from_offset` (T1.3 replay cursor) from a
/// `SUBSCRIBE_EVENTS` request. Absent/non-numeric yields `None`, reproducing
/// today's live-tail-only behavior — this is intentionally lenient (no error
/// response for a malformed value) since a subscribe request has no normal
/// error-response path once streaming begins.
fn parse_from_offset(params: &serde_json::Value) -> Option<u64> {
    params.get("from_offset").and_then(|v| v.as_u64())
}

/// Accept connections until `listener` is dropped (runs forever on success).
pub async fn serve_listener(
    listener: TcpListener,
    bind_display: String,
    repository_id: String,
    orch: Arc<Orchestrator>,
) -> anyhow::Result<()> {
    serve_listener_with_extra(listener, bind_display, repository_id, orch, None, None).await
}

/// [`serve_listener`] with an optional [`ExtraDispatch`] hook (the daemon binary
/// wires one carrying its MCP `ServerState`) and an optional daemon auth
/// `token` (T0.2) — when `Some`, every connection must present a matching
/// `DispatchRequest.auth_token` or is rejected before any dispatch.
pub async fn serve_listener_with_extra(
    listener: TcpListener,
    bind_display: String,
    repository_id: String,
    orch: Arc<Orchestrator>,
    extra: Option<Arc<dyn ExtraDispatch>>,
    token: Option<Arc<str>>,
) -> anyhow::Result<()> {
    tracing::info!(bind = %bind_display, "vox-orchestrator-d listening");
    loop {
        let (socket, peer) = listener.accept().await?;
        tracing::debug!(%peer, "orch daemon accepted");
        let repo = repository_id.clone();
        let o = orch.clone();
        let ex = extra.clone();
        let tok = token.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, repo, o, ex, tok).await {
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
    run_tcp_server_with_extra(bind, repository_id, orch, None, None).await
}

/// [`run_tcp_server`] with an optional [`ExtraDispatch`] hook and an optional
/// daemon auth `token` (T0.2).
pub async fn run_tcp_server_with_extra(
    bind: &str,
    repository_id: String,
    orch: Arc<Orchestrator>,
    extra: Option<Arc<dyn ExtraDispatch>>,
    token: Option<Arc<str>>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(bind).await?;
    serve_listener_with_extra(
        listener,
        bind.to_string(),
        repository_id,
        orch,
        extra,
        token,
    )
    .await
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
            let from_offset = parse_from_offset(&req.params);
            stream_agent_events_from(&req.id, &orch, &mut stdout, from_offset).await?;
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
