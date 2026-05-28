//! Turso-backed Arca [`VoxDb`]: CAS (`objects`), logical `names`, and typed SQL tables.
//!
//! **Cross-crate SQL access:** the connection lives in the public field `VoxDb::conn`. For a
//! stable borrowed accessor (used by `vox-db` research paths and other crates), prefer
//! `VoxDb::connection` (see `store/ops.rs`) instead of
//! reaching for `.conn` ad hoc in new call sites.

pub mod types;

mod row_cols;

/// Default relative path for the project Arca `VoxDb` SQLite file (under the repo/working tree).
pub const DEFAULT_PROJECT_STORE_PATH: &str = ".vox/store.db";

pub use types::{
    A2AMessageRow, A2aClarificationMessageParams, AccountSecretCiphertextRow, AgentDefEntry,
    AgentEventRow, ArtifactEntry, BehaviorEventEntry, BenchmarkEventRow, BuildRunRow,
    BuilderSessionEntry, CloudDispatchRow, CodexChangeLogEntry, CommandFrequencyEntry,
    ComponentEntry, CrateSampleRow, EmbeddingEntry, EndpointReliabilityEntry, ExecutionEntry,
    ExternalStatusSnapshotParams, ExternalStatusSnapshotRow, ExternalSubmissionAttemptParams,
    ExternalSubmissionAttemptRow, ExternalSubmissionJobRow, ExternalSubmissionJobUpsertParams,
    GamifyLudusKpiRollup, GamifyPolicySnapshotListRow, KnowledgeNodeSummary, LearnedPatternEntry,
    LocalTrainRow, LogExecutionParams, LogInteractionParams, MemoryEntry, ModelScoreboardRow,
    ObservationReport, ObserverAction, PackageSearchResult, PlanNodeRow, PlanSessionRow,
    PlanVersionRow, PublicationAttemptRow, PublicationExternalLinkRow,
    PublicationExternalLinkUpsertParams, PublicationExternalRevisionRow,
    PublicationExternalRevisionUpsertParams, PublicationManifestParams, PublicationManifestRow,
    PublicationMediaAssetParams, PublicationMediaAssetRow, PublicationStatusEventRow,
    PublishArtifactParams, QuestionEventParams, QuestionEventRow, QuestionOptionOutcomeParams,
    QuestionOptionOutcomeRow, QuestionOptionParams, QuestionOptionRow, QuestionRow,
    QuestionSessionCreateParams, QuestionSessionRow, QuestionStopEventParams, QuestionStopEventRow,
    RegisterAgentParams, ReviewEntry, SaveMemoryParams, SaveSnippetParams, ScheduledEntry,
    ScholarlySubmissionRow, SessionEventRow, SessionRow, SessionTurnEntry, SkillExecutionParams,
    SkillExecutionRow, SkillManifestEntry, SkillReliabilityReport, SnippetEntry, StoreError,
    TestDecision, TestDecisionPolicy, ThroughputProfileRow, TierResult, TrainingPair,
    TrustRollupEntry, TypedStreamEventEntry, UpsertAccountSecretCiphertextParams, UserEntry,
    VictoryVerdict, VisusAuditLogRow, VisusBaselineRow, WarningRow, WorkflowExecutionRow,
};

pub use ops_build::{BuildDependencyShape, BuildHealthSummary, CrateSample, RegressionRow};
pub use ops_mens_cloud::CloudCostSummary;
pub use ops_mens_intelligence::{CorpusQualitySummary, GrpoStepRow};

mod open;
mod ops;
mod ops_a2a;
mod ops_agents;
pub mod ops_build;
mod ops_cas;
mod ops_codex;
mod ops_completion;
pub mod ops_convergence;
mod ops_developer_journeys;
pub mod ops_exec_time;
mod ops_external_intelligence;
mod ops_external_review;
mod ops_identity;
mod ops_learning;
mod ops_lineage;
mod ops_mcp_diagnostics;
mod ops_memory;
mod ops_mens_cloud;
mod ops_mens_intelligence;
mod ops_news;
pub mod ops_orchestrator;
mod ops_planning;
mod ops_publication;
mod ops_questioning;
mod ops_retention;
mod ops_finding_candidates;
mod ops_scientia;

pub use ops_finding_candidates::{
    FindingCandidateClass, FindingCandidateRow, InsertOutcome,
};
mod ops_secrets_cloudless;
mod ops_visus;
