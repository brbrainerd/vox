//! Orchestrator agent adapter — event translation only.
//!
//! Translates `AgentEvent`s from `vox-orchestrator`'s `EventBus` into
//! `SessionEvent`s. Does NOT reimplement the agent loop.
//!
//! HITL note: `FeedbackStore::{open_needs_you, resolve}` will be wired here
//! once `claude/context-window-spine` merges to main (adds feedback module to
//! vox-orchestrator). See docs/superpowers/plans/…vox-terminal-warp-parity.md §HITL.

use vox_orchestrator::events::{AgentEvent, AgentEventKind};
use vox_orchestrator::types::AgentId;

use crate::session::SessionEvent;

/// Config for the adapter. `agent_id = Some(id)` filters events to one agent;
/// `None` passes through all agents (useful for single-agent sessions).
#[derive(Debug, Clone, Default)]
pub struct AgentAdapterConfig {
    pub agent_id: Option<AgentId>,
}

/// Pure translation function — maps one `AgentEvent` → `SessionEvent` or `None`.
///
/// Exposed as `pub` so tests can call it directly without a live orchestrator.
pub fn translate_event(
    ev: &AgentEvent,
    cfg: &AgentAdapterConfig,
) -> Option<SessionEvent> {
    // Filter by agent_id if configured.
    if let Some(filter_id) = cfg.agent_id {
        let ev_agent = agent_id_of(ev)?;
        if ev_agent != filter_id {
            return None;
        }
    }

    match &ev.kind {
        AgentEventKind::TokenStreamed { text, .. } => {
            Some(SessionEvent::AgentMessage { text: text.clone() })
        }
        // All other events are not surfaced to the terminal session.
        _ => None,
    }
}

fn agent_id_of(ev: &AgentEvent) -> Option<AgentId> {
    match &ev.kind {
        AgentEventKind::AgentSpawned { agent_id, .. }
        | AgentEventKind::AgentRetired { agent_id }
        | AgentEventKind::AgentHeartbeat { agent_id, .. }
        | AgentEventKind::ActivityChanged { agent_id, .. }
        | AgentEventKind::OperatingModeChanged { agent_id, .. }
        | AgentEventKind::TokenStreamed { agent_id, .. } => Some(*agent_id),
        _ => None,
    }
}
