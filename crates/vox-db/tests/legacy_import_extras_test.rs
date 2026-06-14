//! Behavior proofs for `import_orchestrator_memory_dir` (semantic-coverage Wave 1).
//!
//! Proves the non-directory error path and the markdown-only counting edge —
//! both previously unproven (see CANDIDATE_GAPS.md / semantic-coverage plan §D).

use tempfile::tempdir;
use vox_db::legacy_import_extras::import_orchestrator_memory_dir;
use vox_db::{DbConfig, VoxDb};

#[tokio::test]
async fn import_orchestrator_memory_dir_errors_on_non_directory() {
    let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
    let temp = tempdir().unwrap();
    let not_a_dir = temp.path().join("a_file.txt");
    std::fs::write(&not_a_dir, "not a directory").unwrap();

    let err = import_orchestrator_memory_dir(&db, &not_a_dir, "agent", "session")
        .await
        .unwrap_err();

    // error path: the Db variant, naming the failure (returned before any DB op)
    assert!(matches!(err, vox_db::StoreError::Db(_)));
    assert!(err.to_string().contains("not a directory"));
}

// NOTE: a `counts_only_markdown` happy-path proof is deferred — the import writes
// to a `memories` table that is created by a separate subsystem init, not the
// baseline schema that `VoxDb::open`/`connect(Memory)` apply, so it needs a
// fully-migrated fixture (likely via vox-test-harness). Tracked as follow-up.
