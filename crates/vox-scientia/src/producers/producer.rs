//! The `Producer` trait — a deterministic detector that observes some
//! workspace surface and emits `ResearchEvent` records.

use async_trait::async_trait;
use vox_research_events::ResearchEvent;

/// Inputs threaded through every producer on each invocation.
///
/// Carries the repository root, configured time/commit windows, a clock for
/// deterministic tests, and a session id to tag emitted events.
#[derive(Debug, Clone)]
pub struct ProducerContext {
    /// Repository root used by [`CommitGraphProducer`](crate::commit_graph).
    pub repo_root: std::path::PathBuf,
    /// How many recent commits to scan (commit-graph window).
    pub commit_window: usize,
    /// How many days of activity to scan (time-window producers).
    pub days_window: u32,
    /// Logical "now" in epoch milliseconds; tests inject a fixed value.
    pub now_ms: i64,
    /// Stable session id used in event payloads.
    pub session_id: String,
    /// Optional repository id (mirrored into emitted candidates by the
    /// caller; producers themselves only emit `ResearchEvent`s).
    pub repository_id: Option<String>,
}

impl ProducerContext {
    /// Deterministic context for unit / integration tests.
    pub fn for_test() -> Self {
        Self {
            repo_root: std::env::temp_dir(),
            commit_window: 10,
            days_window: 30,
            now_ms: 1_747_000_000_000,
            session_id: "test-session".into(),
            repository_id: Some("test-repo".into()),
        }
    }
}

/// Deterministic detector. Implementations MUST NOT call an LLM — that lives
/// downstream in the existing claim extractor.
#[async_trait]
pub trait Producer: Send + Sync {
    /// Stable producer name; persisted as `producer_name` in
    /// `scientia_finding_candidates`.
    fn name(&self) -> &'static str;

    /// Observe the configured surface and return zero or more
    /// `ResearchEvent::FindingCandidateProposed` events.
    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent>;
}
