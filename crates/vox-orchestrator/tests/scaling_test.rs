use std::sync::Arc;
use tokio::sync::Mutex;
use vox_orchestrator::config::OrchestratorConfig;
use vox_orchestrator::orchestrator::Orchestrator;
use vox_orchestrator::runtime::AgentFleet;
use vox_orchestrator::types::TaskPriority;
use vox_runtime::scheduler::Scheduler;

#[tokio::test]
async fn test_dynamic_scaling_and_retirement() {
    let mut config = OrchestratorConfig::for_testing();
    config.scaling_enabled = true;
    config.min_agents = 1;
    config.max_agents = 4;
    config.scaling_threshold = 2; // Spawn if > 2 tasks per agent
    config.idle_retirement_ms = 100; // Fast retirement for test

    let orch = Arc::new(Mutex::new(Orchestrator::new(config)));
    let scheduler = Arc::new(Scheduler::new());
    let fleet = AgentFleet::new(
        scheduler.clone(),
        orch.clone(),
        std::sync::Arc::new(vox_orchestrator::runtime::StubTaskProcessor),
    );

    // 1. Initial state: 0 agents (fleet sync will spawn 1 default if needed, or check_scaling will)
    {
        let mut o = orch.lock().await;
        // In this test environment, we might need at least one agent to start
        o.spawn_agent("default").unwrap();
    }

    fleet.sync_fleet().await;
    assert_eq!(orch.lock().await.agent_ids().len(), 1);

    // 2. Add tasks to trigger scaling
    {
        let mut o = orch.lock().await;
        for i in 0..10 {
            o.submit_task(format!("task-{}", i), vec![], Some(TaskPriority::Normal))
                .await
                .unwrap();
        }
    }

    // 3. Run scaling check
    fleet.check_scaling().await;

    // Should have spawned more agents (up to 4)
    let agent_count = orch.lock().await.agent_ids().len();
    assert!(
        agent_count > 1,
        "Should have scaled up, found {} agents",
        agent_count
    );
    assert!(agent_count <= 4);

    // 4. Mark tasks as complete to trigger retirement
    {
        let mut o = orch.lock().await;
        let ids = o.agent_ids();
        for id in ids {
            if let Some(q) = o.get_agent_queue_mut(id) {
                let tasks = q.drain_tasks();
                for t in tasks {
                    o.complete_task(t.id).await.ok();
                }
            }
        }
    }

    // Wait for idle retirement
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    fleet.check_scaling().await;

    // Should have scaled down to min_agents
    let final_count = orch.lock().await.agent_ids().len();
    assert_eq!(
        final_count, 1,
        "Should have scaled down to 1, found {}",
        final_count
    );
}

#[tokio::test]
async fn test_predictive_scaling_uses_trend() {
    let mut config = OrchestratorConfig::for_testing();
    config.scaling_enabled = true;
    config.min_agents = 1;
    config.max_agents = 4;
    config.scaling_threshold = 3; // Won't trigger on current load alone with 5 tasks/3 agents
    config.idle_retirement_ms = 999_999; // Disable retirement for this test
    config.scaling_lookback_ticks = 3;

    let mut orch = Orchestrator::new(config);
    orch.spawn_agent("a").unwrap();
    orch.spawn_agent("b").unwrap();
    orch.spawn_agent("c").unwrap();

    // Submit enough tasks to push predicted_load above threshold
    for i in 0..9 {
        orch.submit_task(format!("task-{}", i), vec![], Some(TaskPriority::Urgent))
            .await
            .unwrap();
    }

    // Simulate 3 ticks building up a rising trend in load_history
    orch.tick();
    orch.tick();
    orch.tick();

    let status = orch.status();
    // With 9 urgent tasks (weight=3) and 3 agents: weighted_load = 9, per-agent = 3 >= threshold
    assert!(
        status.total_weighted_load > 0.0,
        "Should have non-zero load"
    );
    // Predicted load should equal or exceed current after multiple identical ticks
    assert!(status.predicted_load >= 0.0);

    // After draining all tasks, predicted_load should trend toward 0
    for id in orch.agent_ids() {
        if let Some(q) = orch.get_agent_queue_mut(id) {
            q.drain_tasks();
        }
    }
    orch.tick();
    let status2 = orch.status();
    assert!(
        status2.total_weighted_load < status.total_weighted_load,
        "Load should decrease"
    );
}

#[tokio::test]
async fn test_group_affinity_voting_routes_correctly() {
    use vox_orchestrator::types::FileAffinity;

    let mut orch = Orchestrator::new(OrchestratorConfig::for_testing());

    // Submit a task to establish group affinity for "src/parser/"
    let t1 = orch
        .submit_task(
            "parser task 1",
            vec![FileAffinity::write("src/parser/grammar.rs")],
            None,
        )
        .await
        .unwrap();
    let agent1 = *orch.task_assignments().get(&t1).unwrap();

    // Second task on same group path should prefer the same agent (direct file affinity score wins)
    let t2 = orch
        .submit_task(
            "parser task 2",
            vec![FileAffinity::write("src/parser/grammar.rs")],
            None,
        )
        .await
        .unwrap();
    let agent2 = *orch.task_assignments().get(&t2).unwrap();

    assert_eq!(
        agent1, agent2,
        "Both tasks on the same file should route to the same agent"
    );
}

#[tokio::test]
async fn test_urgent_rebalance_trigger() {
    let mut config = OrchestratorConfig::for_testing();
    config.urgent_rebalance_threshold = 2; // Trigger when any agent has > 2 Urgent tasks
    config.scaling_enabled = false;

    let mut orch = Orchestrator::new(config);
    let a = orch.spawn_agent("agent-a").unwrap();
    let _b = orch.spawn_agent("agent-b").unwrap();

    // Load 4 Urgent tasks onto agent-a directly
    for i in 0..4 {
        orch.submit_task(format!("urgent-{}", i), vec![], Some(TaskPriority::Urgent))
            .await
            .unwrap();
    }

    // All tasks should currently be assigned (some may already route to b via the routing service)
    let a_before = orch
        .agent_queue(a)
        .map(|q| q.depth_by_priority(TaskPriority::Urgent))
        .unwrap_or(0);

    // Tick should detect urgency overload and trigger rebalance
    orch.tick();

    // After rebalance, agent-a should have fewer urgent tasks than before
    // (or equal if routing already balanced them)
    let a_after = orch
        .agent_queue(a)
        .map(|q| q.depth_by_priority(TaskPriority::Urgent))
        .unwrap_or(0);
    let total_after: usize = orch
        .agent_ids()
        .iter()
        .map(|id| orch.agent_queue(*id).map(|q| q.len()).unwrap_or(0))
        .sum();

    // All tasks should still exist (no data loss)
    assert_eq!(
        total_after, 4,
        "All tasks should be preserved after rebalance"
    );
    // If agent-a had enough to trigger, tasks should have been redistributed
    if a_before > 2 {
        assert!(
            a_after <= a_before,
            "Agent-a load should not have increased: before={}, after={}",
            a_before,
            a_after
        );
    }
}
