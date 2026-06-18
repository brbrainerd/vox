use vox_graphify_reader::overlay::overlay_test_targets;
use serde_json::json;

#[test]
fn test_static_overlay_targeting() {
    let graph = json!({
        "nodes": [
            {"id": "func_a", "label": "func_a", "kind": "fn"}
        ],
        "links": []
    });
    let test_src = "
        #[test]
        fn test_func_a() {
            func_a();
        }
    ";
    let updated = overlay_test_targets(&graph, "src/test.rs", test_src).unwrap();
    let nodes = updated["nodes"].as_array().unwrap();
    assert_eq!(nodes[0]["targeted_by"].as_array().unwrap()[0].as_str().unwrap(), "test_func_a");
}
