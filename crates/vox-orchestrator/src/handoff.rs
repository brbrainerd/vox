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

/// Metadata key carrying serialized [`crate::ContextEnvelope`] JSON.
pub const CONTEXT_ENVELOPE_JSON_METADATA_KEY: &str = "context_envelope_json";
/// Metadata key carrying serialized [`crate::AgentHarnessSpec`] JSON.
pub const HARNESS_SPEC_JSON_METADATA_KEY: &str = "harness_spec_json";

/// Violation of structured handoff invariants (verification vs pending work).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandoffInvariantError {
    /// Pending tasks require at least one verification criterion for the receiver.
    #[error("handoff with pending tasks must include at least one verification criterion")]
    MissingVerificationCriteria,
    /// Campaign role handoff should preserve role metadata for pending work.
    #[error("handoff with pending tasks should include execution_role metadata")]
    MissingExecutionRoleMetadata,
    /// Metadata advertised a context envelope but it failed to parse.
    #[error("handoff metadata contains invalid context envelope JSON: {0}")]
    InvalidContextEnvelope(String),
    /// Metadata advertised a harness spec but it failed validation.
    #[error("handoff metadata contains invalid harness spec JSON: {0}")]
    InvalidHarnessSpec(String),
    /// Receiver context resolution lacks opaque routing metadata.
    #[error("handoff to a remote agent requires a2a_context_uri and obo_token payloads")]
    MissingOpaqueRoutingContext,
    /// Raw conversational transcript detected in handoff content.
    ///
    /// Research (Context Handoff §4) proves passing full conversational history
    /// across agent boundaries causes context bleed and identity smuggling.
    /// Handoffs must use structured `ContextEnvelope` payloads, not raw text.
    #[error("transcript bleed: raw conversational content detected in handoff field '{field}'")]
    TranscriptBleed { field: String },
}

/// A single step in the execution history preserved during handoff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Task this log line refers to.
    pub task_id: TaskId,
    /// Agent that produced the event.
    pub agent_id: AgentId,
    /// Unix milliseconds when the step occurred.
    pub timestamp: u64,
    /// Short event label (e.g. task phase or tool name).
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
    /// Optional A2A cryptologic reference URI for context retrieval across nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a2a_context_uri: Option<String>,
    /// Optional On-Behalf-Of token session smuggling protection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obo_token: Option<String>,
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
    /// Optional manifest of blob/image attachments for visual auditing or multi-modal continuation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment_manifest: Option<crate::attachment_manifest::AttachmentManifest>,
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
            a2a_context_uri: None,
            obo_token: None,
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
            attachment_manifest: None,
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

    /// Builder: Set an A2A routing context identity map context.
    pub fn with_a2a_context(mut self, uri: impl Into<String>) -> Self {
        self.a2a_context_uri = Some(uri.into());
        self
    }

    /// Builder: Set On-Behalf-Of token metadata.
    pub fn with_obo_token(mut self, tok: impl Into<String>) -> Self {
        self.obo_token = Some(tok.into());
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

    /// Builder: add an attachment manifest.
    pub fn with_attachments(
        mut self,
        manifest: crate::attachment_manifest::AttachmentManifest,
    ) -> Self {
        self.attachment_manifest = Some(manifest);
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

        if let Some(manifest) = &self.attachment_manifest {
            md.push_str("## Attachments\n\n");
            for a in &manifest.attachments {
                md.push_str(&format!(
                    "- **{}** (`{}`) [sha256: {}]\n",
                    a.label, a.mime_type, a.sha256
                ));
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

/// Heuristic markers for raw conversational transcripts that should never
/// cross agent boundaries. Research (Context Handoff §4) proves these leak
/// identity, session state, and prompt-injection vectors.
const TRANSCRIPT_MARKERS: &[&str] = &[
    "<|im_start|>",
    "<|im_end|>",
    "<|system|>",
    "<|user|>",
    "<|assistant|>",
    "Human:",
    "Assistant:",
    "### Human",
    "### Assistant",
    "[INST]",
    "[/INST]",
];

/// Returns `Some(marker)` if `text` contains a raw transcript marker.
fn detect_transcript_bleed(text: &str) -> Option<&'static str> {
    TRANSCRIPT_MARKERS
        .iter()
        .find(|&&marker| text.contains(marker))
        .copied()
        .map(|v| v as _)
}

/// Ensure pending work always carries explicit verification steps for the receiver.
pub fn validate_handoff_invariants(payload: &HandoffPayload) -> Result<(), HandoffInvariantError> {
    if !payload.pending_tasks.is_empty() && payload.verification_criteria.is_empty() {
        return Err(HandoffInvariantError::MissingVerificationCriteria);
    }
    if !payload.pending_tasks.is_empty()
        && !payload.metadata.iter().any(|(k, _)| k == "execution_role")
    {
        return Err(HandoffInvariantError::MissingExecutionRoleMetadata);
    }

    if payload.to_agent.is_some() {
        if payload.a2a_context_uri.is_none() || payload.obo_token.is_none() {
            return Err(HandoffInvariantError::MissingOpaqueRoutingContext);
        }
    }

    // Task 3.3.1: Transcript bleed guard — block raw conversational content.
    if let Some(_marker) = detect_transcript_bleed(&payload.context_notes) {
        return Err(HandoffInvariantError::TranscriptBleed {
            field: "context_notes".to_string(),
        });
    }
    if let Some(_marker) = detect_transcript_bleed(&payload.plan_summary) {
        return Err(HandoffInvariantError::TranscriptBleed {
            field: "plan_summary".to_string(),
        });
    }
    for (key, value) in &payload.metadata {
        if key == CONTEXT_ENVELOPE_JSON_METADATA_KEY || key == HARNESS_SPEC_JSON_METADATA_KEY {
            continue; // structured payloads are allowed
        }
        if let Some(_marker) = detect_transcript_bleed(value) {
            return Err(HandoffInvariantError::TranscriptBleed {
                field: format!("metadata[{key}]"),
            });
        }
    }

    if let Some((_, context_json)) = payload
        .metadata
        .iter()
        .rev()
        .find(|(k, _)| k == CONTEXT_ENVELOPE_JSON_METADATA_KEY)
        && let Err(err) = serde_json::from_str::<crate::ContextEnvelope>(context_json)
    {
        return Err(HandoffInvariantError::InvalidContextEnvelope(
            err.to_string(),
        ));
    }
    if let Some((_, harness_json)) = payload
        .metadata
        .iter()
        .rev()
        .find(|(k, _)| k == HARNESS_SPEC_JSON_METADATA_KEY)
    {
        match serde_json::from_str::<crate::AgentHarnessSpec>(harness_json) {
            Ok(harness) => {
                let expectations = crate::HarnessIngestExpectations {
                    repository_id: harness.subject.repository_id.as_str(),
                    session_id: harness.subject.session_id.as_deref(),
                    thread_id: harness.subject.thread_id.as_deref(),
                };
                if let Err(errs) = crate::validate_agent_harness_ingest(&harness, expectations) {
                    return Err(HandoffInvariantError::InvalidHarnessSpec(errs.join("; ")));
                }
            }
            Err(err) => return Err(HandoffInvariantError::InvalidHarnessSpec(err.to_string())),
        }
    }
    Ok(())
}

/// Returns compact event metadata derived from optional handoff context / harness metadata.
#[must_use]
pub fn handoff_context_event_metadata(
    payload: &HandoffPayload,
) -> (bool, bool, Option<String>, Option<String>) {
    let Some((_, context_json)) = payload
        .metadata
        .iter()
        .rev()
        .find(|(k, _)| k == CONTEXT_ENVELOPE_JSON_METADATA_KEY)
    else {
        let harness = payload
            .metadata
            .iter()
            .rev()
            .find(|(k, _)| k == HARNESS_SPEC_JSON_METADATA_KEY)
            .and_then(|(_, raw)| serde_json::from_str::<crate::AgentHarnessSpec>(raw).ok());
        let has_harness_spec = harness.is_some();
        return (
            false,
            has_harness_spec,
            harness.as_ref().and_then(|h| h.subject.session_id.clone()),
            harness.as_ref().and_then(|h| h.subject.thread_id.clone()),
        );
    };
    let parsed_context = serde_json::from_str::<crate::ContextEnvelope>(context_json).ok();
    let session_id = parsed_context.as_ref().and_then(|env| {
        env.subject
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
    });
    let thread_id = parsed_context.as_ref().and_then(|env| {
        env.subject
            .thread_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
    });
    let has_harness_spec = payload
        .metadata
        .iter()
        .any(|(k, _)| k == HARNESS_SPEC_JSON_METADATA_KEY);
    (true, has_harness_spec, session_id, thread_id)
}

/// Execute a handoff: validate invariants, then emit the event for the receiver.
pub fn execute_handoff(
    payload: &HandoffPayload,
    event_bus: &EventBus,
) -> Result<(), HandoffInvariantError> {
    validate_handoff_invariants(payload)?;
    let (has_context_envelope, has_harness_spec, session_id, thread_id) =
        handoff_context_event_metadata(payload);
    let to_str = payload
        .to_agent
        .map(|a| a.to_string())
        .unwrap_or_else(|| "any".to_string());

    let from_role = payload
        .metadata
        .iter()
        .find(|(k, _)| k == "execution_role")
        .map(|(_, v)| match v.to_lowercase().as_str() {
            "planner" => crate::topology::AgentRole::Planner,
            "executor" | "builder" => crate::topology::AgentRole::Executor,
            "verifier" => crate::topology::AgentRole::Verifier,
            "synthesizer" => crate::topology::AgentRole::Synthesizer,
            "researcher" => crate::topology::AgentRole::Researcher,
            "observer" => crate::topology::AgentRole::Observer,
            _ => crate::topology::AgentRole::Generalist,
        });

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
        has_context_envelope,
        has_harness_spec,
        session_id,
        thread_id,
        from_role,
    });
    Ok(())
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
            .with_a2a_context("uri://agent2")
            .with_obo_token("tok-123")
            .with_completed(vec![TaskId(1), TaskId(2)])
            .with_pending(vec![TaskId(3)])
            .with_files(vec![PathBuf::from("src/parser.rs")])
            .with_context("The parser has 3 failing tests")
            .with_metadata("priority", "high")
            .with_metadata("execution_role", "builder")
            .with_verification_criteria(vec!["cargo check -p vox-orchestrator".to_string()]);

        assert_eq!(payload.from_agent, AgentId(1));
        assert_eq!(payload.to_agent, Some(AgentId(2)));
        assert_eq!(payload.completed_tasks.len(), 2);
        assert_eq!(payload.pending_tasks.len(), 1);
        assert_eq!(payload.owned_files.len(), 1);
        assert_eq!(payload.metadata.len(), 2);
    }

    #[test]
    fn json_roundtrip() {
        let payload = HandoffPayload::new(AgentId(1), None, "Implement type checker")
            .with_pending(vec![TaskId(10)])
            .with_metadata("execution_role", "verifier")
            .with_verification_criteria(vec!["run targeted tests".to_string()]);

        let json = payload.to_json();
        let back = HandoffPayload::from_json(&json).expect("deserialize");
        assert_eq!(back.from_agent, AgentId(1));
        assert_eq!(back.pending_tasks, vec![TaskId(10)]);
    }

    #[test]
    fn markdown_output() {
        let payload = HandoffPayload::new(AgentId(1), Some(AgentId(2)), "Fix bugs")
            .with_a2a_context("uri://agent2")
            .with_obo_token("tok-123")
            .with_completed(vec![TaskId(1)])
            .with_pending(vec![TaskId(2), TaskId(3)])
            .with_metadata("execution_role", "builder")
            .with_verification_criteria(vec!["verify bugfix".to_string()]);

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

        let payload = HandoffPayload::new(AgentId(1), Some(AgentId(2)), "Test handoff")
            .with_a2a_context("uri://agent2")
            .with_obo_token("tok-123");
        execute_handoff(&payload, &bus).expect("handoff invariants");

        // Event should be in the channel
        let event = rx.try_recv().expect("should have event");
        match event.kind {
            AgentEventKind::PlanHandoff {
                from,
                to,
                has_context_envelope,
                has_harness_spec,
                session_id,
                thread_id,
                from_role,
                ..
            } => {
                assert_eq!(from, AgentId(1));
                assert_eq!(to, AgentId(2));
                assert!(!has_context_envelope);
                assert!(!has_harness_spec);
                assert!(session_id.is_none());
                assert!(thread_id.is_none());
                assert!(from_role.is_none());
            }
            _ => panic!("wrong event type"),
        }
    }

    #[test]
    fn handoff_pending_requires_verification() {
        let bus = EventBus::new(4);
        let payload =
            HandoffPayload::new(AgentId(1), None, "Work left").with_pending(vec![TaskId(1)]);
        let err = execute_handoff(&payload, &bus).unwrap_err();
        assert_eq!(err, HandoffInvariantError::MissingVerificationCriteria);
    }

    #[test]
    fn handoff_pending_requires_execution_role_metadata() {
        let bus = EventBus::new(4);
        let payload = HandoffPayload::new(AgentId(1), None, "Work left")
            .with_pending(vec![TaskId(1)])
            .with_verification_criteria(vec!["verify".to_string()]);
        let err = execute_handoff(&payload, &bus).unwrap_err();
        assert_eq!(err, HandoffInvariantError::MissingExecutionRoleMetadata);
    }

    #[test]
    fn validate_handoff_invariants_ok_when_pending_has_role_and_criteria() {
        let payload = HandoffPayload::new(AgentId(1), None, "Work left")
            .with_pending(vec![TaskId(1)])
            .with_metadata("execution_role", "builder")
            .with_verification_criteria(vec!["verify outputs".to_string()]);
        assert!(validate_handoff_invariants(&payload).is_ok());
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

    #[test]
    fn handoff_rejects_invalid_context_envelope_metadata() {
        let bus = EventBus::new(4);
        let payload = HandoffPayload::new(AgentId(1), None, "Work left")
            .with_metadata(CONTEXT_ENVELOPE_JSON_METADATA_KEY, "{not-json");
        let err = execute_handoff(&payload, &bus).unwrap_err();
        assert!(matches!(
            err,
            HandoffInvariantError::InvalidContextEnvelope(_)
        ));
    }

    #[test]
    fn handoff_accepts_valid_context_envelope_metadata() {
        let bus = EventBus::new(4);
        let retrieval = crate::SessionRetrievalEnvelope {
            retrieval_tier: "hybrid".to_string(),
            memory_hit_count: 1,
            knowledge_hit_count: 1,
            chunk_hit_count: 0,
            repo_hit_count: 0,
            rrf_fused_hit_count: 0,
            used_vector: true,
            used_bm25: true,
            used_lexical_fallback: false,
            contradiction_count: 0,
            source_diversity: 2,
            evidence_quality: 0.8,
            citation_coverage: 0.9,
            verification_performed: false,
            verification_reason: None,
            recommended_next_action: None,
        };
        let context = crate::ContextEnvelope::from_session_retrieval("repo", "sess", &retrieval);
        let context_json = serde_json::to_string(&context).expect("serialize context envelope");
        let payload = HandoffPayload::new(AgentId(1), Some(AgentId(2)), "handoff")
            .with_a2a_context("uri://agent2")
            .with_obo_token("tok-123")
            .with_metadata(CONTEXT_ENVELOPE_JSON_METADATA_KEY, context_json);
        execute_handoff(&payload, &bus).expect("valid context envelope metadata");
    }

    #[test]
    fn handoff_rejects_invalid_harness_spec_metadata() {
        let bus = EventBus::new(4);
        let payload = HandoffPayload::new(AgentId(1), None, "Work left")
            .with_metadata(HARNESS_SPEC_JSON_METADATA_KEY, "{not-json");
        let err = execute_handoff(&payload, &bus).unwrap_err();
        assert!(matches!(err, HandoffInvariantError::InvalidHarnessSpec(_)));
    }

    #[test]
    fn handoff_accepts_valid_harness_spec_metadata() {
        let bus = EventBus::new(4);
        let harness = crate::AgentHarnessSpec::minimal_contract_first(
            "repo",
            "handoff harness",
            Some("sid-handoff"),
            Some("thread-handoff"),
            &["artifacts/out.md".to_string()],
        );
        let payload = HandoffPayload::new(AgentId(1), Some(AgentId(2)), "handoff")
            .with_a2a_context("uri://agent2")
            .with_obo_token("tok-123")
            .with_metadata(
                HARNESS_SPEC_JSON_METADATA_KEY,
                serde_json::to_string(&harness).expect("serialize harness"),
            );
        execute_handoff(&payload, &bus).expect("valid harness metadata");
    }

    #[test]
    fn execute_handoff_emits_context_metadata_fields() {
        let bus = EventBus::new(4);
        let mut rx = bus.subscribe();
        let retrieval = crate::SessionRetrievalEnvelope {
            retrieval_tier: "hybrid".to_string(),
            memory_hit_count: 1,
            knowledge_hit_count: 1,
            chunk_hit_count: 0,
            repo_hit_count: 0,
            rrf_fused_hit_count: 0,
            used_vector: true,
            used_bm25: true,
            used_lexical_fallback: false,
            contradiction_count: 0,
            source_diversity: 2,
            evidence_quality: 0.7,
            citation_coverage: 0.8,
            verification_performed: false,
            verification_reason: None,
            recommended_next_action: None,
        };
        let context =
            crate::ContextEnvelope::from_session_retrieval("repo", "sid-event", &retrieval);
        let payload = HandoffPayload::new(AgentId(1), Some(AgentId(2)), "handoff with context")
            .with_a2a_context("uri://agent2")
            .with_obo_token("tok-123")
            .with_metadata(
                CONTEXT_ENVELOPE_JSON_METADATA_KEY,
                serde_json::to_string(&context).expect("serialize context"),
            );
        execute_handoff(&payload, &bus).expect("handoff should succeed");
        let event = rx.try_recv().expect("should have event");
        match event.kind {
            AgentEventKind::PlanHandoff {
                has_context_envelope,
                has_harness_spec,
                session_id,
                thread_id,
                from_role,
                ..
            } => {
                assert!(has_context_envelope);
                assert!(!has_harness_spec);
                assert_eq!(session_id.as_deref(), Some("sid-event"));
                assert!(thread_id.is_none());
                assert!(from_role.is_none());
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn transcript_bleed_blocks_raw_transcript_in_context_notes() {
        let payload = HandoffPayload::new(AgentId(1), None, "Clean plan")
            .with_context("<|im_start|>user\nPlease fix the parser<|im_end|>")
            .with_metadata("execution_role", "builder")
            .with_pending(vec![TaskId(1)])
            .with_verification_criteria(vec!["verify".to_string()]);
        let err = validate_handoff_invariants(&payload).unwrap_err();
        assert!(matches!(err, HandoffInvariantError::TranscriptBleed { .. }));
    }

    #[test]
    fn transcript_bleed_blocks_raw_transcript_in_metadata() {
        let payload = HandoffPayload::new(AgentId(1), None, "Clean plan")
            .with_metadata("execution_role", "builder")
            .with_metadata("raw_history", "Human: fix it\nAssistant: ok")
            .with_pending(vec![TaskId(1)])
            .with_verification_criteria(vec!["verify".to_string()]);
        let err = validate_handoff_invariants(&payload).unwrap_err();
        assert!(matches!(err, HandoffInvariantError::TranscriptBleed { .. }));
    }

    #[test]
    fn transcript_bleed_allows_clean_context() {
        let payload = HandoffPayload::new(AgentId(1), None, "Fix parser")
            .with_context("The parser has 3 failing tests in descent/mod.rs")
            .with_metadata("execution_role", "builder")
            .with_pending(vec![TaskId(1)])
            .with_verification_criteria(vec!["cargo test -p vox-compiler".to_string()]);
        assert!(validate_handoff_invariants(&payload).is_ok());
    }

    #[test]
    fn execute_handoff_emits_harness_metadata_fields() {
        let bus = EventBus::new(4);
        let mut rx = bus.subscribe();
        let harness = crate::AgentHarnessSpec::minimal_contract_first(
            "repo",
            "handoff harness metadata",
            Some("sid-harness-event"),
            Some("thread-harness-event"),
            &["artifacts/out.md".to_string()],
        );
        let payload = HandoffPayload::new(AgentId(1), Some(AgentId(2)), "handoff with harness")
            .with_a2a_context("uri://agent2")
            .with_obo_token("tok-123")
            .with_metadata(
                HARNESS_SPEC_JSON_METADATA_KEY,
                serde_json::to_string(&harness).expect("serialize harness"),
            );
        execute_handoff(&payload, &bus).expect("handoff should succeed");
        let event = rx.try_recv().expect("should have event");
        match event.kind {
            AgentEventKind::PlanHandoff {
                has_context_envelope,
                has_harness_spec,
                session_id,
                thread_id,
                from_role,
                ..
            } => {
                assert!(!has_context_envelope);
                assert!(has_harness_spec);
                assert_eq!(session_id.as_deref(), Some("sid-harness-event"));
                assert_eq!(thread_id.as_deref(), Some("thread-harness-event"));
                assert!(from_role.is_none());
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
