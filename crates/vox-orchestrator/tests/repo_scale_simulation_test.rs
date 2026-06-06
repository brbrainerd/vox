use vox_orchestrator::config::OrchestratorConfig;
use vox_orchestrator::orchestrator::Orchestrator;
use vox_orchestrator::types::{FileAffinity, TaskPriority};

#[tokio::test]
async fn submit_repo_shard_dag_scales_to_100_shards() {
    let orch = Orchestrator::new(OrchestratorConfig::for_testing());
    orch.spawn_agent("default")
        .expect("default agent should spawn");

    let shard_manifests: Vec<Vec<FileAffinity>> = (0..100)
        .map(|i| vec![FileAffinity::write(format!("src/shards/shard_{i}.vox"))])
        .collect();
    let merge_manifest = vec![FileAffinity::write("src/merged/repository.vox")];

    let task_ids = orch
        .submit_repo_shard_dag(
            "Assemble repository from validated shard outputs",
            shard_manifests,
            merge_manifest,
            Some(TaskPriority::Normal),
            Some("repo-scale-sim".to_string()),
        )
        .await
        .expect("repo shard DAG submission should succeed");

    // 100 generation + 100 validation + 1 reducer.
    assert_eq!(task_ids.len(), 201);

    let tasks = orch.all_tasks();
    let shard_gen_count = tasks
        .iter()
        .filter(|t| t.description.contains("[PHASE:SHARD_GEN]"))
        .count();
    let shard_validate_count = tasks
        .iter()
        .filter(|t| t.description.contains("[PHASE:SHARD_VALIDATE]"))
        .count();
    let reducer = tasks
        .iter()
        .find(|t| t.description.contains("[PHASE:REDUCE]"))
        .expect("reducer task should exist");

    assert_eq!(shard_gen_count, 100);
    assert_eq!(shard_validate_count, 100);
    assert_eq!(
        reducer.depends_on.len(),
        100,
        "reducer must depend on every validator task"
    );
}
