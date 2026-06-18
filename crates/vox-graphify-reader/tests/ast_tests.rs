use std::path::Path;
use vox_graphify_reader::ast::extract_ast;

#[test]
fn test_rust_syn_extraction() {
    let path = Path::new("src/main.rs");
    let content = r#"
        fn hello() {
            println!("hello");
        }
        struct World;
    "#;
    let graph = extract_ast(path, content);
    let node_labels: Vec<String> = graph.nodes.iter().map(|n| n.label.clone()).collect();
    assert!(node_labels.contains(&"hello".to_string()));
    assert!(node_labels.contains(&"World".to_string()));
}
