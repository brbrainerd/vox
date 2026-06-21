use serde_json::json;
use vox_graphify_reader::lens::collapse_to_modules;

#[test]
fn collapses_to_modules_with_weighted_edges() {
    let g = json!({
        "nodes": [{"id":"a.rs::f","label":"f"},{"id":"b.rs::g","label":"g"},{"id":"b.rs::h","label":"h"}],
        "links": [{"source":"a.rs::f","target":"b.rs::g"},{"source":"a.rs::f","target":"b.rs::h"}]
    });
    let c = collapse_to_modules(&g);
    let mut ids: Vec<&str> = c["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    ids.sort();
    assert_eq!(ids, vec!["a.rs", "b.rs"]);
    assert!(
        c["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|n| n["kind"] == "module")
    );
    let links = c["links"].as_array().unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["source"], "a.rs");
    assert_eq!(links[0]["target"], "b.rs");
    assert_eq!(links[0]["weight"], 2);
}
#[test]
fn intra_module_edges_dropped() {
    let g = json!({
        "nodes": [{"id":"a.rs::f","label":"f"},{"id":"a.rs::g","label":"g"}],
        "links": [{"source":"a.rs::f","target":"a.rs::g"}]
    });
    let c = collapse_to_modules(&g);
    assert_eq!(c["links"].as_array().unwrap().len(), 0);
    assert_eq!(c["nodes"].as_array().unwrap().len(), 1);
}
