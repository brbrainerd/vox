use crate::orchestrator::{Orchestrator, OrchestratorError};
use crate::services::MessageGateway;
use crate::types::{AgentId, CompletionAttestation, TaskId, TaskStatus};

pub mod gates;
pub mod healing;
pub mod persistence;
pub mod socrates;

impl Orchestrator {
    pub async fn complete_task(&self, task_id: TaskId) -> Result<(), OrchestratorError> {
        self.complete_task_with_attestation(task_id, None).await
    }

    pub async fn complete_task_with_audit(
        &self,
        task_id: TaskId,
        audit_report: String,
    ) -> Result<(), OrchestratorError> {
        let agent_id = crate::sync_lock::rw_read(&*self.task_assignments)
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;

        {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            let queue_lock = agents
                .get(&agent_id)
                .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
            let mut queue = crate::sync_lock::rw_write(&**queue_lock);
            if let Some(task) = queue.find_task_mut(task_id) {
                task.audit_report = Some(audit_report);
            } else if let Some(task) = queue.current_task_mut() {
                if task.id == task_id {
                    task.audit_report = Some(audit_report);
                }
            }
        }

        self.complete_task_with_attestation(task_id, None).await
    }

    pub async fn complete_task_with_attestation(
        &self,
        task_id: TaskId,
        completion_attestation: Option<CompletionAttestation>,
    ) -> Result<(), OrchestratorError> {
        let agent_id = crate::sync_lock::rw_read(&*self.task_assignments)
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;

        self.record_activity();
        crate::sync_lock::rw_write(&self.monitor).record_progress(agent_id);
        crate::sync_lock::rw_read(&*self.budget_manager).record_task_completion(agent_id);

        let (task_clone_opt, phase_label, write_files) = {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            if let Some(queue_lock) = agents.get(&agent_id) {
                let queue = crate::sync_lock::rw_read(&**queue_lock);
                if let Some(t) = queue.current_task() {
                    (
                        Some(t.clone()),
                        Self::extract_phase_label(&t.description),
                        t.write_files().into_iter().cloned().collect::<Vec<_>>(),
                    )
                } else {
                    (None, String::new(), Vec::new())
                }
            } else {
                (None, String::new(), Vec::new())
            }
        };

        let mut trust_score_opt = None;
        if let Some(db) = self.db() {
            if task_clone_opt.is_some() {
                let rows = db
                    .list_trust_scores_for_dimension(
                        "agent",
                        "task_completion",
                        Some(phase_label.as_str()),
                        512,
                    )
                    .await
                    .unwrap_or_default();
                trust_score_opt = Some(
                    rows.into_iter()
                        .find_map(|(id, score)| {
                            id.parse::<u64>()
                                .ok()
                                .filter(|aid| *aid == agent_id.0)
                                .map(|_| score)
                        })
                        .unwrap_or(0.5),
                );
            }
        }

        let link_audit_enabled =
            crate::sync_lock::rw_read(&*self.config).completion_markdown_link_audit_enabled;
        let broken_links_report = if task_clone_opt.is_some() && link_audit_enabled {
            crate::orchestrator::task_dispatch::complete::audit_reporter::verify_broken_links(
                &write_files,
            )
            .await
        } else {
            String::new()
        };

        if let Some(ref t) = task_clone_opt {
            self.apply_vox_healing(agent_id, &t.description, &write_files)
                .await?;
        }

        let mut behavioral_failure = None;
        if task_clone_opt.is_some() {
            let require_behavioral =
                crate::sync_lock::rw_read(&*self.config).behavioral_gate_on_complete;
            let gate = crate::gate::BehavioralGate::new(require_behavioral);
            if let crate::gate::GateResult::BehavioralTestFailed { message } =
                gate.check_behavior(None).await
            {
                behavioral_failure = Some(message);
            }
        }

        {
            let queue_lock = {
                let agents = crate::sync_lock::rw_read(&*self.agents);
                agents
                    .get(&agent_id)
                    .ok_or(OrchestratorError::AgentNotFound(agent_id))?
                    .clone()
            };
            let mut queue = crate::sync_lock::rw_write(&*queue_lock);

            let (
                max_debug_iterations,
                max_toestub_debug_iterations,
                _max_socrates_debug_iterations,
                trust_floor,
                _trust_relax_min,
            ) = {
                let cfg = crate::sync_lock::rw_read(&*self.config);
                (
                    cfg.max_debug_iterations,
                    cfg.max_toestub_debug_iterations,
                    cfg.max_socrates_debug_iterations,
                    cfg.trust_task_completion_floor,
                    cfg.trust_gate_relax_min_reliability,
                )
            };
            let mut auto_debug_requeue: Option<(crate::types::AgentTask, String, usize, usize)> =
                None;

            if let Some(task) = queue.current_task() {
                // Behavioral gate
                if let Some(msg) = gates::check_behavioral_gate(&mut behavioral_failure) {
                    if task.debug_iterations < max_debug_iterations {
                        let mut t = task.clone();
                        t.debug_iterations += 1;
                        t.description.push_str(&format!(
                            "\n\n[BEHAVIORAL GATE]\nBehavioral tests failed. Fix and retry:\n{}",
                            msg
                        ));
                        t.status = TaskStatus::Queued;
                        auto_debug_requeue = Some((t, msg, 1, 0));
                    } else {
                        return Err(OrchestratorError::TaskValidationFailed(msg));
                    }
                }

                // Research JSON gate
                if auto_debug_requeue.is_none() {
                    match gates::check_research_json_gate(
                        task,
                        completion_attestation.as_ref(),
                        max_debug_iterations,
                    ) {
                        Ok(outcome) => auto_debug_requeue = outcome.requeue,
                        Err(e) => return Err(OrchestratorError::TaskValidationFailed(e)),
                    }
                }

                // Approval gate
                if auto_debug_requeue.is_none() {
                    match gates::check_approval_gate(
                        task,
                        completion_attestation.as_ref(),
                        max_debug_iterations,
                    ) {
                        Ok(outcome) => auto_debug_requeue = outcome.requeue,
                        Err(e) => return Err(OrchestratorError::ApprovalAttestationRequired(e)),
                    }
                }

                // Trust gate
                if auto_debug_requeue.is_none() {
                    let outcome = gates::check_trust_gate(
                        task,
                        completion_attestation.as_ref(),
                        trust_score_opt.unwrap_or(0.5),
                        trust_floor,
                        max_toestub_debug_iterations,
                        &phase_label,
                    );
                    auto_debug_requeue = outcome.requeue;
                }

                // Harness gate
                if auto_debug_requeue.is_none() {
                    match gates::check_harness_gate(
                        task,
                        completion_attestation.as_ref(),
                        max_debug_iterations,
                    ) {
                        Ok(outcome) => auto_debug_requeue = outcome.requeue,
                        Err(e) => return Err(OrchestratorError::ScopeDenied(e)),
                    }
                }

                // TOESTUB gate
                #[cfg(feature = "toestub-gate")]
                if auto_debug_requeue.is_none()
                    && crate::sync_lock::rw_read(&*self.config).toestub_gate
                {
                    match gates::check_toestub_gate(
                        task,
                        completion_attestation.as_ref(),
                        max_toestub_debug_iterations,
                    ) {
                        Ok(outcome) => auto_debug_requeue = outcome.requeue,
                        Err(e) => {
                            if e.contains("Lock conflict") {
                                // Extract the message from "Lock conflict: <msg>"
                                let _msg = e.replace("Lock conflict: ", "");
                                return Err(OrchestratorError::LockConflict(
                                    crate::locks::LockConflict::ExclusivelyHeld {
                                        path: std::path::PathBuf::from("unspecified"),
                                        holder: AgentId(0),
                                    },
                                ));
                            }
                            return Err(OrchestratorError::ScopeDenied(e));
                        }
                    }
                }

                // Doc integrity gate
                if auto_debug_requeue.is_none() {
                    let outcome = gates::check_doc_integrity_gate(
                        task,
                        &broken_links_report,
                        &write_files,
                        max_debug_iterations,
                    );
                    auto_debug_requeue = outcome.requeue;
                }
            }

            if let Some((requeue_task, err_report, _err_n, _warn_n)) = auto_debug_requeue {
                tracing::warn!(
                    "Task {} failed validation. Auto-debugging (iteration {})",
                    task_id,
                    requeue_task.debug_iterations
                );
                queue.mark_failed(
                    task_id,
                    format!("Auto-debug validation failure:\n{}", err_report),
                );
                self.record_task_loop_metric(
                    task_id,
                    &phase_label,
                    "toestub_requeue",
                    requeue_task.debug_iterations,
                );
                queue.enqueue(requeue_task);
                return Ok(());
            }
        }

        // Socrates gate (needs await, so drop queue above)
        let socrates_outcome = {
            let (task_clone, max_socrates_iterations) = {
                let agents = crate::sync_lock::rw_read(&*self.agents);
                let queue_lock = agents
                    .get(&agent_id)
                    .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
                let queue = crate::sync_lock::rw_read(&**queue_lock);
                let t = queue
                    .current_task()
                    .cloned()
                    .ok_or(OrchestratorError::TaskNotFound(task_id))?;
                let cfg = crate::sync_lock::rw_read(&*self.config);
                (t, cfg.max_socrates_debug_iterations)
            };

            let trust_relax_min =
                crate::sync_lock::rw_read(&*self.config).trust_gate_relax_min_reliability;
            let trust_relax_gates = crate::sync_lock::rw_read(&*self.config)
                .trust_gate_relax_enabled
                && self
                    .lookup_agent_reliability_sync(agent_id)
                    .is_some_and(|r| r >= trust_relax_min);

            self.check_socrates_gate(
                task_id,
                agent_id,
                &task_clone,
                completion_attestation.as_ref(),
                max_socrates_iterations,
                trust_relax_gates,
            )
            .await?
        };

        if let Some((requeue_task, err_report, _err_n, _warn_n)) = socrates_outcome.requeue {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            let queue_lock = agents
                .get(&agent_id)
                .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
            let mut queue = crate::sync_lock::rw_write(&**queue_lock);
            tracing::warn!(
                "Task {} failed Socrates validation. Auto-debugging (iteration {})",
                task_id,
                requeue_task.debug_iterations
            );
            queue.mark_failed(
                task_id,
                format!("Auto-debug Socrates validation failure:\n{}", err_report),
            );
            self.record_task_loop_metric(
                task_id,
                &phase_label,
                "toestub_requeue",
                requeue_task.debug_iterations,
            );
            queue.enqueue(requeue_task);
            return Ok(());
        }

        let completion_data = {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            let queue_lock = agents
                .get(&agent_id)
                .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
            let mut queue = crate::sync_lock::rw_write(&**queue_lock);

            let current = queue
                .current_task()
                .cloned()
                .ok_or(OrchestratorError::TaskNotFound(task_id))?;
            let plan_meta = match (
                current.plan_session_id.clone(),
                current.plan_node_id.clone(),
                current.plan_version,
            ) {
                (Some(ps), Some(pn), Some(pv)) => Some(crate::planning::PlanningTaskMeta {
                    plan_session_id: ps,
                    plan_node_id: pn,
                    plan_version: pv,
                    execution_policy_json: current.execution_policy_json.clone(),
                    campaign_id: current.campaign_id.clone(),
                    benchmark_tier: current.benchmark_tier,
                    execution_role: current.execution_role,
                }),
                _ => None,
            };
            queue.mark_complete(task_id);
            let bandit_model_id = current
                .model_override
                .clone()
                .or_else(|| current.model_preference.clone());
            (
                write_files,
                current.session_id.clone(),
                current.description.clone(),
                phase_label,
                current.debug_iterations,
                plan_meta,
                current.campaign_id.clone(),
                current.benchmark_tier,
                current.audit_report.clone(),
                bandit_model_id,
                current.tenant_id.clone(),
            )
        };

        let (
            write_files,
            session_id,
            desc,
            phase_label,
            debug_iterations,
            plan_meta,
            campaign_id,
            benchmark_tier,
            audit_report,
            bandit_model_id,
            tenant_id,
        ) = completion_data;

        let (snap_before, db_snap_before) =
            crate::sync_lock::rw_read(&*self.oplog).find_task_snapshots(task_id.0);
        let snapshot_after = self
            .capture_snapshot(
                agent_id,
                &write_files,
                format!("post-task complete: {:.50}", desc),
            )
            .await;
        let db_snap_after = self
            .take_db_snapshot(agent_id, format!("post-task-complete: {}", task_id))
            .await;

        self.record_operation(
            agent_id,
            crate::oplog::OperationKind::TaskComplete { task_id: task_id.0 },
            format!("Completed task {}", task_id),
            snap_before,
            Some(snapshot_after),
            db_snap_before,
            db_snap_after,
        )
        .await;

        self.record_success_persistence(
            task_id,
            agent_id,
            session_id.clone(),
            &phase_label,
            &desc,
            &write_files,
            plan_meta,
            campaign_id,
            benchmark_tier,
            tenant_id.clone(),
        )
        .await;

        for path in &write_files {
            self.lock_manager.release(path, agent_id);
        }

        MessageGateway::publish_task_completed(
            &self.bulletin,
            &self.message_bus,
            &self.event_bus,
            task_id,
            agent_id,
            session_id,
            audit_report,
        );

        self.record_bandit_task_outcome(bandit_model_id.as_deref(), true);

        {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            for queue_lock in agents.values() {
                crate::sync_lock::rw_write(&**queue_lock).unblock(task_id);
            }
        }

        tracing::info!("Task {} completed by agent {}", task_id, agent_id);
        self.record_task_loop_metric(task_id, &phase_label, "completed", debug_iterations);

        if let Some(steps) = crate::sync_lock::rw_write(&*self.task_traces).get_mut(&task_id) {
            steps.push(crate::orchestrator::TaskTraceStep {
                timestamp_ms: crate::types::now_unix_ms(),
                stage: "outcome".to_string(),
                detail: Some("completed".to_string()),
            });
        }

        Ok(())
    }
}
