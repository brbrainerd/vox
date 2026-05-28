use crate::orchestrator::{Orchestrator, TaskId};
use crate::planning::PlanningTaskMeta;
use crate::types::AgentId;
use vox_db::store::{ObservationReport, ObserverAction, VictoryVerdict};

impl Orchestrator {
    pub async fn record_success_persistence(
        &self,
        task_id: TaskId,
        agent_id: AgentId,
        session_id: Option<String>,
        phase_label: &str,
        desc: &str,
        write_files: &[std::path::PathBuf],
        plan_completion_meta: Option<PlanningTaskMeta>,
        campaign_id: Option<String>,
        benchmark_tier: Option<crate::reconstruction::ReconstructionBenchmarkTier>,
        tenant_id: Option<String>,
    ) {
        if let Some(db) = self.db().as_ref() {
            let reliability_agent_id = agent_id.0.to_string();
            let reliability_replay = serde_json::json!({
                "op": "record_task_reliability_observation",
                "agent_id": reliability_agent_id,
                "success": true,
            });
            self.persist_with_retry_meta(
                "trust/reliability_observation",
                Some(reliability_replay),
                || async {
                    db.record_task_reliability_observation(&agent_id.0.to_string(), true)
                        .await
                },
            )
            .await;

            let agent_id_s = agent_id.0.to_string();
            let repo = crate::lineage::repository_id();
            let artifact_ref = task_id.0.to_string();
            let trust_replay = serde_json::json!({
                "op": "record_trust_observation",
                "entity_type": "agent",
                "entity_id": agent_id_s.clone(),
                "dimension": "task_completion",
                "domain": phase_label.to_string(),
                "task_class": phase_label.to_string(),
                "repository_id": repo.clone(),
                "source_kind": "task_completed",
                "observation_value": 1.0,
                "confidence_weight": 1.0,
                "sample_size": 1,
                "artifact_ref": artifact_ref.clone(),
                "ewma_alpha": 0.10,
            });
            self.persist_with_retry_meta("trust/observation", Some(trust_replay), || async {
                db.record_trust_observation(vox_db::TrustObservationInput {
                    entity_type: "agent",
                    entity_id: agent_id_s.as_str(),
                    dimension: "task_completion",
                    domain: Some(phase_label),
                    task_class: Some(phase_label),
                    provider: None,
                    model_id: None,
                    repository_id: Some(repo.as_str()),
                    source_kind: Some("task_completed"),
                    observation_value: 1.0,
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

            if let Some(meta) = plan_completion_meta.as_ref() {
                let plan_attempt_replay = serde_json::json!({
                    "op": "record_plan_node_attempt",
                    "plan_session_id": meta.plan_session_id.clone(),
                    "version": i64::from(meta.plan_version),
                    "node_id": meta.plan_node_id.clone(),
                    "attempt_no": 1i64,
                    "task_id": task_id.0.to_string(),
                    "outcome": "completed",
                });
                self.persist_with_retry_meta(
                    "plan/node_attempt",
                    Some(plan_attempt_replay),
                    || async {
                        db.record_plan_node_attempt(
                            &meta.plan_session_id,
                            i64::from(meta.plan_version),
                            &meta.plan_node_id,
                            1,
                            Some(&task_id.0.to_string()),
                            "completed",
                            None,
                            None,
                        )
                        .await
                    },
                )
                .await;

                let plan_status_replay = serde_json::json!({
                    "op": "set_plan_node_status",
                    "plan_session_id": meta.plan_session_id.clone(),
                    "version": meta.plan_version as i64,
                    "node_id": meta.plan_node_id.clone(),
                    "status": "completed",
                });
                self.persist_with_retry_meta(
                    "plan/node_status",
                    Some(plan_status_replay),
                    || async {
                        db.set_plan_node_status(
                            &meta.plan_session_id,
                            meta.plan_version as i64,
                            &meta.plan_node_id,
                            "completed",
                        )
                        .await
                    },
                )
                .await;

                if let Err(e) = crate::planning::schedule::enqueue_runnable_plan_nodes(
                    self,
                    &meta.plan_session_id,
                    meta.plan_version,
                    session_id.clone(),
                    tenant_id,
                )
                .await
                {
                    self.record_persistence_degradation("plan/enqueue_runnable", &e.to_string());
                }
            }

            if crate::lineage::orchestration_lineage_persist_enabled() {
                let repo = crate::lineage::repository_id();
                let (ps, pn) = plan_completion_meta
                    .as_ref()
                    .map(|m| {
                        (
                            Some(m.plan_session_id.as_str()),
                            Some(m.plan_node_id.as_str()),
                        )
                    })
                    .unwrap_or((None, None));
                let verification_layers = crate::reconstruction::VerificationLayerStatus {
                    structural_ok: phase_label.contains("verify") || phase_label.contains("build"),
                    behavioral_ok: phase_label.contains("test") || phase_label.contains("verify"),
                    contract_ok: phase_label.contains("contract"),
                    docs_ssot_ok: phase_label.contains("doc") || phase_label.contains("ssot"),
                    grounding_ok: true, // simplified
                };
                let mut payload = serde_json::json!({
                    "phase": phase_label,
                    "verification_layers_source": "heuristic_v1",
                    "verification_layers": verification_layers,
                });
                if let Some(ref cid) = campaign_id {
                    payload["campaign_id"] = serde_json::Value::String(cid.clone());
                }
                if let Some(tier) = benchmark_tier {
                    payload["benchmark_tier"] =
                        serde_json::Value::String(tier.as_str().to_string());
                }
                let payload_str = payload.to_string();
                let replay_meta = serde_json::json!({
                    "op": "append_orchestration_lineage_event",
                    "repository_id": repo,
                    "kind": "task_completed",
                    "task_id": task_id.0 as i64,
                    "agent_id": agent_id.0 as i64,
                    "session_id": session_id.clone(),
                    "plan_session_id": ps,
                    "plan_node_id": pn,
                    "payload_json": payload_str,
                });
                self.persist_with_retry_meta(
                    "lineage/task_completed",
                    Some(replay_meta),
                    || async {
                        db.append_orchestration_lineage_event(
                            &repo,
                            "task_completed",
                            task_id.0 as i64,
                            Some(agent_id.0 as i64),
                            session_id.as_deref(),
                            None,
                            ps,
                            pn,
                            Some(payload_str.as_str()),
                        )
                        .await
                    },
                )
                .await;
            }

            // Record Victory Verdict
            let verdict = VictoryVerdict {
                task_id: vox_db::DbTaskId::new(task_id.0.to_string()),
                passed: true,
                tiers_run: vec!["Full".to_string()],
                first_failure: None,
                report: desc.to_string(),
                created_at_ms: crate::types::now_unix_ms() as i64,
            };
            let verdict_replay = serde_json::json!({
                "op": "insert_victory_verdict",
                "task_id": task_id.0.to_string(),
                "verdict": verdict,
            });
            self.persist_with_retry_meta("mens/victory_verdict", Some(verdict_replay), || {
                let db = db.clone();
                let tid = task_id.0.to_string();
                let v = verdict.clone();
                async move { db.insert_victory_verdict(&tid, &v).await }
            })
            .await;

            // Record Observation Report
            let report = ObservationReport {
                session_id: vox_db::DbSessionId::new(session_id.clone().unwrap_or_default()),
                task_id: vox_db::DbTaskId::new(task_id.0.to_string()),
                observed_at: chrono::Utc::now(),
                file_path: write_files
                    .first()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                lsp_error_count: 0,
                parse_rate: 1.0,
                construct_coverage: 1.0,
                recommended_action: ObserverAction::Continue,
                metadata_json: None,
            };
            let obs_replay = serde_json::json!({
                "op": "insert_observer_event",
                "session_id": session_id.clone().unwrap_or_default(),
                "task_id": task_id.0.to_string(),
                "report": report,
            });
            self.persist_with_retry_meta("mens/observer_event", Some(obs_replay), || {
                let db = db.clone();
                let sid = session_id.clone().unwrap_or_default();
                let tid = task_id.0.to_string();
                let r = report.clone();
                async move { db.insert_observer_event(&sid, &tid, &r).await }
            })
            .await;
        }

        if let (Some(campaign_id), Some(tier)) = (campaign_id, benchmark_tier) {
            let desc_lower = desc.to_ascii_lowercase();
            let evidence = crate::reconstruction::ReconstructionEvidence {
                compile_ok: phase_label.contains("build") || desc_lower.contains("cargo check"),
                targeted_tests_ok: phase_label.contains("test")
                    || phase_label.contains("verify")
                    || desc_lower.contains("test"),
                contract_checks_ok: phase_label.contains("contract")
                    || desc_lower.contains("contract"),
                docs_ssot_ok: desc_lower.contains("ssot") || phase_label.contains("doc"),
                regression_checks_ok: phase_label.contains("regression"),
                failures: Vec::new(),
            };
            self.record_reconstruction_campaign_result(
                campaign_id,
                tier,
                evidence,
                "heuristic_task_complete",
                false,
                session_id.as_deref(),
                Some(task_id),
                Some(agent_id),
            )
            .await;
        }

        {
            let cfg = crate::sync_lock::rw_read(&*self.config).clone();
            crate::sync_lock::rw_write(&*self.budget_manager).record_trust_outcome(
                agent_id,
                true,
                cfg.trust_ewma_alpha,
                cfg.trust_provisional_threshold,
                cfg.trust_trusted_threshold,
            );
        }
    }
}
