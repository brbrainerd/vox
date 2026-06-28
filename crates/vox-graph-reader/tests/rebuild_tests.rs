use std::fs;
use vox_graph_reader::rebuild::{RebuildMeta, rebuild_graph};

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
        cli_catalog_json: None,
        ..Default::default()
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

#[test]
fn same_named_fns_in_different_files_do_not_collide() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.rs"), "fn make() {}").unwrap();
    fs::write(src.join("b.rs"), "fn make() {}").unwrap();
    let out = tmp.path().join("out/graph.json");
    rebuild_graph(
        tmp.path(),
        &src,
        &out,
        &tmp.path().join("out/fc"),
        &RebuildMeta::default(),
    )
    .unwrap();
    let g = read_graph(&out);
    let makes: Vec<String> = g["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .filter(|id| id.ends_with("::make"))
        .collect();
    assert_eq!(
        makes.len(),
        2,
        "expected 2 qualified make() nodes, got {makes:?}"
    );
    assert_ne!(makes[0], makes[1]);
}

#[test]
fn intra_file_call_resolves_within_module() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("a.rs"),
        "fn caller() { callee(); }\nfn callee() {}",
    )
    .unwrap();
    let out = tmp.path().join("out/graph.json");
    rebuild_graph(
        tmp.path(),
        &src,
        &out,
        &tmp.path().join("out/fc"),
        &RebuildMeta::default(),
    )
    .unwrap();
    let links = read_graph(&out)["links"].as_array().unwrap().clone();
    assert_eq!(links.len(), 1, "links: {links:?}");
    assert!(links[0]["source"].as_str().unwrap().ends_with("::caller"));
    assert!(links[0]["target"].as_str().unwrap().ends_with("::callee"));
}

#[test]
fn ambiguous_and_self_calls_are_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.rs"), "fn shared() {}").unwrap();
    fs::write(src.join("b.rs"), "fn shared() {}").unwrap();
    fs::write(src.join("c.rs"), "fn user() { shared(); }").unwrap(); // ambiguous → drop
    fs::write(src.join("d.rs"), "fn recur() { recur(); }").unwrap(); // self → drop
    let out = tmp.path().join("out/graph.json");
    rebuild_graph(
        tmp.path(),
        &src,
        &out,
        &tmp.path().join("out/fc"),
        &RebuildMeta::default(),
    )
    .unwrap();
    assert_eq!(read_graph(&out)["links"].as_array().unwrap().len(), 0);
}

#[test]
fn modules_mode_produces_module_graph() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.rs"), "fn alpha() { beta(); gamma(); }").unwrap();
    fs::write(src.join("b.rs"), "fn beta() {} fn gamma() {}").unwrap();

    let out = tmp.path().join("out/graph.json");
    let cache = tmp.path().join("out/file_cache");
    let meta = RebuildMeta {
        corpus_id: "test-corpus".to_string(),
        git_sha: Some("abc123".to_string()),
        scope_path: "src".to_string(),
        extraction_mode: Some("modules".to_string()),
        built_at_rfc3339: "2026-06-18T00:00:00+00:00".to_string(),
        cli_catalog_json: None,
        ..Default::default()
    };
    rebuild_graph(tmp.path(), &src, &out, &cache, &meta).unwrap();

    let g = read_graph(&out);
    let nodes = g["nodes"].as_array().unwrap();
    let links = g["links"].as_array().unwrap();

    let mut ids: Vec<&str> = nodes.iter().map(|n| n["id"].as_str().unwrap()).collect();
    ids.sort();
    assert_eq!(ids, vec!["a.rs", "b.rs"]);

    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["source"], "a.rs");
    assert_eq!(links[0]["target"], "b.rs");
    assert_eq!(links[0]["weight"], 2);

    // Verify manifest count updates
    let m: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("out/.graphify_manifest.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(m["node_count"], 2);
    assert_eq!(m["edge_count"], 1);
}

#[test]
fn graph_digest_is_stable_and_distinct() {
    let a = vox_graph_reader::graph_digest(b"{\"nodes\":[]}");
    let b = vox_graph_reader::graph_digest(b"{\"nodes\":[]}");
    let c = vox_graph_reader::graph_digest(b"{\"nodes\":[{}]}");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert!(a.len() >= 32);
}

#[test]
#[cfg(feature = "tree-sitter-grammars")]
fn tsx_files_contribute_nodes() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("C.tsx"), "function Widget(){ return null; }").unwrap();
    let out = tmp.path().join("out/graph.json");
    let cache = tmp.path().join("out/file_cache");
    let meta = RebuildMeta {
        corpus_id: "t".into(),
        git_sha: None,
        scope_path: "src".into(),
        extraction_mode: Some("structural".into()),
        built_at_rfc3339: "2026-06-26T00:00:00+00:00".into(),
        cli_catalog_json: None,
        ..Default::default()
    };
    rebuild_graph(tmp.path(), &src, &out, &cache, &meta).unwrap();
    let g: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert!(
        g["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["label"] == "Widget"),
        "tsx not walked"
    );
}
