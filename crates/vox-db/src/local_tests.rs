#![allow(deprecated)]
//! Integration tests for `VoxDb` when `local` feature is enabled (`connect(DbConfig::Local/::Memory)` paths).
// Intentionally exercises the deprecated codex_legacy surface (these tests guard it until removal).

use super::*;
use crate::schema::{BASELINE_VERSION, CODEX_CHAT_TABLES};

#[tokio::test]
async fn cas_store_and_load_is_idempotent() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    let hash = db.store("test_kind", b"test_data").await.expect("store");
    let data = db.get(&hash).await.expect("get");
    assert_eq!(data, b"test_data");
}

#[tokio::test]
async fn schema_init_v7_is_ok() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let v = db.schema_version().await.expect("version");
    assert_eq!(v, BASELINE_VERSION);
}

// codex_change_log is quarantined (DORMANT, Task 4, VoxDB audit condensation
// plan) — off by default, see schema/domains/quarantine.rs.
#[tokio::test]
#[cfg(feature = "quarantine")]
async fn append_codex_change_is_ok() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    let id = db
        .append_codex_change("test.topic", None, None, "upsert", None)
        .await
        .expect("append");
    assert!(id > 0);
}

#[tokio::test]
async fn test_connect_memory() {
    let db = VoxDb::connect(DbConfig::Memory)
        .await
        .expect("Failed to connect to memory DB");
    let hash = db
        .store("test_kind", b"test_data")
        .await
        .expect("Store failed");
    assert!(!hash.is_empty());
}

#[tokio::test]
async fn codex_alias_connects() {
    let db: Codex = VoxDb::connect(DbConfig::Memory).await.expect("db");
    assert_eq!(db.schema_version().await.expect("v"), BASELINE_VERSION);
}

// Table list trimmed 2026-08-02 to drop tables slated for schema quarantine
// (docs/src/architecture/2026-08-01-voxdb-audit-condensation-plan.md, Task 3) —
// see graphify-out/quarantine_test_findings.json for the full disposition.
#[tokio::test]
async fn baseline_schema_includes_chat_and_search_tables() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    assert_eq!(
        db.schema_version().await.expect("schema_version"),
        BASELINE_VERSION
    );
    for t in CODEX_CHAT_TABLES {
        let rows = db
            .query_all(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                turso::params![t.to_string()],
            )
            .await
            .expect("sqlite_master");
        assert!(!rows.is_empty(), "missing table {t}");
    }
    for t in ["search_documents", "search_document_chunks"] {
        let rows = db
            .query_all(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                turso::params![t.to_string()],
            )
            .await
            .expect("search table");
        assert!(!rows.is_empty(), "missing search table {t}");
    }
    for t in ["audit_log"] {
        let rows = db
            .query_all(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                (t.to_string(),),
            )
            .await
            .expect("sqlite_master");
        assert!(!rows.is_empty(), "missing V16 table {t}");
    }
    // conversation_versions/conversation_edges/topic_evolution_events restored
    // 2026-08-02 — un-quarantined, see schema/domains/conversations.rs.
    for t in [
        "research_sessions",
        "conversation_versions",
        "conversation_edges",
        "topic_evolution_events",
    ] {
        let rows = db
            .query_all(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                (t.to_string(),),
            )
            .await
            .expect("sqlite_master");
        assert!(!rows.is_empty(), "missing V17 table {t}");
    }
}

#[tokio::test]
async fn raw_sqlite_gamify_profiles_integer_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("raw.db");
    let db = VoxDb::connect(DbConfig::Local {
        path: p.to_string_lossy().into_owned(),
    })
    .await
    .expect("db");
    db.connection()
        .execute(
            "INSERT INTO gamify_profiles (user_id, level, xp) VALUES (?1, ?2, ?3)",
            turso::params!["u1", 3i64, 900i64],
        )
        .await
        .expect("insert");
    let mut q = db
        .connection()
        .query(
            "SELECT xp FROM gamify_profiles WHERE user_id = ?1",
            turso::params!["u1"],
        )
        .await
        .expect("sel");
    let row = q.next().await.expect("r").expect("row");
    assert_eq!(row.get::<i64>(0).expect("xp"), 900);
}

async fn seed_legacy_schema_version_only(path: &std::path::Path, version: i64) {
    let s = path.to_string_lossy().to_string();
    let built = turso::Builder::new_local(&s)
        .build()
        .await
        .expect("legacy build");
    let conn = built.connect().expect("conn");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
    .await
    .expect("schema_version ddl");
    conn.execute(
        "INSERT INTO schema_version (version) VALUES (?1)",
        turso::params![version],
    )
    .await
    .expect("insert version");
}

/// [`VoxDb::connect_default`] returns [`StoreError::LegacySchemaChain`] when the primary DB is not on baseline (no sidecar fallback).
#[allow(unsafe_code)] // Rust 2024: `set_var` / `remove_var` are `unsafe`; mutex serializes this test.
#[allow(clippy::await_holding_lock)] // Lock intentionally held across awaits to serialize env-mutating tests.
#[tokio::test]
async fn connect_default_errors_when_primary_legacy_schema_chain() {
    use std::sync::{Mutex, OnceLock};

    static DATA_DIR_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _g = DATA_DIR_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

    let dir = tempfile::tempdir().expect("tempdir");
    seed_legacy_schema_version_only(&dir.path().join("vox.db"), 99).await;

    let old = std::env::var("VOX_DATA_DIR").ok();
    // SAFETY: `DATA_DIR_LOCK` serializes tests that touch `VOX_DATA_DIR` for this module.
    unsafe {
        std::env::set_var("VOX_DATA_DIR", dir.path());
    }

    let err = match VoxDb::connect_default().await {
        Ok(_) => panic!("legacy primary should not open under baseline migrate"),
        Err(e) => e,
    };
    assert!(
        matches!(err, StoreError::LegacySchemaChain { max_version: 99 }),
        "expected LegacySchemaChain {{ max_version: 99 }}, got {err:?}"
    );

    unsafe {
        match &old {
            Some(s) => std::env::set_var("VOX_DATA_DIR", s),
            None => std::env::remove_var("VOX_DATA_DIR"),
        }
    }
}

#[test]
fn resolve_canonical_matches_resolve_standalone() {
    // Serialise against config::tests which mutate VOX_DB_URL / VOX_DB_TOKEN.
    let _guard = crate::TEST_ENV_LOCK.lock().expect("env lock");
    let a = DbConfig::resolve_canonical().expect("canonical");
    let b = DbConfig::resolve_standalone().expect("standalone");
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}

#[tokio::test]
async fn record_and_query_exec_time() {
    let dir = tempfile::tempdir().unwrap();
    let db = VoxDb::connect(DbConfig::local(
        dir.path().join("test.db").to_str().unwrap(),
    ))
    .await
    .unwrap();

    let record = crate::ExecTimeRecord {
        tool_key: "t1",
        repository_id: "r1",
        duration_ms: 1500,
        timeout_budget_ms: None,
        compute_tokens_used: Some(100),
        vendor_cost_usd_micros: Some(500),
        attention_cost_ms: Some(1500),
        outcome: crate::ExecOutcome::Success,
    };
    db.record_exec_time(&record).await.unwrap();
    db.record_exec_timeout("t1", "r1", 2000).await.unwrap();

    let latency = db.query_tool_latency("t1", "r1", 90, 1.5).await.unwrap();
    assert!(latency.is_some());
}

#[tokio::test]
async fn unified_llm_turn_writes_llm_and_socrates() {
    use vox_orchestrator_types::socrates_policy::RiskDecision;

    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    let outcome = crate::store::types::ModelOutcome {
        session_id: "s-unified",
        user_id: None,
        tenant_id: None,
        prompt: "p",
        response: "r",
        model_id: "openai/gpt-test",
        provider: "openrouter",
        task_category: "General",
        strength_tag: "generalist",
        latency_ms: Some(10),
        input_tokens: Some(3),
        output_tokens: Some(5),
        cache_read_tokens: Some(0),
        trace_id: Some("t1"),
        context_utilization_pct: None,
        success: true,
        cost_usd: Some(0.001),
        quality_score: Some(1.0),
        ttft_ms: None,
        tpot_ms: None,
    };
    let ids = db
        .record_unified_llm_turn(
            outcome,
            Some((
                "repo-hash".to_string(),
                "vox_chat_message".to_string(),
                RiskDecision::Answer,
                0.9,
                0.05,
                Some("openai/gpt-test".to_string()),
                Some(serde_json::json!({"task_class": "chat_turn"})),
            )),
        )
        .await
        .expect("unified");
    assert!(ids.llm_interaction_rowid > 0);
    assert!(ids.socrates_research_metric_rowid.unwrap_or(0) > 0);
}

#[tokio::test]
async fn list_model_arm_stats_aggregates_scoreboard_rows() {
    use crate::store::ModelScoreboardRow;

    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    let w: i64 = 7;
    let now = crate::now_unix_ms() as i64;
    for (task, sr) in [("CodeGen", 1.0_f64), ("Testing", 0.0_f64)] {
        db.upsert_model_scoreboard(ModelScoreboardRow {
            model_id: "openrouter/test-m".to_string(),
            task_category: task.to_string(),
            strength_tag: "generalist".to_string(),
            window_days: w,
            n_calls: 10,
            success_rate: sr,
            p50_latency_ms: None,
            p99_latency_ms: None,
            cost_per_success_usd: None,
            quality_score: 0.0,
            updated_at_ms: now,
            success_count: 0,
            cumulative_cost_usd: 0.0,
            p95_ttft_ms: None,
            p95_tpot_ms: None,
            goodput_tokens_per_sec: None,
        })
        .await
        .expect("upsert");
    }
    let map = db.list_model_arm_stats(w).await.expect("arm stats");
    assert_eq!(map.get("openrouter/test-m").copied(), Some((10, 10)));
}

#[tokio::test]
async fn get_last_llm_attempt_is_none_when_no_attempts_recorded() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    assert!(db.get_last_llm_attempt().await.expect("query").is_none());
}

#[tokio::test]
async fn get_last_llm_attempt_returns_most_recent_row_fresh() {
    use crate::store::types::ModelAttempt;

    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    db.record_llm_attempt(ModelAttempt {
        trace_id: "trace-1",
        attempt_number: 1,
        model_id: "openrouter/free-model",
        provider: "openrouter",
        outcome: "error",
        latency_ms: Some(0),
        error_class: Some("rate-limited"),
    })
    .await
    .expect("record attempt");

    let last = db
        .get_last_llm_attempt()
        .await
        .expect("query")
        .expect("a row was just recorded");
    assert_eq!(last.provider, "openrouter");
    assert_eq!(last.model_id, "openrouter/free-model");
    assert_eq!(last.outcome, "error");
    assert_eq!(last.error_class.as_deref(), Some("rate-limited"));
    // Just recorded via `datetime('now')` — should read back as a few seconds old at
    // most, never negative and never anywhere near the staleness window doctor uses.
    assert!(
        (0.0..30.0).contains(&last.age_seconds),
        "expected a freshly recorded attempt to have a small non-negative age, got {}",
        last.age_seconds
    );
}

#[tokio::test]
async fn get_last_llm_attempt_prefers_the_latest_of_multiple_rows() {
    use crate::store::types::ModelAttempt;

    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    db.record_llm_attempt(ModelAttempt {
        trace_id: "trace-1",
        attempt_number: 1,
        model_id: "openrouter/free-model",
        provider: "openrouter",
        outcome: "error",
        latency_ms: Some(0),
        error_class: Some("rate-limited"),
    })
    .await
    .expect("record first attempt");
    db.record_llm_attempt(ModelAttempt {
        trace_id: "trace-2",
        attempt_number: 1,
        model_id: "openrouter/free-model",
        provider: "openrouter",
        outcome: "success",
        latency_ms: Some(120),
        error_class: None,
    })
    .await
    .expect("record second attempt");

    // Both rows may land on the same `datetime('now')` second in SQLite (second-level
    // resolution); `get_last_llm_attempt`'s `ORDER BY created_at DESC, id DESC` tiebreak
    // on insertion order is what makes this deterministic rather than flaky.
    let last = db
        .get_last_llm_attempt()
        .await
        .expect("query")
        .expect("rows were recorded");
    assert_eq!(
        last.outcome, "success",
        "the second (later-inserted) attempt must win, not the first"
    );
    assert_eq!(last.error_class, None);
}

#[tokio::test]
async fn get_last_llm_attempt_reports_a_large_age_for_a_stale_row() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    // Insert directly (bypassing `record_llm_attempt`'s `datetime('now')` default) to
    // simulate an attempt recorded well outside any reasonable doctor staleness window.
    db.connection()
        .execute(
            "INSERT INTO llm_attempts
                 (trace_id, attempt_number, model_id, provider, outcome, latency_ms, error_class, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now', '-1 hour'))",
            turso::params![
                "trace-stale",
                1i32,
                "openrouter/free-model",
                "openrouter",
                "error",
                0i64,
                "rate-limited",
            ],
        )
        .await
        .expect("insert stale row");

    let last = db
        .get_last_llm_attempt()
        .await
        .expect("query")
        .expect("a row was inserted");
    assert!(
        last.age_seconds > 3000.0,
        "expected an ~1h-old row to report age_seconds well over any doctor staleness \
         window (a few minutes), got {}",
        last.age_seconds
    );
}

#[tokio::test]
async fn history_entries_round_trip() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
    db.connection()
        .execute(
            "INSERT INTO history_entries (repo_id, kind, text, redacted_text, created_at, pinned, source, token_estimate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            turso::params!["r1", "clip", "hello", "hello", 1000i64, 0i64, "cli", 1i64],
        )
        .await
        .expect("insert");
    let mut q = db
        .connection()
        .query(
            "SELECT kind FROM history_entries WHERE repo_id = ?1",
            turso::params!["r1"],
        )
        .await
        .expect("q");
    let row = q.next().await.expect("r").expect("row");
    let kind: String = row.get(0).expect("kind");
    assert_eq!(kind, "clip");
}

#[cfg(feature = "legacy-import")]
mod legacy_tests {
    use super::*;
    use crate::codex_schema::missing_codex_reactivity_tables;
    use crate::legacy::codex::{
        LEGACY_EXPORT_SKIP_TABLES, LEGACY_EXPORT_TABLES, export_legacy_jsonl, import_legacy_jsonl,
        list_sqlite_user_tables, verify_legacy_store,
    };
    use std::io::Cursor;
    use tempfile::tempdir;

    #[tokio::test]
    async fn codex_reactivity_schema_and_legacy_verify() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
        let v = db.schema_version().await.expect("version");
        assert_eq!(v, BASELINE_VERSION);
        assert!(
            missing_codex_reactivity_tables(&db)
                .await
                .expect("cap")
                .is_empty()
        );
        let leg = verify_legacy_store(&db).await.expect("verify");
        assert!(leg.has_codex_reactivity);
        assert!(!leg.is_legacy_schema_chain);
        let id = db
            .append_codex_change("test.topic", None, None, "upsert", None)
            .await
            .expect("change log");
        assert!(id > 0);
    }

    // Un-ignored 2026-09-04. This carried a long `#[ignore]` blaming a "suspected turso
    // 0.6.1 execute_batch bug" for three `scientia_harness_*` tables. That diagnosis rested
    // on the premise that `LEGACY_EXPORT_TABLES` *named* those three while `sqlite_master`
    // lacked them after migrate. Re-checked against the current tree, the first half was
    // simply not true: the list did not name them (0 occurrences, on `main` too), and
    // `sqlite_master` does contain all three. The delta was a stale list, not a lost
    // `CREATE TABLE`.
    //
    // Adding the three names — plus `live_chat_completeness_pending`, which `bd5c14e05`
    // introduced without updating the SSOT — makes this pass. Verified repeatedly, and
    // independently reproduced in review. If the batch-executor symptom ever returns it
    // will now surface as a failure here rather than as a silently skipped gate, which is
    // the point of keeping it live.
    #[tokio::test]
    async fn legacy_export_covers_all_baseline_tables() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
        let mut live = list_sqlite_user_tables(db.connection())
            .await
            .expect("list tables");
        live.retain(|n| !LEGACY_EXPORT_SKIP_TABLES.contains(&n.as_str()));
        live.sort();
        let mut expected: Vec<&str> = LEGACY_EXPORT_TABLES.to_vec();
        expected.sort();

        assert_eq!(
            live,
            expected.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "LEGACY_EXPORT_TABLES must match sqlite_master after migrate (minus skip list)"
        );
    }

    /// Gamification + coordination rows survive JSONL export/import on baseline DBs.
    // `distributed_locks` leg removed 2026-08-02 (quarantine-bound table, see
    // docs/src/architecture/2026-08-01-voxdb-audit-condensation-plan.md Task 3).
    #[tokio::test]
    async fn legacy_jsonl_roundtrips_gamification_and_coordination() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
        db.connection()
            .execute(
                "INSERT INTO gamify_profiles (user_id, level, xp) VALUES ('u1', 3, 900)",
                (),
            )
            .await
            .expect("insert profile");
        db.connection()
            .execute(
                "INSERT INTO gamify_companions (id, user_id, name, language) VALUES ('c1', 'u1', 'Ada', 'vox')",
                (),
            )
            .await
            .expect("insert companion");

        let mut jsonl = Vec::<u8>::new();
        let n = export_legacy_jsonl(&db, &mut jsonl).await.expect("export");
        assert!(n >= 2, "expected ≥2 rows, got {n}");
        let profile_lines = String::from_utf8_lossy(&jsonl)
            .lines()
            .filter(|l| l.contains("\"table\":\"gamify_profiles\""))
            .count();
        assert_eq!(
            profile_lines, 1,
            "export must emit exactly one gamify_profiles row"
        );
        let prof_json: serde_json::Value = String::from_utf8_lossy(&jsonl)
            .lines()
            .find(|l| l.contains("\"table\":\"gamify_profiles\""))
            .and_then(|l| serde_json::from_str(l).ok())
            .expect("parse profile jsonl");
        assert_eq!(
            prof_json["row"]["xp"].as_i64(),
            Some(900),
            "exported JSON must preserve xp: {}",
            prof_json["row"]
        );

        let dir = tempdir().expect("tempdir");
        let fresh_path = dir.path().join("roundtrip.db");
        let fresh_str = fresh_path.to_string_lossy().to_string();
        let db2 = VoxDb::connect(DbConfig::Local {
            path: fresh_str.clone(),
        })
        .await
        .expect("fresh file db");
        let imported = import_legacy_jsonl(&db2, Cursor::new(&jsonl))
            .await
            .expect("import");
        assert!(imported >= 2);

        let mut q = db2
            .connection()
            .query(
                "SELECT xp, level FROM gamify_profiles WHERE user_id = ?1",
                turso::params!["u1"],
            )
            .await
            .expect("q");
        let row = q.next().await.expect("row").expect("has row");
        assert_eq!(row.get::<i64>(0).expect("xp"), 900);
        assert_eq!(row.get::<i64>(1).expect("level"), 3);

        let mut q2 = db2
            .connection()
            .query(
                "SELECT name FROM gamify_companions WHERE id = ?1",
                turso::params!["c1"],
            )
            .await
            .expect("q2");
        let row2 = q2.next().await.expect("row").expect("r2");
        assert_eq!(row2.get::<String>(0).expect("name"), "Ada");
    }

    /// Simulates `vox codex export-legacy` → new file → `vox codex import-legacy` without the CLI.
    #[tokio::test]
    async fn legacy_chain_db_export_then_import_into_baseline_roundtrips_objects() {
        let dir = tempdir().expect("tempdir");
        let legacy_path = dir.path().join("legacy.db");
        let legacy_str = legacy_path.to_string_lossy().to_string();
        let fresh_path = dir.path().join("fresh.db");
        let fresh_str = fresh_path.to_string_lossy().to_string();

        let built = turso::Builder::new_local(&legacy_str)
            .build()
            .await
            .expect("legacy build");
        let conn = built.connect().expect("legacy conn");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
                );",
        )
        .await
        .expect("schema_version ddl");
        conn.execute("INSERT INTO schema_version (version) VALUES (99)", ())
            .await
            .expect("insert v99");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS objects (
                    hash TEXT PRIMARY KEY,
                    kind TEXT NOT NULL,
                    data BLOB NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );",
        )
        .await
        .expect("objects ddl");
        conn.execute(
            "INSERT INTO objects (hash, kind, data) VALUES ('legacy_row_h', 'legacy_kind', X'01020304')",
            (),
        )
        .await
        .expect("insert object");
        drop(conn);

        let err = match VoxDb::connect(DbConfig::local(&legacy_str)).await {
            Ok(_) => panic!("normal open must reject legacy chain"),
            Err(e) => e,
        };
        assert!(
            matches!(err, StoreError::LegacySchemaChain { max_version: 99 }),
            "expected LegacySchemaChain {{ max_version: 99 }}, got {err:?}"
        );

        let export_db = VoxDb::connect_legacy_export_only(DbConfig::local(&legacy_str))
            .await
            .expect("legacy export open");
        let mut jsonl = Vec::<u8>::new();
        let n = export_legacy_jsonl(&export_db, &mut jsonl)
            .await
            .expect("export");
        assert!(n >= 1, "expected at least one exported row");

        let fresh = VoxDb::connect(DbConfig::local(&fresh_str))
            .await
            .expect("fresh baseline");
        assert_eq!(fresh.schema_version().await.expect("sv"), BASELINE_VERSION);
        let imported = import_legacy_jsonl(&fresh, Cursor::new(&jsonl))
            .await
            .expect("import");
        assert!(imported >= 1);

        let imported_twice = import_legacy_jsonl(&fresh, Cursor::new(&jsonl))
            .await
            .expect("re-import");
        assert_eq!(
            imported_twice, imported,
            "second import should replace, not append duplicate rows"
        );

        let mut q = fresh
            .conn
            .query(
                "SELECT kind, hex(data) FROM objects WHERE hash = ?1",
                turso::params!["legacy_row_h"],
            )
            .await
            .expect("select");
        let row = q.next().await.expect("row").expect("has row");
        let kind: String = row.get(0).expect("kind");
        let hex_data: String = row.get(1).expect("hex");
        assert_eq!(kind, "legacy_kind");
        assert_eq!(hex_data.to_uppercase(), "01020304");

        let leg = verify_legacy_store(&fresh).await.expect("verify");
        assert_eq!(leg.schema_version, BASELINE_VERSION);
        assert!(!leg.is_legacy_schema_chain);
    }
}

#[tokio::test]
async fn conversations_archived_at_column_exists_and_defaults_null() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let conv_id = db
        .chat_ensure_gui_session("sess-archived-at-test", "Test session")
        .await
        .expect("create session");
    let mut rows = db
        .connection()
        .query(
            "SELECT archived_at FROM conversations WHERE id = ?1",
            turso::params![conv_id],
        )
        .await
        .expect("query");
    let row = rows.next().await.expect("row").expect("one row");
    let archived_at: Option<String> = row.get(0).expect("archived_at column");
    assert_eq!(archived_at, None, "new sessions must not be pre-archived");
}

#[tokio::test]
async fn migrate_adds_archived_at_to_a_pre_existing_conversations_table() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
    let conn = db.connection();

    // Simulate a pre-this-change table: drop the column-having table and recreate the
    // OLD shape (no archived_at), matching what a real upgrading user's database looks like.
    conn.execute_batch("DROP TABLE conversations;")
        .await
        .unwrap();
    conn.execute_batch(
        "CREATE TABLE conversations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT,
            title TEXT NOT NULL DEFAULT '',
            code_version TEXT,
            repository_id TEXT,
            external_session_id TEXT,
            thread_id TEXT,
            origin_surface TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
    .await
    .unwrap();

    // `VoxDb::connect` above already ran `migrate()` once and recorded the current
    // BASELINE_VERSION in `schema_version` — without resetting it, a second `migrate()`
    // call would see `current_version == BASELINE_VERSION` and skip the whole upgrade
    // branch (including the ALTER TABLE fix under test) as a no-op. Roll it back to
    // simulate a database that has never seen this version, matching what a real
    // upgrading user's `schema_version` row actually looks like before their first
    // launch on the new binary.
    conn.execute_batch("DELETE FROM schema_version;")
        .await
        .unwrap();

    // Re-run the same migration path a real app startup takes.
    crate::VoxDb::migrate(conn)
        .await
        .expect("migrate should backfill archived_at");

    let mut cols = conn
        .query("PRAGMA table_info(conversations)", ())
        .await
        .unwrap();
    let mut found = false;
    while let Some(row) = cols.next().await.unwrap() {
        let name: String = row.get(1).unwrap();
        if name == "archived_at" {
            found = true;
        }
    }
    assert!(
        found,
        "migrate() must add archived_at to a pre-existing conversations table"
    );
}

/// Phase D Task D1 (chat-harness delegation lineage durability): a delegation
/// edge's `chat_session_id`/`origin_turn_id` must survive a daemon restart.
/// `spawn_dynamic_agent_with_parent` writes them via
/// `append_orchestration_lineage_event` (kind = "task_delegated"); this test
/// proves that row is readable from a *fresh* `VoxDb::connect` against the same
/// file — i.e. the process that wrote it can die and a new one can read it
/// back, unlike the in-memory `agent_delegations` HashMap it also populates.
#[tokio::test]
async fn delegation_lineage_survives_reconnect() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("lineage.db").to_string_lossy().into_owned();

    {
        let db = VoxDb::connect(DbConfig::Local { path: path.clone() })
            .await
            .expect("open db (writer)");
        db.append_orchestration_lineage_event(
            "repo-d1",
            "task_delegated",
            42,
            Some(7),
            Some("chat-session-abc"),
            None,
            None,
            None,
            Some(r#"{"reason":"delegate research","is_dynamic":true,"origin_turn_id":"call_xyz"}"#),
        )
        .await
        .expect("append lineage event");
        // `db` (and its connection) is dropped here, simulating the daemon
        // process exiting.
    }

    // A brand-new VoxDb — no shared in-memory state with the writer above —
    // opened against the same on-disk file, standing in for the daemon restart.
    let reopened = VoxDb::connect(DbConfig::Local { path })
        .await
        .expect("reopen db");
    let events = reopened
        .list_orchestration_lineage_events("repo-d1", Some("task_delegated"), 10)
        .await
        .expect("list lineage events");
    assert_eq!(events.len(), 1, "delegation edge must survive reconnect");
    let ev = &events[0];
    assert_eq!(ev["session_id"], "chat-session-abc");
    assert_eq!(ev["agent_id"], 7);
    let payload: serde_json::Value = serde_json::from_str(
        ev["payload_json"]
            .as_str()
            .expect("payload_json is a string"),
    )
    .expect("payload parses");
    assert_eq!(payload["origin_turn_id"], "call_xyz");
    assert_eq!(payload["reason"], "delegate research");
}
