use super::*;
use crate::config::OrchestratorConfig;
use crate::types::{AgentTask, CompletionAttestation, FileAffinity, TaskId, TaskPriority};

/// Shared assertions for affinity / scope / lock alignment with [`Orchestrator::task_assignments`].
#[cfg(test)]
fn assert_assignment_affinity_scope_locks_aligned(orch: &Orchestrator) {
    let assignments = orch.task_assignments.read().unwrap();
    let agents = orch.agents.read().unwrap();
    let scope = orch.scope_guard.read().unwrap();

    for (&task_id, &agent_id) in assignments.iter() {
        let queue_lock = agents.get(&agent_id).unwrap_or_else(|| {
            panic!(
                "task {} assigned to missing agent {}",
                task_id.0, agent_id.0
            )
        });
        let queue = queue_lock.read().unwrap();
        let task_ref = queue
            .tasks()
            .iter()
            .find(|t| t.id == task_id)
            .or_else(|| queue.current_task().filter(|t| t.id == task_id))
            .unwrap_or_else(|| {
                panic!(
                    "task {} assigned to agent {} but not present in that queue",
                    task_id.0, agent_id.0
                )
            });

        for path in task_ref.write_files() {
            assert_eq!(
                orch.affinity_map.lookup(path.as_path()),
                Some(agent_id),
                "affinity owner for task {} path {:?} should match assignment agent {}",
                task_id.0,
                path,
                agent_id.0
            );
            let in_scope = scope
                .agent_scope(agent_id)
                .map(|s| s.contains(path))
                .unwrap_or(false);
            assert!(
                in_scope,
                "agent {} scope should contain {:?} for task {}",
                agent_id.0, path, task_id.0
            );
            let holder = orch.lock_manager.holder(path.as_path());
            assert_eq!(
                holder.map(|(a, _)| a),
                Some(agent_id),
                "lock holder for {:?} should be agent {} (task {})",
                path,
                agent_id.0,
                task_id.0
            );
        }
    }
}

#[cfg(test)]
fn assert_path_has_no_writer_ownership(
    orch: &Orchestrator,
    path: &std::path::Path,
    former: crate::types::AgentId,
) {
    assert!(
        orch.affinity_map.lookup(path).is_none(),
        "affinity should be released for {:?}",
        path
    );
    assert!(
        !orch.lock_manager.is_locked(path),
        "lock should be released for {:?}",
        path
    );
    let scope = orch.scope_guard.read().unwrap();
    if let Some(files) = scope.agent_scope(former) {
        assert!(
            !files.iter().any(|p| p.as_path() == path),
            "former agent {} should not retain {:?} in scope",
            former.0,
            path
        );
    }
}

#[cfg(test)]
mod state_invariants {
    use super::*;
    use std::path::Path;

    fn orch_named_pair() -> Orchestrator {
        let orch = Orchestrator::new(OrchestratorConfig {
            max_agents: 8,
            ..OrchestratorConfig::for_testing()
        });
        orch.spawn_agent("heavy").expect("spawn heavy");
        orch.spawn_agent("light").expect("spawn light");
        orch
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn rebalance_transfer_keeps_assignments_affinity_scope_locks_coherent() {
        let orch = orch_named_pair();
        for i in 0..15 {
            orch.submit_task_with_agent(
                format!("heavy {i}"),
                vec![FileAffinity::write(format!("state_inv/rebalance/h{i}.rs"))],
                None,
                Some("heavy".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .expect("submit heavy");
        }
        orch.submit_task_with_agent(
            "light sole",
            vec![FileAffinity::write("state_inv/rebalance/light_only.rs")],
            None,
            Some("light".to_string()),
            None,
            None,
            None,
            None,
        )
        .await
        .expect("submit light");

        super::assert_assignment_affinity_scope_locks_aligned(&orch);

        let moved = orch.rebalance();
        assert!(
            moved > 0,
            "expected at least one stolen task when heavy is overloaded vs light"
        );

        super::assert_assignment_affinity_scope_locks_aligned(&orch);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cancel_unique_write_does_not_leak_affinity_scope_or_lock() {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        let path = Path::new("state_inv/cancel_unique.rs");
        let tid = orch
            .submit_task("cancel me", vec![FileAffinity::write(path)], None, None, None)
            .await
            .expect("submit");
        let aid = *orch
            .task_assignments
            .read()
            .unwrap()
            .get(&tid)
            .expect("assignment");
        orch.cancel_task(tid).expect("cancel");
        assert!(
            !orch.task_assignments.read().unwrap().contains_key(&tid),
            "cancel should drop task_assignments entry"
        );
        super::assert_path_has_no_writer_ownership(&orch, path, aid);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn fail_in_progress_unique_write_does_not_leak_affinity_scope_or_lock() {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        let path = Path::new("state_inv/fail_unique.rs");
        let tid = orch
            .submit_task("fail me", vec![FileAffinity::write(path)], None, None, None)
            .await
            .expect("submit");
        let aid = *orch
            .task_assignments
            .read()
            .unwrap()
            .get(&tid)
            .expect("assignment");
        orch.agent_queue(aid)
            .expect("queue")
            .write()
            .unwrap()
            .dequeue();
        orch.fail_task(tid, "boom".to_string()).await.expect("fail");
        super::assert_path_has_no_writer_ownership(&orch, path, aid);
    }
}

#[cfg(test)]
mod orch_smoke {
    use super::*;

    fn test_orchestrator() -> Orchestrator {
        Orchestrator::new(OrchestratorConfig::for_testing())
    }

    fn complete_attestation_for_tests() -> CompletionAttestation {
        CompletionAttestation {
            checks_passed: vec!["human_review_approved".to_string()],
            ..Default::default()
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn spawn_agent() {
        let orch = test_orchestrator();
        let id = orch.spawn_agent("parser").expect("spawn");
        assert_eq!(orch.status().agent_count, 1);
        assert!(orch.agent_queue(id).is_some());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn max_agents_enforced() {
        let orch = Orchestrator::new(OrchestratorConfig {
            max_agents: 2,
            ..OrchestratorConfig::for_testing()
        });
        orch.spawn_agent("a").unwrap();
        orch.spawn_agent("b").unwrap();
        let err = orch.spawn_agent("c").unwrap_err();
        assert!(matches!(
            err,
            OrchestratorError::MaxAgentsReached { max: 2 }
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn submit_and_route() {
        let orch = test_orchestrator();
        let task_id = orch
            .submit_task(
                "Fix parser bug",
                vec![FileAffinity::write("grammar.rs")],
                None,
                None,
                None,
            )
            .await
            .expect("submit");
        assert_eq!(orch.status().total_queued, 1);
        assert_eq!(orch.status().agent_count, 1); // auto-spawned
        assert!(orch.task_assignments.read().unwrap().contains_key(&task_id));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn submit_seeds_socrates_from_session_context_envelope_key() {
        let orch = test_orchestrator();
        let sid = "orch-test-session";
        let key = crate::socrates::session_context_envelope_key(sid);
        let env = crate::socrates::SessionRetrievalEnvelope {
            retrieval_tier: "hybrid".to_string(),
            memory_hit_count: 2,
            knowledge_hit_count: 1,
            chunk_hit_count: 0,
            repo_hit_count: 0,
            rrf_fused_hit_count: 0,
            used_vector: true,
            used_bm25: true,
            used_lexical_fallback: false,
            contradiction_count: 0,
            source_diversity: 2,
            evidence_quality: 0.8,
            citation_coverage: 1.0,
            verification_performed: false,
            verification_reason: None,
            recommended_next_action: None,
        };
        let context = crate::ContextEnvelope::from_session_retrieval("repo-orch-test", sid, &env);
        let json = serde_json::to_string(&context).unwrap();
        orch.context_store
            .write()
            .unwrap()
            .set(crate::types::AgentId(0), key, json, 0);

        let tid = orch
            .submit_task(
                "task with session",
                vec![FileAffinity::read("README.md")],
                None,
                Some(sid.to_string()),
                None,
            )
            .await
            .expect("submit");

        let aid = *orch.task_assignments.read().unwrap().get(&tid).unwrap();
        let q_lock = orch.agent_queue(aid).unwrap();
        let q = q_lock.read().unwrap();
        let t = q.tasks().iter().find(|t| t.id == tid).expect("queued task");
        let soc = t.socrates.as_ref().expect("socrates from context store");
        assert_eq!(soc.retrieval_tier.as_deref(), Some("hybrid"));
        assert_eq!(soc.evidence_count, 3);
        assert!(soc.retrieval_used_vector);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn same_file_routes_to_same_agent() {
        let orch = test_orchestrator();
        let t1 = orch
            .submit_task(
                "Task 1",
                vec![FileAffinity::write("src/lib.rs")],
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let t2 = orch
            .submit_task(
                "Task 2",
                vec![FileAffinity::write("src/lib.rs")],
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Both tasks should be assigned to the same agent
        let assignments = orch.task_assignments.read().unwrap();
        assert_eq!(
            assignments.get(&t1),
            assignments.get(&t2),
            "tasks touching the same file should route to the same agent"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn different_files_can_route_to_different_agents() {
        let orch = test_orchestrator();
        orch.submit_task(
            "Parser work",
            vec![FileAffinity::write("crates/vox-parser/src/lib.rs")],
            None,
            None,
            None,
        )
        .await
        .unwrap();
        orch.submit_task(
            "Codegen work",
            vec![FileAffinity::write("crates/vox-codegen-rust/src/lib.rs")],
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Should have spawned at least one agent (may be 1 or 2 depending on routing)
        assert!(orch.status().agent_count >= 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn complete_task_flow() {
        let orch = test_orchestrator();
        let task_id = orch
            .submit_task(
                "Test task",
                vec![FileAffinity::write("test.rs")],
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let agent_id = *orch.task_assignments.read().unwrap().get(&task_id).unwrap();

        // Dequeue the task (simulating an agent picking it up)
        orch.agent_queue(agent_id)
            .unwrap()
            .write()
            .unwrap()
            .dequeue();

        // Complete it (writes can classify as Review/Confirm; attestation satisfies gates)
        orch.complete_task_with_attestation(task_id, Some(complete_attestation_for_tests()))
            .await
            .expect("complete");
        assert_eq!(orch.status().total_completed, 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn complete_task_with_link_audit_enabled_skips_check_links_without_md_writes() {
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.completion_markdown_link_audit_enabled = true;
        let orch = Orchestrator::new(cfg);
        let task_id = orch
            .submit_task(
                "rs-only writes",
                vec![FileAffinity::write("src/link_audit_flag_smoke.rs")],
                None,
                None,
                None,
            )
            .await
            .unwrap();
        let agent_id = *orch.task_assignments.read().unwrap().get(&task_id).unwrap();
        orch.agent_queue(agent_id)
            .unwrap()
            .write()
            .unwrap()
            .dequeue();
        orch.complete_task_with_attestation(task_id, Some(complete_attestation_for_tests()))
            .await
            .expect("complete");
        assert_eq!(orch.status().total_completed, 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn retire_agent_returns_tasks() {
        let orch = test_orchestrator();
        let agent_id = orch.spawn_agent("temp").unwrap();

        // Manually enqueue a task
        let task = AgentTask::new(TaskId(99), "leftover", TaskPriority::Normal, vec![]);
        orch.agent_queue(agent_id)
            .unwrap()
            .write()
            .unwrap()
            .enqueue(task);

        let remaining = orch.retire_agent(agent_id).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(orch.agent_queue(agent_id).is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn pause_resume_agent() {
        let orch = test_orchestrator();
        let agent_id = orch.spawn_agent("test").unwrap();

        orch.pause_agent(agent_id).unwrap();
        assert!(
            orch.agent_queue(agent_id)
                .unwrap()
                .read()
                .unwrap()
                .is_paused()
        );

        orch.resume_agent(agent_id).unwrap();
        assert!(
            !orch
                .agent_queue(agent_id)
                .unwrap()
                .read()
                .unwrap()
                .is_paused()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn disabled_orchestrator_rejects_tasks() {
        let orch = Orchestrator::new(OrchestratorConfig {
            enabled: false,
            ..OrchestratorConfig::for_testing()
        });
        let err = orch
            .submit_task("test", vec![], None, None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, OrchestratorError::Disabled));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn status_snapshot() {
        let orch = test_orchestrator();
        orch.submit_task("t1", vec![FileAffinity::write("a.rs")], None, None, None)
            .await
            .unwrap();
        orch.submit_task("t2", vec![FileAffinity::write("b.rs")], None, None, None)
            .await
            .unwrap();

        let status = orch.status();
        assert!(status.enabled);
        assert!(status.total_queued >= 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn task_trace_after_submit() {
        let orch = test_orchestrator();
        let task_id = orch
            .submit_task("Trace me", vec![FileAffinity::write("x.rs")], None, None, None)
            .await
            .unwrap();
        let steps = orch.task_trace(task_id).expect("trace exists");
        assert!(steps.len() >= 2);
        assert_eq!(steps[0].stage, "ingress");
        assert_eq!(steps[1].stage, "routed");
        assert!(
            steps[1]
                .detail
                .as_ref()
                .map(|d| d.starts_with("agent "))
                .unwrap_or(false)
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn task_trace_after_complete() {
        let orch = test_orchestrator();
        let task_id = orch
            .submit_task("Complete me", vec![FileAffinity::write("y.rs")], None, None, None)
            .await
            .unwrap();
        let agent_id = *orch.task_assignments.read().unwrap().get(&task_id).unwrap();
        orch.agent_queue(agent_id)
            .unwrap()
            .write()
            .unwrap()
            .dequeue();
        orch.complete_task_with_attestation(task_id, Some(complete_attestation_for_tests()))
            .await
            .unwrap();
        let steps = orch.task_trace(task_id).expect("trace exists");
        let outcome = steps
            .iter()
            .find(|s| s.stage == "outcome")
            .expect("outcome step");
        assert_eq!(outcome.detail.as_deref(), Some("completed"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn task_trace_after_fail() {
        let orch = test_orchestrator();
        let task_id = orch
            .submit_task("Fail me", vec![FileAffinity::write("z.rs")], None, None, None)
            .await
            .unwrap();
        let agent_id = *orch.task_assignments.read().unwrap().get(&task_id).unwrap();
        orch.agent_queue(agent_id)
            .unwrap()
            .write()
            .unwrap()
            .dequeue();
        orch.fail_task(task_id, "timeout".to_string())
            .await
            .unwrap();
        let steps = orch.task_trace(task_id).expect("trace exists");
        let outcome = steps
            .iter()
            .find(|s| s.stage == "outcome")
            .expect("outcome step");
        assert!(
            outcome
                .detail
                .as_deref()
                .map(|d| d.starts_with("failed: "))
                .unwrap_or(false)
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn socrates_enforced_gate_requeues_low_confidence_task() {
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.socrates_gate_enforce = true;
        cfg.socrates_gate_shadow = true;
        cfg.max_debug_iterations = 2;
        let orch = Orchestrator::new(cfg);
        let agent_id = orch.spawn_agent("socrates").expect("spawn");

        let task_id = TaskId(9001);
        let mut task = AgentTask::new(
            task_id,
            "grounded answer required",
            TaskPriority::Normal,
            vec![FileAffinity::write("facts.md")],
        );
        task.socrates = Some(crate::socrates::SocratesTaskContext {
            factual_mode: true,
            required_citations: 3,
            evidence_count: 0,
            contradiction_hints: 0,
            risk_budget: "high".to_string(),
            retrieval_tier: None,
            retrieval_used_vector: false,
            retrieval_used_lexical_fallback: false,
            ..Default::default()
        });
        {
            let queue_lock = orch.agent_queue(agent_id).expect("queue");
            let mut queue = queue_lock.write().unwrap();
            queue.enqueue(task);
            let _ = queue.dequeue();
        }
        orch.task_assignments
            .write()
            .unwrap()
            .insert(task_id, agent_id);

        orch.complete_task(task_id).await.expect("gate path");

        let q_lock = orch.agent_queue(agent_id).expect("queue snapshot");
        let q = q_lock.read().unwrap();
        assert_eq!(q.completed_count(), 0);
        assert!(!q.is_empty());
    }

    fn grounding_violating_attestation() -> CompletionAttestation {
        CompletionAttestation {
            completion_summary: Some(
                "The handler returns HTTP 403 for anonymous users when auth middleware is disabled."
                    .to_string(),
            ),
            checks_passed: vec!["human_review_approved".to_string()],
            ..Default::default()
        }
    }

    async fn orch_memory_with_init(
        cfg: OrchestratorConfig,
    ) -> (std::sync::Arc<vox_db::VoxDb>, Orchestrator) {
        let db = std::sync::Arc::new(
            vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
                .await
                .expect("memory db"),
        );
        let orch = Orchestrator::new(cfg).with_db(db.clone());
        orch.init_db(db.clone())
            .await
            .expect("orchestrator init_db");
        (db, orch)
    }

    async fn seed_agent_reliability_high(db: &vox_db::VoxDb, agent_id: AgentId) {
        let sid = agent_id.0.to_string();
        // Five successive outcomes reach (5+2)/(5+3) ≈ 0.875 ≥ default relax floor 0.85.
        for _ in 0..5 {
            db.record_task_reliability_observation(&sid, true)
                .await
                .expect("record reliability");
        }
        let r = db
            .get_agent_reliability(&sid)
            .await
            .expect("read reliability");
        assert!(
            r.is_some_and(|x| x >= 0.85),
            "expected Laplace reliability >= 0.85 after five successes, got {r:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn trust_relax_allows_completion_under_grounding_enforce_when_agent_reliable() {
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.socrates_gate_enforce = false;
        cfg.completion_grounding_enforce = true;
        cfg.completion_grounding_shadow = false;
        cfg.trust_gate_relax_enabled = true;
        cfg.trust_gate_relax_min_reliability = 0.85;
        cfg.max_socrates_debug_iterations = 2;
        let (_db, orch) = orch_memory_with_init(cfg).await;

        let agent_id = orch.spawn_agent("ground-relax").expect("spawn");
        seed_agent_reliability_high(&_db, agent_id).await;

        let task_id = TaskId(9021);
        let mut task = AgentTask::new(
            task_id,
            "factual summary without declared citations",
            TaskPriority::Normal,
            vec![FileAffinity::write("facts.md")],
        );
        task.socrates = Some(crate::socrates::SocratesTaskContext {
            factual_mode: true,
            required_citations: 2,
            evidence_count: 0,
            contradiction_hints: 0,
            risk_budget: "normal".to_string(),
            retrieval_tier: None,
            retrieval_used_vector: false,
            retrieval_used_lexical_fallback: false,
            ..Default::default()
        });
        {
            let queue_lock = orch.agent_queue(agent_id).expect("queue");
            let mut queue = queue_lock.write().unwrap();
            queue.enqueue(task);
            let _ = queue.dequeue();
        }
        orch.task_assignments
            .write()
            .unwrap()
            .insert(task_id, agent_id);

        orch.complete_task_with_attestation(task_id, Some(grounding_violating_attestation()))
            .await
            .expect("complete without grounding requeue when trust relax applies");

        let q_lock = orch.agent_queue(agent_id).expect("queue snapshot");
        let q = q_lock.read().unwrap();
        assert_eq!(
            q.completed_count(),
            1,
            "task should complete, not requeue for grounding"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn completion_grounding_enforce_requeues_when_trust_relax_disabled_even_if_reliable() {
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.socrates_gate_enforce = false;
        cfg.completion_grounding_enforce = true;
        cfg.completion_grounding_shadow = false;
        cfg.trust_gate_relax_enabled = false;
        cfg.trust_gate_relax_min_reliability = 0.85;
        cfg.max_socrates_debug_iterations = 2;
        let (_db, orch) = orch_memory_with_init(cfg).await;

        let agent_id = orch.spawn_agent("ground-strict").expect("spawn");
        seed_agent_reliability_high(&_db, agent_id).await;

        let task_id = TaskId(9022);
        let mut task = AgentTask::new(
            task_id,
            "factual summary without declared citations (strict)",
            TaskPriority::Normal,
            vec![FileAffinity::write("facts2.md")],
        );
        task.socrates = Some(crate::socrates::SocratesTaskContext {
            factual_mode: true,
            required_citations: 2,
            evidence_count: 0,
            contradiction_hints: 0,
            risk_budget: "normal".to_string(),
            retrieval_tier: None,
            retrieval_used_vector: false,
            retrieval_used_lexical_fallback: false,
            ..Default::default()
        });
        {
            let queue_lock = orch.agent_queue(agent_id).expect("queue");
            let mut queue = queue_lock.write().unwrap();
            queue.enqueue(task);
            let _ = queue.dequeue();
        }
        orch.task_assignments
            .write()
            .unwrap()
            .insert(task_id, agent_id);

        orch.complete_task_with_attestation(task_id, Some(grounding_violating_attestation()))
            .await
            .expect("completion returns ok while requeueing");

        let q_lock = orch.agent_queue(agent_id).expect("queue snapshot");
        let q = q_lock.read().unwrap();
        assert_eq!(q.completed_count(), 0);
        assert!(!q.is_empty());
        let requeued = q
            .tasks()
            .iter()
            .chain(q.current_task())
            .find(|t| t.id == task_id)
            .expect("requeued task");
        assert!(
            requeued.description.contains("[GROUNDING GATE]"),
            "expected grounding requeue banner, got {:?}",
            requeued.description
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn submit_goal_falls_back_to_direct_when_planning_disabled() {
        let orch = test_orchestrator();
        let task_id = orch
            .submit_goal(
                "small direct change",
                vec![FileAffinity::write("src/lib.rs")],
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("submit goal");
        assert!(orch.task_assignments.read().unwrap().contains_key(&task_id));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn submit_goal_force_plan_attaches_plan_metadata() {
        let orch = Orchestrator::new(OrchestratorConfig {
            planning_enabled: true,
            planning_router_enabled: true,
            planning_auto_mode_enabled: true,
            ..OrchestratorConfig::for_testing()
        });
        let task_id = orch
            .submit_goal(
                "Refactor authentication helpers and add regression tests for session handling",
                vec![FileAffinity::write("src/lib.rs")],
                None,
                Some(crate::planning::PlanningMode::ForcePlan),
                Some("s1".to_string()),
                None,
                None,
            )
            .await
            .expect("submit planned goal");
        let agent_id = *orch
            .task_assignments
            .read()
            .unwrap()
            .get(&task_id)
            .expect("assignment");
        let queue_lock = orch.agent_queue(agent_id).expect("queue");
        let queue = queue_lock.read().unwrap();
        let has_meta = queue
            .tasks()
            .iter()
            .any(|t| t.id == task_id && t.plan_session_id.is_some() && t.plan_node_id.is_some());
        assert!(has_meta, "planned task should contain planning metadata");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn fail_task_with_replan_trigger_enqueues_recovery_work() {
        let orch = Orchestrator::new(OrchestratorConfig {
            planning_enabled: true,
            planning_replan_enabled: true,
            planning_auto_mode_enabled: true,
            ..OrchestratorConfig::for_testing()
        });
        let agent_id = orch.spawn_agent("planner").expect("spawn");
        let mut t = AgentTask::new(
            TaskId(777),
            "run tests",
            TaskPriority::Normal,
            vec![FileAffinity::write("src/lib.rs")],
        );
        t.plan_session_id = Some("p1".to_string());
        t.plan_node_id = Some("n1".to_string());
        t.plan_version = Some(1);
        t.execution_policy_json = Some(
            serde_json::json!({
                "replan_triggers": ["test_failure_new_regression"]
            })
            .to_string(),
        );
        {
            let queue_lock = orch.agent_queue(agent_id).expect("queue");
            let mut queue = queue_lock.write().unwrap();
            queue.enqueue(t);
            let _ = queue.dequeue();
        }
        orch.task_assignments
            .write()
            .unwrap()
            .insert(TaskId(777), agent_id);
        orch.fail_task(TaskId(777), "test failure in suite".to_string())
            .await
            .expect("fail");
        // Recovery is enqueued via `enqueue_plan_node` (manifest `Cargo.toml`), which routes to
        // whichever agent owns that affinity — not necessarily the agent that failed.
        let st = orch.status();
        assert!(
            st.total_queued >= 1,
            "replan should enqueue recovery work on some agent (total_queued={}, per_agent={:?})",
            st.total_queued,
            st.agents
                .iter()
                .map(|a| (a.id, a.queued))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn plan_dag_unblocks_next_node_on_complete() {
        let db = std::sync::Arc::new(
            vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
                .await
                .expect("memory db"),
        );
        let orch = Orchestrator::new(OrchestratorConfig {
            planning_enabled: true,
            planning_router_enabled: true,
            planning_auto_mode_enabled: true,
            ..OrchestratorConfig::for_testing()
        })
        .with_db(db.clone());

        let tid = orch
            .submit_goal(
                "Implement alpha feature scaffolding in crates/demo and verify beta integration paths",
                vec![FileAffinity::read("Cargo.toml")],
                None,
                Some(crate::planning::PlanningMode::ForcePlan),
                Some("plan-dag-test".into()),
                None,
                None,
            )
            .await
            .expect("planned goal");

        let agent_id = *orch
            .task_assignments
            .read()
            .unwrap()
            .get(&tid)
            .expect("assignment");

        let plan_session_id = {
            let ql = orch.agent_queue(agent_id).expect("queue");
            let q = ql.read().unwrap();
            q.tasks()
                .iter()
                .find(|t| t.id == tid)
                .and_then(|t| t.plan_session_id.clone())
                .expect("plan session on task")
        };

        {
            let ql = orch.agent_queue(agent_id).expect("queue");
            let mut q = ql.write().unwrap();
            let _ = q.dequeue();
        }

        orch.complete_task_with_attestation(tid, Some(complete_attestation_for_tests()))
            .await
            .expect("complete first node");

        let rows = db
            .load_plan_nodes_with_status(&plan_session_id, 1)
            .await
            .expect("nodes");
        let n1 = rows.iter().find(|r| r.node_id == "n1").expect("n1 row");
        let n2 = rows.iter().find(|r| r.node_id == "n2").expect("n2 row");
        assert_eq!(n1.status, "completed");
        assert_eq!(n2.status, "queued");

        let st = orch.status();
        assert!(
            st.total_queued >= 1,
            "successor plan node should be enqueued (total_queued={})",
            st.total_queued
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn complexity_based_routing_test() {
        let _ = tracing_subscriber::fmt::try_init();

        let orch = Orchestrator::new(OrchestratorConfig {
            planning_enabled: false,
            ..OrchestratorConfig::for_testing()
        });

        // This description should trigger SocratesComplexityJudge::estimate_complexity -> MultiHop
        // because it contains 'synthesize' and 'across'.
        let goal = "Synthesize all data across the repository";

        let ctx = orch.generate_goal_search_context(goal, &[]).await;

        assert!(ctx.factual_mode);
    }
}

mod persistence_integrity;
mod populi_single_owner;
