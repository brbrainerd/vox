use std::fs;
use vox_graph_reader::snapshot::{list_snapshots, prune_snapshots, snapshot_corpus};

fn seed(dir: &std::path::Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("graph.json"), "{\"nodes\":[]}").unwrap();
    fs::write(dir.join(".graphify_manifest.v1.json"), "{}").unwrap();
}

#[test]
fn snapshot_list_and_prune_keep_newest() {
    let tmp = tempfile::tempdir().unwrap();
    let corpus = tmp.path().join("corpus");
    seed(&corpus);

    // Stamps sort lexically; oldest first.
    for stamp in [
        "2026-06-01T00-00-00",
        "2026-06-02T00-00-00",
        "2026-06-03T00-00-00",
    ] {
        let dst = snapshot_corpus(&corpus, stamp).unwrap();
        assert!(dst.join("graph.json").is_file(), "graph copied");
        assert!(
            dst.join(".graphify_manifest.v1.json").is_file(),
            "manifest copied"
        );
    }
    assert_eq!(list_snapshots(&corpus).len(), 3);

    let removed = prune_snapshots(&corpus, 2).unwrap();
    assert_eq!(removed, 1);
    let kept = list_snapshots(&corpus);
    assert_eq!(kept, vec!["2026-06-02T00-00-00", "2026-06-03T00-00-00"]); // newest kept
}

#[test]
fn snapshot_of_missing_corpus_is_empty_but_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let corpus = tmp.path().join("none");
    fs::create_dir_all(&corpus).unwrap();
    let dst = snapshot_corpus(&corpus, "s1").unwrap(); // no graph.json present
    assert!(dst.is_dir());
    assert!(!dst.join("graph.json").exists());
}
