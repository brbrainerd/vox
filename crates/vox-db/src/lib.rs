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
pub mod history_store;
/// Explicit namespace for migration-era and cutover-only pathways.
pub mod legacy;
pub mod redact;
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
#[cfg(feature = "legacy-import")]
#[deprecated(
    since = "0.6.0",
    note = "Use vox codex export-legacy CLI; module will be removed in the next major version"
)]
pub mod codex_legacy;
/// Manifest-derived readiness (baseline digest, required tables).
pub mod codex_schema;
pub mod collection;
mod config;
pub mod data_flow;
pub mod ddl;
pub mod error_enrichment;
mod harness_eval;
pub use harness_eval::*;
// `eval_params` types moved to `vox-db-types`; re-exported below.
pub mod exec_time_telemetry;
mod local_cli_introspection;
pub mod sql_util;
pub use exec_time_telemetry::{ExecOutcome, ExecTimeRecord, TimedExecution, ToolLatencyProfile};
pub use local_cli_introspection::{audit_database_json, sample_table_json_objects};
pub mod hash {
    //! SHA3-512 Base32Hex hashing utilities.
    use data_encoding::BASE32HEX_NOPAD;
    use sha3::{Digest, Sha3_512};

    /// Compute a SHA3-512 hash of the given data, returning Base32Hex-encoded string.
    pub fn content_hash(data: &[u8]) -> String {
        let mut hasher = Sha3_512::new();
        hasher.update(data);
        let result = hasher.finalize();
        BASE32HEX_NOPAD.encode(&result)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_deterministic_hash() {
            let h1 = content_hash(b"hello world");
            let h2 = content_hash(b"hello world");
            assert_eq!(h1, h2);
        }

        #[test]
        fn test_different_data_different_hash() {
            let h1 = content_hash(b"hello");
            let h2 = content_hash(b"world");
            assert_ne!(h1, h2);
        }

        #[test]
        fn test_hash_length() {
            let h = content_hash(b"test");
            assert!(h.len() > 50);
        }
    }
}
pub mod learning;
#[cfg(feature = "legacy-import")]
pub mod legacy_import_extras;
/// Parameters for [`VoxDb::store_memory`].
///
/// Alias of [`crate::store::SaveMemoryParams`] so application code can depend on `vox-db` only.
pub mod memory {
    pub type MemoryParams<'a> = crate::store::SaveMemoryParams<'a>;
}
mod mens_scorecard_trust;
/// Declarative SQL migrations using the `schema_version` table (see `crate::schema`).
pub mod migration;
/// SQL normalization and content-addressable hashing.
pub mod normalize;
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
/// Moved to `vox-db-types`; re-exported here for back-compat.
pub use vox_db_types::retrieval;
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
pub use config::{DbConfig, resolve_app_db_url, resolve_codex_db_url};
pub use connect_policy::{
    DbConnectSurface, REMEDIATION_CANONICAL_DB, connect_canonical_optional,
    connect_canonical_strict, format_degraded_optional_connect,
};
pub use data_flow::{DataFlowMap, build_data_flow};
pub use ddl::{SchemaDiff, diff_schemas, table_to_ddl, tables_to_ddl};
pub use error_enrichment::{EnrichedDbError, enrich_error};
pub use facade::agent_runs::AgentRunRow;
pub use facade::hitl_approvals::HitlApprovalRow;
pub use facade::model_prompt::ModelPromptProfileRow;
pub use history_store::{HistoryEntry, add_entry, list_entries};
pub use memory::MemoryParams;
pub use migration::{Migration, builtin_migrations, validate_migrations};
pub use oratio_eval::{OratioEvalRunRecord, OratioEvalRunStartParams, OratioEvalSampleRecord};
pub use outcome_recorder::UnifiedLlmTurnRowIds;
pub use project_store::{open_project_db, open_project_db_at_root};
pub use questioning_telemetry::{QuestioningKpiSnapshot, QuestioningResearchArtifact};
pub use redact::redact;
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
    DiscoveryInboxRow, EmbeddingEntry, EndpointReliabilityEntry, ExecutionEntry,
    ExternalStatusSnapshotParams, ExternalStatusSnapshotRow, ExternalSubmissionAttemptParams,
    ExternalSubmissionAttemptRow, ExternalSubmissionJobRow, ExternalSubmissionJobUpsertParams,
    GamifyLudusKpiRollup, GamifyPolicySnapshotListRow, GrpoStepRow, HopperInboxRow,
    KnowledgeNodeSummary, LearnedPatternEntry, LocalTrainRow, LogExecutionParams,
    LogInteractionParams, MemoryEntry, NewSkillCandidate, PackageSearchResult, PlanNodeRow,
    PlanSessionRow, PlanVersionRow, PublicationAttemptRow, PublicationExternalLinkRow,
    PublicationExternalLinkUpsertParams, PublicationExternalRevisionRow,
    PublicationExternalRevisionUpsertParams, PublicationManifestParams, PublicationManifestRow,
    PublicationMediaAssetParams, PublicationMediaAssetRow, PublicationStatusEventRow,
    PublishArtifactParams, QuestionEventParams, QuestionEventRow, QuestionOptionOutcomeParams,
    QuestionOptionOutcomeRow, QuestionOptionParams, QuestionOptionRow, QuestionRow,
    QuestionSessionCreateParams, QuestionSessionRow, QuestionStopEventParams, QuestionStopEventRow,
    RegisterAgentParams, RegressionRow, ReviewEntry, SaveMemoryParams, SaveSnippetParams,
    ScheduledEntry, ScholarlySubmissionRow, SessionEventRow, SessionRow, SessionTurnEntry,
    SkillCandidateRow, SkillExecutionParams, SkillExecutionRow, SkillManifestEntry,
    SkillReliabilityReport, SnippetEntry, StoreError, ThroughputProfileRow, TrainingPair,
    TrustRollupEntry, TypedStreamEventEntry, UpsertAccountSecretCiphertextParams, UserEntry,
    WarningRow, WorkflowExecutionRow,
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

/// Row returned by KB queries from VoxDb.
#[derive(Debug, Clone)]
pub struct KbRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub entry_count: i64,
}

/// Row returned by KB entry queries from VoxDb.
#[derive(Debug, Clone)]
pub struct KbEntryRow {
    pub id: String,
    pub kb_id: String,
    pub content: String,
    pub source_signal: String,
    pub source_ref: Option<String>,
    pub routing_confidence: f64,
    pub tags: String,
    pub created_at_ms: i64,
    pub last_accessed_at_ms: Option<i64>,
    pub access_count: i64,
    pub accepted: i64,
    pub mens_queued: i64,
}

/// Row returned by KB routing rule queries from VoxDb.
#[derive(Debug, Clone)]
pub struct KbRuleRow {
    pub id: String,
    pub kb_id: String,
    pub rule_type: String,
    pub pattern: String,
    pub priority: i64,
    pub created_at_ms: i64,
}

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
/// **Concurrency:** one connection per `VoxDb` handle; `Sync`/safe across concurrent callers
/// because [`GuardedConnection`] serializes access to the shared `turso::Connection` (see its
/// docs for why this is required even though `VoxDb`/`Connection` are `Clone`).
#[derive(Clone)]
pub struct VoxDb {
    pub(crate) conn: GuardedConnection,
    pub(crate) sync_db: Option<turso::sync::Database>,
    /// Keeps local `:memory:` / file databases alive while `conn` is in use (Turso drops
    /// in-memory catalogs when the owning [`turso::Database`] is released).
    #[expect(dead_code, reason = "retains Arc<Database> for connection lifetime")]
    pub(crate) local_db: Option<std::sync::Arc<turso::Database>>,
    pub(crate) writer: Option<crate::VoxWriteHandle>,
    pub(crate) breaker: std::sync::Arc<DbCircuitBreaker>,
    /// Lazily filled by [`VoxDb::sqlite_capabilities_snapshot`](crate::VoxDb::sqlite_capabilities_snapshot).
    pub(crate) sqlite_probe_cache:
        std::sync::Arc<tokio::sync::RwLock<Option<capabilities::SqliteProbeSnapshot>>>,
}

impl VoxDb {
    pub(crate) fn assembled(
        conn: turso::Connection,
        sync_db: Option<turso::sync::Database>,
        local_db: Option<std::sync::Arc<turso::Database>>,
    ) -> Self {
        Self {
            conn: GuardedConnection::new(conn),
            sync_db,
            local_db,
            writer: None,
            breaker: std::sync::Arc::new(DbCircuitBreaker::from_env()),
            sqlite_probe_cache: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        }
    }
}

/// Serializes access to a shared `turso::Connection` to avoid
/// `turso::Error::Misuse("concurrent use forbidden")`.
///
/// ## Root cause this fixes
///
/// `turso::Connection::clone()` does **not** create an independent connection: it clones an
/// `Arc` to the same underlying `turso_sdk_kit::rsapi::TursoConnection`, which itself guards
/// every `step()` (the primitive under `query`/`execute`/`execute_batch`/`prepare`) with an
/// atomic [`ConcurrentGuard`]. Two async tasks that call into *any* clones of the same
/// connection at literally the same instant (e.g. two Tauri GUI commands dispatched close
/// together on a multi-threaded Tokio runtime, or a background poll racing a user action) can
/// have their `step()` polls genuinely overlap on different OS threads, tripping that guard and
/// returning `Err(Misuse("concurrent use forbidden"))`. Since `VoxDb` is `Clone` and is shared
/// as one `Arc<VoxDb>` across all GUI commands (see `vox-gui/src/commands/gui_db_pool.rs`), this
/// happened routinely under ordinary concurrent chat usage, and the resulting error surfaced to
/// users as a "Message not saved" toast — the write silently never reached the database.
///
/// ## Fix
///
/// Hold a `tokio::sync::Mutex<()>` around each real network/IO-bearing call
/// (`query`/`execute`/`execute_batch`/`pragma_update`) before delegating to the wrapped
/// `turso::Connection`. The lock lives behind an `Arc` so every clone of a `GuardedConnection`
/// (and every clone of the owning `VoxDb`) shares the *same* lock — matching the fact that they
/// already share the same underlying Turso connection.
///
/// Implements [`std::ops::Deref`] to `turso::Connection` so call sites that need a raw
/// `&turso::Connection` (e.g. [`crate::auto_migrate::AutoMigrator::new`], schema
/// migrations run once at connect time) keep compiling unchanged via deref coercion. Method
/// resolution prefers inherent methods on `GuardedConnection` over ones reached through `Deref`,
/// so the ~400 existing call sites across this crate (and the many downstream crates that call
/// `VoxDb::connection()`, e.g. `vox-gamify`, `vox-populi`, `vox-sql`, generated `vox-codegen`
/// output) that do `conn.query(..)` / `conn.execute(..)` / `conn.execute_batch(..)` get the lock
/// automatically without any call-site changes.
///
/// ## Known residual gap
///
/// `turso::Connection::last_insert_rowid()` is a synchronous, non-blocking read of
/// connection-local state and does **not** go through `ConcurrentGuard` at all (confirmed by
/// reading `turso-0.6.1`'s source), so it is intentionally left un-guarded here — wrapping it
/// would require holding the lock across the `execute` + `last_insert_rowid` pair (a call-site
/// change) rather than per-call. Callers that do this pairing (e.g.
/// `codex_chat.rs::chat_ensure_workspace_conversation`) are *not* actually safe from this crate's
/// concurrency bug class: on the multi-threaded Tokio runtime this crate runs under, a different
/// task on a different OS thread can execute its own guarded `execute()` (acquiring and releasing
/// the lock) between this caller's `execute()` and its `last_insert_rowid()` read, since nothing
/// holds the lock across that gap — no explicit `.await` yield is required for that interleaving.
/// This does not reproduce the `Misuse("concurrent use forbidden")` bug this type fixes (the two
/// individual calls are each correctly guarded), but it can silently return the *other* task's
/// row id. It is noted here as a narrower, pre-existing (not introduced by this change)
/// correctness edge case for future hardening (e.g. `INSERT ... RETURNING`).
#[derive(Clone)]
pub struct GuardedConnection {
    inner: turso::Connection,
    lock: std::sync::Arc<tokio::sync::Mutex<()>>,
}

impl GuardedConnection {
    pub(crate) fn new(inner: turso::Connection) -> Self {
        Self {
            inner,
            lock: std::sync::Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Guarded [`turso::Connection::query`].
    #[inline]
    pub async fn query(
        &self,
        sql: impl AsRef<str>,
        params: impl turso::IntoParams,
    ) -> turso::Result<turso::Rows> {
        let _guard = self.lock.lock().await;
        self.inner.query(sql, params).await
    }

    /// Guarded [`turso::Connection::execute`].
    #[inline]
    pub async fn execute(
        &self,
        sql: impl AsRef<str>,
        params: impl turso::IntoParams,
    ) -> turso::Result<u64> {
        let _guard = self.lock.lock().await;
        self.inner.execute(sql, params).await
    }

    /// Guarded [`turso::Connection::execute_batch`].
    #[inline]
    pub async fn execute_batch(&self, sql: impl AsRef<str>) -> turso::Result<()> {
        let _guard = self.lock.lock().await;
        self.inner.execute_batch(sql).await
    }

    /// Guarded [`turso::Connection::pragma_update`].
    #[inline]
    pub async fn pragma_update<V: std::fmt::Display>(
        &self,
        pragma_name: &str,
        pragma_value: V,
    ) -> turso::Result<Vec<turso::Row>> {
        let _guard = self.lock.lock().await;
        self.inner.pragma_update(pragma_name, pragma_value).await
    }
}

impl std::ops::Deref for GuardedConnection {
    type Target = turso::Connection;

    fn deref(&self) -> &turso::Connection {
        &self.inner
    }
}

#[cfg(test)]
mod guarded_connection_tests {
    /// Reproduces the reported data-loss bug: many concurrent tasks writing through clones of
    /// the same `VoxDb` (as `gui_db_pool.rs` does with one shared `Arc<VoxDb>`) must ALL
    /// succeed, never surface `turso::Error::Misuse("concurrent use forbidden")`.
    ///
    /// Before the `GuardedConnection` fix this test flakes/fails under `cargo test` (which uses
    /// a multi-threaded Tokio test runtime by default) with exactly that Misuse error on at
    /// least one of the concurrent branches.
    #[cfg(feature = "local")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn concurrent_writes_through_shared_voxdb_all_succeed() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("in-memory connect");
        db.connection()
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS concurrency_probe (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    label TEXT NOT NULL
                )",
            )
            .await
            .expect("create probe table");

        let db = std::sync::Arc::new(db);
        let mut handles = Vec::new();
        const N: usize = 64;
        for i in 0..N {
            let db = db.clone();
            handles.push(tokio::spawn(async move {
                // Mix `execute` and `query` (mirrors chat_append_message-style writes plus reads)
                // to exercise both guarded methods concurrently.
                let label = format!("task-{i}");
                db.connection()
                    .execute(
                        "INSERT INTO concurrency_probe (label) VALUES (?1)",
                        turso::params![label.clone()],
                    )
                    .await?;
                let mut rows = db
                    .connection()
                    .query(
                        "SELECT COUNT(*) FROM concurrency_probe WHERE label = ?1",
                        turso::params![label],
                    )
                    .await?;
                let _ = rows.next().await?;
                Ok::<(), turso::Error>(())
            }));
        }

        let mut misuse_errors = Vec::new();
        for h in handles {
            if let Err(e) = h.await.expect("task panicked") {
                misuse_errors.push(e.to_string());
            }
        }
        assert!(
            misuse_errors.is_empty(),
            "expected all {N} concurrent DB operations to succeed, got errors: {misuse_errors:?}"
        );

        let mut rows = db
            .connection()
            .query("SELECT COUNT(*) FROM concurrency_probe", ())
            .await
            .expect("count query");
        let row = rows
            .next()
            .await
            .expect("count row")
            .expect("count row present");
        let count: i64 = row.get(0).expect("count value");
        assert_eq!(
            count, N as i64,
            "every concurrent insert must be durably persisted, not silently dropped"
        );
    }
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

#[cfg(test)]
mod semcov_wave18_tests;
