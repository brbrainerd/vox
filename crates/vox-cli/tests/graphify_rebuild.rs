use std::fs;
use tempfile::tempdir;

#[test]
fn test_cli_graphify_rebuild_success() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("crates");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "fn hello() {}").unwrap();

    let output_file = tmp.path().join("graph.json");
    let cache_dir = tmp.path().join("cache");

    let res = vox_graphify_reader::rebuild::rebuild_graph(
        tmp.path(),
        &src,
        &output_file,
        &cache_dir,
        &vox_graphify_reader::rebuild::RebuildMeta::default(),
    );

    assert!(res.is_ok());
    assert!(output_file.exists());
    let graph_content = fs::read_to_string(output_file).unwrap();
    assert!(graph_content.contains("hello"));
}

#[test]
fn rebuild_then_assess_is_fresh_and_detects_drift() {
    use chrono::Utc;
    use vox_config::graphify::{assess_corpus_status, GraphifyCorpus};
    use vox_graphify_reader::rebuild::{rebuild_graph, RebuildMeta};

    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "fn one() { two(); }\nfn two() {}").unwrap();

    let graph_rel = ".vox/cache/graphify/repo-code-graph/graph.json";
    let out = tmp.path().join(graph_rel);
    let meta = RebuildMeta {
        corpus_id: "repo-code-graph".to_string(),
        git_sha: Some("headsha".to_string()),
        scope_path: "src".to_string(),
        extraction_mode: Some("structural".to_string()),
        built_at_rfc3339: Utc::now().to_rfc3339(),
    };
    rebuild_graph(tmp.path(), &src, &out, &out.parent().unwrap().join("file_cache"), &meta).unwrap();

    let corpus = GraphifyCorpus {
        id: "repo-code-graph".to_string(),
        title: "t".to_string(),
        scope_path: "src".to_string(),
        graph_path: graph_rel.to_string(),
        manifest_path: ".vox/cache/graphify/repo-code-graph/.graphify_manifest.v1.json".to_string(),
        extraction_mode: Some("structural".to_string()),
        default_for_intents: vec![],
        is_virtual: false,
    };
    let fresh = assess_corpus_status(tmp.path(), &corpus, Some("headsha"), Utc::now(), 30);
    assert!(fresh.is_fresh, "stale: {:?}", fresh.stale_reasons);
    let drifted = assess_corpus_status(tmp.path(), &corpus, Some("other"), Utc::now(), 30);
    assert!(drifted.stale_reasons.contains(&"git_drift".to_string()));
}
