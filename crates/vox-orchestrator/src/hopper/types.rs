//! Core domain types for the unified task hopper (Hp-T1).
//!
//! These types are forward-compatible with Option B (persistent) storage: IDs
//! are content-addressed UUIDs; item state is an explicit enum that maps
//! cleanly to `hopper_inbox.state` when the vox-db table lands in Hp-T5.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::types::{PrioritySource, TaskPriority};

// ── Identifiers ───────────────────────────────────────────────────────────────

/// Opaque identifier for a hopper intake item.
///
/// Re-exported from events so the event bus and hopper share one type.
pub use crate::events::HopperItemId;

// ── Source of an intake submission ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntakeSource {
    /// Submitted interactively by a developer (chat, CLI, dashboard).
    Developer,
    /// Submitted by an automated agent in response to an event.
    Agent,
    /// Submitted by a webhook or external integration.
    Webhook,
}

impl IntakeSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Developer => "developer",
            Self::Agent => "agent",
            Self::Webhook => "webhook",
        }
    }
}

// ── Priority hint ─────────────────────────────────────────────────────────────

/// A caller-supplied hint that the classifier uses as one input among many.
/// The classifier can ignore or override it; only a `DeveloperOverride` pins
/// the final priority unconditionally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityHint {
    Urgent,
    Normal,
    Background,
    /// No hint — let the classifier decide.
    Unspecified,
}

impl PriorityHint {
    pub fn as_task_priority(&self) -> Option<TaskPriority> {
        match self {
            Self::Urgent => Some(TaskPriority::Urgent),
            Self::Normal => Some(TaskPriority::Normal),
            Self::Background => Some(TaskPriority::Background),
            Self::Unspecified => None,
        }
    }
}

// ── Item lifecycle state ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// Waiting to be picked up by an agent.
    Inbox,
    /// Bound to an agent session, currently being worked.
    Assigned { agent_id: String },
    /// Completed — terminal state.
    Done,
    /// Overridden / cancelled — terminal state.
    Overridden,
}

impl ItemState {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Assigned { .. } => "assigned",
            Self::Done => "done",
            Self::Overridden => "overridden",
        }
    }
}

// ── Audit trail entry ─────────────────────────────────────────────────────────

/// Records every priority override for auditability (SSOT §5.7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityOverrideRecord {
    pub ts_micros: u64,
    pub actor: String,
    pub original_priority: TaskPriority,
    pub new_priority: TaskPriority,
    pub reason: String,
    /// Signed audit-log ID from `audit_log::AuditWriter` (P4-T7).
    pub audit_id: String,
}

// ── Core intake item ──────────────────────────────────────────────────────────

/// A single unit of developer intent flowing through the hopper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntakeItem {
    pub item_id: HopperItemId,
    /// Human-readable intent from the developer or agent.
    pub intent: String,
    /// Affinity hints: file paths, crate names, or agent names the item prefers.
    pub affinity_hints: Vec<String>,
    /// Caller-supplied priority hint (advisory; may be overridden by classifier).
    pub priority_hint: PriorityHint,
    /// Origin of the submission.
    pub source: IntakeSource,
    /// Optional session context (chat session, CLI session, etc.).
    pub session_id: Option<String>,
    /// Classified priority assigned by the intake classifier.
    pub classified_priority: TaskPriority,
    /// Who last set `classified_priority` (Hp-T3 typed partial order).
    ///
    /// Defaults to `Orchestrator` on initial intake. Set to `Developer` when
    /// a `DeveloperOverride` capability token is used in `reprioritize`. A
    /// `Developer`-sourced priority MUST NOT be mutated by any automated
    /// policy without a new `DeveloperOverride` cap.
    #[serde(default = "default_priority_source")]
    pub priority_source: PrioritySource,
    /// Classifier confidence 0–1.
    pub confidence: f32,
    /// Privacy class derived from context (mirrors `vox.mesh.privacy_class`).
    pub privacy_class: String,
    /// Current lifecycle state.
    pub state: ItemState,
    /// Unix micros when this item was submitted.
    pub submitted_at: u64,
    /// Full override history (each `DeveloperOverride` appends here).
    pub override_history: Vec<PriorityOverrideRecord>,
}

fn default_priority_source() -> PrioritySource {
    PrioritySource::Orchestrator
}

impl IntakeItem {
    pub fn new(
        intent: String,
        affinity_hints: Vec<String>,
        priority_hint: PriorityHint,
        source: IntakeSource,
        session_id: Option<String>,
    ) -> Self {
        let classified = priority_hint
            .as_task_priority()
            .unwrap_or(TaskPriority::Normal);

        Self {
            item_id: HopperItemId(uuid::Uuid::new_v4().simple().to_string()),
            intent,
            affinity_hints,
            priority_hint,
            source,
            session_id,
            classified_priority: classified,
            priority_source: PrioritySource::Orchestrator,
            confidence: 0.85,
            privacy_class: "local-only".into(),
            state: ItemState::Inbox,
            submitted_at: now_micros(),
            override_history: vec![],
        }
    }
}

pub fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}
