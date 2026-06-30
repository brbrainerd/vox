use vox_graph_reader::registry::cli_command_nodes;

const CATALOG_JSON: &str = r#"{
  "entries": [
    { "path": ["ci", "lint"], "command": "lint", "source_group": "ci" },
    { "path": ["db", "query"], "command": "query", "source_group": "db" },
    { "path": ["search"], "command": "search", "source_group": "search" }
  ]
}"#;

#[test]
fn cli_nodes_have_group_scoped_ids_and_skip_top_level_groups() {
    let nodes = cli_command_nodes(CATALOG_JSON);
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(
        ids.contains(&"cli:ci:lint"),
        "expected cli:ci:lint, got {ids:?}"
    );
    assert!(ids.contains(&"cli:db:query"));
    // Top-level group with no subcommand (len==1) is the group node, not a leaf.
    assert!(!ids.contains(&"cli:search:search"));
    assert!(
        nodes
            .iter()
            .filter(|n| n.id.starts_with("cli:") && n.id.matches(':').count() == 2)
            .all(|n| n.kind == "cli-command")
    );
}

#[test]
fn malformed_json_yields_empty_not_panic() {
    assert!(cli_command_nodes("not json").is_empty());
}

/// End-to-end `gui-wiring` rebuild with a CLI catalog: a `cli:ci:lint` leaf must be
/// folded in and joined (declared-confidence) to the same-named `cmd:lint` impl node.
#[cfg(feature = "tree-sitter-grammars")]
#[test]
fn cli_leaf_joins_to_same_named_command() {
    use vox_graph_reader::rebuild::{RebuildMeta, rebuild_graph};

    let tmp = std::env::temp_dir().join(format!("vox-cli-join-{}", std::process::id()));
    let gui = tmp.join("crates/vox-gui");
    std::fs::create_dir_all(gui.join("ui/src")).unwrap();
    std::fs::create_dir_all(gui.join("src/commands")).unwrap();
    std::fs::write(gui.join("ui/src/S.tsx"), "function go(){ invoke('lint'); }").unwrap();
    std::fs::write(
        gui.join("src/commands/x.rs"),
        "#[tauri::command]\npub fn lint() {}\n",
    )
    .unwrap();
    std::fs::write(
        gui.join("src/main.rs"),
        "fn main(){ tauri::generate_handler![commands::x::lint]; }",
    )
    .unwrap();

    let catalog = r#"{ "entries": [ { "path": ["ci", "lint"], "command": "lint" } ] }"#;
    let out = tmp.join("out/graph.json");
    let cache = tmp.join("cache");
    let meta = RebuildMeta {
        corpus_id: "vox-gui-surface".into(),
        git_sha: None,
        scope_path: "crates/vox-gui".into(),
        extraction_mode: Some("gui-wiring".into()),
        built_at_rfc3339: "2026-06-26T00:00:00Z".into(),
        cli_catalog_json: Some(catalog.to_string()),
        ..Default::default()
    };
    rebuild_graph(&tmp, &gui, &out, &cache, &meta).unwrap();

    let graph: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let nodes = graph["nodes"].as_array().unwrap();
    assert!(
        nodes.iter().any(|n| n["id"] == "cli:ci:lint"),
        "cli:ci:lint node folded in; nodes: {nodes:?}"
    );
    let links = graph["links"].as_array().unwrap();
    let join = links
        .iter()
        .find(|e| e["source"] == "cli:ci:lint" && e["target"] == "cmd:lint")
        .expect("cli:ci:lint -> cmd:lint join edge");
    assert_eq!(join["confidence"], "declared");
}
