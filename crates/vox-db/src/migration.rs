//! User-defined SQL migrations sharing the same `schema_version` table as built-in Arca migrations.
//!
//! Prefer [`crate::builtin_migrations`] when you need the canonical baseline snapshot as a single
//! migration row at [`crate::schema::BASELINE_VERSION`]. For custom migrations, ensure [`Migration::up_sql`] is compatible with
//! [`turso::Connection::execute_batch`] (no row-returning statements).
//!
//! **Warning:** versions other than [`crate::schema::BASELINE_VERSION`] make `MAX(schema_version) != BASELINE_VERSION`,
//! so the next normal [`crate::VoxDb::connect`] will report [`crate::StoreError::LegacySchemaChain`]. Use custom
//! [`Migration`] rows only on ephemeral DBs, tests, or with a plan to re-baseline the file.

use crate::store::StoreError;

/// One forward migration applied in monotonically increasing [`Self::version`] order.
#[derive(Debug, Clone)]
pub struct Migration {
    /// Must be unique, greater than zero, and strictly increasing across the slice passed to [`crate::VoxDb::apply_migrations`].
    pub version: i64,
    /// Human-readable label (logging only; not stored in DB).
    pub name: String,
    /// Semicolon-separated SQL executed via `execute_batch` when `version` is ahead of the DB.
    pub up_sql: String,
}

impl Migration {
    /// Construct a migration entry (does not run SQL until [`crate::VoxDb::apply_migrations`]).
    pub fn new(version: i64, name: impl Into<String>, up_sql: impl Into<String>) -> Self {
        Self {
            version,
            name: name.into(),
            up_sql: up_sql.into(),
        }
    }
}

/// Validate strictly increasing versions and no duplicates.
///
/// **Note:** validation failures are reported as [`StoreError::InvalidMigration`].
pub fn validate_migrations(migrations: &[Migration]) -> Result<(), StoreError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut last = 0i64;
    for migration in migrations {
        if migration.version <= 0 {
            return Err(StoreError::InvalidMigration(
                "migration version must be > 0".to_string(),
            ));
        }
        if migration.version <= last {
            return Err(StoreError::InvalidMigration(
                "migrations must be sorted by increasing version".to_string(),
            ));
        }
        if !seen.insert(migration.version) {
            return Err(StoreError::InvalidMigration(format!(
                "duplicate migration version {}",
                migration.version
            )));
        }
        last = migration.version;
    }
    Ok(())
}

/// Returns the canonical baseline migration at [`crate::schema::BASELINE_VERSION`] from [`crate::schema::baseline_sql`].
pub fn builtin_migrations() -> Vec<Migration> {
    vec![Migration::new(
        crate::schema::BASELINE_VERSION,
        "arca_baseline",
        crate::schema::baseline_sql().to_string(),
    )]
}

// ---------------------------------------------------------------------------------------------
// Existing-DB quarantine drop (Task 5, docs/src/architecture/2026-08-01-voxdb-audit-condensation-plan.md)
// ---------------------------------------------------------------------------------------------
//
// Databases created before the Task 4 re-baseline (schema_version < 85) still physically contain
// all 219 pre-condensation tables, including the 48 that Task 4 quarantined out of the default
// `baseline_sql()`. This section provides a two-phase, Rust-side-orchestrated mechanism to bring
// such a file in line with the new baseline by dropping those 48 tables, *if and only if* every
// one of them is empty.
//
// This CANNOT be a `Migration.up_sql` batch: `up_sql` runs through `execute_batch`, which cannot
// branch on a query result, and a `SELECT COUNT(*)`-then-abort has to inspect a row. See the
// module doc above and the plan's "Blocker-level correction" note under Task 5.
//
// This also does NOT mint a new `schema_version` entry of its own. The existing
// `VoxDb::migrate()` (`crate::store::open`) already advances any DB with `schema_version <
// BASELINE_VERSION` to exactly `BASELINE_VERSION` (85) via `baseline_sql()`, which is idempotent
// (`CREATE TABLE IF NOT EXISTS`) and no longer declares the 48 quarantined tables. Phase 2 here
// only adds the `DROP TABLE` step for the tables `baseline_sql()` silently leaves behind, then
// defers to `VoxDb::migrate()` for the actual version bump — so there remains exactly one code
// path that ever writes `schema_version = 85` (`VoxDb::migrate`), not a second competing one.
//
// **No automated rollback.** `Migration` has no `down_sql`/reverse mechanism anywhere in this
// crate, and this mechanism doesn't add one. `DROP TABLE` is not reversible short of restoring
// from a file-level backup. **Callers of [`migrate_dropping_quarantine`] must keep a copy of the
// database file (e.g. `.vox/store.db`) taken immediately before invoking it.** This function does
// not take that backup for you.
//
// **Not wired into `VoxDb::connect()`/`open()` by this change.** This is an explicit, opt-in
// upgrade step a caller invokes deliberately (e.g. a `vox` upgrade command), not something that
// runs implicitly on every local launch. If a future change does wire it into the automatic
// connect path, the abort error message above is worded to make that scenario's recovery story
// explicit regardless: a `QuarantineDropAborted` means the database is pinned below the new
// baseline until the named table(s) are cleared or exported, and that will recur on every
// subsequent attempt until resolved by hand.

/// The 47 (of Task 4's 48 quarantined) tables this automated migration will `DROP TABLE` for.
///
/// This is 48 minus `developer_journey_definitions`: per the plan's Task 2.1 decision, that
/// table's `CREATE TABLE` DDL bakes in an unconditional `INSERT OR IGNORE ... VALUES
/// ('canonical_journey.v1.greenfield_vox_mens_devloop', ...)` seed row that fires on every
/// `baseline_sql()` run — so *every* pre-Task-4 database has exactly one row in it, always, by
/// construction, not because of real user data. Including it in the auto-DROP list would make
/// [`precheck_quarantine_tables_empty`] abort on literally every legacy database forever (this
/// was confirmed empirically: running this migration against a copy of the real
/// `.vox/store.db` aborted on exactly this table alone during Task 5.2's sanity check). Per the
/// plan, this table is excluded from the automated drop pending a one-time manual row export; its
/// DDL still moved to `schema/domains/quarantine.rs` in Task 4, it's just not in scope for this
/// mechanism. `crate::schema::domains::quarantine::SCHEMA_QUARANTINE` still declares it (42
/// tables total there, 41 of which are also in this list).
///
/// This is the single source of truth for [`migrate_dropping_quarantine`]'s drop list; keep it in
/// sync with `crate::schema::domains::quarantine::SCHEMA_QUARANTINE` (42 tables with literal DDL,
/// including `developer_journey_definitions`), `handoff_payloads` (declared only via
/// `CollectionInfo`, see `schema::spec::orchestrator_schema_digest`), and the 5 fully-orphaned
/// tables with no declaration anywhere in current source (`archive_membership`, `chunk_members`,
/// `context_window_items`, `context_windows`, `zstd_dictionaries`).
pub const QUARANTINE_DROP_TABLES: &[&str] = &[
    // 41 tables with literal DDL, moved to schema/domains/quarantine.rs by Task 4.
    // (developer_journey_definitions, the 42nd, is deliberately excluded — see doc comment above.)
    "activity_result_cache",
    "artifact_reviews",
    "builder_sessions",
    "codex_change_log",
    "codex_projection_versions",
    "codex_query_snapshots",
    "codex_schema_lineage",
    "codex_subscriptions",
    "conversation_edges",
    "conversation_message_topics",
    "conversation_tool_calls",
    "conversation_topics",
    "conversation_versions",
    "external_review_outcome",
    "news_publish_approvals",
    "package_deps",
    "populi_reviews",
    "processing_run_steps",
    "processing_runs",
    "publication_external_links",
    "publication_external_revisions",
    "question_option_outcomes",
    "scholarly_publication_records",
    "scientia_citations",
    "scientia_prereg",
    "scientia_provider_runs",
    "scientia_publication_attempts",
    "scientia_training_pairs",
    "search_indexing_jobs",
    "session_turns",
    "skill_executions",
    "skill_reliability",
    "syndication_events",
    "test_decisions",
    "toestub_file_cache",
    "topic_evolution_events",
    "trusted_evidence_bundles",
    "typed_stream_events",
    "usage_counter_snapshots",
    "usage_limit_definitions",
    "workflow_executions",
    // Declared only via `CollectionInfo` (schema::spec::orchestrator_schema_digest), not DDL.
    "handoff_payloads",
    // Fully orphaned: no declaration anywhere in current source (leftover from a removed feature).
    "archive_membership",
    "chunk_members",
    "context_window_items",
    "context_windows",
    "zstd_dictionaries",
];

/// Phase 1: refuse to drop anything if any quarantined table still has rows.
///
/// Runs `SELECT COUNT(*) FROM <table>` for each of [`QUARANTINE_DROP_TABLES`] individually (not
/// batched — a batch can't branch on the result). A "no such table" error is treated as count 0
/// (safe to skip) rather than propagated, since older or partial-baseline database files may be
/// missing some of these tables entirely.
///
/// Returns `Ok(())` if every table is empty or absent. Returns
/// [`StoreError::QuarantineDropAborted`] naming every non-empty table if any has rows; the caller
/// must not proceed to [`migrate_dropping_quarantine`]'s drop step in that case. This function
/// performs no writes.
pub async fn precheck_quarantine_tables_empty(
    conn: &turso::Connection,
) -> Result<(), StoreError> {
    let mut non_empty = Vec::new();
    for table in QUARANTINE_DROP_TABLES {
        let quoted = quote_ident(table);
        match conn
            .query(&format!("SELECT COUNT(*) FROM {quoted}"), ())
            .await
        {
            Ok(mut rows) => {
                let count: i64 = match rows.next().await? {
                    Some(row) => row.get(0)?,
                    None => 0,
                };
                if count > 0 {
                    non_empty.push((*table).to_string());
                }
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.to_lowercase().contains("no such table") {
                    // Older/partial-baseline DB never had this table — safe to skip.
                    continue;
                }
                return Err(StoreError::from(e));
            }
        }
    }
    if !non_empty.is_empty() {
        return Err(StoreError::QuarantineDropAborted { tables: non_empty });
    }
    Ok(())
}

/// Phase 2 + orchestration: drop every quarantined table (Phase 1 permitting) and advance
/// `schema_version` to [`crate::schema::BASELINE_VERSION`] via the normal [`crate::VoxDb::migrate`]
/// path.
///
/// Runs [`precheck_quarantine_tables_empty`] first. If it errors, this function returns that same
/// error immediately — no `DROP TABLE` is issued and the database is left completely untouched
/// (including `schema_version`, which does not advance). If it passes, issues an unconditional
/// `DROP TABLE IF EXISTS` for every table in [`QUARANTINE_DROP_TABLES`], then calls
/// [`crate::VoxDb::migrate`] to bring `schema_version` to exactly `BASELINE_VERSION` — the same
/// version fresh installs reach, via the same single code path.
///
/// **No automated rollback and no backup taken here** — see the module-level note above. Callers
/// must keep their own backup of the database file before invoking this.
pub async fn migrate_dropping_quarantine(conn: &turso::Connection) -> Result<(), StoreError> {
    precheck_quarantine_tables_empty(conn).await?;
    for table in QUARANTINE_DROP_TABLES {
        let quoted = quote_ident(table);
        conn.execute(&format!("DROP TABLE IF EXISTS {quoted}"), ())
            .await
            .map_err(StoreError::from)?;
    }
    crate::VoxDb::migrate(conn).await
}

fn quote_ident(name: &str) -> String {
    let mut s = String::with_capacity(name.len() + 2);
    s.push('"');
    for c in name.chars() {
        if c == '"' {
            s.push_str("\"\"");
        } else {
            s.push(c);
        }
    }
    s.push('"');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_sorted_unique() {
        let migrations = vec![
            Migration::new(1, "one", "CREATE TABLE a(id INTEGER);"),
            Migration::new(2, "two", "CREATE TABLE b(id INTEGER);"),
        ];
        assert!(validate_migrations(&migrations).is_ok());
    }

    // -----------------------------------------------------------------------------------------
    // 5.1a: quarantine-drop safety-rail tests (Task 5, voxdb-audit-condensation plan).
    // -----------------------------------------------------------------------------------------

    async fn mem_conn() -> turso::Connection {
        let db = turso::Builder::new_local(":memory:")
            .build()
            .await
            .expect("build mem db");
        db.connect().expect("connect")
    }

    async fn row_count(conn: &turso::Connection, table: &str) -> i64 {
        let mut rows = conn
            .query(&format!("SELECT COUNT(*) FROM {table}"), ())
            .await
            .expect("count query");
        rows.next()
            .await
            .expect("next")
            .expect("row")
            .get(0)
            .expect("count")
    }

    async fn schema_version(conn: &turso::Connection) -> i64 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .await
        .expect("ensure schema_version table");
        let mut rows = conn
            .query("SELECT COALESCE(MAX(version), 0) FROM schema_version", ())
            .await
            .expect("query max version");
        rows.next()
            .await
            .expect("next")
            .expect("row")
            .get(0)
            .expect("version")
    }

    /// (a) A non-empty to-be-quarantined table aborts the drop, leaves every table intact
    /// (including the non-empty one), and does not advance `schema_version`.
    #[tokio::test]
    async fn quarantine_drop_aborts_on_non_empty_table_and_leaves_db_untouched() {
        let conn = mem_conn().await;

        // Simulate an old, pre-Task-4 DB copy: a handful of quarantined tables physically exist,
        // one of them (`workflow_executions`) has a real row, plus a live table for control.
        conn.execute_batch(
            "CREATE TABLE workflow_executions (id INTEGER PRIMARY KEY, note TEXT);
             INSERT INTO workflow_executions (note) VALUES ('real data, must survive');
             CREATE TABLE skill_reliability (id INTEGER PRIMARY KEY);
             CREATE TABLE conversations (id INTEGER PRIMARY KEY, title TEXT);",
        )
        .await
        .expect("seed old-schema tables");

        let version_before = schema_version(&conn).await;

        let result = migrate_dropping_quarantine(&conn).await;
        let err = result.expect_err("must abort: workflow_executions has rows");
        match &err {
            StoreError::QuarantineDropAborted { tables } => {
                assert!(
                    tables.iter().any(|t| t == "workflow_executions"),
                    "error must name the offending table, got: {tables:?}"
                );
            }
            other => panic!("expected QuarantineDropAborted, got: {other:?}"),
        }
        // Named table + one-line remediation must be in the rendered message.
        let msg = err.to_string();
        assert!(msg.contains("workflow_executions"), "msg: {msg}");
        assert!(
            msg.to_lowercase().contains("export") && msg.to_lowercase().contains("retry"),
            "msg must give a remediation hint: {msg}"
        );

        // Every table, including the non-empty one, is untouched.
        assert_eq!(row_count(&conn, "workflow_executions").await, 1);
        assert_eq!(row_count(&conn, "skill_reliability").await, 0);
        assert_eq!(row_count(&conn, "conversations").await, 0);

        // schema_version did not advance.
        assert_eq!(schema_version(&conn).await, version_before);
        assert_ne!(
            schema_version(&conn).await,
            crate::schema::BASELINE_VERSION,
            "abort must not advance schema_version to baseline"
        );
    }

    /// (b) All quarantined tables empty (or entirely absent) — drop succeeds and schema_version
    /// advances to exactly BASELINE_VERSION (85). Also covers a table that doesn't exist at all
    /// in the DB copy (older/partial baseline): treated as 0 rows, not an error.
    ///
    /// Not run under `--features quarantine`: with that feature on, `baseline_sql()` itself
    /// re-declares the quarantined DDL, so the subsequent `VoxDb::migrate()` call inside
    /// `migrate_dropping_quarantine` legitimately recreates the tables this test just dropped —
    /// that's correct behavior for an opt-in quarantine build, not a bug, but it means the
    /// "table is gone after the call" assertion below only holds for the default (non-quarantine)
    /// baseline this migration path exists to serve.
    #[cfg(not(feature = "quarantine"))]
    #[tokio::test]
    async fn quarantine_drop_succeeds_when_all_empty_and_advances_to_baseline() {
        let conn = mem_conn().await;

        // Simulate an old DB where only *some* quarantined tables physically exist (empty), and
        // others (e.g. `handoff_payloads`, the 5 fully-orphaned ones) are simply absent — an
        // older/partial baseline that predates them.
        conn.execute_batch(
            "CREATE TABLE workflow_executions (id INTEGER PRIMARY KEY, note TEXT);
             CREATE TABLE skill_reliability (id INTEGER PRIMARY KEY);",
        )
        .await
        .expect("seed old-schema tables");
        // Note: handoff_payloads, archive_membership, chunk_members, context_window_items,
        // context_windows, zstd_dictionaries, and most of QUARANTINE_DROP_TABLES are absent here
        // entirely — exercising the "no such table" => safe-to-skip path. `conversations` is
        // deliberately NOT pre-seeded with a fake schema here: it's a real baseline table, and
        // migrate()'s subsequent `CREATE TABLE IF NOT EXISTS` would silently no-op over a
        // mismatched hand-rolled shape, which isn't what this test is checking.

        migrate_dropping_quarantine(&conn)
            .await
            .expect("must succeed: every quarantined table is empty or absent");

        // Dropped tables are gone.
        let mut rows = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name = 'workflow_executions'",
                (),
            )
            .await
            .expect("query sqlite_master");
        assert!(
            rows.next().await.expect("next").is_none(),
            "workflow_executions must be dropped"
        );
        let mut rows = conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name = 'skill_reliability'",
                (),
            )
            .await
            .expect("query sqlite_master");
        assert!(
            rows.next().await.expect("next").is_none(),
            "skill_reliability must be dropped"
        );

        // migrate() ran the full baseline over the connection: a genuine live baseline table now
        // exists with its real schema (not just the quarantined tables being gone).
        assert_eq!(row_count(&conn, "conversations").await, 0);

        assert_eq!(schema_version(&conn).await, crate::schema::BASELINE_VERSION);
    }

    /// Absent quarantined table alone (no non-empty tables at all) is safe and not an error —
    /// dedicated coverage for the "doesn't exist at all" case independent of the success-path test
    /// above, per 5.1a's explicit requirement.
    #[tokio::test]
    async fn precheck_treats_missing_table_as_empty_not_error() {
        let conn = mem_conn().await;
        // No quarantined tables exist at all in this fresh DB.
        precheck_quarantine_tables_empty(&conn)
            .await
            .expect("missing tables must be treated as count-0, not an error");
    }
}
