//! Request parameters and row shapes for `vox_db::VoxDb` (Arca on Turso).
//!
//! **SSOT:** Schema versions live under `vox_db::schema`. Public Codex naming and env vars are
//! defined in ADR 004 and the repo-root `AGENTS.md`—do not restate deployment semantics on
//! every struct; link those docs when adding new persisted fields.
//!
//! **Pattern:** Parameter structs that mirror a `VoxDb::…` method include a rustdoc link to that
//! method so call sites stay discoverable.

/// Parameters for logging an execution (workflow / activity).
pub struct LogExecutionParams<'a> {
    /// Workflow identifier.
    pub workflow_id: &'a str,
    /// Owning agent id.
    pub agent_id: Option<&'a str>,
    /// Skill identifier.
    pub skill_id: Option<&'a str>,
    /// Activity or step name.
    pub activity_name: &'a str,
    /// Outcome label (e.g. `ok`, `error`).
    pub status: &'a str,
    /// Attempt number (1-based).
    pub attempt: u32,
    /// Wall-clock latency in milliseconds.
    pub duration_ms: i64,
    /// Output payload size in bytes.
    pub output_size: i64,
    /// Serialized input payload, if any.
    pub input: Option<&'a [u8]>,
    /// Serialized output payload, if any.
    pub output: Option<&'a [u8]>,
    /// Error message, if failed.
    pub error: Option<&'a str>,
    /// Optional JSON or opaque options string.
    pub options: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::save_memory`.
#[derive(Debug, Clone)]
pub struct SaveMemoryParams<'a> {
    /// Owning agent id.
    pub agent_id: &'a str,
    /// Conversation or session id.
    pub session_id: &'a str,
    /// Memory category / type label.
    pub memory_type: &'a str,
    /// Stored text content.
    pub content: &'a str,
    /// Optional JSON metadata.
    pub metadata: Option<&'a str>,
    /// Relative importance score.
    pub importance: f64,
    /// Optional VCS snapshot correlation id.
    pub vcs_snapshot_id: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::log_interaction`.
#[derive(Debug, Clone)]
pub struct LogInteractionParams<'a> {
    /// Session id for the interaction.
    pub session_id: &'a str,
    /// End-user id when known.
    pub user_id: Option<&'a str>,
    /// User prompt text.
    pub prompt: &'a str,
    /// Model response text.
    pub response: &'a str,
    /// Model identifier or version string.
    pub model_version: &'a str,
    /// Wall-clock latency in milliseconds, if recorded.
    pub latency_ms: Option<i64>,
    /// Token usage, if recorded.
    pub token_count: Option<i64>,
}

/// Parameters for `vox_db::VoxDb::record_llm_outcome`.
#[derive(Debug, Clone)]
pub struct ModelOutcome<'a> {
    pub session_id: &'a str,
    pub user_id: Option<&'a str>,
    pub tenant_id: Option<&'a str>,
    pub prompt: &'a str,
    pub response: &'a str,
    pub model_id: &'a str,
    pub provider: &'a str,
    pub task_category: &'a str,
    pub strength_tag: &'a str,
    pub latency_ms: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub trace_id: Option<&'a str>,
    pub context_utilization_pct: Option<f64>,
    pub success: bool,
    pub cost_usd: Option<f64>,
    pub quality_score: Option<f64>,
}

/// A report on a single LLM request attempt (success or failure).
#[derive(Debug, Clone)]
pub struct ModelAttempt<'a> {
    pub trace_id: &'a str,
    pub attempt_number: i32,
    pub model_id: &'a str,
    pub provider: &'a str,
    pub outcome: &'a str,
    pub latency_ms: Option<i64>,
    pub error_class: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::save_snippet`.
#[derive(Debug, Clone)]
pub struct SaveSnippetParams<'a> {
    /// Programming language tag.
    pub language: &'a str,
    /// Short title.
    pub title: &'a str,
    /// Source code body.
    pub code: &'a str,
    /// Optional description.
    pub description: Option<&'a str>,
    /// Optional comma-separated or JSON tags.
    pub tags: Option<&'a str>,
    /// Author user id, if any.
    pub author_id: Option<&'a str>,
    /// External reference (URL, path, etc.).
    pub source_ref: Option<&'a str>,
    /// Reserved for future embedding linkage.
    pub embedding_ref: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::publish_artifact`.
#[derive(Debug, Clone)]
pub struct PublishArtifactParams<'a> {
    /// Stable artifact id.
    pub id: &'a str,
    /// Type discriminator (e.g. model, dataset).
    pub artifact_type: &'a str,
    /// Human-readable name.
    pub name: &'a str,
    /// Optional description.
    pub description: Option<&'a str>,
    /// Publishing user id.
    pub author_id: &'a str,
    /// CAS hash of artifact bytes.
    pub content_hash: &'a str,
    /// Semantic version string.
    pub version: &'a str,
    /// Optional tags string.
    pub tags: Option<&'a str>,
    /// Lifecycle status (e.g. draft, published).
    pub status: &'a str,
}

/// Parameters for `vox_db::VoxDb::upsert_publication_manifest`.
#[derive(Debug, Clone)]
pub struct PublicationManifestParams<'a> {
    /// Stable publication id (e.g. news id or scientia artifact id).
    pub publication_id: &'a str,
    /// Logical content type (`news`, `scientia`, `paper`, ...).
    pub content_type: &'a str,
    /// Optional upstream source reference (URL, repo path, DOI, etc.).
    pub source_ref: Option<&'a str>,
    /// Human title.
    pub title: &'a str,
    /// Author name/identifier string.
    pub author: &'a str,
    /// Optional abstract.
    pub abstract_text: Option<&'a str>,
    /// Canonical markdown body.
    pub body_markdown: &'a str,
    /// Optional serialized citations payload.
    pub citations_json: Option<&'a str>,
    /// Optional serialized metadata payload.
    pub metadata_json: Option<&'a str>,
    pub revision_history_json: Option<&'a str>,
    /// Immutable digest for approval and publication checks.
    pub content_sha3_256: &'a str,
    /// Lifecycle status (`draft`, `approved`, `submitted`, `published`, ...).
    pub state: &'a str,
}

/// Parameters for `vox_db::VoxDb::upsert_publication_media_asset`.
#[derive(Debug, Clone)]
pub struct PublicationMediaAssetParams<'a> {
    /// Stable publication id owning this asset.
    pub publication_id: &'a str,
    /// Repository/local asset reference key.
    pub asset_ref: &'a str,
    /// Media type (`video`, `image`, `dataset`, ...).
    pub media_type: &'a str,
    /// Optional storage URI (local path, object store URL, external id).
    pub storage_uri: Option<&'a str>,
    /// Lifecycle status (`pending`, `uploaded`, `failed`, ...).
    pub status: &'a str,
    /// Optional JSON metadata blob.
    pub metadata_json: Option<&'a str>,
}

/// Parameters for inserting or updating one `external_submission_jobs` row by idempotency key.
#[derive(Debug, Clone)]
pub struct ExternalSubmissionJobUpsertParams<'a> {
    pub publication_id: &'a str,
    pub content_sha3_256: &'a str,
    pub adapter: &'a str,
    pub operation: &'a str,
    pub idempotency_key: &'a str,
    pub status: &'a str,
    pub lock_owner: Option<&'a str>,
    pub lock_expires_at_ms: Option<i64>,
    pub next_retry_at_ms: Option<i64>,
    pub attempt_count: i64,
    pub last_error_class: Option<&'a str>,
    pub last_error_message: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::record_external_submission_attempt`.
#[derive(Debug, Clone)]
pub struct ExternalSubmissionAttemptParams<'a> {
    pub job_id: i64,
    pub http_status: Option<i32>,
    pub error_class: Option<&'a str>,
    pub retryable: bool,
    pub request_fingerprint: Option<&'a str>,
    pub response_fingerprint: Option<&'a str>,
    pub detail_json: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::insert_external_status_snapshot`.
#[derive(Debug, Clone)]
pub struct ExternalStatusSnapshotParams<'a> {
    pub adapter: &'a str,
    pub external_submission_id: &'a str,
    pub publication_id: &'a str,
    pub content_sha3_256: &'a str,
    pub snapshot_json: &'a str,
}

/// Parameters for `vox_db::VoxDb::upsert_publication_external_link`.
#[derive(Debug, Clone)]
pub struct PublicationExternalLinkUpsertParams<'a> {
    pub publication_id: &'a str,
    pub content_sha3_256: &'a str,
    pub adapter: &'a str,
    pub link_kind: &'a str,
    pub link_value: &'a str,
    pub metadata_json: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::upsert_publication_external_revision`.
#[derive(Debug, Clone)]
pub struct PublicationExternalRevisionUpsertParams<'a> {
    pub publication_id: &'a str,
    pub content_sha3_256: &'a str,
    pub adapter: &'a str,
    pub external_revision: &'a str,
    pub metadata_json: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::register_agent`.
#[derive(Debug, Clone)]
pub struct RegisterAgentParams<'a> {
    /// Agent id (primary key).
    pub id: &'a str,
    /// Display name.
    pub name: &'a str,
    /// Optional description.
    pub description: Option<&'a str>,
    /// System prompt text, if any.
    pub system_prompt: Option<&'a str>,
    /// JSON tool manifest or config, if any.
    pub tools: Option<&'a str>,
    /// JSON model configuration, if any.
    pub model_config: Option<&'a str>,
    /// Owning user id, if any.
    pub owner_id: Option<&'a str>,
    /// Agent definition version.
    pub version: &'a str,
    /// Whether the agent is visible to other users.
    pub is_public: bool,
}
/// Parameters for `record_skill_execution`.
#[derive(Debug, Clone)]
pub struct SkillExecutionParams<'a> {
    pub skill_id: &'a str,
    pub version: &'a str,
    pub session_id: Option<&'a str>,
    pub workflow_id: Option<&'a str>,
    pub agent_id: Option<&'a str>,
    pub status: &'a str,
    pub duration_ms: i64,
    pub input_hash: Option<&'a str>,
    pub output_size: i64,
    pub error_kind: Option<&'a str>,
    pub reflection_score: Option<f64>,
}

/// Parameters for creating a row in `question_sessions`.
#[derive(Debug, Clone)]
pub struct QuestionSessionCreateParams<'a> {
    pub session_id: &'a str,
    pub repository_id: &'a str,
    pub task_id: Option<&'a str>,
    pub policy_version: &'a str,
    pub started_at_ms: i64,
}

/// Parameters for recording an asked/answered question event.
#[derive(Debug, Clone)]
pub struct QuestionEventParams<'a> {
    pub question_session_id: i64,
    pub question_id: &'a str,
    pub turn_index: i64,
    pub actor: &'a str,
    pub question_kind: &'a str,
    pub prompt: &'a str,
    pub expected_information_gain_bits: f64,
    pub expected_user_cost: f64,
    pub utility_bits_per_cost: f64,
    pub answer_text: Option<&'a str>,
    pub answer_type: Option<&'a str>,
    pub answered_at_ms: Option<i64>,
    pub created_at_ms: i64,
}

/// Parameters for one multiple-choice option row linked to a question event.
#[derive(Debug, Clone)]
pub struct QuestionOptionParams<'a> {
    pub question_event_id: i64,
    pub option_id: &'a str,
    pub label: &'a str,
    pub prior_probability: Option<f64>,
    pub posterior_probability: Option<f64>,
    pub is_other: bool,
}

/// Parameters for one option-outcome diagnostic row.
#[derive(Debug, Clone)]
pub struct QuestionOptionOutcomeParams<'a> {
    pub question_event_id: i64,
    pub option_id: &'a str,
    pub selected: bool,
    pub diagnostic_weight: f64,
    pub information_contribution_bits: f64,
    pub created_at_ms: i64,
}

/// Parameters for finalizing a clarification loop.
#[derive(Debug, Clone)]
pub struct QuestionStopEventParams<'a> {
    pub question_session_id: i64,
    pub stop_reason: &'a str,
    pub confidence_at_stop: Option<f64>,
    pub marginal_gain_bits: Option<f64>,
    pub expected_user_cost: Option<f64>,
    pub turn_index: Option<i64>,
    pub created_at_ms: i64,
}

/// Parameters for standardized A2A clarification payload envelopes.
#[derive(Debug, Clone)]
pub struct A2aClarificationMessageParams<'a> {
    pub message_uuid: &'a str,
    pub sender_agent: &'a str,
    pub receiver_agent: &'a str,
    /// `clarification_request` | `clarification_response` | `clarification_stop`
    pub msg_type: &'a str,
    pub repository_id: &'a str,
    pub thread_id: Option<&'a str>,
    pub priority: i64,
    pub clarification_intent: &'a str,
    pub hypothesis_set_id: &'a str,
    pub question_kind: Option<&'a str>,
    pub expected_information_gain_bits: Option<f64>,
    pub expected_user_cost: Option<f64>,
    pub requested_evidence_dimensions_json: Option<&'a str>,
    pub urgency: Option<&'a str>,
    pub stop_policy_json: Option<&'a str>,
}

/// Parameters for upserting one Cloudless encrypted account secret row.
#[derive(Debug, Clone)]
pub struct UpsertAccountSecretCiphertextParams<'a> {
    pub account_id: &'a str,
    pub secret_id: &'a str,
    pub ciphertext: &'a [u8],
    pub nonce: &'a [u8],
    pub cipher_version: i64,
    pub dek_wrapped: &'a [u8],
    pub dek_wrap_alg: &'a str,
    pub kek_ref: &'a str,
    pub kek_version: i64,
    pub aad_hash: Option<&'a str>,
    pub updated_at_ms: i64,
    pub rotation_epoch: i64,
    pub rotated_at_ms: Option<i64>,
    pub consistency_origin: &'a str,
    pub consistency_version: i64,
    pub last_synced_at_ms: Option<i64>,
    pub checksum_blake3: &'a str,
}

/// Parameters for `vox_db::VoxDb::insert_external_review_run`.
#[derive(Debug, Clone)]
pub struct ExternalReviewRunParams<'a> {
    pub provider: &'a str,
    pub repository_id: &'a str,
    pub owner: &'a str,
    pub repo: &'a str,
    pub pr_number: i64,
    pub commit_sha: Option<&'a str>,
    pub trigger_kind: &'a str,
    pub idempotency_key: Option<&'a str>,
    pub item_count: i64,
    pub metadata_json: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::upsert_external_review_thread`.
#[derive(Debug, Clone)]
pub struct ExternalReviewThreadParams<'a> {
    pub provider: &'a str,
    pub repository_id: &'a str,
    pub pr_number: i64,
    pub thread_identity: &'a str,
    pub placement_kind: &'a str,
    pub line_anchor_state: &'a str,
    pub file_path: Option<&'a str>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub source_comment_id: Option<i64>,
    pub parent_comment_id: Option<i64>,
    pub source_payload_hash: &'a str,
    pub raw_payload_json: &'a str,
}

/// Parameters for `vox_db::VoxDb::upsert_external_review_finding`.
#[derive(Debug, Clone)]
pub struct ExternalReviewFindingParams<'a> {
    pub run_id: i64,
    pub provider: &'a str,
    pub repository_id: &'a str,
    pub pr_number: i64,
    pub finding_identity: &'a str,
    pub thread_identity: Option<&'a str>,
    pub source_comment_id: Option<i64>,
    pub placement_kind: &'a str,
    pub line_anchor_state: &'a str,
    pub file_path: Option<&'a str>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub category: &'a str,
    pub anti_pattern_id: Option<&'a str>,
    pub severity: &'a str,
    pub title: &'a str,
    pub details: &'a str,
    pub suggested_fix: Option<&'a str>,
    pub extraction_confidence: Option<f64>,
    pub source_payload_hash: &'a str,
    pub fingerprint: &'a str,
    pub status: &'a str,
}

/// Parameters for `vox_db::VoxDb::append_external_review_finding_state`.
#[derive(Debug, Clone)]
pub struct ExternalReviewFindingStateParams<'a> {
    pub finding_id: i64,
    pub previous_state: Option<&'a str>,
    pub new_state: &'a str,
    pub reason: Option<&'a str>,
    pub confidence: Option<f64>,
    pub evidence_ref: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::insert_external_review_outcome`.
#[derive(Debug, Clone)]
pub struct ExternalReviewOutcomeParams<'a> {
    pub finding_id: i64,
    pub outcome_kind: &'a str,
    pub outcome_ref: Option<&'a str>,
    pub outcome_json: Option<&'a str>,
}

/// Parameters for `vox_db::VoxDb::insert_external_review_deadletter`.
#[derive(Debug, Clone)]
pub struct ExternalReviewDeadletterParams<'a> {
    pub provider: &'a str,
    pub repository_id: &'a str,
    pub pr_number: i64,
    pub source_kind: &'a str,
    pub source_comment_id: Option<i64>,
    pub source_payload_hash: &'a str,
    pub error_class: &'a str,
    pub error_message: Option<&'a str>,
    pub raw_payload_json: &'a str,
}

/// Parameters for `vox_db::VoxDb::insert_external_review_kpi_snapshot`.
#[derive(Debug, Clone)]
pub struct ExternalReviewKpiSnapshotParams<'a> {
    pub repository_id: &'a str,
    pub period_start: &'a str,
    pub period_end: &'a str,
    pub coverage_ratio: Option<f64>,
    pub ingest_to_fix_latency_ms: Option<f64>,
    pub repeated_finding_rate: Option<f64>,
    pub post_training_regression_rate: Option<f64>,
    pub auto_fix_acceptance_rate: Option<f64>,
}
