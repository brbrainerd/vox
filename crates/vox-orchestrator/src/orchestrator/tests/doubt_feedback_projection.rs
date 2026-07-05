use super::*;
use crate::config::OrchestratorConfig;
use crate::types::{AgentTask, TaskId, TaskPriority};

/// Regression for the rev-2 gap where Phase-1 Task 9 (doubt projection) was skipped:
/// a doubted task must surface as a non-gating `Doubt` card in the unified feedback
/// inbox, carrying `doubted_task_id` so the Needs-You "Overrule" button can dispatch
/// the real `overrule_task`.
#[tokio::test]
async fn doubt_task_surfaces_feedback_card() {
    let orch = Orchestrator::new(OrchestratorConfig::for_testing());
    orch.spawn_agent("worker").expect("spawn");
    let aid = orch.agent_ids()[0];
    let tid = TaskId(4242);

    let task = AgentTask::new(tid, "suspect task", TaskPriority::Normal, vec![]);
    {
        let ql = orch.agent_queue(aid).expect("queue");
        crate::sync_lock::rw_write(&*ql).enqueue(task);
    }
    crate::sync_lock::rw_write(&*orch.task_assignments).insert(tid, aid);

    let outcome = orch
        .doubt_task(tid, Some("conflicting spec".into()))
        .expect("doubt_task");
    orch.emit_doubt_events(tid, &outcome);

    let open = orch.feedback().open_needs_you();
    let doubt = open
        .iter()
        .find(|r| r.kind == crate::feedback::FeedbackKind::Doubt)
        .expect("doubt should surface as a Needs-You card");
    assert_eq!(
        doubt.doubted_task_id,
        Some(tid),
        "Overrule target must be the doubted task id"
    );
    assert!(doubt.gates.is_empty(), "doubts are non-gating");
    assert!(
        doubt.prompt.contains("conflicting spec"),
        "prompt should carry the doubt reason"
    );
}
