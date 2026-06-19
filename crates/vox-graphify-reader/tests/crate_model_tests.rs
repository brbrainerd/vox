use std::collections::HashMap;
use vox_graphify_reader::crate_model::crate_metrics;

#[test]
fn crate_metrics_count_and_seconds() {
    // a -> b -> c  (a depends on b, b depends on c)
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    adj.insert("a".into(), vec!["b".into()]);
    adj.insert("b".into(), vec!["c".into()]);
    let mut self_s = HashMap::new();
    self_s.insert("a".into(), 1.0);
    self_s.insert("b".into(), 2.0);
    self_s.insert("c".into(), 4.0);

    let m = crate_metrics(&adj, &self_s);
    // transitive dependents: c<-{b,a}=2, b<-{a}=1, a<-{}=0
    assert_eq!(m["c"].dependents, 2);
    assert_eq!(m["b"].dependents, 1);
    assert_eq!(m["a"].dependents, 0);
    // blast seconds: self + dependents' self
    assert_eq!(m["c"].blast_s, 7.0);
    assert_eq!(m["a"].blast_s, 1.0);
}

#[test]
fn crate_metrics_handles_cycles() {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    adj.insert("x".into(), vec!["y".into()]);
    adj.insert("y".into(), vec!["x".into()]); // 2-cycle
    let self_s = HashMap::new(); // no times -> blast_s 0, counts still computed
    let m = crate_metrics(&adj, &self_s);
    assert_eq!(m["x"].dependents, 1); // y depends on x
    assert_eq!(m["y"].dependents, 1);
    assert_eq!(m["x"].blast_s, 0.0);
}

use serde_json::json;
use vox_graphify_reader::crate_model::build_crate_map;

#[test]
fn build_crate_map_is_complete_and_deterministic() {
    let crate_graph = json!({ "schema_version": 1, "crates": { "a": ["b"], "b": ["c"], "c": [] }});
    let audit = json!([
        {"crate":"a","compile_s":"1.0","loc":10,"layer":5},
        {"crate":"b","compile_s":"2.0","loc":20,"layer":3},
        {"crate":"c","compile_s":"4.0","loc":40,"layer":0}
    ]);
    let m1 = build_crate_map(&crate_graph, &audit);
    let m2 = build_crate_map(&crate_graph, &audit);
    assert_eq!(m1, m2, "crate map must be byte-identical across runs");
    let nodes = m1["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3);
    let c = nodes.iter().find(|n| n["id"] == "c").unwrap();
    assert_eq!(c["blast_s"], 7.0);
    assert_eq!(c["dependents"], 2);
    assert_eq!(c["fan_in"], 1);
    assert_eq!(c["loc"], 40);
    assert!(c.get("community").is_some());
    assert_eq!(m1["links"].as_array().unwrap().len(), 2);
}

#[test]
fn build_crate_map_works_without_audit_times() {
    let crate_graph = json!({ "schema_version": 1, "crates": { "a": ["b"], "b": [] }});
    let audit = json!([]); // no compile times available (fresh checkout)
    let m = build_crate_map(&crate_graph, &audit);
    let b = m["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "b")
        .unwrap();
    assert_eq!(b["dependents"], 1); // a depends on b
    assert_eq!(b["blast_s"], 0.0); // unknown times -> 0, but dependents still ranks
}
