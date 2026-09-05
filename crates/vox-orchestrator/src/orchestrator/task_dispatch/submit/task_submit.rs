use crate::locks::LockKind;
use crate::oplog::OperationKind;
use crate::planning::PlanningTaskMeta;
use crate::scope::ScopeEnforcement;
use crate::services::persistence_obs::log_persistence_failure;
use crate::services::{PolicyCheckResult, PolicyEngine, PolicyTrustRelax};
use crate::types::{AccessKind, AgentTask, FileAffinity, TaskEnqueueHints, TaskId, TaskPriority};
use std::path::PathBuf;

use super::super::super::{MAX_TASK_TRACES, Orchestrator, OrchestratorError, TaskTraceStep};
#[cfg(feature = "runtime")]
use super::AGENT_NOTIFY_TIMEOUT;
use super::attention_fields::{populate_task_attention_fields, submission_approval_block_reason};

/// T5.3: releases pre-dispatch budget reservations made by
/// [`crate::budget::BudgetManager::try_reserve_agent_tokens`] /
/// `try_reserve_tenant_tokens` if `process_task_submission_logic` returns early
/// (via `?` or an explicit `return Err(..)`) after reserving but before the
/// task is durably committed. Reservations must not outlive a failed
/// submission, or the agent/tenant's budget would be permanently short by the
/// estimate even though no work was ever dispatched.
///
/// The two reservations have different success-path handling, reflecting the
/// different storage models they close a race on:
/// - **Agent** tokens are folded directly into `BudgetManager`'s real
///   `tokens_used` counter at reservation time (see
///   `try_reserve_agent_tokens`), so on success the reservation IS the record
///   of usage — it is not released, only ever released on failure.
/// - **Tenant** tokens use a separate `pending_tenant_tokens` ledger because
///   the tenant gate's "current usage" is a DB `SUM` over completed cost
///   records that cannot itself be locked. That ledger only needs to survive
///   long enough to close the concurrent-submission race; once this
///   submission call returns (success or failure) it is released
///   unconditionally by [`Self::finish`] — on success the task is durably
///   enqueued and its real usage will accumulate into `cost_records` on its
///   own, on failure nothing was ever dispatched.
#[derive(Default)]
struct BudgetReservationGuard {
    budget_manager: Option<std::sync::Arc<std::sync::RwLock<crate::budget::BudgetManager>>>,
    agent: Option<(crate::types::AgentId, usize)>,
    tenant: Option<(String, i64)>,
}

impl BudgetReservationGuard {
    /// Call on the success path: disarms the agent-token release (that
    /// reservation is kept as the real usage record) and lets `Drop` release
    /// the transient tenant reservation (see struct docs).
    fn finish(mut self) {
        self.agent = None;
        // `self.tenant` (if any) is left set so `Drop` releases it below.
    }
}

impl Drop for BudgetReservationGuard {
    fn drop(&mut self) {
        let Some(bm_lock) = self.budget_manager.take() else {
            return;
        };
        let bm = crate::sync_lock::rw_read(&*bm_lock);
        if let Some((agent_id, tokens)) = self.agent.take() {
            bm.release_agent_tokens(agent_id, tokens);
        }
        if let Some((tenant_id, tokens)) = self.tenant.take() {
            bm.release_tenant_tokens(&tenant_id, tokens);
        }
    }
}

impl Orchestrator {
    // ORCH-01 SPLIT TARGET:
    //   new() / with_groups() / init_db() → orchestrator/core.rs
    //   submit_task*() / submit_batch() / resolve_route() / spawn_agent*() → orchestrator/task_dispatch.rs
    //   map_agent_session() / retire_agent() / heartbeat() / pause/resume_agent() → orchestrator/agent_state.rs
    //   All construction, lifecycle, scaling, and VCS methods are in sub-modules:
    //   core.rs, agent_lifecycle.rs, scaling.rs, vcs_ops.rs

    /// Submit a new task to the orchestrator (async).
    ///
    /// The orchestrator will:
    /// 1. Analyze the file manifest against the affinity map
    /// 2. Route the task to an existing agent or spawn a new one
    /// 3. Acquire file locks
    /// 4. Enqueue the task
    pub async fn submit_task(
        &self,
        description: impl Into<String>,
        file_manifest: Vec<FileAffinity>,
        priority: Option<TaskPriority>,
        session_id: Option<String>,
        tenant_id: Option<String>,
    ) -> Result<TaskId, OrchestratorError> {
        self.submit_task_with_agent(
            description,
            file_manifest,
            priority,
            None,
            None,
            None,
            session_id,
            tenant_id,
        )
        .await
    }

    /// Submit a new task to the orchestrator, potentially targeting a specific agent name (async).
    pub async fn submit_task_with_agent(
        &self,
        description: impl Into<String>,
        file_manifest: Vec<FileAffinity>,
        priority: Option<TaskPriority>,
        target_agent: Option<String>,
        capability_requirements: Option<crate::contract::TaskCapabilityHints>,
        enqueue_hints: Option<TaskEnqueueHints>,
        session_id: Option<String>,
        tenant_id: Option<String>,
    ) -> Result<TaskId, OrchestratorError> {
        let (default_priority, _scope_enforcement) = {
            let config_guard = crate::sync_lock::rw_read(&*self.config);
            if !config_guard.enabled {
                return Err(OrchestratorError::Disabled);
            }
            if self.is_stopped() {
                return Err(OrchestratorError::Stopped);
            }
            (
                config_guard.default_priority,
                config_guard.scope_enforcement,
            )
        };

        let task_id = self.task_id_gen.next();
        let priority = priority.unwrap_or(default_priority);

        let mut task = AgentTask::new(task_id, description, priority, file_manifest.clone());
        task.capability_requirements = capability_requirements.clone();
        task.session_id = session_id.clone();
        task.tenant_id = tenant_id.clone();
        if let Some(h) = &enqueue_hints {
            task.apply_hints(h);
        }

        // Initialize trace_id if missing (FIX-14)
        if task.trace_id.is_none() {
            task.trace_id = Some(uuid::Uuid::new_v4().to_string());
        }
        #[cfg(feature = "populi-transport")]
        let _relay_thread_id_seed = task.thread_id.clone();
        #[cfg(feature = "populi-transport")]
        let _relay_harness_spec_json_seed = task.harness_spec_json.clone();
        task.start(); // ensure started_at_ms is populated for orchestrator-submitted tasks
        if let (Some(campaign_id), Some(tier)) = (task.campaign_id.clone(), task.benchmark_tier) {
            if let Err(e) = self
                .begin_reconstruction_campaign(
                    campaign_id.clone(),
                    tier,
                    task.description.clone(),
                    session_id.as_deref(),
                )
                .await
            {
                // Campaign init failed; downstream lookups against this campaign_id would
                // dangle. Surface the failure to operators and clear the id from the task
                // so subsequent code paths don't reference a non-existent campaign row.
                // Refs: docs/src/architecture/semantic-gap-audit-2026.md F5.
                log_persistence_failure("submit.campaign_init", e);
                task.campaign_id = None;
            }
        }

        // Route to the right agent via RoutingService
        let agent_id = self
            .resolve_route(
                &file_manifest,
                target_agent.as_deref(),
                capability_requirements.as_ref(),
                Some(task.description.as_str()),
                Some(task_id),
            )
            .await?;
        if !crate::sync_lock::rw_read(&*self.agents).contains_key(&agent_id) {
            return Err(OrchestratorError::AgentNotFound(agent_id));
        }

        populate_task_attention_fields(self, &mut task, agent_id, &file_manifest);
        if let Some(reason) = submission_approval_block_reason(&task) {
            return Err(OrchestratorError::ApprovalBlocked(reason));
        }

        self.process_task_submission_logic(&mut task, agent_id, &file_manifest)
            .await?;
        Ok(task_id)
    }

    /// Re-submit an existing task to the orchestrator (e.g. after agent retirement).
    pub async fn submit_existing_task(
        &self,
        mut task: AgentTask,
    ) -> Result<TaskId, OrchestratorError> {
        let task_id = task.id;
        let file_manifest = task.file_manifest.clone();

        // Re-route the task
        let agent_id = self
            .resolve_route(
                &file_manifest,
                None, // Don't force target agent for re-route
                task.capability_requirements.as_ref(),
                Some(task.description.as_str()),
                Some(task_id),
            )
            .await?;

        if !crate::sync_lock::rw_read(&*self.agents).contains_key(&agent_id) {
            return Err(OrchestratorError::AgentNotFound(agent_id));
        }

        self.process_task_submission_logic(&mut task, agent_id, &file_manifest)
            .await?;
        Ok(task_id)
    }

    pub async fn process_task_submission_logic(
        &self,
        task: &mut AgentTask,
        agent_id: crate::types::AgentId,
        file_manifest: &[FileAffinity],
    ) -> Result<(), OrchestratorError> {
        let task_id = task.id;
        let session_id = task.session_id.clone();
        let _capability_requirements = task.capability_requirements.clone();
        let _relay_thread_id_seed = task.thread_id.clone();
        let _relay_harness_spec_json_seed = task.harness_spec_json.clone();

        let (policy_trust, scope_enforcement) = {
            let cfg = crate::sync_lock::rw_read(&*self.config);
            let mut t = PolicyTrustRelax::default();
            if cfg.trust_gate_relax_enabled {
                t.relax_scope_strict_on_high_reliability = true;
                t.min_reliability = cfg.trust_gate_relax_min_reliability;
                t.agent_reliability = self.lookup_agent_reliability_sync(agent_id);
            }
            (t, cfg.scope_enforcement)
        };
        // §5.3(b): multi-agent runs escalate a configured `Warn` to `Strict`.
        // An explicit `Disabled` opt-out is never silently flipped.
        let active_agents = crate::sync_lock::rw_read(&*self.agents).len();
        let scope_enforcement =
            crate::scope::multi_agent_enforcement(active_agents, scope_enforcement);
        // §5.1/§5.3(a): the effective isolation strategy for this agent decides
        // whether a file-lock conflict is a hard error (SharedBranch) or tolerated
        // (SplitChanges/SeparateBranches → recorded as a conflict at merge).
        let isolation_strategy = {
            let handle = self.isolation_policy_handle();
            let plan = crate::sync_lock::rw_read(&handle);
            plan.strategy_for(agent_id)
        };

        // Cheap budget gates — run before any expensive work (Socrates research,
        // gate evaluation, persistence). Per CodeRabbit review on PR #61: an agent
        // already in a doom loop or already over budget should be rejected before
        // it can incur additional Socrates / autonomous-research cost.
        //
        // T5.3: both gates below use `BudgetManager::try_reserve_*` to make the
        // check-and-reserve atomic (single write-lock critical section), closing
        // the TOCTOU race where two concurrent submissions could both pass the
        // same stale snapshot. `budget_reservation_guard` releases any reservation
        // made here if a later step in this function returns early — see its
        // `Drop` impl below.
        let mut budget_reservation_guard = BudgetReservationGuard {
            budget_manager: Some(self.budget_manager.clone()),
            agent: None,
            tenant: None,
        };

        // Tenant budget enforcement (D7).
        if let Some(ref tenant_id) = task.tenant_id {
            let db_opt = crate::sync_lock::rw_read(&*self.db).clone();
            if let Some(db) = db_opt {
                let monthly_usage: i64 =
                    vox_gamify::db::get_tenant_monthly_token_usage(&db, tenant_id)
                        .await
                        .unwrap_or(0);
                let estimated_tokens = task.estimated_token_count();

                // For now, assume "free" tier. A future lookup table in VoxDb will resolve this.
                let tier = "free";

                let reserve_result = {
                    let bm = crate::sync_lock::rw_read(&*self.budget_manager);
                    let gate = crate::sync_lock::rw_read(&*self.tenant_budget_gate);
                    bm.try_reserve_tenant_tokens(
                        tenant_id,
                        tier,
                        monthly_usage,
                        estimated_tokens as i64,
                        &gate,
                    )
                };
                if let Err(msg) = reserve_result {
                    tracing::error!(tenant_id = %tenant_id, %msg, "blocking submission: tenant budget exceeded");
                    return Err(OrchestratorError::BudgetExceeded(msg));
                }
                budget_reservation_guard.tenant =
                    Some((tenant_id.clone(), estimated_tokens as i64));
            }
        }

        let gate_result = {
            let bm = crate::sync_lock::rw_read(&*self.budget_manager);
            crate::gate::BudgetGate::check_doom_loop(&bm, agent_id)
        };
        if let crate::gate::GateResult::DoomLoop { message } = gate_result {
            tracing::error!(agent_id = ?agent_id, %message, "blocking submission: doom-loop");
            return Err(crate::orchestrator::OrchestratorError::DoomLoop(message));
        }

        // Pre-dispatch token estimation (M7).
        // T5.3: atomic check-and-reserve — see `try_reserve_agent_tokens` doc
        // comment for how this closes the concurrent-submission race.
        {
            let estimated_tokens =
                task.description.len() / 4 + file_manifest.len().saturating_mul(200);
            let reserved = {
                let bm = crate::sync_lock::rw_read(&*self.budget_manager);
                bm.try_reserve_agent_tokens(agent_id, estimated_tokens)
            };
            if !reserved {
                tracing::warn!(
                    agent_id = ?agent_id,
                    estimated_tokens,
                    "blocking task submission: estimated tokens would exceed budget"
                );
                return Err(crate::orchestrator::OrchestratorError::BudgetExceeded(
                    format!(
                        "Pre-dispatch estimate of {} tokens would exceed remaining budget",
                        estimated_tokens
                    ),
                ));
            }
            budget_reservation_guard.agent = Some((agent_id, estimated_tokens));
        }

        // Phase 2: Socratic execution limits (Risk-based policies)
        let socrates_gate_enforce = {
            let cfg = crate::sync_lock::rw_read(&*self.config);
            cfg.socrates_gate_enforce
        };
        if socrates_gate_enforce {
            if let Some(ref soc_ctx) = task.socrates {
                let policy = {
                    let cfg = crate::sync_lock::rw_read(&*self.config);
                    cfg.effective_socrates_policy()
                };
                let mut augmented = soc_ctx.clone();
                if crate::sync_lock::rw_read(&*self.budget_manager).is_fatigued() {
                    augmented.fatigue_active = true;
                }
                let outcome = crate::socrates::evaluate_socrates_gate(
                    &augmented,
                    &policy,
                    task.description.as_str(),
                );
                if outcome.decision
                    == vox_orchestrator_types::socrates_policy::RiskDecision::Abstain
                {
                    return Err(OrchestratorError::ScopeDenied(format!(
                        "Socratic Gate blocked execution of task {} due to Abstain risk policy (band: {:?})",
                        task.id, outcome.band
                    )));
                }

                if outcome.research_decision.should_research {
                    let queries = outcome
                        .research_decision
                        .suggested_query
                        .clone()
                        .map(|q| vec![q])
                        .unwrap_or_else(|| vec![task.description.clone()]);
                    let results = self
                        .perform_autonomous_research(
                            Some(agent_id),
                            Some(task.id),
                            queries,
                            &outcome.research_decision.trigger,
                        )
                        .await
                        .unwrap_or_default();
                    if !results.is_empty() {
                        let old_quality = augmented.evidence_quality;
                        self.inject_research_results(&mut augmented, results);
                        task.socrates = Some(augmented.clone());

                        tracing::info!(
                            target: "vox_orchestrator::socrates",
                            task_id = task.id.0,
                            quality_improvement = augmented.evidence_quality - old_quality,
                            "proactive autonomous research injected; evidence quality boosted"
                        );
                    }
                }
            }
        }

        // Budget evaluation for autonomous self-correction
        let budget_signal =
            crate::sync_lock::rw_read(&*self.budget_manager).agent_budget_signal(agent_id);
        match budget_signal {
            crate::budget::BudgetSignal::CostExceeded {
                cost_usd,
                limit_usd,
            } => {
                self.event_bus
                    .emit(crate::events::AgentEventKind::BudgetAlert {
                        agent_id,
                        signal: budget_signal,
                    });
                return Err(OrchestratorError::BudgetExceeded(format!(
                    "Cost cap of ${:.2} exceeded (${:.2})",
                    limit_usd, cost_usd
                )));
            }
            crate::budget::BudgetSignal::Critical { usage_ratio, .. } => {
                self.event_bus
                    .emit(crate::events::AgentEventKind::BudgetAlert {
                        agent_id,
                        signal: budget_signal,
                    });
                return Err(OrchestratorError::BudgetExceeded(format!(
                    "Token cap reached ({:.1}%)",
                    usage_ratio * 100.0
                )));
            }
            crate::budget::BudgetSignal::HighLoad { .. } => {
                self.event_bus
                    .emit(crate::events::AgentEventKind::BudgetAlert {
                        agent_id,
                        signal: budget_signal,
                    });
            }
            _ => {}
        }

        // Pre-queue policy check (locks; scope when enforcement enabled).
        // The scope READ guard must not overlap `assign_file`, which takes a WRITE lock on the
        // same `RwLock` — that self-deadlocks on typical OS RwLock implementations.
        {
            let scope_guard_lock = (scope_enforcement != ScopeEnforcement::Disabled)
                .then_some(crate::sync_lock::rw_read(&*self.scope_guard));
            let scope_guard_ref = scope_guard_lock.as_deref();
            match PolicyEngine::check_before_queue_with_locks(
                &self.lock_manager,
                scope_guard_ref,
                &self.event_bus,
                file_manifest,
                agent_id,
                policy_trust,
                isolation_strategy == crate::isolation::IsolationStrategy::SharedBranch,
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

        // Try to acquire locks for write files.
        //
        // Under SharedBranch the file lock is AUTHORITATIVE (spec §5.3a): a failed
        // exclusive acquire on a contested path is a hard conflict, not best-effort.
        // SplitChanges / SeparateBranches tolerate overlap (conflicts recorded later),
        // so they keep the best-effort acquire.
        for fa in file_manifest {
            if fa.access == AccessKind::Write {
                let lock_kind = LockKind::Exclusive;
                match self.lock_manager.try_acquire(&fa.path, agent_id, lock_kind) {
                    Ok(()) => {}
                    Err(e)
                        if isolation_strategy
                            == crate::isolation::IsolationStrategy::SharedBranch =>
                    {
                        return Err(OrchestratorError::LockConflict(e));
                    }
                    Err(_) => { /* tolerated: overlap becomes a recorded conflict at merge */ }
                }
            }
        }

        // Assign files to the agent in the affinity map and scope guard
        for fa in file_manifest {
            if fa.access == AccessKind::Write {
                self.affinity_map.assign(&fa.path, agent_id);
                crate::sync_lock::rw_write(&*self.scope_guard)
                    .assign_file(agent_id, fa.path.clone());
            }
        }

        // Capture pre-task snapshot for version control (persisted to VoxDb)
        let snapshot_before = {
            let paths: Vec<PathBuf> = file_manifest.iter().map(|f| f.path.clone()).collect();
            let desc_str = task.description.clone();
            let snap_desc = format!("pre-task: {:.50}", desc_str);
            let snap_id = self
                .capture_snapshot(agent_id, &paths, snap_desc.clone())
                .await;
            self.event_bus
                .emit(crate::events::AgentEventKind::SnapshotCaptured {
                    agent_id,
                    snapshot_id: snap_id.to_string(),
                    file_count: paths.len(),
                    description: snap_desc,
                    session_id: task.session_id.clone(),
                });
            snap_id
        };

        self.record_operation(
            agent_id,
            OperationKind::TaskSubmit { task_id: task.id.0 },
            format!("Submitted task {}", task.id),
            Some(snapshot_before),
            None,
            None,
            None,
        )
        .await;

        self.record_activity();
        crate::sync_lock::rw_write(&self.monitor).record_progress(agent_id);

        let remote_relay_desc = task.description.clone();
        let lineage_desc_preview: String = remote_relay_desc.chars().take(240).collect();
        let lineage_campaign_id = task.campaign_id.clone();
        let lineage_benchmark_tier = task.benchmark_tier;
        let lineage_execution_role = task.execution_role;
        let cleanup_claims = |agent_id: crate::types::AgentId| {
            for fa in file_manifest {
                if fa.access == AccessKind::Write {
                    self.lock_manager.release(&fa.path, agent_id);
                    self.affinity_map.release(&fa.path);
                    crate::sync_lock::rw_write(&*self.scope_guard).revoke_file(agent_id, &fa.path);
                }
            }
        };

        #[cfg_attr(not(feature = "populi-transport"), allow(unused_variables))]
        let (lease_gated, remote_params, agent_busy) = {
            let c = crate::sync_lock::rw_read(&*self.config);
            let lease_gated = crate::populi_remote::task_matches_populi_remote_lease_gate(task, &c);
            let rp = if !cfg!(feature = "populi-transport") || !c.populi_remote_execute_experimental
            {
                None
            } else {
                match (
                    c.populi_control_url
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty()),
                    c.populi_remote_execute_receiver_agent
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty()),
                ) {
                    (Some(b), Some(r)) => Some((
                        b.to_string(),
                        r.to_string(),
                        c.populi_http_timeout_ms,
                        c.populi_scope_id.clone(),
                        c.populi_remote_execute_sender_agent.clone(),
                        crate::populi_remote::lease_claimer_node_id(&c),
                    )),
                    _ => None,
                }
            };
            let busy = crate::sync_lock::rw_read(&*self.agents)
                .get(&agent_id)
                .map(|ql| crate::sync_lock::rw_read(&**ql).has_in_progress())
                .unwrap_or(false);
            (lease_gated, rp, busy)
        };

        // Capture mesh policy before moving task into task_for_enqueue.
        #[cfg_attr(not(feature = "populi-transport"), allow(unused_variables))]
        let task_mesh_policy_local_only =
            matches!(task.mesh_policy, crate::types::MeshPolicy::LocalOnly);
        #[cfg_attr(not(feature = "populi-transport"), allow(unused_variables))]
        let task_mesh_policy_excludes: Vec<String> =
            if let crate::types::MeshPolicy::Exclude(ref ids) = task.mesh_policy {
                ids.clone()
            } else {
                vec![]
            };
        let mut task_for_enqueue = Some(task);
        #[cfg_attr(not(feature = "populi-transport"), allow(unused_mut))]
        let mut held_remote = false;
        #[cfg_attr(not(feature = "populi-transport"), allow(unused_mut))]
        let mut placement_reason = crate::populi_remote::PlacementReasonCode::LocalQueueDefault;
        // Populated inside #[cfg(feature = "populi-transport")] when a lease is granted.
        #[cfg_attr(not(feature = "populi-transport"), allow(unused_mut))]
        let mut routing_lease_id: Option<String> = None;
        #[cfg_attr(not(feature = "populi-transport"), allow(unused_mut))]
        let mut retrieval_context_attached = false;

        #[cfg(feature = "populi-transport")]
        if lease_gated && remote_params.is_some() && !agent_busy && !task_mesh_policy_local_only {
            let (mut base, recv_s, timeout_ms, scope, send_opt, claimer_node_id) =
                remote_params.clone().expect("checked is_some");
            if let Ok(recv_id) = recv_s.parse::<u64>() {
                let send_s = send_opt.unwrap_or_default();
                let send_id = send_s.trim().parse::<u64>().unwrap_or(1);
                let client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(
                    &base,
                    std::time::Duration::from_millis(timeout_ms.max(1000)),
                )
                .with_env_deliver_token();
                let now = crate::types::now_unix_ms();
                let cap_json = _capability_requirements
                    .as_ref()
                    .and_then(|c| serde_json::to_string(c).ok())
                    .unwrap_or_else(|| "{}".to_string());
                let idempotency_key = format!("orch-remote-{}-{}", task_id.0, now);
                let scope_key = format!("task:{}", task_id.0);
                let repository_id = scope
                    .clone()
                    .unwrap_or_else(|| "orchestrator-local".to_string());
                let lease_node =
                    vox_populi::node_record_for_current_process(claimer_node_id.clone(), None);
                let _ = client.join(&lease_node).await;

                // W2 admission control: if the task declares a minimum VRAM requirement,
                // verify at least one healthy registered node can satisfy it before
                // granting a lease. Fall back to local queue rather than dispatch a job
                // the mesh cannot run.
                let vram_admission_ok = if let Some(required_vram) = _capability_requirements
                    .as_ref()
                    .and_then(|c| c.min_vram_mb)
                    .filter(|&v| v > 0)
                {
                    match client.list_nodes().await {
                        Ok(registry) => {
                            let fits = registry.nodes.iter().any(|n| {
                                n.maintenance != Some(true)
                                    && n.quarantined != Some(true)
                                    && n.capabilities
                                        .min_vram_mb
                                        .is_some_and(|v| v >= required_vram)
                            });
                            if !fits {
                                tracing::info!(
                                        task_id = task_id.0,
                                        required_vram_mb = required_vram,
                                        placement_reason = crate::populi_remote::PlacementReasonCode::LocalQueueFallbackInsufficientVram.as_str(),
                                        "populi admission: no node meets min_vram_mb; falling back to local queue"
                                    );
                                placement_reason = crate::populi_remote::PlacementReasonCode::LocalQueueFallbackInsufficientVram;
                            }
                            fits
                        }
                        Err(err) => {
                            tracing::debug!(
                                task_id = task_id.0,
                                error = %err,
                                "populi admission: list_nodes failed; skipping vram check"
                            );
                            true // optimistic: let lease grant decide
                        }
                    }
                } else {
                    true // no VRAM requirement; skip check
                };

                let mut lease_id = if !vram_admission_ok {
                    None
                } else {
                    match client
                        .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
                            claimer_node_id: claimer_node_id.clone(),
                            scope_key: scope_key.clone(),
                        })
                        .await
                    {
                        Ok(grant) => Some(grant.lease_id),
                        Err(err) => {
                            tracing::info!(
                                error = %err,
                                task_id = task_id.0,
                                scope_key,
                                placement_reason = crate::populi_remote::PlacementReasonCode::LocalQueueFallbackAfterRemoteRelayError.as_str(),
                                "populi lease-gated exec lease grant failed; falling back to local queue"
                            );
                            placement_reason = crate::populi_remote::PlacementReasonCode::LocalQueueFallbackAfterRemoteRelayError;
                            None
                        }
                    }
                };
                if lease_id.is_none() {
                    // Phase 1 Federation Proxy: Try to find a peer mesh if local denies
                    if let Ok(dir) = client.federation_directory().await {
                        let mut candidates: Vec<_> = dir
                            .entries
                            .into_iter()
                            .filter(|e| e.public)
                            // MeshPolicy::Exclude: skip nodes in the exclusion list.
                            .filter(|e| {
                                task_mesh_policy_excludes.is_empty()
                                    || !task_mesh_policy_excludes.contains(&e.scope_id)
                            })
                            .collect();
                        candidates.sort_by_key(|e| e.current_queue_depth.unwrap_or(usize::MAX));
                        for peer in candidates {
                            let peer_client =
                                vox_populi::http_client::PopuliHttpClient::new_with_timeout(
                                    &peer.control_url,
                                    std::time::Duration::from_millis(timeout_ms.max(1000)),
                                )
                                .with_env_deliver_token();

                            if let Ok(grant) = peer_client
                                .exec_lease_grant(
                                    &vox_populi::transport::RemoteExecLeaseGrantRequest {
                                        claimer_node_id: claimer_node_id.clone(),
                                        scope_key: scope_key.clone(),
                                    },
                                )
                                .await
                            {
                                tracing::info!(
                                    task_id = task_id.0,
                                    peer_scope = %peer.scope_id,
                                    peer_url = %peer.control_url,
                                    "Federation routing successful; proxying task to remote mesh"
                                );
                                lease_id = Some(grant.lease_id);
                                base = peer.control_url;
                                break;
                            }
                        }
                    }
                }

                if lease_id.is_none() {
                    // Fall through to local enqueue only.
                } else {
                    let t = task_for_enqueue.take().expect("task present before hold");
                    let held_thread_id = t.thread_id.clone();
                    let held_harness_spec_json = t.harness_spec_json.clone();
                    t.populi_remote_delegate = Some(crate::types::PopuliRemoteDelegate {
                        idempotency_key: idempotency_key.clone(),
                        lease_id: lease_id.clone(),
                        claimer_node_id: Some(claimer_node_id.clone()),
                    });
                    // Stamp the executing node for mesh audit (Task E-2).
                    t.executor_node_id = Some(claimer_node_id.clone());
                    enum HoldOutcome {
                        Held,
                        AgentBusy,
                        AgentMissing,
                    }
                    let hold_outcome = {
                        let agents = crate::sync_lock::rw_read(&*self.agents);
                        if let Some(queue_lock) = agents.get(&agent_id) {
                            let mut queue = crate::sync_lock::rw_write(&**queue_lock);
                            self.event_bus
                                .emit(crate::events::AgentEventKind::TaskSubmitted {
                                    task_id,
                                    agent_id,
                                    description: t.description.clone(),
                                    session_id: t.session_id.clone(),
                                });
                            match queue.hold_for_populi_remote((*t).clone()) {
                                Ok(()) => HoldOutcome::Held,
                                Err(crate::queue::PopuliRemoteHoldError::AgentBusy) => {
                                    HoldOutcome::AgentBusy
                                }
                            }
                        } else {
                            HoldOutcome::AgentMissing
                        }
                    };
                    match hold_outcome {
                        HoldOutcome::Held => {
                            crate::sync_lock::rw_write(&*self.task_assignments)
                                .insert(task_id, agent_id);
                            tracing::info!(
                                placement_reason = crate::populi_remote::PlacementReasonCode::PopuliRemoteLeaseHold.as_str(),
                                task_id = task_id.0,
                                agent_id = agent_id.0,
                                "Task {} held for Populi remote on agent {}",
                                task_id,
                                agent_id
                            );
                            held_remote = true;
                            placement_reason =
                                crate::populi_remote::PlacementReasonCode::PopuliRemoteLeaseHold;
                        }
                        HoldOutcome::AgentBusy => {
                            cleanup_claims(agent_id);
                            return Err(OrchestratorError::PopuliRemoteHoldRace);
                        }
                        HoldOutcome::AgentMissing => {
                            cleanup_claims(agent_id);
                            return Err(OrchestratorError::AgentNotFound(agent_id));
                        }
                    }

                    let attached_retrieval =
                        self.attach_session_retrieval_envelope_if_present(task_id, &session_id);
                    if !attached_retrieval {
                        self.attach_goal_search_context_with_retrieval(
                            task_id,
                            &lineage_desc_preview,
                            file_manifest,
                        )
                        .await;
                    }
                    retrieval_context_attached = true;

                    let context_envelope_json = session_id.as_ref().and_then(|sid| {
                        let key = crate::socrates::session_context_envelope_key(sid);
                        crate::sync_lock::rw_read(&*self.context_store).get(&key)
                    });
                    let payload = serde_json::json!({
                        "task_description": remote_relay_desc,
                        "assigned_agent_id": agent_id.0,
                        "session_id": session_id.clone(),
                        "thread_id": held_thread_id.clone(),
                        "context_envelope_json": context_envelope_json,
                        "harness_spec_json": held_harness_spec_json.clone(),
                    })
                    .to_string();
                    let campaign_id = lineage_campaign_id.clone().filter(|s| !s.is_empty());
                    let envelope = crate::a2a::RemoteTaskEnvelope {
                        idempotency_key: idempotency_key.clone(),
                        task_id: task_id.0,
                        repository_id,
                        capability_requirements_json: cap_json,
                        payload,
                        privacy_class: None,
                        populi_scope_id: scope.clone(),
                        submitted_unix_ms: Some(now),
                        exec_lease_id: lease_id.clone(),
                        campaign_id,
                        artifact_refs_json: None,
                        session_id: session_id.clone(),
                        thread_id: held_thread_id,
                        context_envelope_json: context_envelope_json.clone(),
                        harness_spec_json: held_harness_spec_json,
                        parent_task_id: None,
                        caller_agent_id: None,
                        trace_id: None,
                        span_depth: None,
                        bundle_ref: None,
                        bundle_inline_b64: None,
                        // Task-delegation envelopes carry a task payload, not raw
                        // `.vox` source; the worker's real-execution path only fires
                        // when a script-dispatch sender populates these.
                        exec_source_b64: None,
                        exec_source_blake3_hex: None,
                        exec_bundle_b64: None,
                        exec_bundle_blake3_hex: None,
                    };
                    let relay_client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(
                        &base,
                        std::time::Duration::from_millis(timeout_ms.max(1000)),
                    )
                    .with_env_deliver_token();

                    if let Err(err) = crate::a2a::relay_remote_task_envelope(
                        &relay_client,
                        crate::types::AgentId(send_id),
                        crate::types::AgentId(recv_id),
                        &envelope,
                    )
                    .await
                    {
                        if let Some(active_lease_id) = lease_id.clone() {
                            let _ = relay_client
                                .exec_lease_release(
                                    &vox_populi::transport::RemoteExecLeaseReleaseRequest {
                                        lease_id: active_lease_id,
                                        claimer_node_id: claimer_node_id.clone(),
                                    },
                                )
                                .await;
                        }
                        let _ = self.fallback_populi_remote_task_locally(
                            task_id,
                            "remote_relay_failed_after_hold",
                        );
                        held_remote = false;
                        tracing::info!(
                            error = %err,
                            task_id = task_id.0,
                            placement_reason = crate::populi_remote::PlacementReasonCode::LocalQueueFallbackAfterRemoteRelayError.as_str(),
                            "populi lease-gated relay failed after hold; task moved back to local queue"
                        );
                        placement_reason =
                            crate::populi_remote::PlacementReasonCode::LocalQueueFallbackAfterRemoteRelayError;
                    }
                }
                routing_lease_id = lease_id.clone();
            }
        }

        let handle = if let Some(task) = task_for_enqueue.take() {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            if let Some(queue_lock) = agents.get(&agent_id) {
                let mut queue = crate::sync_lock::rw_write(&**queue_lock);
                self.event_bus
                    .emit(crate::events::AgentEventKind::TaskSubmitted {
                        task_id,
                        agent_id,
                        description: task.description.clone(),
                        session_id: task.session_id.clone(),
                    });
                let q_len = queue.len();
                queue.enqueue((*task).clone());
                crate::sync_lock::rw_write(&*self.task_assignments).insert(task_id, agent_id);

                tracing::info!(
                    placement_reason = placement_reason.as_str(),
                    task_id = task_id.0,
                    agent_id = agent_id.0,
                    lease_id = routing_lease_id.as_deref().unwrap_or(""),
                    "Task {} routed to agent {} (queue len: {})",
                    task_id,
                    agent_id,
                    q_len + 1
                );
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
                cleanup_claims(agent_id);
                return Err(OrchestratorError::AgentNotFound(agent_id));
            }
        } else {
            #[cfg(feature = "runtime")]
            {
                crate::sync_lock::rw_read(&*self.agent_handles)
                    .get(&agent_id)
                    .cloned()
            }
            #[cfg(not(feature = "runtime"))]
            {
                None
            }
        };

        // Notify the agent process to wake up and process (outside the locks)
        #[cfg(feature = "runtime")]
        if let Some(handle) = handle {
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
                        tracing::warn!("submit_task: agent notify send failed: {e:?}");
                    }
                }
                Err(_) => tracing::warn!(
                    "submit_task: agent notify timed out after {:?}",
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
            let mut steps = vec![
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
            ];
            if held_remote {
                steps.push(TaskTraceStep {
                    stage: "populi_remote_held".to_string(),
                    timestamp_ms: now_ms,
                    detail: Some("single-owner mesh delegation".to_string()),
                });
            }
            traces.insert(task_id, steps);
        }

        // Code-review fix (gui-axis-chat-harness-fixes, post-Task-4): this used
        // to also skip for TaskCategory::Chat, on the premise that chat tasks
        // ran a separate fast, non-agentic `ChatTaskProcessor` where CRAG's
        // latency wasn't worth paying. `ChatTaskProcessor` is deleted — Chat
        // now runs the identical `AiTaskProcessor` pipeline as every other
        // category, so skipping context attachment only for Chat left it with
        // strictly worse grounding than an identical prompt under any other
        // category, for a latency rationale that no longer applies.
        if !retrieval_context_attached {
            let attached_retrieval =
                self.attach_session_retrieval_envelope_if_present(task_id, &session_id);
            if !attached_retrieval {
                self.attach_goal_search_context_with_retrieval(
                    task_id,
                    &lineage_desc_preview,
                    file_manifest,
                )
                .await;
            }
        }

        #[cfg(feature = "populi-transport")]
        if !task_mesh_policy_local_only {
            if let Some((base, recv_s, timeout_ms, scope, send_opt, _claimer_node_id)) =
                remote_params.filter(|_| !lease_gated)
            {
                let task_id_u = task_id.0;
                let agent_u = agent_id.0;
                let desc = remote_relay_desc;
                let caps = _capability_requirements.clone();
                let relay_campaign_id = lineage_campaign_id.clone();
                let relay_session_id = session_id.clone();
                let relay_thread_id = _relay_thread_id_seed.clone();
                let relay_harness_spec_json = _relay_harness_spec_json_seed.clone();
                let relay_context_envelope_json = relay_session_id.as_ref().and_then(|sid| {
                    let key = crate::socrates::session_context_envelope_key(sid);
                    crate::sync_lock::rw_read(&*self.context_store).get(&key)
                });
                let send_s = send_opt.unwrap_or_default();
                tokio::spawn(async move {
                    use std::time::Duration;

                    let Ok(recv_id) = recv_s.parse::<u64>() else {
                        tracing::warn!(
                            "populi remote relay: receiver agent id must be a u64 (got {:?})",
                            recv_s
                        );
                        return;
                    };
                    let send_id = send_s.trim().parse::<u64>().unwrap_or(1);
                    let client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(
                        &base,
                        Duration::from_millis(timeout_ms.max(1000)),
                    )
                    .with_env_deliver_token();
                    let now = crate::types::now_unix_ms();
                    let cap_json = caps
                        .as_ref()
                        .and_then(|c| serde_json::to_string(c).ok())
                        .unwrap_or_else(|| "{}".to_string());
                    let idempotency_key = format!("orch-remote-{task_id_u}-{now}");
                    let payload = serde_json::json!({
                        "task_description": desc,
                        "assigned_agent_id": agent_u,
                        "session_id": relay_session_id,
                        "thread_id": relay_thread_id,
                        "context_envelope_json": relay_context_envelope_json,
                        "harness_spec_json": relay_harness_spec_json,
                    })
                    .to_string();
                    let repository_id = scope
                        .clone()
                        .unwrap_or_else(|| "orchestrator-local".to_string());
                    let envelope = crate::a2a::RemoteTaskEnvelope {
                        idempotency_key,
                        task_id: task_id_u,
                        repository_id,
                        capability_requirements_json: cap_json,
                        payload,
                        privacy_class: None,
                        populi_scope_id: scope.clone(),
                        submitted_unix_ms: Some(now),
                        exec_lease_id: None,
                        campaign_id: relay_campaign_id.filter(|s| !s.is_empty()),
                        artifact_refs_json: None,
                        session_id: relay_session_id.clone(),
                        thread_id: relay_thread_id.clone(),
                        context_envelope_json: relay_context_envelope_json.clone(),
                        harness_spec_json: relay_harness_spec_json.clone(),
                        parent_task_id: None,
                        caller_agent_id: None,
                        trace_id: None,
                        span_depth: None,
                        bundle_ref: None,
                        bundle_inline_b64: None,
                        exec_source_b64: None,
                        exec_source_blake3_hex: None,
                        exec_bundle_b64: None,
                        exec_bundle_blake3_hex: None,
                    };
                    if let Err(err) = crate::a2a::relay_remote_task_envelope(
                        &client,
                        crate::types::AgentId(send_id),
                        crate::types::AgentId(recv_id),
                        &envelope,
                    )
                    .await
                    {
                        tracing::debug!(
                            error = %err,
                            task_id = task_id_u,
                            "populi experimental remote relay failed (local queue still owns execution)"
                        );
                    }
                });
            }
        } // end: if !task_mesh_policy_local_only

        if crate::lineage::orchestration_lineage_persist_enabled() {
            if let Some(db) = self.db() {
                let repo = crate::lineage::repository_id();
                let mut payload = serde_json::json!({
                    "description_preview": lineage_desc_preview,
                });
                if let Some(ref campaign_id) = lineage_campaign_id {
                    payload["task_campaign_id"] = serde_json::Value::String(campaign_id.clone());
                }
                if let Some(tier) = lineage_benchmark_tier {
                    payload["benchmark_tier"] =
                        serde_json::Value::String(tier.as_str().to_string());
                }
                if let Some(role) = lineage_execution_role {
                    payload["execution_role"] =
                        serde_json::Value::String(role.as_str().to_string());
                }
                if let Some(cid) = crate::lineage::orchestration_campaign_id() {
                    payload["orchestration_campaign_id"] = serde_json::Value::String(cid);
                }
                let payload_str = payload.to_string();
                if let Err(e) = db
                    .append_orchestration_lineage_event(
                        &repo,
                        "task_submitted",
                        task_id.0 as i64,
                        Some(agent_id.0 as i64),
                        session_id.as_deref(),
                        None,
                        None,
                        None,
                        Some(payload_str.as_str()),
                    )
                    .await
                {
                    // Lineage write failures leave a permanent gap in the audit trail;
                    // surface to operators rather than swallow.
                    // Refs: docs/src/architecture/semantic-gap-audit-2026.md F4.
                    log_persistence_failure("lineage.task_submitted", e);
                }
            }
        }

        // Submission committed successfully. See `BudgetReservationGuard::finish`
        // for why the agent-token reservation is kept while the tenant one is
        // released here.
        budget_reservation_guard.finish();

        Ok(())
    }

    /// Submit a task with planning metadata attached.
    pub async fn submit_task_with_agent_planned(
        &self,
        description: impl Into<String>,
        file_manifest: Vec<FileAffinity>,
        priority: Option<TaskPriority>,
        target_agent: Option<String>,
        capability_requirements: Option<crate::contract::TaskCapabilityHints>,
        session_id: Option<String>,
        enqueue_hints: Option<TaskEnqueueHints>,
        tenant_id: Option<String>,
        planning_meta: Option<PlanningTaskMeta>,
    ) -> Result<TaskId, OrchestratorError> {
        let task_id = self
            .submit_task_with_agent(
                description,
                file_manifest,
                priority,
                target_agent,
                capability_requirements,
                enqueue_hints,
                session_id,
                tenant_id,
            )
            .await?;
        if let Some(meta) = planning_meta
            && let Some(agent_id) = crate::sync_lock::rw_read(&*self.task_assignments)
                .get(&task_id)
                .copied()
            && let Some(q_lock) = crate::sync_lock::rw_read(&*self.agents).get(&agent_id)
        {
            let _ = crate::sync_lock::rw_write(&**q_lock).attach_planning_meta(task_id, &meta);
        }
        Ok(task_id)
    }

    /// Attach Socrates evidence context to an already submitted task.
    pub fn attach_socrates_context(
        &self,
        task_id: TaskId,
        ctx: crate::socrates::SocratesTaskContext,
    ) -> Result<(), OrchestratorError> {
        let agent_id = crate::sync_lock::rw_read(&*self.task_assignments)
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;
        let agents = crate::sync_lock::rw_read(&*self.agents);
        let queue_lock = agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        let attached =
            crate::sync_lock::rw_write(&**queue_lock).attach_socrates_context(task_id, ctx);
        if attached {
            Ok(())
        } else {
            Err(OrchestratorError::TaskNotFound(task_id))
        }
    }
}

#[cfg(test)]
mod mesh_dispatch_policy_tests {
    use crate::types::{AgentTask, MeshPolicy, TaskId, TaskPriority};

    /// Verifies the guard logic: a LocalOnly task must not be eligible for mesh relay.
    ///
    /// In submit_task_with_agent, the relay block is guarded by
    /// `!task_mesh_policy_local_only`.  This test checks the *policy contract* that
    /// backs that guard — if MeshPolicy::LocalOnly.allows_node("remote-peer") is
    /// false, the dispatch code correctly skips the relay branch.
    #[test]
    fn local_only_policy_prevents_mesh_relay() {
        let mut task = AgentTask::new(TaskId(1), "do work", TaskPriority::Normal, vec![]);
        task.mesh_policy = MeshPolicy::LocalOnly;

        // The guard in submit_task_with_agent reads: !task_mesh_policy_local_only
        // where task_mesh_policy_local_only = matches!(task.mesh_policy, MeshPolicy::LocalOnly)
        let task_mesh_policy_local_only = matches!(task.mesh_policy, MeshPolicy::LocalOnly);
        // Task should NOT be relayed to mesh.
        assert!(
            task_mesh_policy_local_only,
            "LocalOnly policy must set local_only flag → relay guard evaluates to false"
        );

        // Verify the allows_node contract that underpins the guard.
        assert!(!task.mesh_policy.allows_node("remote-peer-7"));
        assert!(!task.mesh_policy.allows_node("node-42"));
        assert!(
            task.mesh_policy.allows_node("local"),
            "LocalOnly must still allow the local node"
        );
    }

    #[test]
    fn exclude_policy_filters_named_node() {
        let mut task = AgentTask::new(TaskId(2), "compute", TaskPriority::Normal, vec![]);
        task.mesh_policy = MeshPolicy::Exclude(vec!["peer-7".into(), "peer-9".into()]);

        // Excluded nodes must not be in the candidate set.
        assert!(!task.mesh_policy.allows_node("peer-7"));
        assert!(!task.mesh_policy.allows_node("peer-9"));
        // Non-excluded nodes are allowed.
        assert!(task.mesh_policy.allows_node("peer-11"));
        assert!(task.mesh_policy.allows_node("local"));
    }

    #[test]
    fn executor_node_id_none_by_default() {
        let task = AgentTask::new(TaskId(3), "audit", TaskPriority::Normal, vec![]);
        assert!(
            task.executor_node_id.is_none(),
            "executor_node_id must be None until a node is assigned"
        );
    }
}

/// Code-review fix (gui-axis-chat-harness-fixes, post-Task-4): this crate used
/// to skip `attach_goal_search_context_with_retrieval` for `TaskCategory::Chat`,
/// on the premise that chat tasks ran a separate fast, non-agentic
/// `ChatTaskProcessor` where CRAG's latency wasn't worth paying. Now that
/// `ChatTaskProcessor` is deleted and Chat runs the identical `AiTaskProcessor`
/// pipeline as every other category, that skip only left Chat-tagged tasks
/// with worse grounding than any other category for no remaining benefit —
/// so Chat now gets the same context attachment as everything else.
#[cfg(test)]
mod chat_skips_goal_search_context_tests {
    use crate::orchestrator::{Orchestrator, OrchestratorConfig};
    use crate::types::{TaskCategory, TaskEnqueueHints};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn chat_category_task_also_gets_goal_search_context_attached() {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        let hints = TaskEnqueueHints {
            task_category: Some(TaskCategory::Chat),
            ..Default::default()
        };
        let task_id = orch
            .submit_task_with_agent(
                "Say hello in one word.",
                vec![],
                None,
                None,
                None,
                Some(hints),
                None,
                None,
            )
            .await
            .unwrap();

        let task = orch
            .all_tasks()
            .into_iter()
            .find(|t| t.id == task_id)
            .expect("submitted task must be queued");
        assert!(
            task.socrates.is_some(),
            "chat-category task must get goal/CRAG context attached like every other category"
        );
    }

    /// Contrast case: a non-chat task also gets goal search context attached
    /// (via the fast no-DB heuristic path in test config) — proves both
    /// categories now behave identically, not that either is special-cased.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn non_chat_task_still_gets_goal_search_context_attached() {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        let task_id = orch
            .submit_task("Implement a new feature.", vec![], None, None, None)
            .await
            .unwrap();

        let task = orch
            .all_tasks()
            .into_iter()
            .find(|t| t.id == task_id)
            .expect("submitted task must be queued");
        assert!(
            task.socrates.is_some(),
            "non-chat task must still get goal/CRAG context attached"
        );
    }
}
