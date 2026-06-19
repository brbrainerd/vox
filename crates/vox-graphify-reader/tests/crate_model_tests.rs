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
