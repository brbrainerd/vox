use std::fs;
use vox_graphify_reader::rebuild::{rebuild_graph, RebuildMeta};

fn read_graph(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn manifest_has_freshness_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.rs"), "fn alpha() { beta(); }\nfn beta() {}").unwrap();

    let out = tmp.path().join("out/graph.json");
    let cache = tmp.path().join("out/file_cache");
    let meta = RebuildMeta {
        corpus_id: "test-corpus".to_string(),
        git_sha: Some("abc123".to_string()),
        scope_path: "src".to_string(),
        extraction_mode: Some("structural".to_string()),
        built_at_rfc3339: "2026-06-18T00:00:00+00:00".to_string(),
    };
    rebuild_graph(tmp.path(), &src, &out, &cache, &meta).unwrap();

    let m: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("out/.graphify_manifest.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(m["git_sha"], "abc123"); // field is git_sha, NOT git_sha256
    assert_eq!(m["built_at"], "2026-06-18T00:00:00+00:00");
    assert_eq!(m["corpus_id"], "test-corpus");
    assert_eq!(m["scope_path"], "src");
    assert_eq!(m["extraction_mode"], "structural");
    assert!(m["graph_json_sha256"].as_str().unwrap().len() >= 32);
    assert!(m["node_count"].as_u64().unwrap() >= 2);
}
