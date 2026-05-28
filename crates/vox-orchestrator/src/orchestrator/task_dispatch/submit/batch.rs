use crate::locks::LockKind;
use crate::oplog::OperationKind;
use crate::scope::ScopeEnforcement;
use crate::services::{
    PolicyCheckResult, PolicyEngine, PolicyTrustRelax, RouteResult, RoutingService,
};
use crate::types::{AccessKind, AgentId, AgentTask, FileAffinity, TaskId};
use std::collections::HashMap;
use std::path::PathBuf;

use super::super::super::{MAX_TASK_TRACES, Orchestrator, OrchestratorError, TaskTraceStep};
#[cfg(feature = "runtime")]
use super::AGENT_NOTIFY_TIMEOUT;
use super::attention_fields::{populate_task_attention_fields, submission_approval_block_reason};

impl Orchestrator {
    /// Submit a batch of interdependent tasks (async).
    pub async fn submit_batch(
        &self,
        descriptors: Vec<crate::types::TaskDescriptor>,
    ) -> Result<Vec<TaskId>, OrchestratorError> {
        let (enabled, default_priority, scope_enforcement) = {
            let config = crate::sync_lock::rw_read(&*self.config);
            (
                config.enabled,
                config.default_priority,
                config.scope_enforcement,
            )
        };
        if !enabled {
            return Err(OrchestratorError::Disabled);
        }

        let mut assigned_ids: Vec<TaskId> = Vec::with_capacity(descriptors.len());

        // Pre-allocate task IDs
        for _ in 0..descriptors.len() {
            assigned_ids.push(self.task_id_gen.next());
        }

        let mut results = Vec::new();

        // Second pass: construct tasks with resolved IDs and submit
        for (i, mut desc) in descriptors.into_iter().enumerate() {
            let my_id = assigned_ids[i];

            // Resolve temporary deps into actual TaskIds
            for tmp_dep_idx in desc.temp_deps {
                if tmp_dep_idx < assigned_ids.len() {
                    desc.depends_on.push(assigned_ids[tmp_dep_idx]);
                } else {
                    return Err(OrchestratorError::ScopeDenied(format!(
                        "Task descriptor {} referenced out-of-bounds temp dep {}",
                        i, tmp_dep_idx
                    )));
                }
            }

            let priority = desc.priority.unwrap_or(default_priority);
            let mut task = AgentTask::new(
                my_id,
                desc.description.clone(),
                priority,
                desc.file_manifest.clone(),
            );
            task.capability_requirements = desc.capability_requirements.clone();
            task.session_id = desc.session_id.clone();
            task.tenant_id = desc.tenant_id.clone();
            if desc.requires_approval {
                task.status = crate::types::TaskStatus::BlockedOnApproval;
            }
            task.start(); // ensure started_at_ms is populated

            // Add all collected deps
            for dep in desc.depends_on {
                task = task.depends_on(dep);
            }

            // Route to best agent via RoutingService
            let agent_id = self
                .resolve_route(
                    &desc.file_manifest,
                    None,
                    desc.capability_requirements.as_ref(),
                    Some(desc.description.as_str()),
                    Some(my_id),
                )
                .await?;
            if !crate::sync_lock::rw_read(&*self.agents).contains_key(&agent_id) {
                return Err(OrchestratorError::AgentNotFound(agent_id));
            }

            populate_task_attention_fields(self, &mut task, agent_id, &desc.file_manifest);
            if let Some(reason) = submission_approval_block_reason(&task) {
                return Err(OrchestratorError::ApprovalBlocked(reason));
            }

            let policy_trust = {
                let cfg = crate::sync_lock::rw_read(&*self.config);
                let mut t = PolicyTrustRelax::default();
                if cfg.trust_gate_relax_enabled {
                    t.relax_scope_strict_on_high_reliability = true;
                    t.min_reliability = cfg.trust_gate_relax_min_reliability;
                    t.agent_reliability = self.lookup_agent_reliability_sync(agent_id);
                }
                t
            };

            {
                let scope_guard_lock = (scope_enforcement != ScopeEnforcement::Disabled)
                    .then_some(crate::sync_lock::rw_read(&*self.scope_guard));
                let scope_guard_ref = scope_guard_lock.as_deref();
                match PolicyEngine::check_before_queue(
                    &self.lock_manager,
                    scope_guard_ref,
                    &self.event_bus,
                    &desc.file_manifest,
                    agent_id,
                    policy_trust,
                ) {
                    PolicyCheckResult::Allowed => {}
                    PolicyCheckResult::LockConflict(e) => {
                        return Err(OrchestratorError::LockConflict(e));
                    }
                    PolicyCheckResult::ScopeDenied(msg) => {
                        return Err(OrchestratorError::ScopeDenied(msg));
                    }
                }
            }

            // Acquire locks and assign scope (after releasing scope read guard; see task_submit)
            for fa in &desc.file_manifest {
                if fa.access == AccessKind::Write {
                    let _ = self
                        .lock_manager
                        .try_acquire(&fa.path, agent_id, LockKind::Exclusive);
                    self.affinity_map.assign(&fa.path, agent_id);
                    crate::sync_lock::rw_write(&*self.scope_guard)
                        .assign_file(agent_id, fa.path.clone());
                }
            }

            // Capture pre-task snapshot for version control
            let snapshot_before = {
                let paths: Vec<PathBuf> =
                    desc.file_manifest.iter().map(|f| f.path.clone()).collect();
                self.capture_snapshot(
                    agent_id,
                    &paths,
                    format!("pre-task batch: {:.50}", task.description),
                )
                .await
            };

            self.record_operation(
                agent_id,
                OperationKind::TaskSubmit { task_id: my_id.0 },
                format!("Submitted batch task {}", my_id),
                Some(snapshot_before),
                None,
                None,
                None,
            )
            .await;

            self.record_activity();
            crate::sync_lock::rw_write(&self.monitor).record_progress(agent_id);
            let session_id_for_retrieval = task.session_id.clone();
            let intake_description = task.description.clone();
            let cleanup_claims = || {
                for fa in &desc.file_manifest {
                    if fa.access == AccessKind::Write {
                        self.lock_manager.release(&fa.path, agent_id);
                        self.affinity_map.release(&fa.path);
                        crate::sync_lock::rw_write(&*self.scope_guard)
                            .revoke_file(agent_id, &fa.path);
                    }
                }
            };
            // Enqueue
            let handle_to_notify = {
                let agents = crate::sync_lock::rw_read(&*self.agents);
                if let Some(queue_lock) = agents.get(&agent_id) {
                    let mut queue = crate::sync_lock::rw_write(&**queue_lock);
                    self.event_bus
                        .emit(crate::events::AgentEventKind::TaskSubmitted {
                            task_id: my_id,
                            agent_id,
                            description: task.description.clone(),
                            session_id: task.session_id.clone(),
                        });
                    queue.enqueue(task);
                    crate::sync_lock::rw_write(&*self.task_assignments).insert(my_id, agent_id);

                    // Grab the handle for notification outside the agents lock
                    #[cfg(feature = "runtime")]
                    {
                        crate::sync_lock::rw_read(&*self.agent_handles)
                            .get(&agent_id)
                            .cloned()
                    }
                    #[cfg(not(feature = "runtime"))]
                    {
                        None::<()>
                    }
                } else {
                    cleanup_claims();
                    return Err(OrchestratorError::AgentNotFound(agent_id));
                }
            };

            // Notify outside all locks
            #[cfg(feature = "runtime")]
            if let Some(handle) = handle_to_notify {
                let json = serde_json::to_string(&crate::runtime::AgentCommand::ProcessQueue)
                    .unwrap_or_else(|e| {
                        tracing::warn!("serialize ProcessQueue: {e}");
                        "{}".to_string()
                    });
                let env = vox_actor_runtime::mailbox::Envelope::Message(
                    vox_actor_runtime::mailbox::Message {
                        from: vox_actor_runtime::Pid::new(),
                        payload: vox_actor_runtime::mailbox::MessagePayload::Json(json.into()),
                    },
                );
                let handle: &vox_actor_runtime::process::ProcessHandle = &handle;
                match tokio::time::timeout(AGENT_NOTIFY_TIMEOUT, handle.send(env)).await {
                    Ok(send_res) => {
                        if let Err(e) = send_res {
                            tracing::warn!("submit_batch: agent notify send failed: {e:?}");
                        }
                    }
                    Err(_) => tracing::warn!(
                        "submit_batch: agent notify timed out after {:?}",
                        AGENT_NOTIFY_TIMEOUT
                    ),
                }
            }

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            {
                let mut traces = crate::sync_lock::rw_write(&*self.task_traces);
                if traces.len() >= MAX_TASK_TRACES {
                    if let Some(min_id) = traces.keys().min().copied() {
                        traces.remove(&min_id);
                    }
                }
                traces.insert(
                    my_id,
                    vec![
                        TaskTraceStep {
                            stage: "ingress".to_string(),
                            timestamp_ms: now_ms,
                            detail: None,
                        },
                        TaskTraceStep {
                            stage: "routed".to_string(),
                            timestamp_ms: now_ms,
                            detail: Some(format!("agent {}", agent_id)),
                        },
                    ],
                );
            }

            let attached_retrieval =
                self.attach_session_retrieval_envelope_if_present(my_id, &session_id_for_retrieval);
            if !attached_retrieval {
                self.attach_goal_search_context_with_retrieval(
                    my_id,
                    &intake_description,
                    &desc.file_manifest,
                )
                .await;
            }

            results.push(my_id);
        }

        tracing::info!("Submitted batch of {} tasks", results.len());
        Ok(results)
    }

    /// Submit a standard 4-phase coding DAG:
    /// plan -> draft -> critique -> repair.
    ///
    /// This uses existing dependency primitives (`temp_deps`) so each phase is only
    /// eligible once the previous phase has completed.
    pub async fn submit_codegen_phase_dag(
        &self,
        objective: impl Into<String>,
        file_manifest: Vec<FileAffinity>,
        priority: Option<crate::types::TaskPriority>,
        session_id: Option<String>,
    ) -> Result<Vec<TaskId>, OrchestratorError> {
        let objective = objective.into();
        let descriptors = vec![
            crate::types::TaskDescriptor {
                description: format!(
                    "[PHASE:PLAN]\nCreate a concise implementation plan for:\n{}",
                    objective
                ),
                priority,
                file_manifest: file_manifest.clone(),
                depends_on: vec![],
                temp_deps: vec![],
                capability_requirements: None,
                session_id: session_id.clone(),
                thread_id: None,
                requires_approval: false,
                test_decision: None,
                tenant_id: None,
            },
            crate::types::TaskDescriptor {
                description: format!(
                    "[PHASE:DRAFT]\nProduce an implementation draft for:\n{}",
                    objective
                ),
                priority,
                file_manifest: file_manifest.clone(),
                depends_on: vec![],
                temp_deps: vec![0],
                capability_requirements: None,
                session_id: session_id.clone(),
                thread_id: None,
                requires_approval: false,
                test_decision: None,
                tenant_id: None,
            },
            crate::types::TaskDescriptor {
                description: format!(
                    "[PHASE:CRITIQUE]\nReview the draft for defects, regressions, and missing tests:\n{}",
                    objective
                ),
                priority,
                file_manifest: file_manifest.clone(),
                depends_on: vec![],
                temp_deps: vec![1],
                capability_requirements: None,
                session_id: session_id.clone(),
                thread_id: None,
                requires_approval: false,
                test_decision: None,
                tenant_id: None,
            },
            crate::types::TaskDescriptor {
                description: format!(
                    "[PHASE:REPAIR]\nApply fixes from critique and verify the result:\n{}",
                    objective
                ),
                priority,
                file_manifest,
                depends_on: vec![],
                temp_deps: vec![2],
                capability_requirements: None,
                session_id,
                thread_id: None,
                requires_approval: false,
                test_decision: None,
                tenant_id: None,
            },
        ];
        self.submit_batch(descriptors).await
    }

    /// Submit a repository-scale shard workflow:
    /// shard generation -> shard validation -> reducer merge.
    ///
    /// Returns all task ids in descriptor order:
    /// - first N ids are shard generation tasks,
    /// - next N ids are validation tasks,
    /// - final id is the reducer merge task.
    pub async fn submit_repo_shard_dag(
        &self,
        objective: impl Into<String>,
        shard_manifests: Vec<Vec<FileAffinity>>,
        merge_manifest: Vec<FileAffinity>,
        priority: Option<crate::types::TaskPriority>,
        session_id: Option<String>,
    ) -> Result<Vec<TaskId>, OrchestratorError> {
        let objective = objective.into();
        let descriptors = build_repo_shard_descriptors(
            &objective,
            shard_manifests,
            merge_manifest,
            priority,
            session_id,
        );
        self.submit_batch(descriptors).await
    }

    /// Resolve route via RoutingService and spawn if needed.
    pub(crate) async fn resolve_route(
        &self,
        manifest: &[FileAffinity],
        target_agent: Option<&str>,
        task_capability_requirements: Option<&crate::contract::TaskCapabilityHints>,
        task_description: Option<&str>,
        source_task_id: Option<TaskId>,
    ) -> Result<AgentId, OrchestratorError> {
        if let Some(agent_name) = target_agent {
            // First check if an agent with this name exists
            let agents = crate::sync_lock::rw_read(&*self.agents);
            for (id, queue_lock) in agents.iter() {
                if crate::sync_lock::rw_read(&**queue_lock).name == agent_name {
                    return Ok(*id);
                }
            }
            drop(agents);
            // Otherwise, spawn an agent with this name
            return self.spawn_agent(agent_name);
        }

        let reputation_routing =
            crate::sync_lock::rw_read(&*self.config).socrates_reputation_routing;
        let task_domain = task_description
            .map(extract_phase_domain)
            .unwrap_or_else(|| "single_shot".to_string());
        let reliability_map: Option<HashMap<AgentId, f64>> = if reputation_routing {
            self.db().map(|db| {
                db.block_on(async { db.list_agent_reliability().await })
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(id, r): (String, f64)| {
                        let numeric_id = id.parse::<u64>().unwrap_or(0);
                        (AgentId(numeric_id), r)
                    })
                    .collect()
            })
        } else {
            None
        };
        let task_completion_trust_scores: Option<HashMap<AgentId, f64>> = if reputation_routing {
            self.db().map(|db| {
                db.block_on(async {
                    db.list_trust_scores_for_dimension(
                        "agent",
                        "task_completion",
                        Some(task_domain.as_str()),
                        2048,
                    )
                    .await
                })
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(id, score)| id.parse::<u64>().ok().map(|aid| (AgentId(aid), score)))
                .collect()
            })
        } else {
            None
        };

        let mut effective_requirements = task_capability_requirements.cloned().unwrap_or_default();
        if let Some(desc) = task_description {
            if desc.contains("[[category:visus]]") || desc.contains("[[visus]]") {
                effective_requirements.visus_eligible = true;
                effective_requirements.multi_modal = true;
            }
        }

        let attention_trust_scores = if crate::sync_lock::rw_read(&*self.config).attention_enabled {
            Some(crate::sync_lock::rw_read(&*self.budget_manager).trust_snapshot())
        } else {
            None
        };

        let remote_hints = crate::sync_lock::rw_read(&*self.remote_populi_routing_hints);
        let remote = if remote_hints.is_empty() {
            None
        } else {
            Some(remote_hints.as_slice())
        };

        let result = {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            let groups = crate::sync_lock::rw_read(&*self.groups);
            let config = crate::sync_lock::rw_read(&*self.config);
            let budget = crate::sync_lock::rw_read(&*self.budget_manager);
            let local_tokens = budget.local_inference_tokens();

            RoutingService::route(
                manifest,
                &self.affinity_map,
                &groups,
                &agents,
                &config,
                local_tokens,
                reliability_map.as_ref(),
                Some(&effective_requirements),
                task_description,
                remote,
                task_completion_trust_scores.as_ref(),
                attention_trust_scores.as_ref(),
            )
        };
        drop(remote_hints);

        match result {
            RouteResult::Existing(id) => Ok(id),
            RouteResult::SpawnAgent(name) => self.spawn_dynamic_agent_with_parent(
                &name,
                None,
                Some("route_spawn"),
                source_task_id,
                Some(effective_requirements),
            ),
        }
    }
}

fn extract_phase_domain(desc: &str) -> String {
    const PREFIX: &str = "[PHASE:";
    if let Some(start) = desc.find(PREFIX) {
        let suffix = &desc[start + PREFIX.len()..];
        if let Some(end) = suffix.find(']') {
            return suffix[..end].trim().to_ascii_lowercase();
        }
    }
    "single_shot".to_string()
}

fn build_repo_shard_descriptors(
    objective: &str,
    shard_manifests: Vec<Vec<FileAffinity>>,
    merge_manifest: Vec<FileAffinity>,
    priority: Option<crate::types::TaskPriority>,
    session_id: Option<String>,
) -> Vec<crate::types::TaskDescriptor> {
    let shard_count = shard_manifests.len();
    let mut descriptors = Vec::with_capacity(shard_count.saturating_mul(2).saturating_add(1));
    let schedule = crate::services::CampaignScheduler::plan(objective, shard_count);
    let mode_tag = schedule.mode.as_tag();
    let mut gen_descriptor_index_by_shard = std::collections::HashMap::<usize, usize>::new();

    // Generation tasks (indices 0..N-1)
    for shard_idx in &schedule.shard_order {
        let file_manifest = &shard_manifests[*shard_idx];
        let descriptor_idx = descriptors.len();
        descriptors.push(crate::types::TaskDescriptor {
            description: format!(
                "[PHASE:SHARD_GEN][MODE:{mode_tag}][SHARD:{}]\nGenerate repository shard implementation for:\n{}",
                shard_idx, objective
            ),
            priority,
            file_manifest: file_manifest.clone(),
            depends_on: vec![],
            temp_deps: vec![],
            capability_requirements: None,
            session_id: session_id.clone(),
            thread_id: None,
                requires_approval: false,
                test_decision: None,
                tenant_id: None,
        });
        gen_descriptor_index_by_shard.insert(*shard_idx, descriptor_idx);
    }

    // Validation tasks (indices N..2N-1), each depends on its corresponding generator.
    let mut validation_temp_deps = Vec::with_capacity(shard_count);
    for shard_idx in &schedule.shard_order {
        let file_manifest = &shard_manifests[*shard_idx];
        let gen_dep = *gen_descriptor_index_by_shard.get(shard_idx).unwrap_or(&0);
        descriptors.push(crate::types::TaskDescriptor {
            description: format!(
                "[PHASE:SHARD_VALIDATE][MODE:{mode_tag}][SHARD:{}]\nValidate generated shard for canonical output and parseability:\n{}",
                shard_idx, objective
            ),
            priority,
            file_manifest: file_manifest.clone(),
            depends_on: vec![],
            temp_deps: vec![gen_dep],
            capability_requirements: None,
            session_id: session_id.clone(),
            thread_id: None,
                requires_approval: false,
                test_decision: None,
                tenant_id: None,
        });
        validation_temp_deps.push(descriptors.len().saturating_sub(1));
    }

    // Reducer task depends on *all* validation tasks (validation-first merge gate).
    descriptors.push(crate::types::TaskDescriptor {
        description: format!(
            "[PHASE:REDUCE][MODE:{mode_tag}]\nMerge validated shard outputs into repository-scale result for:\n{}",
            objective
        ),
        priority,
        file_manifest: merge_manifest,
        depends_on: vec![],
        temp_deps: validation_temp_deps,
        capability_requirements: None,
        session_id,
        thread_id: None,
                requires_approval: false,
                test_decision: None,
                tenant_id: None,
    });

    descriptors
}

#[cfg(test)]
mod tests {
    use super::build_repo_shard_descriptors;
    use crate::types::{FileAffinity, TaskPriority};

    #[test]
    fn repo_shard_descriptors_enforce_validation_before_reduce() {
        let shard_manifests = vec![
            vec![FileAffinity::write("src/a.vox")],
            vec![FileAffinity::write("src/b.vox")],
        ];
        let merge_manifest = vec![FileAffinity::write("src/merged.vox")];
        let descriptors = build_repo_shard_descriptors(
            "Implement feature",
            shard_manifests,
            merge_manifest,
            Some(TaskPriority::Normal),
            Some("session-1".to_string()),
        );

        // 2 gen + 2 validate + 1 reducer
        assert_eq!(descriptors.len(), 5);
        assert_eq!(descriptors[0].temp_deps, Vec::<usize>::new());
        assert_eq!(descriptors[1].temp_deps, Vec::<usize>::new());
        assert_eq!(descriptors[2].temp_deps, vec![0]);
        assert_eq!(descriptors[3].temp_deps, vec![1]);
        assert_eq!(descriptors[4].temp_deps, vec![2, 3]);
        assert!(
            descriptors[4].description.contains("[PHASE:REDUCE]"),
            "last descriptor must be reducer"
        );
    }
}
