use serde::Serialize;
use serde::de::DeserializeOwned;
use vox_db::*;
use vox_db_types::store_types::*;

fn assert_serde<T: Serialize + DeserializeOwned>() {}

#[test]
fn all_row_types_implement_serde() {
    // exec_time.rs
    assert_serde::<ToolLatencyProfile>();

    // store_types/mens.rs
    assert_serde::<ObservationReport>();
    assert_serde::<TierResult>();

    // store_types/build.rs
    assert_serde::<BuildHealthSummary>();
    assert_serde::<RegressionRow>();

    // store_types/research.rs
    assert_serde::<ResearchIngestResult>();

    // store_types/rows_core.rs
    assert_serde::<ExecutionEntry>();
    assert_serde::<ScheduledEntry>();
    assert_serde::<ComponentEntry>();
    assert_serde::<MemoryEntry>();
    assert_serde::<EmbeddingEntry>();
    assert_serde::<LearnedPatternEntry>();
    assert_serde::<BehaviorEventEntry>();
    assert_serde::<CommandFrequencyEntry>();
    assert_serde::<TrainingPair>();
    assert_serde::<UserEntry>();
    assert_serde::<AgentDefEntry>();
    assert_serde::<SnippetEntry>();
    assert_serde::<PackageSearchResult>();
    assert_serde::<ArtifactEntry>();
    assert_serde::<SkillManifestEntry>();
    assert_serde::<KnowledgeNodeSummary>();
    assert_serde::<BuilderSessionEntry>();
    assert_serde::<SessionTurnEntry>();
    assert_serde::<TypedStreamEventEntry>();
    assert_serde::<ReviewEntry>();
    assert_serde::<CodexChangeLogEntry>();
    assert_serde::<NodeIdentityRow>();
    assert_serde::<ModelScoreboardRow>();
    assert_serde::<ModelPricingCatalogRow>();

    // store_types/rows_extended.rs
    assert_serde::<SkillReliabilityReport>();
    assert_serde::<EndpointReliabilityEntry>();
    assert_serde::<TrustRollupEntry>();
    assert_serde::<CorpusRow>();
    assert_serde::<SkillExecutionRow>();
    assert_serde::<WorkflowExecutionRow>();
    assert_serde::<QuestionRow>();
    assert_serde::<QuestionSessionRow>();
    assert_serde::<QuestionEventRow>();
    assert_serde::<QuestionOptionRow>();
    assert_serde::<QuestionOptionOutcomeRow>();
    assert_serde::<QuestionStopEventRow>();
    assert_serde::<A2AMessageRow>();
    assert_serde::<AgentEventRow>();
    assert_serde::<BenchmarkEventRow>();
    assert_serde::<SessionRow>();
    assert_serde::<SessionEventRow>();
    assert_serde::<BuildRunRow>();
    assert_serde::<CrateSampleRow>();
    assert_serde::<CloudDispatchRow>();
    assert_serde::<ThroughputProfileRow>();
    assert_serde::<PublicationManifestRow>();
    assert_serde::<ScholarlySubmissionRow>();
    assert_serde::<PublicationMediaAssetRow>();
    assert_serde::<PublicationAttemptRow>();
    assert_serde::<PublicationStatusEventRow>();
    assert_serde::<ExternalSubmissionJobRow>();
    assert_serde::<ExternalSubmissionAttemptRow>();
    assert_serde::<ExternalStatusSnapshotRow>();
    assert_serde::<PublicationExternalLinkRow>();
    assert_serde::<PublicationExternalRevisionRow>();
    assert_serde::<LocalTrainRow>();
    assert_serde::<WarningRow>();
    assert_serde::<PlanSessionRow>();
    assert_serde::<PlanVersionRow>();
    assert_serde::<PlanNodeRow>();
    assert_serde::<PlanNodeAttemptRow>();
    assert_serde::<GamifyPolicySnapshotListRow>();
    assert_serde::<GamifyLudusKpiRollup>();
    assert_serde::<AccountSecretCiphertextRow>();
    assert_serde::<ExternalReviewRunRow>();
    assert_serde::<ExternalReviewFindingRow>();
    assert_serde::<ExternalReviewDeadletterRow>();
    assert_serde::<ExternalReviewKpiSnapshotRow>();
    assert_serde::<VisusBaselineRow>();
    assert_serde::<VisusAuditLogRow>();
}
