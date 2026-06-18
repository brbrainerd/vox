use vox_graphify_reader::reachability::ingest_lcov_reachability;
use serde_json::json;

#[test]
fn test_lcov_reachability_ingest() {
    let graph = json!({
        "nodes": [
            {"id": "hello", "label": "hello", "kind": "fn"}
        ],
        "links": []
    });
    let lcov = "
        SF:src/main.rs
        FN:3,hello
        FNDA:5,hello
        end_of_record
    ";
    let updated = ingest_lcov_reachability(&graph, lcov).unwrap();
    let nodes = updated["nodes"].as_array().unwrap();
    assert_eq!(nodes[0]["execution_count"].as_u64().unwrap(), 5);
}
