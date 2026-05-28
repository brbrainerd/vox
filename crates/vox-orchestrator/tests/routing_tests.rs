use std::collections::HashMap;
use vox_orchestrator::{
    AgentId, OrchestratorConfig,
    affinity::FileAffinityMap,
    contract::TaskCapabilityHints,
    groups::AffinityGroupRegistry,
    queue::AgentQueue,
    services::{RouteResult, RoutingService},
    types::FileAffinity,
};

#[tokio::test]
async fn test_routing_service_affinity_assignment() {
    let mut config = OrchestratorConfig::default();
    config
        .default_agent_capabilities
        .labels
        .push("research".to_string());

    let affinity_map = FileAffinityMap::new();
    let groups = AffinityGroupRegistry::defaults();
    let mut agents = HashMap::new();

    let a1 = AgentId(1);
    let mut q1 = AgentQueue::new(a1, "pm-group");
    q1.capabilities.labels.push("research".to_string());
    agents.insert(a1, std::sync::Arc::new(std::sync::RwLock::new(q1)));

    let task_manifest = vec![FileAffinity::write("crates/vox-package/src/lib.rs")];
    let caps = TaskCapabilityHints {
        labels: vec!["research".to_string()],
        ..Default::default()
    };

    // Route using the static method
    let route = RoutingService::route(
        &task_manifest,
        &affinity_map,
        &groups,
        &agents,
        &config,
        0,
        None,
        Some(&caps),
        None,
        None,
        None,
        None,
    );

    match route {
        RouteResult::Existing(id) => assert_eq!(id, a1),
        _ => panic!("Expected existing agent routing"),
    }
}

#[tokio::test]
async fn test_routing_service_load_balancing() {
    let config = OrchestratorConfig::default();
    let affinity_map = FileAffinityMap::new();
    let groups = AffinityGroupRegistry::defaults();
    let mut agents = HashMap::new();

    let a1 = AgentId(1);
    let a2 = AgentId(2);

    let q1 = AgentQueue::new(a1, "a1");
    let q2 = AgentQueue::new(a2, "a2");

    agents.insert(a1, std::sync::Arc::new(std::sync::RwLock::new(q1)));
    agents.insert(a2, std::sync::Arc::new(std::sync::RwLock::new(q2)));

    let task_manifest = vec![]; // No file affinity

    let route = RoutingService::route(
        &task_manifest,
        &affinity_map,
        &groups,
        &agents,
        &config,
        0,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    match route {
        RouteResult::Existing(id) => assert!(id == a1 || id == a2),
        _ => panic!("Expected existing agent routing"),
    }
}

#[tokio::test]
async fn test_routing_exploration_epsilon_fallback() {
    let config = OrchestratorConfig {
        attention_enabled: true,
        routing_exploration_epsilon: 1.0, // 100% random exploration
        ..OrchestratorConfig::default()
    };

    let affinity_map = FileAffinityMap::new();
    let groups = AffinityGroupRegistry::defaults();
    let mut agents = HashMap::new();

    let a1 = AgentId(1);
    let a2 = AgentId(2);
    let a3 = AgentId(3);

    agents.insert(
        a1,
        std::sync::Arc::new(std::sync::RwLock::new(AgentQueue::new(a1, "a1"))),
    );
    agents.insert(
        a2,
        std::sync::Arc::new(std::sync::RwLock::new(AgentQueue::new(a2, "a2"))),
    );
    agents.insert(
        a3,
        std::sync::Arc::new(std::sync::RwLock::new(AgentQueue::new(a3, "a3"))),
    );

    let task_manifest = vec![FileAffinity::write("dummy")];

    let mut counts = HashMap::new();
    for _ in 0..1000 {
        let route = RoutingService::route(
            &task_manifest,
            &affinity_map,
            &groups,
            &agents,
            &config,
            0,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        match route {
            RouteResult::Existing(id) => *counts.entry(id).or_insert(0) += 1,
            _ => panic!("Expected existing agent routing"),
        }
    }

    for (agent_id, count) in &counts {
        assert!(
            *count < 700,
            "Agent {} exceeded 70% share with full exploration bounds",
            agent_id
        );
    }
}
