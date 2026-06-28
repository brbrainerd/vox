#![cfg(feature = "tree-sitter-grammars")]
use std::collections::HashMap;
use std::path::Path;
use vox_graphify_reader::ast::extract_ast_in_module_with_wrappers;
use vox_graphify_reader::rebuild::{RebuildMeta, rebuild_graph};

#[test]
fn invoke_and_wrapper_boundary_edges() {
    let mut wrappers = HashMap::new();
    wrappers.insert(
        "doubtTask".to_string(),
        "cmd:doubt_orchestrator_task".to_string(),
    );
    let content = "function save(){ invoke('save_settings'); voxTransport.doubtTask(7); \
        invoke('invoke_mcp_tool', { tool: 'vox_resolve_feedback' }); }";
    let g = extract_ast_in_module_with_wrappers(Path::new("S.tsx"), content, "S.tsx", &wrappers);
    let t: Vec<&str> = g.edges.iter().map(|e| e.target.as_str()).collect();
    assert!(t.contains(&"cmd:save_settings"), "edges: {:?}", g.edges);
    assert!(
        t.contains(&"cmd:doubt_orchestrator_task"),
        "edges: {:?}",
        g.edges
    );
    assert!(t.contains(&"tool:vox_resolve_feedback"), "edges: {:?}", g.edges);
    assert!(
        g.edges
            .iter()
            .filter(|e| e.target == "cmd:save_settings")
            .count()
            == 1,
        "no double-count"
    );
}

/// End-to-end `gui-wiring` mode: a tempdir with a surface that invokes a registered
/// command (`do_it`) and a missing one (`gone`). After rebuild, `graph.json` must carry
/// a `cmd:do_it` node (kind command) and a `missing`-flagged `cmd:gone` node, plus edges
/// from the surface to both.
#[test]
fn gui_wiring_registry_ingest_and_missing_nodes() {
    let tmp = std::env::temp_dir().join(format!("vox-gw-{}", std::process::id()));
    let gui = tmp.join("crates/vox-gui");
    std::fs::create_dir_all(gui.join("ui/src")).unwrap();
    std::fs::create_dir_all(gui.join("src/commands")).unwrap();
    std::fs::write(
        gui.join("ui/src/S.tsx"),
        "function go(){ invoke('do_it'); invoke('gone'); }",
    )
    .unwrap();
    std::fs::write(
        gui.join("src/commands/x.rs"),
        "#[tauri::command]\npub fn do_it() {}\n",
    )
    .unwrap();
    std::fs::write(
        gui.join("src/main.rs"),
        "fn main(){ tauri::generate_handler![commands::x::do_it]; }",
    )
    .unwrap();

    let out = tmp.join("out/graph.json");
    let cache = tmp.join("cache");
    let meta = RebuildMeta {
        corpus_id: "vox-gui-surface".into(),
        git_sha: None,
        scope_path: "crates/vox-gui".into(),
        extraction_mode: Some("gui-wiring".into()),
        built_at_rfc3339: "2026-06-26T00:00:00Z".into(),
        cli_catalog_json: None,
    };
    rebuild_graph(&tmp, &gui, &out, &cache, &meta).unwrap();

    let graph: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let nodes = graph["nodes"].as_array().unwrap();
    let do_it = nodes
        .iter()
        .find(|n| n["id"] == "cmd:do_it")
        .expect("cmd:do_it node");
    assert_eq!(do_it["kind"], "command");
    let gone = nodes
        .iter()
        .find(|n| n["id"] == "cmd:gone")
        .expect("cmd:gone node synthesized for the dead-end");
    assert_eq!(gone["missing"], serde_json::json!(true));

    let links = graph["links"].as_array().unwrap();
    assert!(
        links.iter().any(|e| e["target"] == "cmd:do_it"),
        "edge to cmd:do_it; links: {links:?}"
    );
    assert!(
        links.iter().any(|e| e["target"] == "cmd:gone"),
        "edge to cmd:gone; links: {links:?}"
    );
}
