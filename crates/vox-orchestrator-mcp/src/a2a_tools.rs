//! A2A (Agent-to-Agent) MCP tools — send, inbox, ack, broadcast, history.
//!
//! When attention gating is enabled and a message may surface to the pilot, `a2a_send` evaluates interruption policy and may
//! call [`ServerState::record_attention_event`](crate::server_state::ServerState::record_attention_event) for defer / proceed decisions
//! — **attention debit telemetry** on the orchestrator + optional Codex mirror, not usage analytics.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use vox_actor_runtime::supervisor::spawn_supervised_infallible;

use crate::attention_policy::{
    a2a_escalation_signals, channel_label, decision_label, evaluate_with_state,
    pending_backlog_for_session, trust_for_session,
};
use crate::params::ToolResult;
use crate::server_state::ServerState;
use vox_orchestrator::a2a::{A2ADeliveryPlane, A2AInboxPlane};
use vox_orchestrator::types::{A2AMessageType, AgentId};

#[inline]
fn a2a_message_may_surface_to_pilot(msg_type: &A2AMessageType) -> bool {
    matches!(
        msg_type,
        A2AMessageType::HelpRequest
            | A2AMessageType::ErrorReport
            | A2AMessageType::ConflictDetected
    )
}

#[inline]
fn is_high_priority_a2a_type(msg_type: &A2AMessageType) -> bool {
    matches!(
        msg_type,
        A2AMessageType::ErrorReport | A2AMessageType::ConflictDetected
    )
}

const REM_A2A_ACK: &str = "List inbox with `a2a_inbox` and use a pending `message_id` for this agent; ids are consumed after ack.";

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

/// MCP tool arguments: point-to-point message on the orchestrator A2A bus.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct A2ASendParams {
    /// Numeric id of the sending agent.
    pub sender_id: u64,
    /// Numeric id of the receiving agent.
    pub receiver_id: u64,
    /// Logical type string (e.g. `plan_handoff`, `scope_request`); unknown values become `FreeForm`.
    pub msg_type: String,
    /// Opaque UTF-8 payload stored in the message record.
    pub payload: String,
    /// Optional end-to-end correlation id for tracing.
    pub correlation_id: Option<String>,
    /// Optional tenant/session id for tracing and dashboards.
    pub tenant_id: Option<String>,
    /// When the sender agent has a mapped orchestrator session, this must match it; when unmapped, optional (see response `binding_advisory`).
    pub sender_session_id: Option<String>,
    /// Delivery route semantics: `local` (in-process), `db` (durable inbox), `mesh` (Populi relay).
    pub route: Option<String>,
}

/// MCP tool arguments: fetch unacknowledged inbox entries for one agent.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct A2AInboxParams {
    /// Agent whose inbox is read.
    pub agent_id: u64,
    /// Inbox source semantics: `merged` (default), `local`, or `mesh`.
    pub source: Option<String>,
    /// Optional cap for remote mesh inbox rows per call (non-claimer path).
    pub max_messages: Option<usize>,
    /// Optional remote mesh cursor (`id < before_message_id`) for non-claimer paging.
    pub before_message_id: Option<u64>,
    /// When this agent has a mapped orchestrator session, this must match it.
    pub agent_session_id: Option<String>,
}

/// MCP tool arguments: mark one inbox message as read/processed.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct A2AAckParams {
    /// Agent holding the inbox entry.
    pub agent_id: u64,
    /// Message id returned by send/broadcast tools.
    pub message_id: u64,
    /// When this agent has a mapped orchestrator session, this must match it.
    pub agent_session_id: Option<String>,
}

/// MCP tool arguments: fan-out to every agent except the sender.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct A2ABroadcastParams {
    /// Originating agent (excluded from delivery list).
    pub sender_id: u64,
    /// Same semantics as [`A2ASendParams::msg_type`].
    pub msg_type: String,
    /// Broadcast payload bytes as UTF-8 text.
    pub payload: String,
    /// When the sender agent has a mapped orchestrator session, this must match it; when unmapped, optional (see response `binding_advisory`).
    pub sender_session_id: Option<String>,
}

/// MCP tool arguments: audit trail query (`since_ms` filters by wall-clock ms; `limit` caps rows).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct A2AHistoryParams {
    /// Only messages at or after this epoch milliseconds (inclusive).
    pub since_ms: Option<u64>,
    /// Maximum rows to return (default applied in handler when `None`).
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// JSON row returned in inbox/history tool responses (mirrors orchestrator message bus records).
#[derive(Debug, Serialize)]
pub struct A2AMessageInfo {
    /// Stable message id in the bus.
    pub id: u64,
    /// Sender agent id.
    pub sender: u64,
    /// `None` for broadcast-style entries without a single receiver.
    pub receiver: Option<u64>,
    /// Resolved type name for the payload.
    pub msg_type: String,
    /// Raw message body.
    pub payload: String,
    /// Creation time in epoch milliseconds.
    pub timestamp_ms: u64,
    /// Whether the receiver has acked this message.
    pub acknowledged: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_msg_type(s: &str) -> A2AMessageType {
    match s.to_lowercase().as_str() {
        "plan_handoff" | "planhandoff" => A2AMessageType::PlanHandoff,
        "scope_request" | "scoperequest" => A2AMessageType::ScopeRequest,
        "scope_grant" | "scopegrant" => A2AMessageType::ScopeGrant,
        "progress_update" | "progressupdate" => A2AMessageType::ProgressUpdate,
        "help_request" | "helprequest" => A2AMessageType::HelpRequest,
        "completion_notice" | "completionnotice" => A2AMessageType::CompletionNotice,
        "error_report" | "errorreport" => A2AMessageType::ErrorReport,
        "conflict_detected" | "conflictdetected" => A2AMessageType::ConflictDetected,
        "conflict_resolved" | "conflictresolved" => A2AMessageType::ConflictResolved,
        "vcs_event" | "vcsevent" => A2AMessageType::VcsEvent,
        "cancel_request" | "cancelrequest" => A2AMessageType::CancelRequest,
        "snapshot_share" | "snapshotshare" => A2AMessageType::SnapshotShare,
        "broadcast_news" | "broadcastnews" => A2AMessageType::BroadcastNews,
        "remote_task_envelope" | "remotetaskenvelope" => A2AMessageType::PlanHandoff,
        "remote_task_ack" | "remotetaskack" => A2AMessageType::ProgressUpdate,
        "remote_task_result" | "remotetaskresult" => A2AMessageType::CompletionNotice,
        _ => A2AMessageType::FreeForm,
    }
}

// Wire format matches [`A2AMessageType::into_str`] (snake_case) — same as DB `a2a_messages.msg_type`.
fn msg_type_wire(mt: &A2AMessageType) -> String {
    mt.into_str().to_string()
}

fn fnv1a64(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for p in parts {
        for b in p.as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn default_idempotency_key(sender: u64, receiver: u64, msg_type: &str, payload: &str) -> String {
    format!(
        "mcp-a2a-{sender}-{receiver}-{:016x}",
        fnv1a64(&[msg_type, payload])
    )
}

fn normalize_route(route: Option<&str>) -> Result<A2ADeliveryPlane, String> {
    match route
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        None | Some("local") => Ok(A2ADeliveryPlane::LocalEphemeral),
        Some("db") => Ok(A2ADeliveryPlane::LocalDurable),
        Some("mesh") => Ok(A2ADeliveryPlane::RemoteMesh),
        Some(other) => Err(format!(
            "Unsupported route '{other}', expected local|db|mesh"
        )),
    }
}

fn normalize_inbox_source(source: Option<&str>) -> Result<A2AInboxPlane, String> {
    match source
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        None | Some("merged") => Ok(A2AInboxPlane::Merged),
        Some("local") => Ok(A2AInboxPlane::Local),
        Some("mesh") => Ok(A2AInboxPlane::Mesh),
        Some(other) => Err(format!(
            "Unsupported inbox source '{other}', expected merged|local|mesh"
        )),
    }
}

/// Result of sender/session checks for `a2a_send` / `a2a_broadcast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SenderBindingOutcome {
    /// Orchestrator has a non-empty mapped session and the caller supplied a matching id.
    Verified,
    /// No mapped session for this agent; send proceeds; response must surface [`A2A_SENDER_UNBOUND_ADVISORY`].
    Unbound,
}

/// Shown in tool JSON when the sender has no mapped orchestrator session (spoofing window).
const A2A_SENDER_UNBOUND_ADVISORY: &str = "Sender agent has no mapped orchestrator session (agent_session_id unset). Any caller may spoof sender_id until the client maps this agent via vox_map_agent_session (or wire alias) and passes the same value as sender_session_id on A2A tools.";

fn mapped_session_id(summary: &vox_orchestrator::orchestrator::AgentSummary) -> Option<&str> {
    summary
        .agent_session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn normalize_sender_session_id(sender_session_id: Option<&str>) -> Option<&str> {
    sender_session_id.map(str::trim).filter(|s| !s.is_empty())
}

/// When a session is mapped, `sender_session_id` is required and must match. When unmapped, returns
/// [`SenderBindingOutcome::Unbound`] so the handler can attach payload advisories (not log-only).
fn resolve_sender_binding(
    orch: &vox_orchestrator::Orchestrator,
    sender: AgentId,
    sender_session_id: Option<&str>,
) -> Result<SenderBindingOutcome, String> {
    let status = orch.status();
    let Some(summary) = status.agents.iter().find(|a| a.id == sender) else {
        return Err(format!("Sender agent {} is not registered", sender.0));
    };

    let mapped = mapped_session_id(summary);
    let provided = normalize_sender_session_id(sender_session_id);

    match (mapped, provided) {
        (Some(expected), Some(got)) if got == expected => Ok(SenderBindingOutcome::Verified),
        (Some(_), Some(_)) => Err(format!(
            "sender_session_id does not match mapped session for agent {}",
            sender.0
        )),
        (Some(_), None) => Err(format!(
            "sender_session_id is required for agent {} because an orchestrator session is mapped (vox_map_agent_session); pass the same session id",
            sender.0
        )),
        (None, _) => Ok(SenderBindingOutcome::Unbound),
    }
}

fn extend_binding_fields(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    outcome: SenderBindingOutcome,
) {
    match outcome {
        SenderBindingOutcome::Verified => {
            obj.insert(
                "sender_identity_binding".into(),
                serde_json::Value::String("verified".into()),
            );
            obj.insert("binding_advisory".into(), serde_json::Value::Null);
        }
        SenderBindingOutcome::Unbound => {
            obj.insert(
                "sender_identity_binding".into(),
                serde_json::Value::String("unbound".into()),
            );
            obj.insert(
                "binding_advisory".into(),
                serde_json::Value::String(A2A_SENDER_UNBOUND_ADVISORY.into()),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Send a targeted A2A message from one agent to another (async).
pub async fn a2a_send(state: &ServerState, params: A2ASendParams) -> String {
    let orch = &state.orchestrator;

    let sender = AgentId(params.sender_id);
    let receiver = AgentId(params.receiver_id);
    let msg_type = parse_msg_type(&params.msg_type);
    let route = match normalize_route(params.route.as_deref()) {
        Ok(route) => route,
        Err(msg) => return ToolResult::<String>::err(msg).to_json(),
    };
    let binding_outcome =
        match resolve_sender_binding(orch, sender, params.sender_session_id.as_deref()) {
            Ok(o) => o,
            Err(msg) => return ToolResult::<String>::err(msg).to_json(),
        };
    if !orch.status().agents.iter().any(|a| a.id == receiver) {
        return ToolResult::<String>::err(format!(
            "Receiver agent {} is not registered",
            params.receiver_id
        ))
        .to_json();
    }
    if state.orchestrator_config.attention_enabled && a2a_message_may_surface_to_pilot(&msg_type) {
        let bm = state.orchestrator.budget_manager_handle();
        let snap = vox_orchestrator::sync_lock::rw_read(&*bm).attention_snapshot();
        if let vox_orchestrator::GateResult::AttentionExhausted { message, .. } =
            vox_orchestrator::BudgetGate::check_attention_snapshot(
                &snap,
                &state.orchestrator_config,
            )
        {
            return ToolResult::<String>::err_with_remediation(
                message,
                "Pilot attention budget is exhausted; resolve MCP clarifications or raise VOX_ORCHESTRATOR_ATTENTION_BUDGET_MS / disable VOX_ORCHESTRATOR_ATTENTION_ENABLED.",
            )
            .to_json();
        }
        let backlog = pending_backlog_for_session(state, params.sender_session_id.as_deref());
        let trust = trust_for_session(state, params.sender_session_id.as_deref());
        let signals = a2a_escalation_signals(
            &msg_type,
            &params.payload,
            backlog,
            trust,
            state.orchestrator_config.attention_interrupt_cost_ms,
        );
        let decision = evaluate_with_state(state, &signals, &snap);
        match decision {
            vox_orchestrator::InterruptionDecision::RequireHumanBeforeContinue {
                reason, ..
            } => {
                return ToolResult::<String>::err_with_remediation(
                    format!("A2A escalation blocked pending human review: {reason}"),
                    "Reduce blast radius or include explicit human-approved context before resubmitting this escalation.",
                )
                .to_json();
            }
            vox_orchestrator::InterruptionDecision::DeferUntilCheckpoint { ref reason }
            | vox_orchestrator::InterruptionDecision::BatchWithExistingPrompt { ref reason } => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                state.record_attention_event(vox_orchestrator::AttentionEvent {
                    agent_id: sender,
                    task_id: None,
                    event_type: vox_orchestrator::AttentionEventType::PolicyDeferred,
                    tier: vox_orchestrator::ApprovalTier::Confirm,
                    cost_ms: 0,
                    outcome: vox_orchestrator::ApprovalOutcome::AutoApproved,
                    trust_score_at_time: trust,
                    effective_complexity: (params.payload.len() as f64 / 120.0).clamp(0.0, 10.0),
                    decision_entropy_bits: signals.expected_information_gain_bits,
                    timestamp_ms: ts,
                    channel: Some("a2a_send".to_string()),
                    policy_reason: Some(reason.clone()),
                });
                let mut data = serde_json::json!({
                    "deferred": true,
                    "decision": decision_label(&decision),
                    "decision_legacy_debug": format!("{decision:?}"),
                    "surface": "a2a_send",
                    "channel": channel_label(vox_orchestrator::InterruptionChannel::A2AEscalation),
                    "reason": reason,
                    "timestamp_ms": ts,
                    "sender": params.sender_id,
                    "receiver": params.receiver_id,
                    "delivery_plane": route.as_str(),
                });
                if let Some(obj) = data.as_object_mut() {
                    extend_binding_fields(obj, binding_outcome);
                }
                return ToolResult::ok(data).to_json();
            }
            vox_orchestrator::InterruptionDecision::ProceedAutonomously { ref reason } => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                state.record_attention_event(vox_orchestrator::AttentionEvent {
                    agent_id: sender,
                    task_id: None,
                    event_type: vox_orchestrator::AttentionEventType::PolicyProceedAuto,
                    tier: vox_orchestrator::ApprovalTier::AutoApprove,
                    cost_ms: 0,
                    outcome: vox_orchestrator::ApprovalOutcome::AutoApproved,
                    trust_score_at_time: trust,
                    effective_complexity: (params.payload.len() as f64 / 120.0).clamp(0.0, 10.0),
                    decision_entropy_bits: signals.expected_information_gain_bits,
                    timestamp_ms: ts,
                    channel: Some("a2a_send".to_string()),
                    policy_reason: Some(reason.clone()),
                });
                let mut data = serde_json::json!({
                    "deferred": false,
                    "decision": "ProceedAutonomously",
                    "surface": "a2a_send",
                    "channel": channel_label(vox_orchestrator::InterruptionChannel::A2AEscalation),
                    "reason": reason,
                    "timestamp_ms": ts,
                    "sender": params.sender_id,
                    "receiver": params.receiver_id,
                    "delivery_plane": route.as_str(),
                });
                if let Some(obj) = data.as_object_mut() {
                    extend_binding_fields(obj, binding_outcome);
                }
                return ToolResult::ok(data).to_json();
            }
            vox_orchestrator::InterruptionDecision::InterruptNow { .. } => {}
        }
    }
    let idem = params
        .correlation_id
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            default_idempotency_key(
                params.sender_id,
                params.receiver_id,
                &msg_type_wire(&msg_type),
                &params.payload,
            )
        });
    let idem = {
        let t = idem.trim();
        if t.is_empty() {
            default_idempotency_key(
                params.sender_id,
                params.receiver_id,
                &msg_type_wire(&msg_type),
                &params.payload,
            )
        } else {
            t.to_string()
        }
    };

    let mut local_message_id: Option<u64> = None;
    let mut db_message_uuid: Option<String> = None;
    let mut relay_attempted = false;
    let mut relay_ok = false;
    let route_ok;
    match route {
        A2ADeliveryPlane::LocalEphemeral => {
            local_message_id = Some(
                orch.send_a2a(sender, receiver, msg_type.clone(), params.payload.clone())
                    .0,
            );
            route_ok = true;
        }
        A2ADeliveryPlane::LocalDurable => {
            let Some(db) = orch.db() else {
                return ToolResult::<String>::err_with_remediation(
                    "No orchestrator DB configured for route=db",
                    "Configure VoxDb for this orchestrator session or use route=local/mesh.",
                )
                .to_json();
            };
            let repository_id =
                vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRepositoryRoot)
                    .expose()
                    .unwrap_or("default")
                    .to_string();
            match vox_orchestrator::a2a::send_to_db_with_breaker(
                &db,
                sender,
                receiver,
                msg_type.clone(),
                params.payload.clone(),
                vox_orchestrator::types::MessagePriority::Normal,
                None,
                repository_id.as_str(),
            )
            .await
            {
                Ok(uuid) => {
                    db_message_uuid = Some(uuid);
                    route_ok = true;
                }
                Err(e) => return ToolResult::<String>::err(e).to_json(),
            }
        }
        A2ADeliveryPlane::RemoteMesh => {
            relay_ok = false;
            relay_attempted = false;
            #[cfg(feature = "populi-transport")]
            {
                if let Some(base) = vox_populi::http_lifecycle::populi_http_control_base_from_env()
                    && !base.trim().is_empty()
                {
                    relay_attempted = true;
                    let client =
                        vox_populi::http_client::PopuliHttpClient::new(base).with_env_token();
                    relay_ok = client
                        .relay_a2a(&vox_populi::transport::A2ADeliverRequest {
                            sender_agent_id: params.sender_id.to_string(),
                            receiver_agent_id: params.receiver_id.to_string(),
                            message_type: msg_type_wire(&msg_type),
                            payload: params.payload.clone(),
                            idempotency_key: Some(idem.clone()),
                            privacy_class: None,
                            payload_blake3_hex: None,
                            worker_ed25519_sig_b64: None,
                            jwe_payload: None,
                            task_kind: None,
                            model_id: None,
                            traceparent: None,
                            priority: 128,
                        })
                        .await
                        .is_ok();
                }
            }
            route_ok = relay_ok;
        }
    }

    let mut data = serde_json::json!({
        "message_id": local_message_id,
        "db_message_uuid": db_message_uuid,
        "sender": params.sender_id,
        "receiver": params.receiver_id,
        "route": route.legacy_route_token(),
        "delivery_plane": route.as_str(),
        "route_ok": route_ok,
        "idempotency_key": idem,
        "correlation_id": params.correlation_id,
        "tenant_id": params.tenant_id,
        "relay_attempted": relay_attempted,
        "relay_ok": relay_ok,
        "dropped_messages": orch.message_bus().dropped_messages(),
    });

    if route_ok {
        if let Some(db) = &state.db {
            let trace_data = serde_json::json!({
                "prompt": format!("Sender: {}\nReceiver: {}\nMsgType: {}", params.sender_id, params.receiver_id, msg_type_wire(&msg_type)),
                "response": params.payload,
                "category": "a2a_trace",
                "schema_version": "vox_dogfood_v1",
            });
            let agent_str = params.sender_id.to_string();
            let _ = vox_gamify::db::insert_event(
                db,
                &agent_str,
                "a2a_trace",
                Some(&trace_data.to_string()),
            )
            .await;
        }
    }

    if let Some(obj) = data.as_object_mut() {
        extend_binding_fields(obj, binding_outcome);
    }
    ToolResult::ok(data).to_json()
}

/// Read unacknowledged messages in an agent's inbox (async).
pub async fn a2a_inbox(state: &ServerState, params: A2AInboxParams) -> String {
    let orch = &state.orchestrator;
    let source = match normalize_inbox_source(params.source.as_deref()) {
        Ok(s) => s,
        Err(msg) => return ToolResult::<String>::err(msg).to_json(),
    };
    let binding_outcome = match resolve_sender_binding(
        orch,
        AgentId(params.agent_id),
        params.agent_session_id.as_deref(),
    ) {
        Ok(o) => o,
        Err(msg) => return ToolResult::<String>::err(msg).to_json(),
    };

    let agent_id = AgentId(params.agent_id);
    let mut messages: Vec<A2AMessageInfo> = if source == A2AInboxPlane::Mesh {
        Vec::new()
    } else {
        orch.message_bus()
            .inbox(agent_id)
            .into_iter()
            .map(|m| A2AMessageInfo {
                id: m.id.0,
                sender: m.sender.0,
                receiver: m.receiver.map(|r| r.0),
                msg_type: msg_type_wire(&m.msg_type),
                payload: format!("{} [Received {}s ago]", m.payload, m.elapsed_ms() / 1000),
                timestamp_ms: m.timestamp_ms,
                acknowledged: m.acknowledged,
            })
            .collect()
    };
    #[cfg_attr(not(feature = "populi-transport"), allow(unused_mut))]
    let mut remote_attempted = false;
    #[cfg_attr(not(feature = "populi-transport"), allow(unused_mut))]
    let mut remote_ok = false;
    #[cfg(feature = "populi-transport")]
    if source != A2AInboxPlane::Local
        && let Some(base) = vox_populi::http_lifecycle::populi_http_control_base_from_env()
    {
        if !base.trim().is_empty() {
            remote_attempted = true;
            let client = vox_populi::http_client::PopuliHttpClient::new(base).with_env_token();
            if let Ok(remote) = client
                .relay_a2a_inbox_limited(
                    &params.agent_id.to_string(),
                    params.max_messages,
                    params.before_message_id,
                )
                .await
            {
                remote_ok = true;
                let mut seen = std::collections::HashSet::new();
                for m in &messages {
                    seen.insert(m.id);
                }
                for m in remote.messages {
                    if seen.insert(m.id) {
                        messages.push(A2AMessageInfo {
                            id: m.id,
                            sender: m.sender_agent_id.parse::<u64>().unwrap_or(0),
                            receiver: m.receiver_agent_id.parse::<u64>().ok(),
                            msg_type: m.message_type,
                            payload: m.payload,
                            timestamp_ms: m.created_unix_ms,
                            acknowledged: m.acknowledged,
                        });
                    }
                }
            }
        }
    }
    messages.sort_by_key(|m| m.timestamp_ms);

    let focus_depth = if state.orchestrator_config.attention_enabled {
        let bm = state.orchestrator.budget_manager_handle();
        let snap = vox_orchestrator::sync_lock::rw_read(&*bm).attention_snapshot();
        snap.focus_depth()
    } else {
        vox_orchestrator::FocusDepth::Ambient
    };

    let suppressed_count = if focus_depth == vox_orchestrator::FocusDepth::Deep {
        let before = messages.len();
        messages.retain(|m| {
            let mt = parse_msg_type(&m.msg_type);
            is_high_priority_a2a_type(&mt)
        });
        let diff = before - messages.len();
        if diff > 0 {
            let bm = state.orchestrator.budget_manager_handle();
            let bml = vox_orchestrator::sync_lock::rw_read(&*bm);
            bml.record_inbox_suppression(diff as u32);
        }
        diff
    } else {
        0
    };

    let mut data = serde_json::json!({
        "agent_id": params.agent_id,
        "source": source.as_str(),
        "delivery_planes": match source {
            A2AInboxPlane::Local => vec![A2ADeliveryPlane::LocalEphemeral.as_str()],
            A2AInboxPlane::Mesh => vec![A2ADeliveryPlane::RemoteMesh.as_str()],
            A2AInboxPlane::Merged => vec![
                A2ADeliveryPlane::LocalEphemeral.as_str(),
                A2ADeliveryPlane::RemoteMesh.as_str(),
            ],
        },
        "unread_count": messages.len(),
        "remote_attempted": remote_attempted,
        "remote_ok": remote_ok,
        "dropped_messages": orch.message_bus().dropped_messages(),
        "suppressed_count": suppressed_count,
        "focus_depth_filter_active": focus_depth == vox_orchestrator::FocusDepth::Deep,
        "messages": messages,
    });
    if let Some(obj) = data.as_object_mut() {
        extend_binding_fields(obj, binding_outcome);
    }
    ToolResult::ok(data).to_json()
}

/// Acknowledge a message in an agent's inbox (async).
pub async fn a2a_ack(state: &ServerState, params: A2AAckParams) -> String {
    let orch = &state.orchestrator;
    let binding_outcome = match resolve_sender_binding(
        orch,
        AgentId(params.agent_id),
        params.agent_session_id.as_deref(),
    ) {
        Ok(o) => o,
        Err(msg) => return ToolResult::<String>::err(msg).to_json(),
    };

    let agent_id = AgentId(params.agent_id);
    let message_id = vox_orchestrator::types::MessageId(params.message_id);

    // Need mutable access to message_bus for ack
    let local_success = orch.message_bus_mut().acknowledge(agent_id, message_id);
    #[cfg_attr(not(feature = "populi-transport"), allow(unused_mut))]
    let mut remote_attempted = false;
    #[cfg_attr(not(feature = "populi-transport"), allow(unused_mut))]
    let mut remote_success = false;
    #[cfg(feature = "populi-transport")]
    if let Some(base) = vox_populi::http_lifecycle::populi_http_control_base_from_env() {
        if !base.trim().is_empty() {
            remote_attempted = true;
            let client = vox_populi::http_client::PopuliHttpClient::new(base).with_env_token();
            remote_success = client
                .relay_a2a_ack(&params.agent_id.to_string(), params.message_id)
                .await
                .unwrap_or(false);
        }
    }
    let success = local_success || remote_success;

    if success {
        let mut data = serde_json::json!({
            "acknowledged": true,
            "message_id": params.message_id,
            "local_acknowledged": local_success,
            "remote_attempted": remote_attempted,
            "remote_acknowledged": remote_success,
        });
        if let Some(obj) = data.as_object_mut() {
            extend_binding_fields(obj, binding_outcome);
        }
        ToolResult::ok(data).to_json()
    } else {
        ToolResult::<String>::err_with_remediation(
            format!(
                "Message {} not found in agent {}'s inbox",
                params.message_id, params.agent_id
            ),
            REM_A2A_ACK,
        )
        .to_json()
    }
}

/// Broadcast an A2A message to all agents except sender (async).
pub async fn a2a_broadcast(state: &ServerState, params: A2ABroadcastParams) -> String {
    let orch = &state.orchestrator;

    let sender = AgentId(params.sender_id);
    let binding_outcome =
        match resolve_sender_binding(orch, sender, params.sender_session_id.as_deref()) {
            Ok(o) => o,
            Err(msg) => return ToolResult::<String>::err(msg).to_json(),
        };
    let msg_type = parse_msg_type(&params.msg_type);
    let agent_count = orch.agent_ids().len().saturating_sub(1);

    if state.orchestrator_config.attention_enabled && a2a_message_may_surface_to_pilot(&msg_type) {
        let bm = state.orchestrator.budget_manager_handle();
        let snap = vox_orchestrator::sync_lock::rw_read(&*bm).attention_snapshot();
        if let vox_orchestrator::GateResult::AttentionExhausted { message, .. } =
            vox_orchestrator::BudgetGate::check_attention_snapshot(
                &snap,
                &state.orchestrator_config,
            )
        {
            return ToolResult::<String>::err_with_remediation(
                message,
                "Pilot attention budget is exhausted; resolve MCP clarifications or block broadcasts.",
            )
            .to_json();
        }

        let mut backlog = pending_backlog_for_session(state, params.sender_session_id.as_deref());
        backlog = backlog.saturating_add(agent_count as u32);

        let trust = trust_for_session(state, params.sender_session_id.as_deref());
        let signals = a2a_escalation_signals(
            &msg_type,
            &params.payload,
            backlog,
            trust,
            state.orchestrator_config.attention_interrupt_cost_ms,
        );
        let decision = evaluate_with_state(state, &signals, &snap);
        match decision {
            vox_orchestrator::InterruptionDecision::RequireHumanBeforeContinue {
                reason, ..
            } => {
                return ToolResult::<String>::err_with_remediation(
                    format!("A2A broadcast blocked pending human review: {reason}"),
                    "Reduce blast radius or include explicit human-approved context before resubmitting.",
                )
                .to_json();
            }
            vox_orchestrator::InterruptionDecision::DeferUntilCheckpoint { ref reason }
            | vox_orchestrator::InterruptionDecision::BatchWithExistingPrompt { ref reason } => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                state.record_attention_event(vox_orchestrator::AttentionEvent {
                    agent_id: sender,
                    task_id: None,
                    event_type: vox_orchestrator::AttentionEventType::A2AInterrupt,
                    tier: vox_orchestrator::ApprovalTier::Confirm,
                    cost_ms: 0,
                    outcome: vox_orchestrator::ApprovalOutcome::AutoApproved,
                    trust_score_at_time: trust,
                    effective_complexity: (params.payload.len() as f64 / 120.0).clamp(0.0, 10.0),
                    decision_entropy_bits: signals.expected_information_gain_bits,
                    timestamp_ms: ts,
                    channel: Some("a2a_broadcast".to_string()),
                    policy_reason: Some(reason.clone()),
                });
                let mut data = serde_json::json!({
                    "deferred": true,
                    "decision": decision_label(&decision),
                    "surface": "a2a_broadcast",
                    "channel": channel_label(vox_orchestrator::InterruptionChannel::A2AEscalation),
                    "reason": reason,
                    "timestamp_ms": ts,
                    "sender": params.sender_id,
                });
                if let Some(obj) = data.as_object_mut() {
                    extend_binding_fields(obj, binding_outcome);
                }
                return ToolResult::ok(data).to_json();
            }
            vox_orchestrator::InterruptionDecision::ProceedAutonomously { ref reason } => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                state.record_attention_event(vox_orchestrator::AttentionEvent {
                    agent_id: sender,
                    task_id: None,
                    event_type: vox_orchestrator::AttentionEventType::A2AInterrupt,
                    tier: vox_orchestrator::ApprovalTier::AutoApprove,
                    cost_ms: 0,
                    outcome: vox_orchestrator::ApprovalOutcome::AutoApproved,
                    trust_score_at_time: trust,
                    effective_complexity: (params.payload.len() as f64 / 120.0).clamp(0.0, 10.0),
                    decision_entropy_bits: signals.expected_information_gain_bits,
                    timestamp_ms: ts,
                    channel: Some("a2a_broadcast".to_string()),
                    policy_reason: Some(reason.clone()),
                });
            }
            vox_orchestrator::InterruptionDecision::InterruptNow {
                scaled_cost_ms,
                ref reason,
                ..
            } => {
                let cost_ms = scaled_cost_ms.saturating_mul(agent_count as u64);
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                state.record_attention_event(vox_orchestrator::AttentionEvent {
                    agent_id: sender,
                    task_id: None,
                    event_type: vox_orchestrator::AttentionEventType::A2AInterrupt,
                    tier: vox_orchestrator::ApprovalTier::Review,
                    cost_ms,
                    outcome: vox_orchestrator::ApprovalOutcome::Approved,
                    trust_score_at_time: trust,
                    effective_complexity: (params.payload.len() as f64 / 120.0).clamp(0.0, 10.0),
                    decision_entropy_bits: signals.expected_information_gain_bits,
                    timestamp_ms: ts,
                    channel: Some("a2a_broadcast".to_string()),
                    policy_reason: Some(reason.clone()),
                });
            }
        }
    }

    let msg_id = orch.broadcast_a2a(sender, msg_type.clone(), params.payload.clone());

    // Optional local JSONL for dogfood / debugging only — **not** an audit or delivery SSOT.
    // Durable A2A audit uses Codex `a2a_messages` when route=db; see docs/agents/database-nomenclature.md.
    if let Some(path) = state.dogfood_trace_path_for("a2a_traces.jsonl") {
        let trace_data = serde_json::json!({
            "prompt": format!("Sender: {}\nBroadcast: true\nMsgType: {}", params.sender_id, msg_type_wire(&msg_type)),
            "response": params.payload,
            "category": "a2a_trace",
            "schema_version": "vox_dogfood_v1",
        });
        spawn_supervised_infallible("a2a_dogfood_trace", async move {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(line) = serde_json::to_string(&trace_data) {
                use tokio::io::AsyncWriteExt;
                if let Ok(mut f) = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .await
                {
                    let _ = f.write_all(format!("{line}\n").as_bytes()).await;
                }
            }
        });
    }

    let mut data = serde_json::json!({
        "message_id": msg_id.0,
        "sender": params.sender_id,
        "delivered_to": agent_count,
        "dropped_messages": orch.message_bus().dropped_messages(),
    });
    if let Some(obj) = data.as_object_mut() {
        extend_binding_fields(obj, binding_outcome);
    }
    ToolResult::ok(data).to_json()
}

/// Query the A2A audit trail (async).
pub async fn a2a_history(state: &ServerState, params: A2AHistoryParams) -> String {
    let orch = &state.orchestrator;

    let limit = params.limit.unwrap_or(50);

    let messages: Vec<A2AMessageInfo> = if let Some(since) = params.since_ms {
        orch.message_bus()
            .audit_since(since)
            .into_iter()
            .take(limit)
            .map(|m| A2AMessageInfo {
                id: m.id.0,
                sender: m.sender.0,
                receiver: m.receiver.map(|r| r.0),
                msg_type: msg_type_wire(&m.msg_type),
                payload: format!("{} [Received {}s ago]", m.payload, m.elapsed_ms() / 1000),
                timestamp_ms: m.timestamp_ms,
                acknowledged: m.acknowledged,
            })
            .collect()
    } else {
        let bus_guard = orch.message_bus();
        bus_guard
            .audit_trail()
            .iter()
            .rev()
            .take(limit)
            .map(|m| A2AMessageInfo {
                id: m.id.0,
                sender: m.sender.0,
                receiver: m.receiver.map(|r| r.0),
                msg_type: msg_type_wire(&m.msg_type),
                payload: format!("{} [Received {}s ago]", m.payload, m.elapsed_ms() / 1000),
                timestamp_ms: m.timestamp_ms,
                acknowledged: m.acknowledged,
            })
            .collect()
    };

    ToolResult::ok(serde_json::json!({
        "total_messages": orch.message_bus().total_messages(),
        "dropped_messages": orch.message_bus().dropped_messages(),
        "returned": messages.len(),
        "messages": messages,
    }))
    .to_json()
}

#[cfg(test)]
mod tests {
    use super::{normalize_inbox_source, normalize_route};
    use vox_orchestrator::a2a::{A2ADeliveryPlane, A2AInboxPlane};

    #[test]
    fn inbox_source_defaults_to_merged() {
        assert_eq!(normalize_inbox_source(None).unwrap(), A2AInboxPlane::Merged);
        assert_eq!(
            normalize_inbox_source(Some("")).unwrap(),
            A2AInboxPlane::Merged
        );
        assert_eq!(
            normalize_inbox_source(Some("   ")).unwrap(),
            A2AInboxPlane::Merged
        );
    }

    #[test]
    fn inbox_source_accepts_local_mesh_and_merged() {
        assert_eq!(
            normalize_inbox_source(Some("local")).unwrap(),
            A2AInboxPlane::Local
        );
        assert_eq!(
            normalize_inbox_source(Some("mesh")).unwrap(),
            A2AInboxPlane::Mesh
        );
        assert_eq!(
            normalize_inbox_source(Some("merged")).unwrap(),
            A2AInboxPlane::Merged
        );
        assert_eq!(
            normalize_inbox_source(Some(" MeSh ")).unwrap(),
            A2AInboxPlane::Mesh
        );
    }

    #[test]
    fn inbox_source_rejects_unknown_value() {
        let err = normalize_inbox_source(Some("db")).unwrap_err();
        assert!(err.contains("expected merged|local|mesh"));
    }

    #[test]
    fn route_defaults_and_maps_to_canonical_planes() {
        assert_eq!(
            normalize_route(None).unwrap(),
            A2ADeliveryPlane::LocalEphemeral
        );
        assert_eq!(
            normalize_route(Some("db")).unwrap(),
            A2ADeliveryPlane::LocalDurable
        );
        assert_eq!(
            normalize_route(Some(" mesh ")).unwrap(),
            A2ADeliveryPlane::RemoteMesh
        );
    }
}

#[cfg(test)]
mod semcov_wave1d_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn parse_msg_type_maps_aliases_case_and_unknown() {
        // canonical snake_case
        assert_eq!(parse_msg_type("plan_handoff"), A2AMessageType::PlanHandoff);
        // concatenated spelling
        assert_eq!(parse_msg_type("scoperequest"), A2AMessageType::ScopeRequest);
        // case-insensitive folding
        assert_eq!(parse_msg_type("ErRoR_RePoRt"), A2AMessageType::ErrorReport);
        // remote_task_* aliases remap onto distinct canonical variants
        assert_eq!(
            parse_msg_type("remote_task_envelope"),
            A2AMessageType::PlanHandoff
        );
        assert_eq!(
            parse_msg_type("remote_task_ack"),
            A2AMessageType::ProgressUpdate
        );
        assert_eq!(
            parse_msg_type("remote_task_result"),
            A2AMessageType::CompletionNotice
        );
        // unknown input falls through to FreeForm
        assert_eq!(parse_msg_type("totally_unknown"), A2AMessageType::FreeForm);
        assert_eq!(parse_msg_type(""), A2AMessageType::FreeForm);
    }

    #[test]
    fn msg_type_wire_emits_snake_case_wire_strings() {
        use vox_orchestrator::a2a::A2AMessageType;
        assert_eq!(
            super::msg_type_wire(&A2AMessageType::HelpRequest),
            "help_request"
        );
        assert_eq!(super::msg_type_wire(&A2AMessageType::FreeForm), "free_form");
    }

    #[test]
    fn fnv1a64_matches_canonical_values_and_is_concatenative() {
        // Empty input yields the FNV-1a 64-bit offset basis unchanged.
        assert_eq!(fnv1a64(&[]), 0xcbf2_9ce4_8422_2325);
        // Canonical FNV-1a-64 hash of the single byte "a".
        assert_eq!(fnv1a64(&["a"]), 0xaf63_dc4c_8601_ec8c);
        // Parts are hashed as a byte stream, so splitting is invariant.
        assert_eq!(fnv1a64(&["ab"]), fnv1a64(&["a", "b"]));
        // An extra empty part does not change the result.
        assert_eq!(fnv1a64(&["a", "b"]), fnv1a64(&["a", "", "b"]));
    }

    #[test]
    fn default_idempotency_key_matches_fnv1a_format() {
        // FNV-1a over the concatenated bytes of "ping" then "hi", lower-hex 16-wide.
        let key = default_idempotency_key(1, 2, "ping", "hi");
        assert_eq!(key, "mcp-a2a-1-2-21f52f76e48dc9ae");

        // The hash region is exactly 16 lowercase hex digits.
        let hash_hex = key.strip_prefix("mcp-a2a-1-2-").expect("prefix");
        assert_eq!(hash_hex.len(), 16);
        assert!(
            hash_hex
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );

        // Sender/receiver are interpolated verbatim and independent of the hash.
        let other = default_idempotency_key(7, 9, "ping", "hi");
        assert_eq!(other, "mcp-a2a-7-9-21f52f76e48dc9ae");

        // Hash is over the *joined* (msg_type, payload): moving the split boundary
        // changes nothing, but changing the joined bytes changes the key.
        assert_eq!(
            default_idempotency_key(1, 2, "pin", "ghi"),
            default_idempotency_key(1, 2, "ping", "hi"),
        );
        assert_ne!(
            default_idempotency_key(1, 2, "ping", "ho"),
            default_idempotency_key(1, 2, "ping", "hi"),
        );
    }

    #[test]
    fn a2a_message_may_surface_to_pilot_classifies_pilot_surfacing_types() {
        // The three pilot-surfacing types return true.
        assert!(a2a_message_may_surface_to_pilot(
            &A2AMessageType::HelpRequest
        ));
        assert!(a2a_message_may_surface_to_pilot(
            &A2AMessageType::ErrorReport
        ));
        assert!(a2a_message_may_surface_to_pilot(
            &A2AMessageType::ConflictDetected
        ));

        // Representative non-surfacing types return false.
        assert!(!a2a_message_may_surface_to_pilot(
            &A2AMessageType::PlanHandoff
        ));
        assert!(!a2a_message_may_surface_to_pilot(
            &A2AMessageType::ProgressUpdate
        ));
        assert!(!a2a_message_may_surface_to_pilot(&A2AMessageType::FreeForm));
        assert!(!a2a_message_may_surface_to_pilot(
            &A2AMessageType::ConflictResolved
        ));
    }
}
