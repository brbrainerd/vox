//! A2A (Agent-to-Agent) MCP tools — send, inbox, ack, broadcast, history.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ServerState, ToolResult};
use vox_orchestrator::types::{A2AMessageType, AgentId};

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct A2ASendParams {
    pub sender_id: u64,
    pub receiver_id: u64,
    pub msg_type: String,
    pub payload: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct A2AInboxParams {
    pub agent_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct A2AAckParams {
    pub agent_id: u64,
    pub message_id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct A2ABroadcastParams {
    pub sender_id: u64,
    pub msg_type: String,
    pub payload: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct A2AHistoryParams {
    pub since_ms: Option<u64>,
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct A2AMessageInfo {
    pub id: u64,
    pub sender: u64,
    pub receiver: Option<u64>,
    pub msg_type: String,
    pub payload: String,
    pub timestamp_ms: u64,
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
        _ => A2AMessageType::FreeForm,
    }
}

fn msg_type_name(mt: &A2AMessageType) -> String {
    match mt {
        A2AMessageType::PlanHandoff => "PlanHandoff".to_string(),
        A2AMessageType::ScopeRequest => "ScopeRequest".to_string(),
        A2AMessageType::ScopeGrant => "ScopeGrant".to_string(),
        A2AMessageType::ProgressUpdate => "ProgressUpdate".to_string(),
        A2AMessageType::HelpRequest => "HelpRequest".to_string(),
        A2AMessageType::CompletionNotice => "CompletionNotice".to_string(),
        A2AMessageType::ErrorReport => "ErrorReport".to_string(),
        A2AMessageType::FreeForm => "FreeForm".to_string(),
        A2AMessageType::ConflictDetected => "ConflictDetected".to_string(),
        A2AMessageType::ConflictResolved => "ConflictResolved".to_string(),
        A2AMessageType::VcsEvent => "VcsEvent".to_string(),
        A2AMessageType::CancelRequest => "CancelRequest".to_string(),
        A2AMessageType::SnapshotShare => "SnapshotShare".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Send a targeted A2A message from one agent to another (async).
pub async fn a2a_send(state: &ServerState, params: A2ASendParams) -> String {
    let mut orch = state.orchestrator.lock().await;

    let sender = AgentId(params.sender_id);
    let receiver = AgentId(params.receiver_id);
    let msg_type = parse_msg_type(&params.msg_type);

    let msg_id = orch.send_a2a(sender, receiver, msg_type, params.payload);

    ToolResult::ok(serde_json::json!({
        "message_id": msg_id.0,
        "sender": params.sender_id,
        "receiver": params.receiver_id,
    }))
    .to_json()
}

/// Read unacknowledged messages in an agent's inbox (async).
pub async fn a2a_inbox(state: &ServerState, params: A2AInboxParams) -> String {
    let orch = state.orchestrator.lock().await;

    let agent_id = AgentId(params.agent_id);
    let messages: Vec<A2AMessageInfo> = orch
        .message_bus()
        .inbox(agent_id)
        .into_iter()
        .map(|m| A2AMessageInfo {
            id: m.id.0,
            sender: m.sender.0,
            receiver: m.receiver.map(|r| r.0),
            msg_type: msg_type_name(&m.msg_type),
            payload: m.payload.clone(),
            timestamp_ms: m.timestamp_ms,
            acknowledged: m.acknowledged,
        })
        .collect();

    ToolResult::ok(serde_json::json!({
        "agent_id": params.agent_id,
        "unread_count": messages.len(),
        "messages": messages,
    }))
    .to_json()
}

/// Acknowledge a message in an agent's inbox (async).
pub async fn a2a_ack(state: &ServerState, params: A2AAckParams) -> String {
    let mut orch = state.orchestrator.lock().await;

    let agent_id = AgentId(params.agent_id);
    let message_id = vox_orchestrator::types::MessageId(params.message_id);

    // Need mutable access to message_bus for ack
    let success = orch.message_bus_mut().acknowledge(agent_id, message_id);

    if success {
        ToolResult::ok(serde_json::json!({
            "acknowledged": true,
            "message_id": params.message_id,
        }))
        .to_json()
    } else {
        ToolResult::<String>::err(format!(
            "Message {} not found in agent {}'s inbox",
            params.message_id, params.agent_id
        ))
        .to_json()
    }
}

/// Broadcast an A2A message to all agents except sender (async).
pub async fn a2a_broadcast(state: &ServerState, params: A2ABroadcastParams) -> String {
    let mut orch = state.orchestrator.lock().await;

    let sender = AgentId(params.sender_id);
    let msg_type = parse_msg_type(&params.msg_type);

    let msg_id = orch.broadcast_a2a(sender, msg_type, params.payload);

    let agent_count = orch.agent_ids().len().saturating_sub(1);

    ToolResult::ok(serde_json::json!({
        "message_id": msg_id.0,
        "sender": params.sender_id,
        "delivered_to": agent_count,
    }))
    .to_json()
}

/// Query the A2A audit trail (async).
pub async fn a2a_history(state: &ServerState, params: A2AHistoryParams) -> String {
    let orch = state.orchestrator.lock().await;

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
                msg_type: msg_type_name(&m.msg_type),
                payload: m.payload.clone(),
                timestamp_ms: m.timestamp_ms,
                acknowledged: m.acknowledged,
            })
            .collect()
    } else {
        let trail = orch.message_bus().audit_trail();
        trail
            .iter()
            .rev()
            .take(limit)
            .map(|m| A2AMessageInfo {
                id: m.id.0,
                sender: m.sender.0,
                receiver: m.receiver.map(|r| r.0),
                msg_type: msg_type_name(&m.msg_type),
                payload: m.payload.clone(),
                timestamp_ms: m.timestamp_ms,
                acknowledged: m.acknowledged,
            })
            .collect()
    };

    ToolResult::ok(serde_json::json!({
        "total_messages": orch.message_bus().total_messages(),
        "returned": messages.len(),
        "messages": messages,
    }))
    .to_json()
}
