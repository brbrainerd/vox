use serde_json::json;
use vox_graphify_reader::reachability::ingest_lcov_reachability;

#[test]
fn test_lcov_reachability_ingest() {
    let graph = json!({
        "nodes": [
            {"id": "hello", "label": "hello", "kind": "fn"}
        ],
        "links": []
    });
    let lcov = "SF:src/main.rs\nFN:3,hello\nFNDA:5,hello\nend_of_record\n";
    let updated = ingest_lcov_reachability(&graph, lcov).unwrap();
    let nodes = updated["nodes"].as_array().unwrap();
    assert_eq!(nodes[0]["execution_count"].as_u64().unwrap(), 5);
}
