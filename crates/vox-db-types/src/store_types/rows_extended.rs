/// A report on the reliability of a given skill.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillReliabilityReport {
    pub skill_id: String,
    pub total: u64,
    pub success_rate: f64,
    pub median_duration_ms: i64,
    pub p99_duration_ms: i64,
    pub last_error_kind: Option<String>,
}

/// One row from `endpoint_reliability`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EndpointReliabilityEntry {
    pub endpoint_url: String,
    pub model_id: String,
    pub total_requests: u64,
    pub hallucination_proxy_ewma: f64,
    pub contradiction_ratio_ewma: f64,
    pub infra_failure_ewma: f64,
    pub rate_limit_hits: u64,
    pub timeout_hits: u64,
    pub updated_at_ms: i64,
}

/// One row from `trust_rollups`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrustRollupEntry {
    pub entity_type: String,
    pub entity_id: String,
    pub dimension: String,
    pub domain: String,
    pub task_class: String,
    pub provider: String,
    pub model_id: String,
    pub repository_id: String,
    pub score: f64,
    pub sample_size: u64,
    pub ewma_alpha: f64,
    pub updated_at_ms: i64,
}

/// A single training data row exported from the database for Mens.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CorpusRow {
    pub category: String,
    pub language: String,
    pub source_id: String,
    pub instruction: String,
    pub output: String,
    pub quality_score: f64,
    pub tags: Vec<String>,
}

impl CorpusRow {
    /// Serialize the row to a plain JSON line for streaming.
    pub fn to_jsonl(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}
/// A single row from `skill_executions` — DB DTO (never used in business logic).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillExecutionRow {
    /// Primary key of this execution row.
    pub id: i64,
    /// Skill identifier (foreign key to `skill_manifests`).
    pub skill_id: String,
    /// Version string of the skill at execution time.
    pub version: String,
    /// MCP/agent session that triggered the execution, if any.
    pub session_id: Option<crate::ids::DbSessionId>,
    /// Workflow execution identifier, if any.
    pub workflow_id: Option<String>,
    /// Agent that ran the skill.
    pub agent_id: Option<crate::ids::DbAgentId>,
    /// `"ok"` | `"error"` | `"timeout"`.
    pub status: String,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: Option<i64>,
    /// SHA3 hash of the input payload (for deduplication).
    pub input_hash: Option<String>,
    /// Byte size of the output payload.
    pub output_size: Option<i64>,
    /// Short error category when status != `"ok"`.
    pub error_kind: Option<String>,
    /// Socrates reflection score 0..=100, if evaluated.
    pub reflection_score: Option<f64>,
    /// ISO-8601 creation timestamp (UTC).
    pub created_at: String,
}

/// A single row from `workflow_executions` — DB DTO.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowExecutionRow {
    /// Stable workflow identifier.
    pub workflow_id: String,
    /// `"running"` | `"ok"` | `"error"`.
    pub status: String,
    /// Number of planned steps in this execution.
    pub step_count: i64,
    /// Number of steps that produced errors.
    pub error_count: i64,
    /// ISO-8601 start timestamp.
    pub started_at: String,
    /// ISO-8601 finish timestamp; `None` when still running.
    pub finished_at: Option<String>,
}

/// A single row from `agent_questions`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionRow {
    pub id: i64,
    pub asker_id: String,
    pub responder_id: String,
    pub question_text: String,
    pub answer_text: Option<String>,
    pub status: String,
    pub created_at_ms: i64,
}

/// One row from `question_sessions`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionSessionRow {
    pub id: i64,
    pub session_id: crate::ids::DbSessionId,
    pub repository_id: String,
    pub task_id: Option<crate::ids::DbTaskId>,
    pub policy_version: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub resolution_status: String,
    pub belief_state_json: Option<String>,
}

/// One row from `question_events`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionEventRow {
    pub id: i64,
    pub question_session_id: i64,
    pub question_id: String,
    pub turn_index: i64,
    pub actor: String,
    pub question_kind: String,
    pub prompt: String,
    pub expected_information_gain_bits: f64,
    pub expected_user_cost: f64,
    pub utility_bits_per_cost: f64,
    pub answer_text: Option<String>,
    pub answer_type: Option<String>,
    pub answered_at_ms: Option<i64>,
    pub created_at_ms: i64,
}

/// One row from `question_options`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionOptionRow {
    pub id: i64,
    pub question_event_id: i64,
    pub option_id: String,
    pub label: String,
    pub prior_probability: Option<f64>,
    pub posterior_probability: Option<f64>,
    pub is_other: bool,
}

/// One row from `question_option_outcomes`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionOptionOutcomeRow {
    pub id: i64,
    pub question_event_id: i64,
    pub option_id: String,
    pub selected: bool,
    pub diagnostic_weight: f64,
    pub information_contribution_bits: f64,
    pub created_at_ms: i64,
}

/// One row from `question_stop_events`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionStopEventRow {
    pub id: i64,
    pub question_session_id: i64,
    pub stop_reason: String,
    pub confidence_at_stop: Option<f64>,
    pub marginal_gain_bits: Option<f64>,
    pub expected_user_cost: Option<f64>,
    pub turn_index: Option<i64>,
    pub created_at_ms: i64,
}

/// One unacknowledged row from `a2a_messages` as returned by `vox_db::VoxDb::poll_a2a_inbox`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct A2AMessageRow {
    pub id: i64,
    pub message_uuid: String,
    pub sender_agent: String,
    pub receiver_agent: String,
    pub msg_type: String,
    pub payload: String,
    pub priority: i64,
    pub thread_id: Option<String>,
    pub acknowledged: bool,
    pub created_at: String,
    pub repository_id: String,
    pub claim_owner: Option<String>,
    pub claim_until_ms: Option<i64>,
    pub delivery_attempts: i64,
    pub last_claim_error: Option<String>,
    pub processed_at_ms: Option<i64>,
}

/// A single row from `agent_events` (telemetry / gamify audit).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentEventRow {
    pub id: i64,
    pub agent_id: crate::ids::DbAgentId,
    pub event_type: String,
    pub payload_json: Option<String>,
    pub cli_version: Option<String>,
    pub timestamp: String,
}

/// A single row from `benchmark_events`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BenchmarkEventRow {
    pub id: i64,
    pub benchmark_id: String,
    pub model_id: String,
    pub score: f64,
    pub latency_ms: i64,
    pub metadata_json: String,
    pub created_at_ms: i64,
}

/// Row shape for `agent_sessions` (MCP/orchestrator session lifecycle in Codex).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionRow {
    pub id: String,
    pub agent_id: crate::ids::DbAgentId,
    pub agent_name: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub task_snapshot: Option<String>,
    pub context_summary: Option<String>,
}

/// A single row from `agent_session_events`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionEventRow {
    pub id: i64,
    pub session_id: crate::ids::DbSessionId,
    pub event_type: String,
    pub payload_json: String,
    pub created_at_ms: i64,
}

/// A single row from `build_runs`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildRunRow {
    pub id: i64,
    pub build_id: String,
    pub repository_id: String,
    pub status: String,
    pub duration_ms: i64,
    pub metadata_json: String,
    pub created_at_ms: i64,
}

/// A single row from `build_crate_samples`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrateSampleRow {
    pub id: i64,
    pub build_id: String,
    pub crate_name: String,
    pub duration_ms: i64,
    pub status: String,
    pub created_at_ms: i64,
}

/// A single row from `cloud_dispatch_log`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CloudDispatchRow {
    pub id: i64,
    pub provider: String,
    pub endpoint_type: String,
    pub status: String,
    pub duration_ms: i64,
    pub metadata_json: String,
    pub created_at_ms: i64,
}

/// A single row from `model_throughput_profiles`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThroughputProfileRow {
    pub id: i64,
    pub model_id: String,
    pub hardware_id: String,
    pub tokens_per_sec: f64,
    pub latency_p50_ms: i64,
    pub updated_at_ms: i64,
}

/// One row from `publication_manifests`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PublicationManifestRow {
    pub publication_id: String,
    pub content_type: String,
    pub source_ref: Option<String>,
    pub title: String,
    pub author: String,
    pub abstract_text: Option<String>,
    pub body_markdown: String,
    pub citations_json: Option<String>,
    pub metadata_json: Option<String>,
    pub revision_history_json: Option<String>,
    pub content_sha3_256: String,
    pub version: i64,
    pub state: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// One row from `scholarly_submissions`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScholarlySubmissionRow {
    pub id: i64,
    pub publication_id: String,
    pub content_sha3_256: String,
    pub adapter: String,
    pub external_submission_id: String,
    pub status: String,
    pub submitted_at_ms: i64,
    pub updated_at_ms: i64,
    pub response_fingerprint: Option<String>,
    pub metadata_json: Option<String>,
}

/// One row from `publication_media_assets`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PublicationMediaAssetRow {
    pub id: i64,
    pub publication_id: String,
    pub asset_ref: String,
    pub media_type: String,
    pub storage_uri: Option<String>,
    pub status: String,
    pub metadata_json: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// One row from `publication_attempts`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PublicationAttemptRow {
    pub id: i64,
    pub publication_id: String,
    pub content_sha3_256: String,
    pub channel: String,
    pub attempted_at_ms: i64,
    pub outcome_json: String,
}

/// One row from `publication_status_events`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PublicationStatusEventRow {
    pub id: i64,
    pub publication_id: String,
    pub status: String,
    pub detail_json: Option<String>,
    pub recorded_at_ms: i64,
}

/// One row from `external_submission_jobs`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalSubmissionJobRow {
    pub id: i64,
    pub publication_id: String,
    pub content_sha3_256: String,
    pub adapter: String,
    pub operation: String,
    pub idempotency_key: String,
    pub status: String,
    pub lock_owner: Option<String>,
    pub lock_expires_at_ms: Option<i64>,
    pub next_retry_at_ms: Option<i64>,
    pub attempt_count: i64,
    pub last_error_class: Option<String>,
    pub last_error_message: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// One row from `external_submission_attempts`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalSubmissionAttemptRow {
    pub id: i64,
    pub job_id: i64,
    pub attempted_at_ms: i64,
    pub http_status: Option<i64>,
    pub error_class: Option<String>,
    pub retryable: bool,
    pub request_fingerprint: Option<String>,
    pub response_fingerprint: Option<String>,
    pub detail_json: Option<String>,
}

/// One row from `external_status_snapshots`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalStatusSnapshotRow {
    pub id: i64,
    pub adapter: String,
    pub external_submission_id: String,
    pub publication_id: String,
    pub content_sha3_256: String,
    pub snapshot_json: String,
    pub fetched_at_ms: i64,
}

/// One row from `publication_external_links`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PublicationExternalLinkRow {
    pub id: i64,
    pub publication_id: String,
    pub content_sha3_256: String,
    pub adapter: String,
    pub link_kind: String,
    pub link_value: String,
    pub metadata_json: Option<String>,
    pub created_at_ms: i64,
}

/// One row from `publication_external_revisions`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PublicationExternalRevisionRow {
    pub id: i64,
    pub publication_id: String,
    pub content_sha3_256: String,
    pub adapter: String,
    pub external_revision: String,
    pub metadata_json: Option<String>,
    pub updated_at_ms: i64,
}

/// A single row from `local_train_log`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LocalTrainRow {
    pub id: i64,
    pub run_id: String,
    pub model_id: String,
    pub dataset_id: String,
    pub status: String,
    pub loss: Option<f64>,
    pub step: i64,
    pub created_at_ms: i64,
}

/// A single row from `arca_warnings`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WarningRow {
    pub id: i64,
    pub warning_type: String,
    pub message: String,
    pub metadata_json: String,
    pub created_at_ms: i64,
}

/// One row from `plan_sessions`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanSessionRow {
    pub plan_session_id: crate::ids::DbPlanSessionId,
    pub origin_session_id: Option<String>,
    pub goal_text: String,
    pub strategy: String,
    pub current_version: i64,
    pub status: String,
}

/// One row from `plan_versions`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanVersionRow {
    pub plan_session_id: crate::ids::DbPlanSessionId,
    pub version: i64,
    pub parent_version: Option<i64>,
    pub trigger_event: Option<String>,
    pub trigger_payload_json: Option<String>,
    pub quality_score: f64,
    pub reviewer_verdict: String,
}

/// One row from `plan_nodes`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanNodeRow {
    pub plan_session_id: crate::ids::DbPlanSessionId,
    pub version: i64,
    pub node_id: String,
    pub description: String,
    pub dependencies_json: String,
    pub execution_policy_json: String,
    pub status: String,
    pub workflow_invocation: Option<String>,
}

/// One row from `plan_node_attempts`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanNodeAttemptRow {
    pub plan_session_id: crate::ids::DbPlanSessionId,
    pub version: i64,
    pub node_id: String,
    pub attempt_no: i64,
    pub task_id: Option<crate::ids::DbTaskId>,
    pub outcome: String,
    pub error_text: Option<String>,
    pub latency_ms: Option<i64>,
    pub created_at: String,
}

/// One row from `gamify_policy_snapshots` for Ludus KPI / audit lists.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GamifyPolicySnapshotListRow {
    /// Event type label stored on the snapshot.
    pub event_type: String,
    /// Base XP before multipliers.
    pub base_xp: i64,
    /// Base crystals before multipliers.
    pub base_crystals: i64,
    /// Mode / policy bucket label.
    pub mode_label: String,
    /// Effective multiplier applied.
    pub effective_multiplier: f64,
    /// Final awarded XP.
    pub awarded_xp: i64,
    /// Final awarded crystals.
    pub awarded_crystals: i64,
    /// SQLite truthy grind-cap flag.
    pub grind_capped: i64,
    /// Lumens component if present.
    pub lumens: i64,
    /// Optional metadata / recognition context.
    pub metadata: Option<String>,
    /// `created_at` column as stored (string timestamp).
    pub created_at: String,
}

/// Aggregated Ludus-facing KPI counters for one user (`load_kpi_summary`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GamifyLudusKpiRollup {
    /// Policy snapshot rows for this user.
    pub events_recorded: i64,
    /// Sum of awarded XP across policy snapshots.
    pub total_xp_awarded: i64,
    /// Sum of awarded crystals across policy snapshots.
    pub total_crystals_awarded: i64,
    /// Count of grind-capped snapshots.
    pub grind_capped_events: i64,
    /// Average effective multiplier across snapshots.
    pub avg_effective_multiplier: f64,
    /// Total hint telemetry rows.
    pub hint_events_logged: i64,
    /// Completed quest rows for this user.
    pub quests_completed_total: i64,
    /// Unread notifications.
    pub notifications_unread: i64,
    /// Hint rows with `action = 'shown'`.
    pub hints_shown: i64,
    /// Hint rows with `action = 'dismissed'`.
    pub hints_dismissed: i64,
}

/// One row from `clavis_account_secrets` for Secrets Cloudless encrypted secret material.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountSecretCiphertextRow {
    pub account_id: String,
    pub secret_id: String,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub cipher_version: i64,
    pub dek_wrapped: Vec<u8>,
    pub dek_wrap_alg: String,
    pub kek_ref: String,
    pub kek_version: i64,
    pub aad_hash: Option<String>,
    pub updated_at_ms: i64,
    pub rotation_epoch: i64,
    pub rotated_at_ms: Option<i64>,
    pub consistency_origin: String,
    pub consistency_version: i64,
    pub last_synced_at_ms: Option<i64>,
    pub checksum_blake3: String,
}

/// One row from `external_review_run`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalReviewRunRow {
    pub id: i64,
    pub provider: String,
    pub repository_id: String,
    pub owner: String,
    pub repo: String,
    pub pr_number: i64,
    pub commit_sha: Option<String>,
    pub trigger_kind: String,
    pub idempotency_key: Option<String>,
    pub item_count: i64,
    pub started_at: String,
    pub finished_at: String,
    pub metadata_json: Option<String>,
}

/// One row from `external_review_finding`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalReviewFindingRow {
    pub id: i64,
    pub run_id: i64,
    pub provider: String,
    pub repository_id: String,
    pub pr_number: i64,
    pub finding_identity: String,
    pub thread_identity: Option<String>,
    pub source_comment_id: Option<i64>,
    pub placement_kind: String,
    pub line_anchor_state: String,
    pub file_path: Option<String>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub category: String,
    pub anti_pattern_id: Option<String>,
    pub severity: String,
    pub title: String,
    pub details: String,
    pub suggested_fix: Option<String>,
    pub extraction_confidence: Option<f64>,
    pub source_payload_hash: String,
    pub fingerprint: String,
    pub status: String,
}

/// One row from `external_review_deadletter`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalReviewDeadletterRow {
    pub id: i64,
    pub provider: String,
    pub repository_id: String,
    pub pr_number: i64,
    pub source_kind: String,
    pub source_comment_id: Option<i64>,
    pub source_payload_hash: String,
    pub error_class: String,
    pub error_message: Option<String>,
    pub raw_payload_json: String,
    pub retry_state: String,
    pub created_at: String,
    pub retried_at: Option<String>,
}

/// One row from `external_review_kpi_snapshot`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalReviewKpiSnapshotRow {
    pub id: i64,
    pub repository_id: String,
    pub period_start: String,
    pub period_end: String,
    pub coverage_ratio: Option<f64>,
    pub ingest_to_fix_latency_ms: Option<f64>,
    pub repeated_finding_rate: Option<f64>,
    pub post_training_regression_rate: Option<f64>,
    pub auto_fix_acceptance_rate: Option<f64>,
    pub created_at: String,
}

/// One row from `visus_baselines`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VisusBaselineRow {
    pub id: String,
    pub target_url: String,
    pub viewport: String,
    pub theme: String,
    pub screenshot_cas: String,
    pub ax_tree_cas: String,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

/// One row from `visus_audit_log`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VisusAuditLogRow {
    pub id: String,
    pub baseline_id: Option<String>,
    pub target_url: String,
    pub outcome: String,
    pub findings_json: String,
    pub screenshot_cas: Option<String>,
    pub created_at: String,
}
