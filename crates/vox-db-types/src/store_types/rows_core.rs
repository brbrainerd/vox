/// One row from `execution_log`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionEntry {
    /// Row id.
    pub id: i64,
    /// Activity name.
    pub activity_name: String,
    /// Status label.
    pub status: String,
    /// Attempt number.
    pub attempt: u32,
    /// Error text when status indicates failure.
    pub error: Option<String>,
    /// SQLite `datetime` string.
    pub created_at: String,
}

/// One row from `scheduled` (pending or historical).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScheduledEntry {
    /// Row id.
    pub id: i64,
    /// Content hash of the function to invoke.
    pub function_hash: String,
    /// Serialized call arguments.
    pub args: Option<Vec<u8>>,
    /// ISO-like run time or cron anchor.
    pub run_at: String,
    /// Optional cron expression.
    pub cron_expr: Option<String>,
}

/// Registered UI or service component metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ComponentEntry {
    /// Component name.
    pub name: String,
    /// Namespace path.
    pub namespace: String,
    /// Version string.
    pub version: String,
    /// Optional description.
    pub description: Option<String>,
}

/// One row from `memories`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntry {
    /// Row id.
    pub id: i64,
    /// Agent that owns this memory.
    pub agent_id: crate::ids::DbAgentId,
    /// Session scope.
    pub session_id: crate::ids::DbSessionId,
    /// Memory type label.
    pub memory_type: String,
    /// Text content.
    pub content: String,
    /// Optional JSON metadata.
    pub metadata: Option<String>,
    /// Importance score.
    pub importance: f64,
    /// Creation timestamp string.
    pub created_at: String,
}

/// Vector embedding row from `embeddings`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingEntry {
    /// Row id.
    pub id: i64,
    /// Source object type, if set.
    pub source_type: Option<String>,
    /// Source object id.
    pub source_id: String,
    /// Vector dimensionality.
    pub dim: i64,
    /// Optional JSON metadata.
    pub metadata: Option<String>,
}

/// Learned preference or habit pattern.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LearnedPatternEntry {
    /// Row id.
    pub id: i64,
    /// User or agent id the pattern belongs to.
    pub user_id: crate::ids::DbUserId,
    /// Pattern classifier.
    pub pattern_type: String,
    /// Category bucket.
    pub category: String,
    /// Human-readable description.
    pub description: String,
    /// Model confidence in `[0, 1]`.
    pub confidence: f64,
    /// Optional VCS snapshot correlation.
    pub vcs_snapshot_id: Option<String>,
}

/// Analytics event from `behavior_events`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BehaviorEventEntry {
    /// Row id.
    pub id: i64,
    /// Subject user id.
    pub user_id: crate::ids::DbUserId,
    /// Event type label.
    pub event_type: String,
    /// Free-form context (e.g. command string).
    pub context: Option<String>,
    /// JSON metadata blob.
    pub metadata: Option<String>,
    /// Event timestamp string.
    pub created_at: String,
}

/// Aggregated CLI command statistics for a user.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommandFrequencyEntry {
    /// Command or context key.
    pub command: String,
    /// Total invocations.
    pub count: i64,
    /// Count inferred as successful from metadata heuristics.
    pub success_count: i64,
    /// Average duration in ms when present in metadata.
    pub avg_duration_ms: Option<f64>,
}

/// Prompt/response pair plus optional human feedback.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingPair {
    /// User prompt.
    pub prompt: String,
    /// Model response.
    pub response: String,
    /// Optional numeric rating.
    pub rating: Option<i64>,
    /// Preferred or corrected response text.
    pub correction: Option<String>,
    /// Feedback channel label.
    pub feedback_type: String,
}

/// Row from `users`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserEntry {
    /// User id.
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// Email when stored.
    pub email: Option<String>,
    /// Avatar URL.
    pub avatar_url: Option<String>,
    /// Role label.
    pub role: String,
}

/// Agent definition as stored in `agents` (by name ordering in list APIs).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentDefEntry {
    /// Agent name.
    pub name: String,
    /// Definition version.
    pub version: String,
    /// Description text.
    pub description: Option<String>,
    /// System prompt.
    pub system_prompt: Option<String>,
    /// Tools JSON.
    pub tools: Option<String>,
    /// Model config JSON.
    pub model_config: Option<String>,
    /// Public visibility flag.
    pub is_public: bool,
}

/// Code snippet catalog row.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnippetEntry {
    /// Row id.
    pub id: i64,
    /// Language tag.
    pub language: String,
    /// Title.
    pub title: String,
    /// Source code.
    pub code: String,
    /// Description.
    pub description: Option<String>,
    /// Tags string.
    pub tags: Option<String>,
}

/// One matching package from `search_packages`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PackageSearchResult {
    /// Package name.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Description.
    pub description: Option<String>,
    /// Author string.
    pub author: Option<String>,
    /// SPDX or license label.
    pub license: Option<String>,
}

/// Artifact registry row (search/list).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArtifactEntry {
    /// Artifact name.
    pub name: String,
    /// Type discriminator.
    pub artifact_type: String,
    /// Version string.
    pub version: String,
    /// Author user id.
    pub author_id: String,
    /// Download counter.
    pub downloads: i64,
    /// Average star or quality rating.
    pub avg_rating: f64,
    /// Lifecycle status.
    pub status: String,
    /// Description text.
    pub description: Option<String>,
}

/// Published skill: manifest JSON plus markdown body.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillManifestEntry {
    /// Skill id.
    pub id: String,
    /// Skill version.
    pub version: String,
    /// Serialized manifest JSON.
    pub manifest_json: String,
    /// Skill markdown documentation.
    pub skill_md: String,
}

/// Minimal knowledge graph node (id + label).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeNodeSummary {
    /// Node id.
    pub id: String,
    /// Short label.
    pub label: String,
}

/// One row from `codex_change_log` (Codex reactivity / SSE).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodexChangeLogEntry {
    /// Monotonic change id.
    pub id: i64,
    /// Subscription topic.
    pub topic: String,
    /// Affected entity type, if any.
    pub entity_kind: Option<String>,
    /// Affected entity id, if any.
    pub entity_id: Option<String>,
    /// Change operation label.
    pub change_kind: String,
    /// Optional JSON details.
    pub payload_json: Option<String>,
    /// Insertion timestamp.
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeIdentityRow {
    pub node_id: String,
    pub pubkey_hex: String,
    pub label: Option<String>,
    pub account_id: Option<String>,
    pub created_at: String,
    pub last_seen_at: String,
}

/// One row from `model_scoreboard`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelScoreboardRow {
    pub model_id: String,
    pub task_category: String,
    pub strength_tag: String,
    pub window_days: i64,
    pub n_calls: i64,
    pub success_rate: f64,
    pub p50_latency_ms: Option<i64>,
    pub p99_latency_ms: Option<i64>,
    pub cost_per_success_usd: Option<f64>,
    pub quality_score: f64,
    pub updated_at_ms: i64,
    pub success_count: i64,
    pub cumulative_cost_usd: f64,
    /// Task M3: p95 time-to-first-token, ms. `None` until at least one row in the window has
    /// `ttft_ms` recorded (see `crates/vox-db-types/src/store_types/params.rs`'s `ModelOutcome`).
    pub p95_ttft_ms: Option<i64>,
    /// Task M3: p95 time-per-output-token, ms. Same NULL condition as `p95_ttft_ms`.
    pub p95_tpot_ms: Option<f64>,
    /// Task M3: mean successful-call throughput (output_tokens / (latency_ms / 1000.0)) over
    /// the window — "goodput" in the LLM-serving sense.
    pub goodput_tokens_per_sec: Option<f64>,
}

/// The most recently recorded row from `llm_attempts`, with its age precomputed in SQL
/// (`julianday('now') - julianday(created_at)`) so callers doing staleness filtering
/// (e.g. `vox doctor` only trusting this within a few minutes, so it never shows an
/// hours-stale rate-limit as if it were current) don't need to parse the SQLite
/// `datetime` string themselves.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LastLlmAttemptRow {
    pub provider: String,
    pub model_id: String,
    /// `"success"` or `"error"` (see `record_telemetry_attempt` callers in
    /// `vox_actor_runtime::llm::chat`).
    pub outcome: String,
    /// e.g. `"rate-limited"`, `"transport-error"`, `"client-error"`, `"server-error"`,
    /// `"decode-error"` — see `vox_actor_runtime::llm::chat::map_egress_error`. `None`
    /// for a successful attempt.
    pub error_class: Option<String>,
    /// Seconds since this attempt was recorded, computed in SQL at query time.
    pub age_seconds: f64,
}

/// One row from `model_pricing_catalog`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelPricingCatalogRow {
    pub model_id: String,
    pub provider: String,
    pub observed_blended_per_1k: Option<f64>,
    pub observed_input_per_1k: Option<f64>,
    pub observed_output_per_1k: Option<f64>,
    pub catalog_input_per_1k: f64,
    pub catalog_output_per_1k: f64,
    pub n_provider_reported: i64,
    pub n_estimated: i64,
    pub n_free: i64,
    pub confidence: String,
    pub last_observed_at_ms: Option<i64>,
    pub updated_at_ms: i64,
}
