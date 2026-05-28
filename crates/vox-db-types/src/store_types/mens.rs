//! MENS and GRPO training types for `vox_db::VoxDb`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A report on an agent interaction observed by the `Observer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationReport {
    pub session_id: crate::ids::DbSessionId,
    pub task_id: crate::ids::DbTaskId,
    pub observed_at: DateTime<Utc>,
    pub file_path: String,
    pub lsp_error_count: usize,
    pub parse_rate: f32,
    pub construct_coverage: f32,
    pub recommended_action: ObserverAction,
    pub metadata_json: Option<String>,
}

/// Action recommended by the observer based on language invariants.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ObserverAction {
    Continue,
    RequestMoreEvidence,
    TriggerReplan,
    EscalateToHuman,
    EmitNegativeExample,
}

/// A decision made by the testing policy for a task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TestDecision {
    Required,
    Recommended,
    Optional,
    Deferred,
    Skip,
}

/// The policy used to decide if and what testing is required.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDecisionPolicy {
    pub complexity_threshold: u8,
    pub file_count_threshold: usize,
    pub risk_multiplier: f32,
    pub keyword_triggers: Vec<String>,
}

/// The multi-tier victory condition to evaluate against.
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VictoryCondition {
    #[default]
    CompilationOnly,
    WithDocTests,
    WithUnitTests,
    WithCorpusValidation,
    Full,
}

impl Default for TestDecisionPolicy {
    fn default() -> Self {
        Self {
            complexity_threshold: 7,
            file_count_threshold: 2,
            risk_multiplier: 1.0,
            keyword_triggers: vec!["auth".into(), "security".into(), "schema".into()],
        }
    }
}

/// A multi-tier victory condition verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VictoryVerdict {
    pub task_id: crate::ids::DbTaskId,
    pub passed: bool,
    pub tiers_run: Vec<String>,
    pub first_failure: Option<String>,
    pub report: String,
    pub created_at_ms: i64,
}

/// Result of a single victory tier check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierResult {
    pub tier: String,
    pub passed: bool,
    pub diagnostic: String,
}
