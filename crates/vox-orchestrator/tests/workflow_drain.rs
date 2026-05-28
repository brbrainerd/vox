//! P2-T3 acceptance: WorkflowDrainState correctly gates new dispatches.

use vox_orchestrator::drain_oplog::workflow_drain::{WorkflowDrainStarted, WorkflowDrainState};

#[test]
fn drain_started_marks_hash_no_new_starts() {
    let mut state = WorkflowDrainState::default();
    let fn_hash = [0xAA; 64];
    state.record_drain(WorkflowDrainStarted {
        fn_hash,
        started_at_unix_ms: 1_000,
    });
    assert!(state.is_draining(&fn_hash));
    assert!(!state.is_draining(&[0xBB; 64]));
}

#[test]
fn dispatcher_predicate_refuses_drained() {
    let mut state = WorkflowDrainState::default();
    let fn_hash = [0xCC; 64];
    state.record_drain(WorkflowDrainStarted {
        fn_hash,
        started_at_unix_ms: 500,
    });
    assert!(
        !state.may_start_new_run(&fn_hash),
        "drained hash must refuse new starts"
    );
    assert!(
        state.may_start_new_run(&[0xDD; 64]),
        "non-drained hash must still allow new starts"
    );
}

#[test]
fn snapshot_returns_all_draining_entries() {
    let mut state = WorkflowDrainState::default();
    state.record_drain(WorkflowDrainStarted {
        fn_hash: [0x01; 64],
        started_at_unix_ms: 100,
    });
    state.record_drain(WorkflowDrainStarted {
        fn_hash: [0x02; 64],
        started_at_unix_ms: 200,
    });
    let snap = state.snapshot();
    assert_eq!(snap.len(), 2);
}
