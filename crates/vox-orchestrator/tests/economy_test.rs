use vox_orchestrator::config::{CostPreference, OrchestratorConfig};
use vox_orchestrator::models::ModelSpec;
use vox_orchestrator::orchestrator::Orchestrator;
use vox_orchestrator::types::TaskPriority;

#[tokio::test]
async fn test_economy_preference_rebalancing() {
    let mut config = OrchestratorConfig::for_testing();
    config.cost_preference = CostPreference::Economy;

    let mut orch = Orchestrator::new(config);

    // Register 2 agents: one expensive (default), one cheap (override)
    let expensive_id = orch.spawn_agent("expensive").unwrap();
    let cheap_id = orch.spawn_agent("cheap").unwrap();

    orch.models_mut().register(ModelSpec {
        id: "cheap-model".to_string(),
        provider: "local".to_string(),
        provider_type: vox_orchestrator::models::ProviderType::Ollama,
        max_tokens: 4096,
        cost_per_1k: 0.0001,
        is_free: false,
        strengths: vec!["parsing".to_string()],
    });

    // Override cheap agent's model
    orch.models_mut()
        .set_override(cheap_id.0, "cheap-model".to_string());

    // Fill expensive agent with tasks
    for i in 0..10 {
        let _task_id = orch
            .submit_task(
                format!("expensive-task-{}", i),
                vec![],
                Some(TaskPriority::Normal),
            )
            .await
            .unwrap();
        // Force assignment to expensive_id for setup (manually move)
        let _ = orch.retire_agent(expensive_id); // This is a trick to get tasks, but I'll just use private methods if I could
    }

    // Re-setup: put many tasks on expensive, 0 on cheap
    let expensive_id = orch.spawn_agent("expensive").unwrap(); // id might change if gen increments
                                                               // Actually, I'll just use the IDs I have.

    // Manually populate queues for the test
    let task = vox_orchestrator::types::AgentTask::new(
        vox_orchestrator::types::TaskId(100),
        "test-task",
        TaskPriority::Normal,
        vec![],
    );

    if let Some(q) = orch.get_agent_queue_mut(expensive_id) {
        for i in 0..10 {
            let mut t = task.clone();
            t.id = vox_orchestrator::types::TaskId(i as u64);
            q.enqueue(t);
        }
    }

    // Rebalance
    orch.rebalance();

    // Cheap agent should have taken tasks
    let cheap_queue = orch.agent_queue(cheap_id).unwrap();
    assert!(
        cheap_queue.len() > 0,
        "Cheap agent should have stolen tasks"
    );
}

#[tokio::test]
async fn test_model_selection_preference() {
    let config = OrchestratorConfig::default();
    let mut orch = Orchestrator::new(config);

    orch.models_mut().register(ModelSpec {
        id: "budget-coder".to_string(),
        provider: "local".to_string(),
        provider_type: vox_orchestrator::models::ProviderType::Ollama,
        max_tokens: 8192,
        cost_per_1k: -1.0,
        is_free: true,
        strengths: vec!["codegen".to_string()],
    });

    // Performance preference (default) should pick Sonnet
    let best_perf = orch
        .models()
        .best_for(
            vox_orchestrator::types::TaskCategory::CodeGen,
            5,
            CostPreference::Performance,
        )
        .unwrap();
    assert_eq!(best_perf.id, "anthropic/claude-sonnet-4.5");

    // Economy preference should pick budget-coder
    let best_econ = orch
        .models()
        .best_for(
            vox_orchestrator::types::TaskCategory::CodeGen,
            5,
            CostPreference::Economy,
        )
        .unwrap();
    assert_eq!(best_econ.id, "budget-coder");

    // Dynamic Tiering: Low complexity (2) should pick budget-coder even in Performance mode
    let best_dynamic = orch
        .models()
        .best_for(
            vox_orchestrator::types::TaskCategory::CodeGen,
            2,
            CostPreference::Performance,
        )
        .unwrap();
    assert_eq!(best_dynamic.id, "budget-coder");
}
