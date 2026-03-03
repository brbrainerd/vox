//! Plan/context handoff protocol between agents.
//!
//! Enables one agent to serialise its current state (plan, completed tasks,
//! context summary) into a portable document that another agent can load
//! and resume from. This is critical for scaling beyond a single long-lived
//! agent session.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::events::{AgentEventKind, EventBus};
use crate::types::{AgentId, TaskId};

/// A single step in the execution history preserved during handoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub timestamp: u64,
    pub event: String,
}

/// A portable handoff document containing everything a receiving agent
/// needs to resume the sender's work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffPayload {
    /// Who is handing off.
    pub from_agent: AgentId,
    /// Who should receive (None = any available agent).
    pub to_agent: Option<AgentId>,
    /// When this handoff was created (unix ms).
    pub created_at: u64,
    /// Optional timeout for this handoff.
    pub timeout_ms: Option<u64>,
    /// Human-readable summary of the plan/work being handed off.
    pub plan_summary: String,
    /// Tasks that have been completed by the sender.
    pub completed_tasks: Vec<TaskId>,
    /// Tasks that are still pending and should be continued.
    pub pending_tasks: Vec<TaskId>,
    /// Files the sender was working on (for affinity transfer).
    pub owned_files: Vec<PathBuf>,
    /// Detailed history of execution steps taken so far.
    #[serde(default)]
    pub execution_history: Vec<ExecutionStep>,
    /// Free-form context notes for the receiver.
    pub context_notes: String,
    /// Key-value metadata for machine-readable context.
    pub metadata: Vec<(String, String)>,
    /// Unresolved objectives the receiver must satisfy (deterministic handoff invariant).
    #[serde(default)]
    pub unresolved_objectives: Vec<String>,
    /// Verification criteria the receiver should check before considering work done.
    #[serde(default)]
    pub verification_criteria: Vec<String>,
}

impl HandoffPayload {
    /// Create a new handoff payload.
    pub fn new(
        from_agent: AgentId,
        to_agent: Option<AgentId>,
        plan_summary: impl Into<String>,
    ) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            from_agent,
            to_agent,
            created_at,
            timeout_ms: None,
            plan_summary: plan_summary.into(),
            completed_tasks: Vec::new(),
            pending_tasks: Vec::new(),
            owned_files: Vec::new(),
            execution_history: Vec::new(),
            context_notes: String::new(),
            metadata: Vec::new(),
            unresolved_objectives: Vec::new(),
            verification_criteria: Vec::new(),
        }
    }

    /// Builder: add a timeout.
    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    /// Builder: add an execution history step.
    pub fn with_step(mut self, step: ExecutionStep) -> Self {
        self.execution_history.push(step);
        self
    }

    /// Builder: add multiple execution history steps.
    pub fn with_history(mut self, history: Vec<ExecutionStep>) -> Self {
        self.execution_history = history;
        self
    }

    /// Builder: add completed tasks.
    pub fn with_completed(mut self, tasks: Vec<TaskId>) -> Self {
        self.completed_tasks = tasks;
        self
    }

    /// Builder: add pending tasks.
    pub fn with_pending(mut self, tasks: Vec<TaskId>) -> Self {
        self.pending_tasks = tasks;
        self
    }

    /// Builder: add owned files.
    pub fn with_files(mut self, files: Vec<PathBuf>) -> Self {
        self.owned_files = files;
        self
    }

    /// Builder: add context notes.
    pub fn with_context(mut self, notes: impl Into<String>) -> Self {
        self.context_notes = notes.into();
        self
    }

    /// Builder: add a metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push((key.into(), value.into()));
        self
    }

    /// Builder: add unresolved objectives for the receiver.
    pub fn with_unresolved_objectives(mut self, objectives: Vec<String>) -> Self {
        self.unresolved_objectives = objectives;
        self
    }

    /// Builder: add verification criteria the receiver must check.
    pub fn with_verification_criteria(mut self, criteria: Vec<String>) -> Self {
        self.verification_criteria = criteria;
        self
    }

    /// Serialize to JSON for transmission.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Generate a markdown summary of this handoff.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# Handoff from {}\n\n", self.from_agent));
        if let Some(to) = self.to_agent {
            md.push_str(&format!("**To:** {}\n\n", to));
        }
        md.push_str(&format!("## Plan\n\n{}\n\n", self.plan_summary));

        if !self.completed_tasks.is_empty() {
            md.push_str("## Completed Tasks\n\n");
            for t in &self.completed_tasks {
                md.push_str(&format!("- [x] {}\n", t));
            }
            md.push('\n');
        }

        if !self.pending_tasks.is_empty() {
            md.push_str("## Pending Tasks\n\n");
            for t in &self.pending_tasks {
                md.push_str(&format!("- [ ] {}\n", t));
            }
            md.push('\n');
        }

        if !self.owned_files.is_empty() {
            md.push_str("## Files\n\n");
            for f in &self.owned_files {
                md.push_str(&format!("- `{}`\n", f.display()));
            }
            md.push('\n');
        }

        if !self.context_notes.is_empty() {
            md.push_str(&format!("## Context\n\n{}\n\n", self.context_notes));
        }

        if !self.unresolved_objectives.is_empty() {
            md.push_str("## Unresolved Objectives\n\n");
            for o in &self.unresolved_objectives {
                md.push_str(&format!("- [ ] {}\n", o));
            }
            md.push('\n');
        }

        if !self.verification_criteria.is_empty() {
            md.push_str("## Verification Criteria\n\n");
            for c in &self.verification_criteria {
                md.push_str(&format!("- {}\n", c));
            }
            md.push('\n');
        }

        md
    }
}

/// Execute a handoff: emit the event and return the payload for the receiver.
pub fn execute_handoff(payload: &HandoffPayload, event_bus: &EventBus) {
    let to_str = payload
        .to_agent
        .map(|a| a.to_string())
        .unwrap_or_else(|| "any".to_string());

    tracing::info!(
        from = %payload.from_agent,
        to = %to_str,
        pending = payload.pending_tasks.len(),
        "executing plan handoff"
    );

    event_bus.emit(AgentEventKind::PlanHandoff {
        from: payload.from_agent,
        to: payload.to_agent.unwrap_or(AgentId(0)),
        plan_summary: payload.plan_summary.clone(),
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_builder() {
        let payload = HandoffPayload::new(AgentId(1), Some(AgentId(2)), "Fix parser bugs")
            .with_completed(vec![TaskId(1), TaskId(2)])
            .with_pending(vec![TaskId(3)])
            .with_files(vec![PathBuf::from("src/parser.rs")])
            .with_context("The parser has 3 failing tests")
            .with_metadata("priority", "high");

        assert_eq!(payload.from_agent, AgentId(1));
        assert_eq!(payload.to_agent, Some(AgentId(2)));
        assert_eq!(payload.completed_tasks.len(), 2);
        assert_eq!(payload.pending_tasks.len(), 1);
        assert_eq!(payload.owned_files.len(), 1);
        assert_eq!(payload.metadata.len(), 1);
    }

    #[test]
    fn json_roundtrip() {
        let payload = HandoffPayload::new(AgentId(1), None, "Implement type checker")
            .with_pending(vec![TaskId(10)]);

        let json = payload.to_json();
        let back = HandoffPayload::from_json(&json).expect("deserialize");
        assert_eq!(back.from_agent, AgentId(1));
        assert_eq!(back.pending_tasks, vec![TaskId(10)]);
    }

    #[test]
    fn markdown_output() {
        let payload = HandoffPayload::new(AgentId(1), Some(AgentId(2)), "Fix bugs")
            .with_completed(vec![TaskId(1)])
            .with_pending(vec![TaskId(2), TaskId(3)]);

        let md = payload.to_markdown();
        assert!(md.contains("# Handoff from A-01"));
        assert!(md.contains("**To:** A-02"));
        assert!(md.contains("[x] T-0001"));
        assert!(md.contains("[ ] T-0002"));
    }

    #[test]
    fn execute_handoff_emits_event() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let payload = HandoffPayload::new(AgentId(1), Some(AgentId(2)), "Test handoff");
        execute_handoff(&payload, &bus);

        // Event should be in the channel
        let event = rx.try_recv().expect("should have event");
        match event.kind {
            AgentEventKind::PlanHandoff { from, to, .. } => {
                assert_eq!(from, AgentId(1));
                assert_eq!(to, AgentId(2));
            }
            _ => panic!("wrong event type"),
        }
    }

    #[test]
    fn handoff_history_and_timeout() {
        let step = ExecutionStep {
            task_id: TaskId(1),
            agent_id: AgentId(1),
            timestamp: 12345678,
            event: "Started task".to_string(),
        };
        let payload = HandoffPayload::new(AgentId(1), None, "Test history")
            .with_timeout(1000)
            .with_step(step.clone());

        assert_eq!(payload.timeout_ms, Some(1000));
        assert_eq!(payload.execution_history.len(), 1);
        assert_eq!(payload.execution_history[0].task_id, TaskId(1));
    }
}
