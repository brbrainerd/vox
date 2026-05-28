use crate::oplog::OperationKind;
use crate::orchestrator::{Orchestrator, OrchestratorError, TaskTraceStep};
use crate::planning::PlanningTaskMeta;
use crate::services::MessageGateway;
use crate::types::{AgentId, TaskId};
use vox_db::store::VictoryVerdict;

impl Orchestrator {
    /// Mark a task as failed (async).
    pub async fn fail_task(
        &self,
        task_id: TaskId,
        reason: String,
    ) -> Result<(), OrchestratorError> {
        self.fail_task_with_audit(task_id, reason, None).await
    }

    /// Mark a task as failed and attach an audit report.
    pub async fn fail_task_with_audit(
        &self,
        task_id: TaskId,
        reason: String,
        audit_report: Option<String>,
    ) -> Result<(), OrchestratorError> {
        let agent_id = crate::sync_lock::rw_read(&*self.task_assignments)
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;

        let (
            session_id,
            planning_meta,
            failed_desc,
            campaign_id,
            benchmark_tier,
            failed_write_paths,
            audit_report,
            bandit_model_id,
            tenant_id,
        ) = {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            let queue_lock = agents
                .get(&agent_id)
                .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
            let mut queue = crate::sync_lock::rw_write(&**queue_lock);

            if let Some(task) = queue.find_task_mut(task_id) {
                task.audit_report = audit_report.clone();
            } else if let Some(task) = queue.current_task_mut() {
                if task.id == task_id {
                    task.audit_report = audit_report.clone();
                }
            }

            let session_id = queue.current_task().and_then(|t| t.session_id.clone());
            let planning_meta = queue.current_task().and_then(|t| {
                if let (Some(plan_session_id), Some(plan_node_id), Some(plan_version)) = (
                    t.plan_session_id.clone(),
                    t.plan_node_id.clone(),
                    t.plan_version,
                ) {
                    Some(PlanningTaskMeta {
                        plan_session_id,
                        plan_node_id,
                        plan_version,
                        execution_policy_json: t.execution_policy_json.clone(),
                        campaign_id: t.campaign_id.clone(),
                        benchmark_tier: t.benchmark_tier,
                        execution_role: t.execution_role,
                    })
                } else {
                    None
                }
            });
            let failed_desc = queue
                .current_task()
                .map(|t| t.description.clone())
                .unwrap_or_default();
            let failed_write_paths = queue
                .current_task()
                .map(|t| {
                    t.write_files()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<std::path::PathBuf>>()
                })
                .unwrap_or_default();
            let campaign_id = queue.current_task().and_then(|t| t.campaign_id.clone());
            let benchmark_tier = queue.current_task().and_then(|t| t.benchmark_tier);
            let bandit_model_id = queue.current_task().and_then(|t| {
                t.model_override
                    .clone()
                    .or_else(|| t.model_preference.clone())
            });
            let tenant_id = queue.current_task().and_then(|t| t.tenant_id.clone());
            if failed_desc.contains("[PHASE:SHARD_VALIDATE]") {
                queue.recent_shard_validation_failures =
                    queue.recent_shard_validation_failures.saturating_add(1);
            }
            queue.mark_failed(task_id, reason.clone());
            (
                session_id,
                planning_meta,
                failed_desc,
                campaign_id,
                benchmark_tier,
                failed_write_paths,
                audit_report.clone(),
                bandit_model_id,
                tenant_id,
            )
        };

        if let Some(db) = self.db() {
            let reliability_agent_id = agent_id.0.to_string();
            let reliability_replay = serde_json::json!({
                "op": "record_task_reliability_observation",
                "agent_id": reliability_agent_id,
                "success": false,
            });
            self.persist_with_retry_meta(
                "trust/reliability_observation",
                Some(reliability_replay),
                || async {
                    db.record_task_reliability_observation(&agent_id.0.to_string(), false)
                        .await
                },
            )
            .await;
            let agent_id_s = agent_id.0.to_string();
            let domain = Self::extract_phase_label(&failed_desc);
            let artifact_ref = task_id.0.to_string();
            let repo = crate::lineage::repository_id();
            let trust_replay = serde_json::json!({
                "op": "record_trust_observation",
                "entity_type": "agent",
                "entity_id": agent_id_s.clone(),
                "dimension": "task_completion",
                "domain": domain.clone(),
                "task_class": domain.clone(),
                "provider": serde_json::Value::Null,
                "model_id": serde_json::Value::Null,
                "repository_id": repo.clone(),
                "source_kind": "task_failed",
                "observation_value": 0.0,
                "confidence_weight": 1.0,
                "sample_size": 1,
                "artifact_ref": artifact_ref.clone(),
                "metadata_json": serde_json::Value::Null,
                "ewma_alpha": 0.10,
            });
            self.persist_with_retry_meta("trust/observation", Some(trust_replay), || async {
                db.record_trust_observation(vox_db::TrustObservationInput {
                    entity_type: "agent",
                    entity_id: &agent_id_s,
                    dimension: "task_completion",
                    domain: Some(domain.as_str()),
                    task_class: Some(domain.as_str()),
                    provider: None,
                    model_id: None,
                    repository_id: Some(repo.as_str()),
                    source_kind: Some("task_failed"),
                    observation_value: 0.0,
                    confidence_weight: 1.0,
                    sample_size: 1,
                    artifact_ref: Some(artifact_ref.as_str()),
                    metadata_json: None,
                    ewma_alpha: 0.10,
                })
                .await
                .map(|_| ())
            })
            .await;
        }
        if let (Some(campaign_id), Some(tier)) = (campaign_id.clone(), benchmark_tier) {
            let reason_lower = reason.to_ascii_lowercase();
            let failures = Self::classify_verification_failures(&reason_lower);
            let evidence = crate::reconstruction::ReconstructionEvidence {
                compile_ok: !reason_lower.contains("compile"),
                targeted_tests_ok: !reason_lower.contains("test"),
                contract_checks_ok: !reason_lower.contains("contract"),
                docs_ssot_ok: !reason_lower.contains("doc"),
                regression_checks_ok: false,
                failures: failures.clone(),
            };
            self.record_reconstruction_campaign_result(
                campaign_id.clone(),
                tier,
                evidence,
                "heuristic_task_fail",
                false,
                session_id.as_deref(),
                Some(task_id),
                Some(agent_id),
            )
            .await;

            if let Some(primary_failure) = failures.first().copied() {
                let key = format!(
                    "{}repair_recommendation:{}",
                    crate::reconstruction::campaign_context_prefix(campaign_id.as_str()),
                    task_id.0
                );
                let payload = serde_json::json!({
                    "campaign_id": campaign_id,
                    "task_id": task_id.0,
                    "failure_kind": Self::failure_kind_tag(primary_failure),
                    "reason": reason,
                    "write_paths": failed_write_paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>(),
                });
                crate::sync_lock::rw_read(&*self.context_store).set(
                    AgentId(0),
                    key,
                    payload.to_string(),
                    0,
                );
            }
        }
        {
            let cfg = crate::sync_lock::rw_read(&*self.config).clone();
            crate::sync_lock::rw_write(&*self.budget_manager).record_trust_outcome(
                agent_id,
                false,
                cfg.trust_ewma_alpha,
                cfg.trust_provisional_threshold,
                cfg.trust_trusted_threshold,
            );
        }

        // Hardening MENS Intelligence Persistence (Task Failure)
        if let Some(db) = self.db() {
            let task_id_s = task_id.0.to_string();
            let verdict = VictoryVerdict {
                task_id: vox_db::DbTaskId::new(task_id_s.clone()),
                passed: false,
                tiers_run: vec!["Full".to_string()],
                first_failure: Some("TaskFailure".to_string()),
                report: reason.clone(),
                created_at_ms: crate::types::now_unix_ms() as i64,
            };
            let verdict_replay = serde_json::json!({
                "op": "insert_victory_verdict",
                "task_id": task_id_s.clone(),
                "verdict": verdict,
            });
            self.persist_with_retry_meta("mens/victory_verdict_fail", Some(verdict_replay), || {
                let db = db.clone();
                let tid = task_id_s.clone();
                let v = verdict.clone();
                async move { db.insert_victory_verdict(&tid, &v).await }
            })
            .await;
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if let Some(steps) = crate::sync_lock::rw_write(&*self.task_traces).get_mut(&task_id) {
            steps.push(TaskTraceStep {
                stage: "outcome".to_string(),
                timestamp_ms: now_ms,
                detail: Some(format!("failed: {}", reason)),
            });
        }

        // Release task-specific ownership first, then any residual locks.
        for path in &failed_write_paths {
            self.lock_manager.release(path, agent_id);
            self.affinity_map.release(path);
            crate::sync_lock::rw_write(&*self.scope_guard).revoke_file(agent_id, path);
        }
        self.lock_manager.release_all(agent_id);

        // Find pre-task snapshots to link this failure
        let (snap_before, db_snap_before) =
            crate::sync_lock::rw_read(&*self.oplog).find_task_snapshots(task_id.0);

        // Record failure in oplog (async to support DB snapshot)
        self.record_operation(
            agent_id,
            OperationKind::TaskFail {
                task_id: task_id.0,
                reason: reason.clone(),
            },
            format!("Failed task {}", task_id),
            snap_before,
            None,
            db_snap_before,
            None,
        )
        .await;

        MessageGateway::publish_task_failed(
            &self.bulletin,
            &self.event_bus,
            task_id,
            agent_id,
            reason.clone(),
            session_id.clone(),
            audit_report,
        );

        self.record_bandit_task_outcome(bandit_model_id.as_deref(), false);

        let planning_cfg = crate::sync_lock::rw_read(&*self.config).clone();
        if planning_cfg.planning_enabled
            && planning_cfg.planning_replan_enabled
            && let Some(ref meta) = planning_meta
            && crate::planning::replan::trigger_matches(
                &reason,
                meta.execution_policy_json.as_deref(),
            )
        {
            if let Some(db) = self.db() {
                if let Err(e) = db
                    .record_plan_node_attempt(
                        &meta.plan_session_id,
                        i64::from(meta.plan_version),
                        &meta.plan_node_id,
                        1,
                        Some(&task_id.0.to_string()),
                        "failed",
                        Some(&reason),
                        None,
                    )
                    .await
                {
                    self.record_persistence_degradation("plan/replan_node_attempt", &e.to_string());
                }
                let next_version = i64::from(meta.plan_version) + 1;
                if let Err(e) = db
                    .append_plan_version(
                        &meta.plan_session_id,
                        next_version,
                        Some(i64::from(meta.plan_version)),
                        Some("task_failed"),
                        Some(
                            &serde_json::json!({
                                "task_id": task_id.0,
                                "reason": reason,
                            })
                            .to_string(),
                        ),
                    )
                    .await
                {
                    self.record_persistence_degradation_with_meta(
                        "plan/replan_version",
                        &e.to_string(),
                        Some(serde_json::json!({
                            "op": "append_plan_version",
                            "plan_session_id": meta.plan_session_id.clone(),
                            "version": next_version,
                            "parent_version": i64::from(meta.plan_version),
                            "origin": "task_failed",
                            "metadata_json": serde_json::json!({
                                "task_id": task_id.0,
                                "reason": reason,
                            }).to_string(),
                        })),
                    );
                }
                self.event_bus
                    .emit(crate::events::AgentEventKind::PlanVersionCreated {
                        plan_session_id: meta.plan_session_id.clone(),
                        version: next_version,
                        parent_version: Some(i64::from(meta.plan_version)),
                    });
                self.event_bus
                    .emit(crate::events::AgentEventKind::ReplanTriggered {
                        plan_session_id: meta.plan_session_id.clone(),
                        node_id: meta.plan_node_id.clone(),
                        reason: reason.clone(),
                        next_version,
                    });
                if crate::lineage::orchestration_lineage_persist_enabled() {
                    let repo = crate::lineage::repository_id();
                    let mut payload = serde_json::json!({
                        "failed_task_id": task_id.0,
                        "next_version": next_version,
                        "reason_preview": reason.chars().take(500).collect::<String>(),
                    });
                    if let Some(cid) = crate::lineage::orchestration_campaign_id() {
                        payload["campaign_id"] = serde_json::Value::String(cid);
                    }
                    let payload_str = payload.to_string();
                    let replay_meta = serde_json::json!({
                        "op": "append_orchestration_lineage_event",
                        "repository_id": repo,
                        "kind": "plan_replan_triggered",
                        "task_id": task_id.0 as i64,
                        "agent_id": agent_id.0 as i64,
                        "session_id": session_id.clone(),
                        "workflow_id": serde_json::Value::Null,
                        "plan_session_id": meta.plan_session_id.clone(),
                        "plan_node_id": meta.plan_node_id.clone(),
                        "payload_json": payload_str,
                    });
                    self.persist_with_retry_meta(
                        "lineage/plan_replan_triggered",
                        Some(replay_meta),
                        || async {
                            db.append_orchestration_lineage_event(
                                &repo,
                                "plan_replan_triggered",
                                task_id.0 as i64,
                                Some(agent_id.0 as i64),
                                session_id.as_deref(),
                                None,
                                Some(meta.plan_session_id.as_str()),
                                Some(meta.plan_node_id.as_str()),
                                Some(payload_str.as_str()),
                            )
                            .await
                        },
                    )
                    .await;
                }
            }
            if let Err(e) = crate::planning::replan::enqueue_recovery_first_node(
                self,
                meta,
                &reason,
                &failed_desc,
                session_id.clone(),
                tenant_id,
            )
            .await
            {
                self.record_persistence_degradation(
                    "plan/replan_recovery_enqueue",
                    &e.to_string(),
                );
            }
        }

        if crate::lineage::orchestration_lineage_persist_enabled() {
            if let Some(db) = self.db() {
                let repo = crate::lineage::repository_id();
                let (ps, pn) = planning_meta
                    .as_ref()
                    .map(|m| {
                        (
                            Some(m.plan_session_id.as_str()),
                            Some(m.plan_node_id.as_str()),
                        )
                    })
                    .unwrap_or((None, None));
                let preview: String = reason.chars().take(500).collect();
                let payload = serde_json::json!({ "reason_preview": preview });
                let payload_str = payload.to_string();
                let replay_meta = serde_json::json!({
                    "op": "append_orchestration_lineage_event",
                    "repository_id": repo,
                    "kind": "task_failed",
                    "task_id": task_id.0 as i64,
                    "agent_id": agent_id.0 as i64,
                    "session_id": session_id.clone(),
                    "workflow_id": serde_json::Value::Null,
                    "plan_session_id": ps,
                    "plan_node_id": pn,
                    "payload_json": payload_str,
                });
                self.persist_with_retry_meta("lineage/task_failed", Some(replay_meta), || async {
                    db.append_orchestration_lineage_event(
                        &repo,
                        "task_failed",
                        task_id.0 as i64,
                        Some(agent_id.0 as i64),
                        session_id.as_deref(),
                        None,
                        ps,
                        pn,
                        Some(payload_str.as_str()),
                    )
                    .await
                })
                .await;
            }
        }

        tracing::warn!("Task {} failed: {}", task_id, reason);
        self.record_task_loop_metric(task_id, "unknown", "failed", 0);
        Ok(())
    }
}
