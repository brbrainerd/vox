/// Task 4.2 — no-dark-loggable-variants guard.
///
/// Asserts that every variant listed in `is_loggable` has a known wired emitter,
/// and that the 8 variants retired this cycle are NOT in the loggable set.
use vox_orchestrator::activity::is_loggable;
use vox_orchestrator::events::AgentEventKind;
use vox_orchestrator::types::AgentId;

// Helper: build a cheap representative instance of every loggable variant and
// confirm each returns true from is_loggable.
fn loggable_samples() -> Vec<AgentEventKind> {
    use vox_orchestrator::events::AgentEventKind::*;
    use vox_orchestrator::types::TaskId;
    vec![
        AgentSpawned {
            agent_id: AgentId(1),
            name: "test".into(),
        },
        AgentRetired {
            agent_id: AgentId(1),
        },
        TaskSubmitted {
            task_id: TaskId(1),
            agent_id: AgentId(1),
            description: "d".into(),
            session_id: None,
        },
        TaskStarted {
            task_id: TaskId(1),
            agent_id: AgentId(1),
            session_id: None,
        },
        TaskPhaseChanged {
            task_id: TaskId(1),
            agent_id: AgentId(1),
            phase: vox_orchestrator::types::TaskPhase::Act,
        },
        TaskCompleted {
            task_id: TaskId(1),
            agent_id: AgentId(1),
            session_id: None,
            audit_report: None,
        },
        TaskFailed {
            task_id: TaskId(1),
            agent_id: AgentId(1),
            error: "e".into(),
            session_id: None,
            audit_report: None,
        },
        LockAcquired {
            agent_id: AgentId(1),
            path: std::path::PathBuf::from("db://row/1"),
            exclusive: true,
        },
        LockReleased {
            agent_id: AgentId(1),
            path: std::path::PathBuf::from("db://row/1"),
        },
        ConflictDetected {
            path: std::path::PathBuf::from("foo.rs"),
            agent_ids: vec![AgentId(1)],
            conflict_id: "c1".into(),
        },
        FeedbackRequested {
            feedback_id: "f1".into(),
            kind: "doubt".into(),
            gates: vec![],
            surface: "needs-you".into(),
        },
        FeedbackResolved {
            feedback_id: "f1".into(),
        },
    ]
}

#[test]
fn wired_variants_are_loggable() {
    for sample in loggable_samples() {
        assert!(
            is_loggable(&sample),
            "Expected is_loggable == true for: {:?}",
            sample
        );
    }
}

/// Retired variants must NOT appear in is_loggable.
#[test]
fn retired_dark_variants_are_not_loggable() {
    use vox_orchestrator::events::AgentEventKind::*;
    use vox_orchestrator::types::TaskId;

    let retired: Vec<AgentEventKind> = vec![
        BuildStage {
            run_id: "r1".into(),
            stage: vox_orchestrator::events::BuildStageKind::Lex,
            status: "ok".into(),
            duration_ms: None,
            diagnostic_count: 0,
        },
        WorkflowStarted {
            workflow_id: "w1".into(),
        },
        WorkflowCompleted {
            workflow_id: "w1".into(),
        },
        WorkflowFailed {
            workflow_id: "w1".into(),
            error: "e".into(),
        },
        TaskDelegated {
            parent_agent_id: AgentId(1),
            child_agent_id: AgentId(2),
            task_id: TaskId(1),
            reason: "r".into(),
        },
        MeshTopologyChanged {
            added_nodes: vec![],
            removed_nodes: vec![],
            changed_edges: 0,
        },
        TaskReprioritized {
            task_id: TaskId(1),
            old_priority: vox_orchestrator::types::TaskPriority::Normal,
            new_priority: vox_orchestrator::types::TaskPriority::Urgent,
            actor: vox_orchestrator::types::PrioritySource::Developer,
            reason: None,
            session_id: None,
        },
        AttentionBudgetAlert {
            agent_id: AgentId(1),
            threshold: 0.9,
            spent_ms: 900,
            max_ms: 1000,
        },
    ];

    for sample in &retired {
        assert!(
            !is_loggable(sample),
            "Expected is_loggable == false (retired) for: {:?}",
            sample
        );
    }
}
