#![cfg(feature = "tree-sitter-grammars")]
use std::path::Path;
use vox_graphify_reader::ast::extract_ast_in_module;

#[test]
fn jsx_usage_emits_composition_edge() {
    let g = extract_ast_in_module(
        Path::new("P.tsx"),
        "function Parent(){ return <Child/>; }\nfunction Child(){ return null; }",
        "P.tsx",
    );
    assert!(
        g.edges
            .iter()
            .any(|e| e.source == "P.tsx::Parent" && e.target == "Child"),
        "edges: {:?}",
        g.edges
    );
}
