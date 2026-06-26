#![cfg(feature = "tree-sitter-grammars")]
use std::collections::HashMap;
use std::path::Path;
use vox_graphify_reader::ast::extract_ast_in_module_with_wrappers;

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
