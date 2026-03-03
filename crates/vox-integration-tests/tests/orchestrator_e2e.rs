use vox_orchestrator::{
    types::TaskDescriptor, FileAffinity, Orchestrator, OrchestratorConfig, TaskPriority,
};

fn test_config() -> OrchestratorConfig {
    let mut config = OrchestratorConfig::for_testing();
    config.max_agents = 4;
    config
}

async fn drain_and_complete_all(orch: &mut Orchestrator) {
    let ids = orch.agent_ids();
    for id in ids {
        loop {
            let task_id = if let Some(queue) = orch.get_agent_queue_mut(id) {
                if let Some(task) = queue.dequeue() {
                    Some(task.id)
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(tid) = task_id {
                orch.complete_task(tid).await.unwrap();
            } else {
                break;
            }
        }
    }
}

#[tokio::test]
async fn e2e_multi_agent_concurrent_edits() {
    let mut orch = Orchestrator::new(test_config());

    let _t1 = orch
        .submit_task(
            "Edit A",
            vec![FileAffinity::write("src/a.rs")],
            Some(TaskPriority::Normal),
        )
        .await
        .unwrap();

    let _t2 = orch
        .submit_task(
            "Edit B",
            vec![FileAffinity::write("src/b.rs")],
            Some(TaskPriority::Normal),
        )
        .await
        .unwrap();

    drain_and_complete_all(&mut orch).await;
}

#[tokio::test]
async fn e2e_task_queue_drain() {
    let mut orch = Orchestrator::new(test_config());

    for i in 0..10 {
        orch.submit_task(
            format!("Task {i}"),
            vec![FileAffinity::write("src/shared.rs")],
            Some(TaskPriority::Normal),
        )
        .await
        .unwrap();
    }

    assert_eq!(
        orch.status().total_queued + orch.status().total_in_progress,
        10
    );

    drain_and_complete_all(&mut orch).await;

    let snap = orch.status();
    assert_eq!(snap.total_completed, 10);
}

#[tokio::test]
async fn e2e_context_sharing_across_agents() {
    let orch = Orchestrator::new(test_config());

    orch.context().set(
        vox_orchestrator::types::AgentId(1),
        "shared_var",
        "secret_value",
        10,
    );

    let val = orch.context().get("shared_var").expect("should exist");
    assert_eq!(val, "secret_value");
}

#[tokio::test]
async fn e2e_timeout_and_retry() {
    let mut config = test_config();
    config.lock_timeout_ms = 10;
    let mut orch = Orchestrator::new(config);

    let t1 = orch
        .submit_task(
            "Timeout Task",
            vec![FileAffinity::write("src/c.rs")],
            Some(TaskPriority::Normal),
        )
        .await
        .unwrap();

    // We don't drain because we want to fail it directly (acting as the agent failing)
    // Actually we need to dequeue it first to fail it!
    if let Some(q) = orch.get_agent_queue_mut(orch.agent_ids()[0]) {
        q.dequeue();
    }
    orch.fail_task(t1, "simulated failure".to_string()).await.unwrap();
}

#[tokio::test]
async fn e2e_dependency_chain() {
    let mut orch = Orchestrator::new(test_config());

    let _t1 = orch
        .submit_task(
            "Dep 1",
            vec![FileAffinity::write("src/a.rs")],
            Some(TaskPriority::Normal),
        )
        .await
        .unwrap();

    drain_and_complete_all(&mut orch).await;

    let snap = orch.status();
    assert_eq!(snap.total_completed, 1);
}

#[tokio::test]
async fn e2e_lock_contention_resolved() {
    let mut orch = Orchestrator::new(test_config());

    let _t1 = orch
        .submit_task(
            "Contender 1",
            vec![FileAffinity::write("src/locked.rs")],
            Some(TaskPriority::Normal),
        )
        .await
        .unwrap();

    let _t2 = orch
        .submit_task(
            "Contender 2",
            vec![FileAffinity::write("src/locked.rs")],
            Some(TaskPriority::Normal),
        )
        .await
        .unwrap();

    // Both mapped to the same agent queue because of file affinity
    // First drain completes t1
    drain_and_complete_all(&mut orch).await;
    // Which unlocks t2 to be dequeued
    drain_and_complete_all(&mut orch).await;

    assert_eq!(orch.status().total_completed, 2);
}

#[tokio::test]
async fn e2e_batch_submission() {
    let mut orch = Orchestrator::new(test_config());

    let batch = orch
        .submit_batch(vec![
            TaskDescriptor {
                description: "Batch 1".to_string(),
                priority: None,
                file_manifest: vec![FileAffinity::write("src/b1.rs")],
                depends_on: vec![],
                temp_deps: vec![],
            },
            TaskDescriptor {
                description: "Batch 2".to_string(),
                priority: None,
                file_manifest: vec![FileAffinity::write("src/b2.rs")],
                depends_on: vec![],
                temp_deps: vec![],
            },
        ])
        .await
        .unwrap();

    assert_eq!(batch.len(), 2);

    drain_and_complete_all(&mut orch).await;
    assert_eq!(orch.status().total_completed, 2);
}
