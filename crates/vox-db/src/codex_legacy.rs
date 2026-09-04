// Legacy database import/export planning for **Codex** greenfield releases.
//
// New call sites should prefer the explicit legacy namespace: `crate::legacy::codex`.
//
// Greenfield Codex stores `schema_version` at [`crate::schema::BASELINE_VERSION`]. Databases whose
// `MAX(schema_version)` is not 0 and not that baseline (historical multi-step chain) must be
// exported with `export_legacy_jsonl` (connection that skips baseline migration) and imported into
// a fresh file. See `docs/src/architecture/codex-vnext-schema.md` and ADR 004.

use std::io::{BufRead, Write};
use turso::Value as SqlValue;
use turso::params;

use crate::StoreError;
use crate::schema::CODEX_REACTIVITY_TABLES;
use crate::sql_util::validate_identifier;

/// Result of [`verify_legacy_store`].
#[derive(Debug, Clone)]
pub struct LegacyVerification {
    /// `MAX(schema_version.version)` from the store (greenfield equals [`crate::schema::BASELINE_VERSION`]).
    pub schema_version: i64,
    /// Whether all [`CODEX_REACTIVITY_TABLES`] exist (manifest-derived; not a numeric threshold).
    pub has_codex_reactivity: bool,
    /// `true` when `schema_version` does not match greenfield baseline (multi-version or stale chain).
    pub is_legacy_schema_chain: bool,
}

async fn table_exists(store: &crate::VoxDb, name: &str) -> Result<bool, StoreError> {
    let mut rows: crate::GuardedRows = store
        .connection()
        .query(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1 LIMIT 1",
            params![name],
        )
        .await?;
    Ok(rows.next().await?.is_some())
}

/// Inspect a connected store for legacy import / migration workflows.
pub async fn verify_legacy_store(store: &crate::VoxDb) -> Result<LegacyVerification, StoreError> {
    let schema_version = store.schema_version().await?;
    let mut has_codex_reactivity = true;
    for t in CODEX_REACTIVITY_TABLES {
        if !table_exists(store, t).await? {
            has_codex_reactivity = false;
            break;
        }
    }
    let is_legacy_schema_chain =
        schema_version != 0 && schema_version != crate::schema::BASELINE_VERSION;
    Ok(LegacyVerification {
        schema_version,
        has_codex_reactivity,
        is_legacy_schema_chain,
    })
}

/// Planned CLI surface (see `vox codex` in `vox-cli`).
pub const PLANNED_CODEX_CLI: &[&str] = &[
    "vox codex export-legacy",
    "vox codex import-legacy",
    "vox codex verify",
];

/// Documented import sources for future implementers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyImportSource {
    /// Current `vox-package` / `VoxDb` dump.
    VoxPmSqliteTurso,
    /// Orchestrator file-first memory (`memory/*.md`, `MEMORY.md`).
    OrchestratorFileMemory,
    /// Published skill bundles on disk or HTTP.
    SkillBundles,
}

impl LegacyImportSource {
    /// Human-readable label for reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VoxPmSqliteTurso => "vox_pm_turso",
            Self::OrchestratorFileMemory => "orchestrator_file_memory",
            Self::SkillBundles => "skill_bundles",
        }
    }
}

/// Tables never exported: Turso/Arca owns `schema_version` via `crate::VoxDb::migrate`.
/// A fresh target DB must already hold [`crate::schema::BASELINE_VERSION`] before [`import_legacy_jsonl`].
pub const LEGACY_EXPORT_SKIP_TABLES: &[&str] = &[
    "convergence_op_log", // mesh replication state — not portable across peers
    "convergence_op_log_backfill_dlq", // transient backfill dead-letter queue
    "mesh_a2a_messages",  // transient mesh execution state
    "mesh_dispatch_results", // transient mesh execution state
    "mesh_exec_leases",   // transient mesh execution state
    "schema_version",
    "scientia_discovery_inbox", // derived surfacing index over draft manifests; regenerable, drafts are SSOT
    "scientia_embedding_cache", // pure derived cache; regenerated on demand from LLM calls
    "scientia_producer_cursor", // ephemeral per-producer scan cursor; drafts are idempotent on re-scan
];

/// User tables included in [`export_legacy_jsonl`] / accepted by [`import_legacy_jsonl`].
///
/// SSOT: must match every `sqlite_master` user table after baseline migrate, except
/// [`LEGACY_EXPORT_SKIP_TABLES`]. Kept sorted for diff review; see unit test
/// `legacy_export_covers_all_baseline_tables`.
pub const LEGACY_EXPORT_TABLES: &[&str] = &[
    "a2a_messages",
    "account_config",
    "activity_log",
    "activity_result_cache",
    "actor_state",
    "agent_events",
    "agent_exec_history",
    "agent_metrics",
    "agent_operations",
    "agent_oplog",
    "agent_reliability",
    "agent_runs",
    "agent_session_events",
    "agent_sessions",
    "agent_telemetry_flat",
    "agents",
    "anomalous_events",
    "artifact_reviews",
    "artifacts",
    "audit_log",
    "behavior_events",
    "build_crate_sample",
    "build_run",
    "build_run_dependency_shape",
    "build_warning",
    "builder_sessions",
    "causal",
    "checkpoint_blobs",
    "ci_completion_detector_snapshot",
    "ci_completion_finding",
    "ci_completion_run",
    "ci_completion_suppression",
    "clavis_account_secrets",
    "cloud_dispatch_log",
    "codex_capability_map",
    "codex_change_log",
    "codex_projection_versions",
    "codex_query_snapshots",
    "codex_schema_lineage",
    "codex_subscriptions",
    "components",
    "conversation_edges",
    "conversation_message_topics",
    "conversation_messages",
    "conversation_tool_calls",
    "conversation_topics",
    "conversation_versions",
    "conversations",
    "corpus_pairs",
    "corpus_snapshots",
    "cost_records",
    "db_snapshots",
    "developer_journey_definitions",
    "developer_journey_steps",
    "discovery_state",
    "distributed_locks",
    "embeddings",
    "endpoint_reliability",
    "eval_runs",
    "execution_log",
    "external_review_comment_thread",
    "external_review_deadletter",
    "external_review_finding",
    "external_review_finding_state_history",
    "external_review_kpi_snapshot",
    "external_review_outcome",
    "external_review_run",
    "external_status_snapshots",
    "external_submission_attempts",
    "external_submission_jobs",
    "gamify_battles",
    "gamify_companions",
    "gamify_profiles",
    "gamify_quests",
    "grpo_training_run",
    "harness_eval_run",
    "harness_eval_task_result",
    "history_entries",
    "hitl_approvals",
    "hopper_inbox",
    "kb_entries",
    "kb_routing_rules",
    "knowledge_bases",
    "knowledge_edges",
    "knowledge_nodes",
    "learned_patterns",
    "live_chat_completeness_pending",
    "llm_attempts",
    "llm_feedback",
    "llm_interactions",
    "local_train_log",
    "lock_leader",
    "memories",
    "mens_corpus_quality",
    "mens_observer_observations",
    "mesh_heartbeats",
    "metadata",
    "model_pricing_catalog",
    "model_prompt_profiles",
    "model_scoreboard",
    "model_selection_event",
    "names",
    "news_publish_approvals",
    "news_publish_approvals_v2",
    "news_publish_attempts",
    "node_identities",
    "node_trust_grants",
    "objects",
    "observer_events",
    "orchestration_lineage_events",
    "package_deps",
    "packages",
    "plan_node_attempts",
    "plan_nodes",
    "plan_sessions",
    "plan_test_decisions",
    "plan_versions",
    "populi_reviews",
    "populi_training_run",
    "processing_run_steps",
    "processing_runs",
    "publication_approvals",
    "publication_attempts",
    "publication_external_links",
    "publication_external_revisions",
    "publication_manifests",
    "publication_media_assets",
    "publication_status_events",
    "published_news",
    "question_events",
    "question_option_outcomes",
    "question_options",
    "question_sessions",
    "question_stop_events",
    "reconstruction_artifacts",
    "reconstruction_benchmark_kpis",
    "reconstruction_campaign_spec",
    "reliability_scores",
    "repository_reliability",
    "research_eval_runs",
    "research_eval_samples",
    "research_metrics",
    "research_sessions",
    "routing_decisions",
    "scheduled",
    "scheduled_runs",
    "scholarly_publication_records",
    "scholarly_submissions",
    "scientia_citations",
    "scientia_claim_verdicts",
    "scientia_claims",
    "scientia_discoveries",
    "scientia_external_intelligence",
    "scientia_feed_sources",
    "scientia_finding_candidates",
    "scientia_harness_decisions",
    "scientia_harness_fix_proposals",
    "scientia_harness_issues",
    "scientia_model_profile_learning",
    "scientia_nanopubs",
    "scientia_prereg",
    "scientia_provider_runs",
    "scientia_publication_attempts",
    "scientia_publication_queue",
    "scientia_research_artifacts",
    "scientia_research_sessions",
    "scientia_review_decisions",
    "scientia_training_pairs",
    "search_document_chunks",
    "search_documents",
    "search_indexing_jobs",
    "session_turns",
    "skill_candidates",
    "skill_executions",
    "skill_identities",
    "skill_manifests",
    "skill_reliability",
    "snippets",
    "syndication_events",
    "test_decisions",
    "toestub_baselines",
    "toestub_file_cache",
    "toestub_suppressions",
    "toestub_task_queue",
    "topic_evolution_events",
    "topics",
    "training_throughput_profiles",
    "trust_observations",
    "trust_rollups",
    "trusted_evidence_bundles",
    "typed_stream_events",
    "usage_counter_snapshots",
    "usage_limit_definitions",
    "user_identities",
    "user_preferences",
    "users",
    "vcs_lock",
    "victory_verdicts",
    "visus_audit_log",
    "visus_baselines",
    "vox_kudos",
    "vox_peer_reputation",
    "workflow_activity_attempt_log",
    "workflow_activity_log",
    "workflow_executions",
    "workflow_reliability",
    "workflow_run_log",
    "workflow_signal_log",
];

/// List user-defined table names (excludes `sqlite_%`), sorted by SQLite.
pub async fn list_sqlite_user_tables(conn: &turso::Connection) -> Result<Vec<String>, StoreError> {
    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            (),
        )
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(
            row.get::<String>(0)
                .map_err(|e| StoreError::Db(e.to_string()))?,
        );
    }
    Ok(out)
}

async fn pragma_foreign_keys_enabled(conn: &turso::Connection) -> Result<bool, StoreError> {
    let mut rows = conn
        .query("PRAGMA foreign_keys", ())
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;
    let v: i64 = rows
        .next()
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?
        .map(|r| r.get::<i64>(0))
        .transpose()
        .map_err(|e| StoreError::Db(e.to_string()))?
        .unwrap_or(0);
    Ok(v != 0)
}

fn sql_value_to_json(v: SqlValue) -> serde_json::Value {
    match v {
        SqlValue::Null => serde_json::Value::Null,
        SqlValue::Integer(i) => serde_json::json!(i),
        SqlValue::Real(f) => serde_json::json!(f),
        SqlValue::Text(s) => serde_json::Value::String(s),
        SqlValue::Blob(b) => {
            serde_json::Value::String(format!("base64:{}", data_encoding::BASE64.encode(&b)))
        }
    }
}

/// Write one JSON object per line: `{"table":"…","columns":[…],"row":{…}}`.
///
/// Despite the line-delimited output shape, this does NOT stream row-by-row
/// from the database: `GuardedConnection::query` (see its doc comment)
/// materializes a table's full result set into memory before this function
/// ever sees the first row, to keep the connection's mutex guard scoped to
/// the actual query instead of the caller's iteration. A very large legacy
/// table is read entirely into memory per `SELECT`, one table at a time.
pub async fn export_legacy_jsonl<W: Write>(
    store: &crate::VoxDb,
    writer: &mut W,
) -> Result<u64, StoreError> {
    let mut count = 0u64;
    for table in LEGACY_EXPORT_TABLES {
        let pragma = format!("PRAGMA table_info({table})");
        let mut cols_rows: crate::GuardedRows = store.connection().query(&pragma, ()).await?;
        let mut columns = Vec::new();
        while let Some(r) = cols_rows.next().await? {
            let name: String = r.get(1)?;
            columns.push(name);
        }
        if columns.is_empty() {
            continue;
        }
        let select = format!("SELECT {} FROM {}", columns.join(", "), table);
        let mut rows: crate::GuardedRows = store
            .connection()
            .query(&select, ())
            .await
            .map_err(|e| StoreError::Db(format!("legacy export {table}: {e}")))?;
        while let Some(row) = rows.next().await? {
            let mut obj = serde_json::Map::new();
            for (i, col) in columns.iter().enumerate() {
                let v = row.get_value(i)?;
                obj.insert(col.clone(), sql_value_to_json(v));
            }
            let line = serde_json::json!({
                "table": table,
                "columns": columns,
                "row": serde_json::Value::Object(obj),
            });
            writeln!(
                writer,
                "{}",
                serde_json::to_string(&line)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?
            )
            .map_err(|e| StoreError::Db(format!("legacy export write: {e}")))?;
            count += 1;
        }
    }
    Ok(count)
}

/// Apply JSONL from [`export_legacy_jsonl`]. Clears each allowlisted table, then uses plain
/// `INSERT` with bound parameters.
///
/// **Why not `INSERT OR REPLACE`:** the bundled `turso`/`libsql` stack mis-binds positional parameters
/// for `INSERT OR REPLACE` in this crate’s tests (values shift; e.g. `xp` becomes `0`). Deleting
/// snapshot tables first matches the documented workflow (**fresh baseline** target DB).
///
/// **Foreign keys:** disabled for the transaction so row order in JSONL need not respect FK edges;
/// the previous `PRAGMA foreign_keys` setting is restored afterward.
pub async fn import_legacy_jsonl<R: BufRead>(
    store: &crate::VoxDb,
    reader: R,
) -> Result<u64, StoreError> {
    let conn = store.conn.clone();
    let fk_was_on = pragma_foreign_keys_enabled(&conn).await?;
    conn.execute("PRAGMA foreign_keys = OFF", ())
        .await
        .map_err(|e| StoreError::Db(format!("legacy import pragma foreign_keys off: {e}")))?;
    conn.execute("BEGIN IMMEDIATE", ())
        .await
        .map_err(|e| StoreError::Db(format!("legacy import begin: {e}")))?;

    let body = async {
        // Replace semantics: clear allowlisted user tables so a second import does not append duplicates.
        // `schema_version` is not in LEGACY_EXPORT_TABLES — baseline row remains intact.
        for table in LEGACY_EXPORT_TABLES.iter().rev() {
            let table = *table;
            let sql = format!("DELETE FROM {table}");
            let c = conn.clone();
            let breaker = store.breaker.clone();
            breaker
                .call(|| async move {
                    c.execute(&sql, ())
                        .await
                        .map_err(|e| StoreError::Db(format!("legacy import clear {table}: {e}")))?;
                    Ok::<(), StoreError>(())
                })
                .await?;
        }
        let mut applied = 0u64;
        for line in reader.lines() {
            let line = line.map_err(|e| StoreError::Db(format!("legacy import read: {e}")))?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(trimmed)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            let table = v
                .get("table")
                .and_then(|x| x.as_str())
                .ok_or_else(|| StoreError::Db("legacy import line missing table".into()))?;
            let row = v
                .get("row")
                .and_then(|x| x.as_object())
                .ok_or_else(|| StoreError::Db("legacy import line missing row object".into()))?;
            if !LEGACY_EXPORT_TABLES.contains(&table) {
                continue;
            }
            let columns: Vec<String> =
                if let Some(arr) = v.get("columns").and_then(|c| c.as_array()) {
                    let out: Vec<String> = arr
                        .iter()
                        .filter_map(|x| x.as_str().map(std::string::ToString::to_string))
                        .collect();
                    if out.is_empty() {
                        row.keys().cloned().collect()
                    } else {
                        out
                    }
                } else {
                    row.keys().cloned().collect()
                };
            if columns.is_empty() {
                continue;
            }
            // Validate column names before interpolating into SQL — they come from user JSONL.
            for col in &columns {
                validate_identifier(col).map_err(|e| {
                    StoreError::Db(format!("legacy import invalid column name {col:?}: {e}"))
                })?;
            }
            let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                table,
                columns.join(", "),
                placeholders.join(", ")
            );
            let mut values: Vec<SqlValue> = Vec::with_capacity(columns.len());
            for c in &columns {
                let cell = row.get(c).unwrap_or(&serde_json::Value::Null);
                values.push(json_to_sql_value(cell)?);
            }
            let c = conn.clone();
            let breaker = store.breaker.clone();
            breaker
                .call(|| async move {
                    c.execute(&sql, values).await?;
                    Ok::<(), StoreError>(())
                })
                .await?;
            applied += 1;
        }
        Ok::<u64, StoreError>(applied)
    };

    let restore_fk = async {
        if fk_was_on {
            conn.execute("PRAGMA foreign_keys = ON", ())
                .await
                .map_err(|e| StoreError::Db(format!("legacy import restore foreign_keys: {e}")))?;
        } else {
            conn.execute("PRAGMA foreign_keys = OFF", ())
                .await
                .map_err(|e| StoreError::Db(format!("legacy import restore foreign_keys: {e}")))?;
        }
        Ok::<(), StoreError>(())
    };

    match body.await {
        Ok(n) => {
            if let Err(e) = conn.execute("COMMIT", ()).await {
                // COMMIT failed — roll back to prevent partial-state leak.
                let _ = conn.execute("ROLLBACK", ()).await;
                let _ = restore_fk.await;
                return Err(StoreError::Db(format!("legacy import commit: {e}")));
            }
            restore_fk.await?;
            Ok(n)
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", ()).await;
            let _ = restore_fk.await;
            Err(e)
        }
    }
}

fn json_to_sql_value(v: &serde_json::Value) -> Result<SqlValue, StoreError> {
    Ok(match v {
        serde_json::Value::Null => SqlValue::Null,
        serde_json::Value::Bool(b) => {
            if *b {
                SqlValue::Integer(1)
            } else {
                SqlValue::Integer(0)
            }
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlValue::Integer(i)
            } else if let Some(u) = n.as_u64() {
                let i = i64::try_from(u).map_err(|_| {
                    StoreError::Serialization(format!("JSON integer too large: {u}"))
                })?;
                SqlValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                SqlValue::Real(f)
            } else {
                SqlValue::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => {
            if let Some(rest) = s.strip_prefix("base64:") {
                let bytes = data_encoding::BASE64
                    .decode(rest.as_bytes())
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                SqlValue::Blob(bytes)
            } else {
                SqlValue::Text(s.clone())
            }
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => SqlValue::Text(
            serde_json::to_string(v).map_err(|e| StoreError::Serialization(e.to_string()))?,
        ),
    })
}
