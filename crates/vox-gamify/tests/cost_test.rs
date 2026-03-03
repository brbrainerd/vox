use vox_gamify::cost::{CostAggregator, CostRecord};

#[test]
fn test_cost_tracker() {
    let mut tracker = CostAggregator::new();
    tracker.set_budget_limit("user1", 0.01);

    tracker.record(CostRecord::new("user1", "provider", "model", 10, 10, 0.005));
    tracker.record(CostRecord::new("user1", "provider", "model", 10, 10, 0.002));

    assert_eq!(tracker.agent_summary("user1").total_cost_usd, 0.007);

    tracker.record(CostRecord::new("user1", "provider", "model", 10, 10, 0.004));

    // Check alert
    let alert = tracker.budget_alert("user1");
    assert!(alert); // 0.011 > 0.01 * 0.8
}
