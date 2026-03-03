use proptest::prelude::*;
use vox_orchestrator::{FileAffinity, Orchestrator, OrchestratorConfig, TaskPriority};

fn test_config() -> OrchestratorConfig {
    let mut config = OrchestratorConfig::for_testing();
    config.max_agents = 10;
    config
}

async fn submit_and_drain(orch: &mut Orchestrator, task_count: usize) {
    for i in 0..task_count {
        orch.submit_task(
            format!("Task {i}"),
            vec![FileAffinity::write(format!("src/file_{}.rs", i % 5))],
            Some(TaskPriority::Normal),
        )
        .await
        .unwrap();
    }

    loop {
        let mut progress = false;
        let ids = orch.agent_ids();
        for id in ids {
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
                orch.complete_task(tid).await
.unwrap();
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
}

proptest! {
    #[test]
    fn submit_and_complete_n_tasks(n in 1usize..100) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut orch = Orchestrator::new(test_config());
            submit_and_drain(&mut orch, n).await;

            // Assert everything completed
            assert_eq!(orch.status().total_completed, n);
            assert_eq!(orch.status().total_queued, 0);
            assert_eq!(orch.status().total_in_progress, 0);
        });
    }

    #[test]
    fn rebalance_maintains_total_tasks(n in 1usize..50) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut orch = Orchestrator::new(test_config());

            // Submit tasks targeting the same agent
            for i in 0..n {
                orch.submit_task(
                    format!("Task {i}"),
                    vec![FileAffinity::write("src/hot_file.rs")],
                    Some(TaskPriority::Normal),
                ).await.unwrap();
            }

            // Submit unrelated tasks
            for i in 0..n {
                orch.submit_task(
                    format!("Other Task {i}"),
                    vec![FileAffinity::write(format!("src/other_{i}.rs"))],
                    Some(TaskPriority::Normal),
                ).await.unwrap();
            }

            let initial_queued = orch.status().total_queued;
            assert_eq!(initial_queued, n * 2);

            // Force a rebalance
            orch.rebalance();

            // Ensure tasks aren't lost or duplicated
            assert_eq!(orch.status().total_queued, initial_queued);
        });
    }
}

#[tokio::test]
async fn stress_test_1000_tasks_10_agents() {
    let mut orch = Orchestrator::new(test_config());
    let task_count = 1000;

    for i in 0..task_count {
        orch.submit_task(
            format!("Stress Task {i}"),
            vec![FileAffinity::write(format!("src/partition_{}.rs", i % 10))],
            Some(TaskPriority::Normal),
        )
        .await
        .unwrap();
    }

    loop {
        let mut progress = false;
        let ids = orch.agent_ids();
        for id in ids {
            let next_task = {
                let queue = orch.get_agent_queue_mut(id).unwrap();
                queue.dequeue()
            };

            if let Some(task) = next_task {
                orch.complete_task(task.id).await.unwrap();
                progress = true;
            }
        }
        if !progress {
            break;
        }
        orch.rebalance(); // Periodically rebalance under stress
    }

    let status = orch.status();
    assert_eq!(status.total_completed, task_count as usize);
    assert_eq!(status.total_queued, 0);
}
