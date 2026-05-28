//! Smoke test for `execute_search_plan` with in-memory DB + temp memory tree.

use std::sync::Arc;

use vox_db::{RetrievalMode, SearchCorpus, SearchPlan, VoxDb};
use vox_search::{SearchPolicy, SearchRuntimeContext, execute_search_plan};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execute_search_plan_hits_memory_or_repo() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path();
    let mem_dir = repo.join("memory");
    std::fs::create_dir_all(&mem_dir).expect("mkdir memory");
    std::fs::write(
        mem_dir.join("2026-05-11.md"),
        "# log\n\nuniquekeyword_alpha beta",
    )
    .expect("write log");
    let md_path = mem_dir.join("longterm.md");
    std::fs::write(&md_path, "# lt\n\ngamma uniquekeyword_alpha").expect("write md");

    // Give repo inventory something to match.
    let src_dir = repo.join("src");
    std::fs::create_dir_all(&src_dir).expect("mkdir src");
    std::fs::write(src_dir.join("uniquekeyword_note.rs"), "// note").expect("write rs");

    let db = Arc::new(VoxDb::open_memory().await.expect("open_memory"));
    let ctx = SearchRuntimeContext::new(repo.to_path_buf(), Some(db.clone()), mem_dir, md_path);

    let plan = SearchPlan {
        normalized_query: "uniquekeyword_alpha".into(),
        corpora: vec![SearchCorpus::Memory, SearchCorpus::RepoInventory],
        retrieval_mode: RetrievalMode::FullText,
        ..Default::default()
    };
    let policy = SearchPolicy::default();

    let exec = execute_search_plan(&ctx, "uniquekeyword_alpha", &plan, 8, &policy, None)
        .await
        .expect("execute_search_plan");

    assert!(
        !exec.memory_lines.is_empty() || !exec.repo_lines.is_empty(),
        "expected memory or repo hits; memory={:?} repo={:?}",
        exec.memory_lines,
        exec.repo_lines
    );
}
