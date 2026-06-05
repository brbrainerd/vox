use vox_research_shim::research::persistence::{slug_from_query, write_research_doc};

#[test]
fn slug_from_query_basic() {
    assert_eq!(slug_from_query("Hello, World! 2026"), "hello-world-2026");
    assert_eq!(slug_from_query(""), "untitled");
    let s = slug_from_query(&"a".repeat(200));
    assert!(s.len() <= 80, "slug capped at 80 chars, got {}", s.len());
}

#[test]
fn write_research_doc_writes_to_tmpdir() {
    let dir = tempfile::tempdir().expect("tmpdir");
    write_research_doc(dir.path(), "test-slug", "Q?", "A.", "stub-model").expect("writes");
    let p = dir.path().join("docs/src/research/test-slug.md");
    assert!(p.exists(), "expected research doc at {:?}", p);
}
