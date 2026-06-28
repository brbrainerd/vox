use vox_graph_reader::gc::{Retention, pick_lens, retention_decision, value_score};

#[test]
fn value_score_rewards_usage_and_recency() {
    // more usage, same everything else → higher score
    assert!(value_score(100, 1.0, 0, 10.0) > value_score(1, 1.0, 0, 10.0));
    // more recent (fewer days since use) → higher score
    assert!(value_score(10, 0.0, 0, 10.0) > value_score(10, 30.0, 0, 10.0));
}

#[test]
fn retention_decision_boundaries() {
    assert_eq!(retention_decision(5.0, 2.0, 0.5), Retention::Maintain);
    assert_eq!(retention_decision(1.0, 2.0, 0.5), Retention::Expire);
    assert_eq!(retention_decision(0.2, 2.0, 0.5), Retention::Discard);
}

#[test]
fn pick_lens_switches_above_threshold() {
    assert_eq!(pick_lens(100, 50_000), "structural");
    assert_eq!(pick_lens(60_000, 50_000), "modules");
}
