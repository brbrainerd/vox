#![cfg(feature = "tree-sitter-grammars")]
use std::path::Path;
use vox_graphify_reader::ast::extract_ast_in_module;

#[test]
fn extracts_python_functions_classes_and_calls() {
    let content =
        "def caller():\n    callee()\n\ndef callee():\n    pass\n\nclass Widget:\n    pass\n";
    let g = extract_ast_in_module(Path::new("mod.py"), content, "pkg/mod.py");
    let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(ids.contains(&"pkg/mod.py::caller"), "ids: {ids:?}");
    assert!(ids.contains(&"pkg/mod.py::callee"), "ids: {ids:?}");
    assert!(ids.contains(&"pkg/mod.py::Widget"), "ids: {ids:?}");
    assert_eq!(
        g.nodes.iter().find(|n| n.label == "Widget").unwrap().kind,
        "struct"
    );
    assert!(
        g.edges
            .iter()
            .any(|e| e.source == "pkg/mod.py::caller" && e.target == "callee"),
        "edges: {:?}",
        g.edges
    );
}
