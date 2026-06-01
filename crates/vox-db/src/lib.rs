//! # vox-db — High-level database facade for Vox
//!
//! Provides a unified API over Turso/libSQL for packages, code artifacts, agent memory, Codex
//! reactivity, and related tables. Prefer this crate over talking to `turso::Connection` directly
//! in application code.
//!
//! ## Nomenclature (avoid mixing layers)
//!
//! | Name | What it is |
//! |------|------------|
//! | **[`VoxDb`]** | Stable **Rust type** for this facade; use it in signatures and tests. |
//! | **[`Codex`]** | **Type alias** for `VoxDb` — same type, product-facing name in docs/UI. |
//! | **Arca** | Internal name for **schema + SQL** owned by this crate (`crates/vox-db/src/schema/`). |
//! | **`vox-package`** | Package registry / artifacts — **not** the SQL schema SSOT. |
//!
//! Use [`VoxDb::store`] (async method) for content-addressed blob writes (`ops_cas`); it is not a getter.
//!
//! ## Connection modes
//!
//! - **Remote** (Turso cloud) — always available
//! - **Local** (file-based Turso) — `local` feature (default)
//! - **In-memory** — `DbConfig::Memory`, tests only (`local` feature)
//! - **Embedded replica** (local + cloud sync) — `replication` feature
//!
//! ## Canonical store (SSOT)
//!
//! User-global relational data uses [`DbConfig::resolve_canonical`] / [`canonical_store::resolve_canonical_config`].
//! Repo-backed interactive surfaces default to the workspace journey store (`.vox/store.db`) via
//! [`workspace_journey_store::connect_workspace_journey_optional`]; set `VOX_WORKSPACE_JOURNEY_STORE=canonical`
//! for legacy user-global / Turso. See [`canonical_store`] and [`workspace_journey_store`].
//!
//! ## Turso batch SQL caveat
//!
//! Built-in and app-supplied migrations run through [`turso::Connection::execute_batch`], which uses
//! `execute` and **fails on statements that return rows** (e.g. bare `SELECT`, assignment `PRAGMA`
//! unless handled with `pragma_update`). [`VoxDb::connect`] / [`VoxDb::open`] apply pragmas via
//! `pragma_update` and skip empty migration bodies; see also [`VoxDb::apply_migrations`].
//!
//! ```no_run
//! use vox_db::{VoxDb, DbConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let db = VoxDb::connect(DbConfig::Remote {
//!         url: "turso://my-db.turso.io".to_string(),
//!         token: "my-token".to_string(),
//!     }).await?;
//!
//!     let hash = db.store("fn", b"fn hello(): return 42").await?;
//!     println!("Stored: {hash}");
//!     Ok(())
//! }
//! ```

#![allow(clippy::collapsible_if)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::single_char_add_str)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::useless_vec)]

/// Compare live SQLite schema to `@table` / collection declarations; non-destructive migrations.
pub mod auto_migrate;
/// Benchmark observations stored in `research_metrics` (`bench:<repository_id>` sessions).
pub mod benchmark_telemetry;
pub mod build_hints;
/// Turso / search tuning helpers (`VOX_EMBEDDING_SEARCH_CANDIDATE_MULT`, etc.).
pub mod capabilities;
/// Circuit breaker for write operations.
pub mod circuit_breaker;
/// User chat, tool calls, usage limits, topics (manifest chat/search slices).
pub mod codex_chat;
/// Research sessions, conversation versions/edges, topic evolution (manifest `v17`).
mod codex_conversation_graph;
/// Canonical connect policy helpers (strict vs optional degraded surfaces).
pub mod connect_policy;
/// Explicit namespace for migration-era and cutover-only pathways.
pub mod legacy;
/// Ludus / extended `gamify_*` contracts and metrics keys (DDL in baseline `schema/domains`).
pub mod research_metrics_contract;
pub mod schema;
/// Idempotent schema extensions (FTS).
pub mod schema_extensions;
/// Legacy import/export planning and verification for greenfield Codex releases.
pub mod store;
pub mod telemetry_sink;

/// Canonical Codex storage policy (`vox.db` vs project store vs training sidecar).
pub mod canonical_store;
#[doc(hidden)]
pub mod codex_legacy;
/// Manifest-derived readiness (baseline digest, required tables).
pub mod codex_schema;
pub mod collection;
mod config;
pub mod data_flow;
pub mod ddl;
pub mod error_enrichment;
// `eval_params` types moved to `vox-db-types`; re-exported below.
pub mod exec_time_telemetry;
mod local_cli_introspection;
pub mod sql_util;
pub use exec_time_telemetry::{ExecOutcome, ExecTimeRecord, TimedExecution, ToolLatencyProfile};
pub use local_cli_introspection::{audit_database_json, sample_table_json_objects};
pub mod hash;
pub mod learning;
pub mod legacy_import_extras;
/// Parameters for [`VoxDb::store_memory`].
pub mod memory;
mod mens_scorecard_trust;
/// Declarative SQL migrations using the `schema_version` table (see `crate::schema`).
pub mod migration;
/// Data directory and per-user id helpers (delegates to `vox_config`).
pub mod writer_actor;
pub use writer_actor::{DbWriteCmd, VoxWriteHandle};
pub mod paths;
pub mod pool;
pub use pool::VoxDbPool;
pub mod outcome_recorder;
/// Mens control-plane audit (`populi_control_event` in `research_metrics`).
pub mod populi_control_telemetry;
/// Opt-in mens local-registry publish rows (`VOX_MESH_CODEX_TELEMETRY`).
pub mod populi_registry_telemetry;
/// Registry-scoped user preferences (stored as JSON in the local config directory).
pub mod preferences;
pub mod project_store;
mod questioning_telemetry;
mod research;
pub mod research_pipeline;
/// Hybrid retrieval helpers (vector / full-text fusion) for RAG-style pipelines.
pub mod retrieval;
/// AST → [`crate::SchemaDigest`] for LLM context and codegen.
pub mod schema_digest;
/// OS keyring helpers for API tokens and similar secrets.
pub mod secrets;
mod socrates_telemetry;
mod sync_invocables;
pub mod syntax_k_telemetry;
pub mod toestub_store;
/// Mens QLoRA training run persistence (CRUD for `populi_training_run` table).
pub mod training_run;
mod trust_drift;
mod trust_propagation;
mod trust_telemetry;
pub mod types;
/// Interpreted workflow journal (`workflow_journal_entry` in `research_metrics`).
pub mod workflow_journal;
/// Workspace journey store resolution (`.vox/store.db` vs canonical) for repo-backed MCP/daemon flows.
pub mod workspace_journey_store;

pub mod mesh_exec_leases;
pub use mesh_exec_leases::ExecLeaseRow;
pub mod mesh_locks;
pub use mesh_locks::{LockKindRow, LockLeaderRow, VcsLockRow};
pub use store::ops_convergence::ConvergenceOpRow;

pub mod oratio_eval;
pub mod plugin_state_backend;

pub use auto_migrate::AutoMigrator;
pub use canonical_store::{resolve_canonical_config, user_global_sqlite_path};
pub use circuit_breaker::{CircuitBreakerError, CircuitState, DbCircuitBreaker};
pub use codex_chat::WorkspaceTranscriptTurnRow;
pub use codex_schema::{
    CodexApiReadiness, evaluate_codex_api_readiness, missing_codex_reactivity_tables,
};
pub use collection::Collection;
pub use config::DbConfig;
pub use connect_policy::{
    DbConnectSurface, REMEDIATION_CANONICAL_DB, connect_canonical_optional,
    connect_canonical_strict, format_degraded_optional_connect,
};
pub use data_flow::{DataFlowMap, build_data_flow};
pub use ddl::{SchemaDiff, diff_schemas, table_to_ddl, tables_to_ddl};
pub use error_enrichment::{EnrichedDbError, enrich_error};
pub use facade::agent_runs::AgentRunRow;
pub use facade::hitl_approvals::HitlApprovalRow;
pub use memory::MemoryParams;
pub use migration::{Migration, builtin_migrations, validate_migrations};
pub use oratio_eval::{OratioEvalRunRecord, OratioEvalRunStartParams, OratioEvalSampleRecord};
pub use outcome_recorder::UnifiedLlmTurnRowIds;
pub use project_store::{open_project_db, open_project_db_at_root};
pub use questioning_telemetry::{QuestioningKpiSnapshot, QuestioningResearchArtifact};
pub use research::{
    CapabilityMapRecord, ExternalResearchPacket, ResearchEvalRunRecord, ResearchEvalSampleRecord,
    ResearchIngestRequest, ResearchIngestResult, RetrievalDiagnostics, retrieval_diagnostics,
};
pub use retrieval::{
    RetrievalEvidenceSource, RetrievalMode, RetrievalQuery, RetrievalResult, SearchBackend,
    SearchCorpus, SearchDiagnostics, SearchIntent, SearchPlan, SearchRefinementAction,
    fuse_hybrid_results, heuristic_search_plan,
};
pub use schema_digest::{SchemaDigest, digest_to_json, format_llm_context, generate_schema_digest};
pub use socrates_telemetry::{
    SocratesSurfaceAggregate, SocratesSurfaceTelemetry, hallucination_risk_proxy,
};
pub use store::{
    A2AMessageRow, A2aClarificationMessageParams, AccountSecretCiphertextRow, AgentDefEntry,
    AgentEventRow, ArtifactEntry, BehaviorEventEntry, BenchmarkEventRow, BuildHealthSummary,
    BuildRunRow, BuilderSessionEntry, CloudCostSummary, CloudDispatchRow, CodexChangeLogEntry,
    CommandFrequencyEntry, ComponentEntry, CorpusQualitySummary, CrateSample, CrateSampleRow,
    EmbeddingEntry, EndpointReliabilityEntry, ExecutionEntry, ExternalStatusSnapshotParams,
    ExternalStatusSnapshotRow, ExternalSubmissionAttemptParams, ExternalSubmissionAttemptRow,
    ExternalSubmissionJobRow, ExternalSubmissionJobUpsertParams, GamifyLudusKpiRollup,
    GamifyPolicySnapshotListRow, GrpoStepRow, KnowledgeNodeSummary, LearnedPatternEntry,
    LocalTrainRow, LogExecutionParams, LogInteractionParams, MemoryEntry, PackageSearchResult,
    PlanNodeRow, PlanSessionRow, PlanVersionRow, PublicationAttemptRow, PublicationExternalLinkRow,
    PublicationExternalLinkUpsertParams, PublicationExternalRevisionRow,
    PublicationExternalRevisionUpsertParams, PublicationManifestParams, PublicationManifestRow,
    PublicationMediaAssetParams, PublicationMediaAssetRow, PublicationStatusEventRow,
    PublishArtifactParams, QuestionEventParams, QuestionEventRow, QuestionOptionOutcomeParams,
    QuestionOptionOutcomeRow, QuestionOptionParams, QuestionOptionRow, QuestionRow,
    QuestionSessionCreateParams, QuestionSessionRow, QuestionStopEventParams, QuestionStopEventRow,
    RegisterAgentParams, RegressionRow, ReviewEntry, SaveMemoryParams, SaveSnippetParams,
    ScheduledEntry, ScholarlySubmissionRow, SessionEventRow, SessionRow, SessionTurnEntry,
    SkillExecutionParams, SkillExecutionRow, SkillManifestEntry, SkillReliabilityReport,
    SnippetEntry, StoreError, ThroughputProfileRow, TrainingPair, TrustRollupEntry,
    TypedStreamEventEntry, UpsertAccountSecretCiphertextParams, UserEntry, WarningRow,
    WorkflowExecutionRow,
};
pub use sync_invocables::InvocableSyncEngine;
pub use syntax_k_telemetry::SyntaxKEventMeta;
pub use toestub_store::{
    add_suppression, get_file_cache_blocking, list_suppressions_blocking, load_baseline,
    load_latest_task_queue, save_baseline, save_task_queue, set_file_cache_blocking,
};
pub use trust_drift::{TrustObservationDriftReport, TrustObservationWindowStats};
pub use trust_propagation::{TrustPropagatedScore, propagate_trust_rollups_domain_cliques};
pub use trust_telemetry::{TrustObservationEntry, TrustObservationInput, TrustRollupGroupSummary};
pub use types::now_unix_ms;
pub use vox_db_types::EvalRunParams;
pub use vox_db_types::{
    DbAgentId, DbCorrelationId, DbPlanSessionId, DbSessionId, DbTaskId, DbUserId,
    ResearchArtifactRecord, ResearchSessionRecord, ResearchSessionSummary,
};
pub use workspace_journey_store::{
    WorkspaceJourneyStoreMode, connect_workspace_journey_optional,
    connect_workspace_journey_optional_at, workspace_journey_diagnostics_json,
    workspace_journey_store_mode_from_env,
};

/// Public product name for the unified database facade (**Codex** over Arca/Turso).
///
/// `VoxDb` remains the stable Rust type name; new documentation should prefer **Codex**.
pub type Codex = VoxDb;

/// Whether to pull embedded-replica updates before application-level reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadConsistency {
    /// Use the current local database state only.
    Local,
    /// Best-effort `pull` when a sync-backed client is attached (no-op for pure local files).
    ReplicaLatest,
}

/// High-level database facade for the Vox ecosystem (**Codex**).
///
/// Owns a single [`VoxDb`] (one Turso connection). Higher-level helpers (memory, learner,
/// schema sync) delegate to that store; advanced callers use [`Self::store`] for direct access.
///
/// **Concurrency:** one connection per `VoxDb` handle; not `Sync` across concurrent writers unless
/// the underlying Turso client serializes access (typical for one handle per task).
#[derive(Clone)]
pub struct VoxDb {
    pub(crate) conn: turso::Connection,
    pub(crate) sync_db: Option<turso::sync::Database>,
    pub(crate) writer: Option<crate::VoxWriteHandle>,
    pub(crate) breaker: std::sync::Arc<DbCircuitBreaker>,
    /// Lazily filled by [`VoxDb::sqlite_capabilities_snapshot`](crate::VoxDb::sqlite_capabilities_snapshot).
    pub(crate) sqlite_probe_cache:
        std::sync::Arc<tokio::sync::RwLock<Option<capabilities::SqliteProbeSnapshot>>>,
}

pub mod facade;

#[cfg(test)]
mod codex_contract {
    use super::{Codex, VoxDb};

    #[test]
    fn codex_alias_same_layout_as_voxdb() {
        assert_eq!(std::mem::size_of::<Codex>(), std::mem::size_of::<VoxDb>());
        assert_eq!(std::mem::align_of::<Codex>(), std::mem::align_of::<VoxDb>());
    }
}

/// Shared mutex that serialises all tests that mutate process-level environment
/// variables (`VOX_DB_URL`, `VOX_DB_TOKEN`, `VOX_SECRETS_*`, …). Both
/// `config::tests` and `local_tests` acquire this lock before touching env.
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(all(test, feature = "local"))]
mod local_tests;
