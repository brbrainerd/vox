//! Stack manifest types and planner handle.

use serde::{Deserialize, Serialize};

/// Configuration for the Stack submit planner.
#[derive(Debug, Clone)]
pub struct StackPlanConfig {
    /// Max files per PR (default: 50, to keep CodeRabbit closely focused per chunk).
    pub max_files_per_pr: u32,
}

impl Default for StackPlanConfig {
    fn default() -> Self {
        Self {
            max_files_per_pr: 50,
        }
    }
}

/// A semantic grouping layer (e.g., scaffolds first, traits next, endpoints last).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackChunk {
    pub order: u32,
    pub name: String,
    pub files: Vec<String>,
}

/// The final generated artifact holding the plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackManifest {
    pub generated_at: String,
    pub chunks: Vec<StackChunk>,
    pub total_files: usize,
}

/// Generates stacked PR manifests for CodeRabbit-friendly code review.
///
/// Partitions a repository's git-tracked files into semantically ordered
/// chunks that stay within CodeRabbit's per-PR file limit.
pub struct StackPlanner {
    pub(crate) config: StackPlanConfig,
}
