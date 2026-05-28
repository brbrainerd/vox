use super::*;
use crate::config::OrchestratorConfig;
use crate::types::{AgentTask, TaskId, TaskPriority, TaskPhase, TaskTurn};
use vox_db::{VoxDb, DbConfig};
use std::sync::Arc;

#[tokio::test]
async fn test_methodological_phase_and_transcript_recovery() {
    // 1. Setup DB and Orchestrator
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db connect");
    let arc_db = Arc::new(db);
    
    let cfg = OrchestratorConfig::for_testing();
    let orch = Orchestrator::new(cfg);
    orch.init_db(arc_db.clone()).await.expect("init db");
    
    orch.spawn_agent("worker").expect("spawn");
    let tid = TaskId(12345);
    let aid = orch.agent_ids()[0];
    
    // 2. Submit a task manually into the queue
    let mut task = AgentTask::new(tid, "persistence test task", TaskPriority::Normal, vec![]);
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let mut q = crate::sync_lock::rw_write(&*ql);
        q.enqueue(task);
    }
    crate::sync_lock::rw_write(&*orch.task_assignments).insert(tid, aid);
    
    // 3. Record some progress into the durable journal
    let turn = TaskTurn {
        agent_id: aid,
        agent_name: "worker".to_string(),
        message: "Starting inspection...".to_string(),
        timestamp_ms: crate::types::now_unix_ms(),
    };
    
    orch.record_workflow_turn(tid, &turn).await;
    orch.record_workflow_phase_change(tid, TaskPhase::Inspect).await;
    
    // 4. Simulate a memory-wipe of the task state (but keep it in the queue for hydration)
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let mut q = crate::sync_lock::rw_write(&*ql);
        let t = q.all_tasks_mut().find(|t| t.id == tid).expect("task");
        t.transcript.clear();
        t.current_phase = None;
    }
    
    // 5. Hydrate from journal
    orch.hydrate_all_tasks_from_journal().await.expect("hydrate");
    
    // 6. Verify recovery
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let q = crate::sync_lock::rw_read(&*ql);
        let t = q.all_tasks().find(|t| t.id == tid).expect("task");
        
        assert_eq!(t.current_phase, Some(TaskPhase::Inspect), "Phase should be recovered");
        assert_eq!(t.transcript.len(), 1, "Transcript should be recovered");
        assert_eq!(t.transcript[0].message, "Starting inspection...");
    }
}

#[tokio::test]
async fn test_phase_transition_persists_across_multiple_steps() {
    let db = VoxDb::connect(DbConfig::Memory).await.expect("db connect");
    let arc_db = Arc::new(db);
    
    let cfg = OrchestratorConfig::for_testing();
    let orch = Orchestrator::new(cfg);
    orch.init_db(arc_db.clone()).await.expect("init db");
    
    orch.spawn_agent("worker").expect("spawn");
    let tid = TaskId(54321);
    let aid = orch.agent_ids()[0];
    
    let task = AgentTask::new(tid, "multi-step test", TaskPriority::Normal, vec![]);
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let mut q = crate::sync_lock::rw_write(&*ql);
        q.enqueue(task);
    }
    crate::sync_lock::rw_write(&*orch.task_assignments).insert(tid, aid);

    // Initial phase
    orch.record_workflow_phase_change(tid, TaskPhase::Inspect).await;
    
    // Transition
    orch.record_workflow_phase_change(tid, TaskPhase::Act).await;

    // Simulate wipe
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let mut q = crate::sync_lock::rw_write(&*ql);
        let t = q.all_tasks_mut().find(|t| t.id == tid).expect("task");
        t.current_phase = None;
    }
    
    // Hydrate
    orch.hydrate_all_tasks_from_journal().await.expect("hydrate");
    
    // Should have the LATEST phase (last one recorded)
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let q = crate::sync_lock::rw_read(&*ql);
        let t = q.all_tasks().find(|t| t.id == tid).expect("task");
        assert_eq!(t.current_phase, Some(TaskPhase::Act), "Should recover the latest phase");
    }
}
