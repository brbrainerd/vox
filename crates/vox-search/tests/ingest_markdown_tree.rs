//! `ingest_markdown_tree` round-trip into in-memory VoxDb.

use vox_db::VoxDb;
use vox_search::ingest::ingest_markdown_tree;

#[tokio::test]
async fn ingest_markdown_tree_inserts_at_least_one_doc() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("page.md"), "# Section\n\nbody text here").expect("write md");

    let db = VoxDb::open_memory().await.expect("open_memory");
    let n = ingest_markdown_tree(&db, dir.path(), "test-ingest:")
        .await
        .expect("ingest");

    assert!(n >= 1, "expected ≥1 markdown file ingested, got {n}");
}
