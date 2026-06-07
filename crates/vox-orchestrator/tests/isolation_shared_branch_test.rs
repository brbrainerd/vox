//! P4 §5.3(a): under the SharedBranch isolation strategy the file lock is
//! AUTHORITATIVE — two agents cannot both hold an exclusive write lease on the
//! same path. The second submit must fail with `LockConflict` (before this wiring
//! the acquire result was discarded with `let _ =`, so both silently "succeeded").

use vox_orchestrator::config::OrchestratorConfig;
use vox_orchestrator::isolation::IsolationStrategy;
use vox_orchestrator::orchestrator::{Orchestrator, OrchestratorError};
use vox_orchestrator::scope::ScopeEnforcement;
use vox_orchestrator::types::{AgentTask, FileAffinity, TaskId, TaskPriority};

#[tokio::test(flavor = "multi_thread")]
async fn shared_branch_denies_concurrent_writers_to_same_file() {
    let mut cfg = OrchestratorConfig::for_testing();
    cfg.isolation_strategy_default = IsolationStrategy::SharedBranch;
    cfg.scope_enforcement = ScopeEnforcement::Strict;
    let orch = Orchestrator::new(cfg);

    let a1 = orch.spawn_agent("agent-one").unwrap();
    let a2 = orch.spawn_agent("agent-two").unwrap();

    let manifest = vec![FileAffinity::write("shared.rs")];

    // Agent 1 takes the exclusive lock on shared.rs.
    let mut t1 = AgentTask::new(
        TaskId(1),
        "write shared",
        TaskPriority::Normal,
        manifest.clone(),
    );
    orch.process_task_submission_logic(&mut t1, a1, &manifest)
        .await
        .expect("first writer acquires the exclusive lease");

    // Agent 2 contends on the same path: SharedBranch makes this a hard conflict.
    let mut t2 = AgentTask::new(
        TaskId(2),
        "write shared again",
        TaskPriority::Normal,
        manifest.clone(),
    );
    let err = orch
        .process_task_submission_logic(&mut t2, a2, &manifest)
        .await
        .expect_err("second writer must be denied under SharedBranch");
    assert!(
        matches!(err, OrchestratorError::LockConflict(_)),
        "expected LockConflict, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn split_changes_tolerates_concurrent_writers() {
    // Under SplitChanges the overlap is tolerated (recorded as a conflict at
    // merge), so the second submit does NOT hard-fail on the lock.
    let mut cfg = OrchestratorConfig::for_testing();
    cfg.isolation_strategy_default = IsolationStrategy::SplitChanges;
    let orch = Orchestrator::new(cfg);

    let a1 = orch.spawn_agent("agent-one").unwrap();
    let a2 = orch.spawn_agent("agent-two").unwrap();

    let manifest = vec![FileAffinity::write("shared.rs")];

    let mut t1 = AgentTask::new(
        TaskId(1),
        "write shared",
        TaskPriority::Normal,
        manifest.clone(),
    );
    orch.process_task_submission_logic(&mut t1, a1, &manifest)
        .await
        .expect("first writer ok");

    let mut t2 = AgentTask::new(
        TaskId(2),
        "write shared again",
        TaskPriority::Normal,
        manifest.clone(),
    );
    let res = orch
        .process_task_submission_logic(&mut t2, a2, &manifest)
        .await;
    // The lock acquire is best-effort under SplitChanges; it must not be a
    // LockConflict (scope is the only other gate, and Strict is not set here).
    assert!(
        !matches!(res, Err(OrchestratorError::LockConflict(_))),
        "SplitChanges must tolerate the overlap, got {res:?}"
    );
}
