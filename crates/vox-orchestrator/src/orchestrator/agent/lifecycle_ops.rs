use crate::orchestrator::OrchestratorError;
use crate::services::MessageGateway;
use crate::types::{AgentId, AgentTask, TaskId, TaskPriority, TaskStatus};

/// Emit one `orch.task.cancelled` telemetry event when a task is explicitly
/// cancelled mid-execution (user interrupt, parent timeout, lease loss, etc.).
///
/// Cancellation paths previously left no row in `research_metrics`, making
/// it impossible to distinguish a quiet cancel from a stalled task offline.
/// `path` disambiguates the queue branch: `"populi_remote"` (Populi remote
/// delegate) or `"queue"` (locally-queued task).
fn emit_task_cancelled(task_id: TaskId, agent_id: AgentId, path: &'static str) {
    let metadata_json = serde_json::json!({
        "task_id": task_id.0,
        "agent_id": agent_id.0,
        "path": path,
    })
    .to_string();
    vox_telemetry::record_event!(&vox_telemetry::TelemetryEvent::ResearchMetric(
        vox_telemetry::ResearchMetricEvent {
            session_id: format!("orch:task:{}", task_id.0),
            metric_type: vox_telemetry::METRIC_TYPE_ORCH_TASK_CANCELLED.into(),
            metric_value: None,
            metadata_json: Some(metadata_json),
        }
    ));
}

impl crate::orchestrator::Orchestrator {
    /// Retire an agent: release all locks/affinity/scope, drain its queue, and return remaining tasks.
    pub async fn retire_agent(
        &self,
        agent_id: AgentId,
    ) -> Result<Vec<AgentTask>, OrchestratorError> {
        let (remaining, session_id) = {
            let queue_lock = crate::sync_lock::rw_write(&*self.agents)
                .remove(&agent_id)
                .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
            let mut queue = crate::sync_lock::rw_write(&*queue_lock);

            self.lock_manager.release_all(agent_id);
            self.affinity_map.release_all(agent_id);
            crate::sync_lock::rw_write(&*self.scope_guard).clear_scope(agent_id);
            crate::sync_lock::rw_write(&*self.dynamic_agents).remove(&agent_id);
            {
                let mut delegations = crate::sync_lock::rw_write(&*self.agent_delegations);
                delegations.remove(&agent_id);
                delegations.retain(|_, binding| binding.parent_agent_id != agent_id);
            }
            crate::sync_lock::rw_write(&*self.dynamic_spawn_context).remove(&agent_id);
            #[cfg(feature = "runtime")]
            crate::sync_lock::rw_write(&*self.agent_handles).remove(&agent_id);
            crate::sync_lock::rw_write(&*self.heartbeat_monitor).unregister(agent_id);

            let mut remaining = queue.drain_tasks();

            // Re-queue the in-progress task if it was interrupted by retirement
            if let Some(mut task) = queue.in_progress.take() {
                task.status = TaskStatus::Queued;
                remaining.insert(0, task);
            }

            let sid = queue.agent_session_id.clone();
            (remaining, sid)
        };

        // If this agent was mapped to a session, flush its socrates thinking context to DB.
        if let Some(sid) = session_id {
            let key = crate::socrates::session_context_envelope_key(&sid);
            let envelope_opt = crate::sync_lock::rw_read(&*self.context_store)
                .get(&key)
                .clone();
            if let Some(envelope_json) = envelope_opt {
                let db_opt = crate::sync_lock::rw_read(&*self.db).clone();
                if let Some(db) = db_opt {
                    let sid_clone = sid.clone();
                    let data_clone = envelope_json.clone();
                    self.persist_with_retry_meta("session_context_retirement_flush", None, move || {
                        let db = db.clone();
                        let sid = sid_clone.clone();
                        let data = data_clone.clone();
                        async move {
                            db.save_memory(vox_db::SaveMemoryParams {
                                agent_id: "orchestrator",
                                session_id: &sid,
                                memory_type: "socrates_session_context",
                                content: "Durable context envelope flushed on agent retirement scaling event",
                                metadata: Some(&data),
                                importance: 1.0,
                                vcs_snapshot_id: None,
                            })
                            .await
                            .map(|_| ())
                        }
                    })
                    .await;
                }
            }
        }

        MessageGateway::publish_agent_retired(&self.event_bus, agent_id);
        tracing::info!(
            "Retired agent {} — {} tasks to redistribute",
            agent_id,
            remaining.len()
        );
        Ok(remaining)
    }

    /// Cancel a queued task, or a Populi remote-delegated in-progress task.
    pub fn cancel_task(&self, task_id: TaskId) -> Result<(), OrchestratorError> {
        let agent_id = crate::sync_lock::rw_read(&self.task_assignments)
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;

        let agents = crate::sync_lock::rw_read(&self.agents);
        let queue_lock = agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        let mut queue = crate::sync_lock::rw_write(queue_lock);

        if let Some(task) = queue.current_task()
            && task.id == task_id
            && task.populi_remote_delegate.is_some()
        {
            #[cfg(feature = "populi-transport")]
            let delegate = task.populi_remote_delegate.clone();
            #[cfg(feature = "populi-transport")]
            let idempotency_key = delegate.as_ref().map(|d| d.idempotency_key.clone());
            let taken = queue.take_in_progress_if(task_id);
            if taken.is_none() {
                return Err(OrchestratorError::TaskNotFound(task_id));
            }
            let still_claimed_by_queue = |path: &std::path::Path, q: &crate::queue::AgentQueue| {
                q.current_task()
                    .is_some_and(|t| t.write_files().iter().any(|p| p.as_path() == path))
                    || q.tasks()
                        .iter()
                        .any(|t| t.write_files().iter().any(|p| p.as_path() == path))
            };
            if let Some(ref t) = taken {
                for path in t.write_files() {
                    if !still_claimed_by_queue(path, &queue) {
                        self.lock_manager.release(path, agent_id);
                        self.affinity_map.release(path);
                        crate::sync_lock::rw_write(&*self.scope_guard).revoke_file(agent_id, path);
                    }
                }
            }
            crate::sync_lock::rw_write(&self.task_assignments).remove(&task_id);
            tracing::info!(
                "Cancelled Populi remote-delegated task {} from agent {}",
                task_id,
                agent_id
            );
            emit_task_cancelled(task_id, agent_id, "populi_remote");
            #[cfg(feature = "populi-transport")]
            {
                let tid = task_id.0;
                match (idempotency_key, tokio::runtime::Handle::try_current()) {
                    (Some(key), Ok(handle)) => {
                        let cfg = crate::sync_lock::rw_read(&*self.config).clone();
                        if !cfg.populi_remote_execute_experimental {
                            tracing::warn!(
                                task_id = tid,
                                "populi remote_task_cancel skipped: populi_remote_execute_experimental disabled; remote node was NOT notified of cancellation"
                            );
                        } else {
                            let base = cfg
                                .populi_control_url
                                .as_deref()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string());
                            let recv_s = cfg
                                .populi_remote_execute_receiver_agent
                                .as_deref()
                                .map(str::trim)
                                .filter(|s| !s.is_empty());
                            match (base, recv_s) {
                                (Some(base), Some(recv_s)) => match recv_s.parse::<u64>() {
                                    Ok(recv_id) => {
                                        let send_id = cfg
                                            .populi_remote_execute_sender_agent
                                            .as_deref()
                                            .map(str::trim)
                                            .filter(|s| !s.is_empty())
                                            .and_then(|s| s.parse::<u64>().ok())
                                            .unwrap_or(1);
                                        let cancel = crate::a2a::RemoteTaskCancel {
                                            idempotency_key: key,
                                            task_id: tid,
                                            reason: Some("orchestrator_cancel".to_string()),
                                        };
                                        let timeout_ms = cfg.populi_http_timeout_ms.max(500);
                                        let lease_id =
                                            delegate.as_ref().and_then(|d| d.lease_id.clone());
                                        let claimer_node_id = delegate
                                            .as_ref()
                                            .and_then(|d| d.claimer_node_id.clone());
                                        handle.spawn(async move {
                                            let client =
                                                vox_populi::http_client::PopuliHttpClient::new_with_timeout(
                                                    &base,
                                                    std::time::Duration::from_millis(timeout_ms),
                                                )
                                                .with_env_deliver_token();
                                            match crate::a2a::relay_remote_task_cancel(
                                                &client,
                                                crate::types::AgentId(send_id),
                                                crate::types::AgentId(recv_id),
                                                &cancel,
                                            )
                                            .await
                                            {
                                                Ok(()) => {
                                                    tracing::info!(
                                                        task_id = tid,
                                                        "populi remote_task_cancel relay acknowledged"
                                                    );
                                                }
                                                Err(e) => {
                                                    tracing::warn!(
                                                        error = %e,
                                                        task_id = tid,
                                                        "populi remote_task_cancel relay failed; remote node may not have received the cancellation"
                                                    );
                                                }
                                            }
                                            if let (Some(lease_id), Some(claimer_node_id)) =
                                                (lease_id, claimer_node_id)
                                            {
                                                if let Err(e) = client
                                                    .exec_lease_release(
                                                        &vox_populi::transport::RemoteExecLeaseReleaseRequest {
                                                            lease_id,
                                                            claimer_node_id,
                                                        },
                                                    )
                                                    .await
                                                {
                                                    tracing::warn!(
                                                        error = %e,
                                                        task_id = tid,
                                                        "populi exec_lease_release failed after cancel"
                                                    );
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            task_id = tid,
                                            receiver = %recv_s,
                                            error = %e,
                                            "populi remote_task_cancel skipped: populi_remote_execute_receiver_agent is not a valid agent id; remote node was NOT notified of cancellation"
                                        );
                                    }
                                },
                                (base, recv_s) => {
                                    tracing::warn!(
                                        task_id = tid,
                                        has_control_url = base.is_some(),
                                        has_receiver_agent = recv_s.is_some(),
                                        "populi remote_task_cancel skipped: populi_control_url and/or populi_remote_execute_receiver_agent not configured; remote node was NOT notified of cancellation"
                                    );
                                }
                            }
                        }
                    }
                    (idempotency_key, handle_result) => {
                        tracing::warn!(
                            task_id = tid,
                            has_idempotency_key = idempotency_key.is_some(),
                            has_tokio_runtime = handle_result.is_ok(),
                            "populi remote_task_cancel skipped: missing idempotency key or no tokio runtime available; remote node was NOT notified of cancellation"
                        );
                    }
                }
            }
            return Ok(());
        }

        if let Some(task) = queue.cancel(task_id) {
            let still_claimed_by_queue = |path: &std::path::Path, q: &crate::queue::AgentQueue| {
                q.current_task()
                    .is_some_and(|t| t.write_files().iter().any(|p| p.as_path() == path))
                    || q.tasks()
                        .iter()
                        .any(|t| t.write_files().iter().any(|p| p.as_path() == path))
            };
            for path in task.write_files() {
                if !still_claimed_by_queue(path, &queue) {
                    self.lock_manager.release(path, agent_id);
                    self.affinity_map.release(path);
                    crate::sync_lock::rw_write(&*self.scope_guard).revoke_file(agent_id, path);
                }
            }
            crate::sync_lock::rw_write(&self.task_assignments).remove(&task_id);
            tracing::info!("Cancelled task {} from agent {}", task_id, agent_id);
            emit_task_cancelled(task_id, agent_id, "queue");
            Ok(())
        } else {
            Err(OrchestratorError::TaskNotFound(task_id))
        }
    }

    /// Signal a running local task to abort by setting its interrupt flag.
    ///
    /// The flag is stored in [`Self::interrupt_flags`]. The running
    /// [`crate::runtime::AiTaskProcessor`] must poll this flag during inference to
    /// actually stop early (see `crates/vox-orchestrator/src/interrupt.rs` for the
    /// full wiring plan).  If the task is queued (not yet in-progress), this
    /// delegates to `cancel_task` instead.
    pub fn interrupt_task(&self, task_id: TaskId) -> Result<(), OrchestratorError> {
        // Try to set the interrupt flag if the task has one registered.
        let flag = crate::sync_lock::rw_read(&self.interrupt_flags)
            .get(&task_id)
            .cloned();
        if let Some(f) = flag {
            f.store(true, std::sync::atomic::Ordering::Release);
            tracing::info!("Interrupt flag set for task {}", task_id);
            // Only SIGNAL intent here. The authoritative `orch.task.cancelled`
            // (`local_interrupt`) event is emitted exactly once by
            // `abort_interrupted_task` when the runtime actually stops the task —
            // with the real `agent_id` — avoiding a double-count.
            return Ok(());
        }
        // Fall back to cancel for queued tasks.
        self.cancel_task(task_id)
    }

    /// Release locks and emit telemetry when an in-progress local task aborts
    /// because its interrupt flag was set (see `runtime::AiTaskProcessor`).
    ///
    /// Mirrors the lock-release path of [`Self::cancel_task`]: it revokes
    /// the file lock, affinity, and scope-guard claims held by `agent_id` for the
    /// interrupted task's write files (unless another queued/running task still
    /// claims them), drops the task assignment, and emits the
    /// `orch.task.cancelled` event with `path = "local_interrupt"`.
    pub fn abort_interrupted_task(&self, task_id: TaskId, agent_id: AgentId) {
        // Best-effort lock release: locate the task's write files from the
        // agent's queue (in-progress or queued) and release any no-longer-claimed.
        if let Some(queue_lock) = self.agent_queue(agent_id) {
            let queue = crate::sync_lock::rw_read(&queue_lock);
            let write_files: Vec<std::path::PathBuf> = queue
                .current_task()
                .filter(|t| t.id == task_id)
                .map(|t| t.write_files().into_iter().cloned().collect::<Vec<_>>())
                .or_else(|| {
                    queue
                        .tasks()
                        .iter()
                        .find(|t| t.id == task_id)
                        .map(|t| t.write_files().into_iter().cloned().collect::<Vec<_>>())
                })
                .unwrap_or_default();
            let still_claimed = |path: &std::path::Path| {
                queue.current_task().is_some_and(|t| {
                    t.id != task_id && t.write_files().iter().any(|p| p.as_path() == path)
                }) || queue
                    .tasks()
                    .iter()
                    .any(|t| t.id != task_id && t.write_files().iter().any(|p| p.as_path() == path))
            };
            for path in &write_files {
                if !still_claimed(path) {
                    self.lock_manager.release(path, agent_id);
                    self.affinity_map.release(path);
                    crate::sync_lock::rw_write(&*self.scope_guard).revoke_file(agent_id, path);
                }
            }
        } else {
            // Agent queue already gone (e.g. agent retired mid-abort): file locks
            // can't be located here, but assignment + flag cleanup below still run.
            tracing::warn!(
                "abort_interrupted_task: no queue for agent {} (task {}); skipping file-lock release",
                agent_id,
                task_id
            );
        }
        // NOTE: task_assignments is intentionally NOT removed here. Removing
        // this task's agent_id -> task_id assignment is the job of whichever
        // terminal-state call follows (fail_task / complete_task), which
        // looks the agent_id up by task_id in this same map. Removing it here
        // made fail_task_with_audit's subsequent lookup bail with
        // TaskNotFound and silently no-op for every task that hit this path
        // (see Task B0 in the orchestrator-chat-latency-reliability plan).
        crate::sync_lock::rw_write(&self.interrupt_flags).remove(&task_id);
        tracing::info!(
            "Aborted interrupted task {} on agent {} (local_interrupt)",
            task_id,
            agent_id
        );
        emit_task_cancelled(task_id, agent_id, "local_interrupt");
    }

    /// Reorder a queued task with a new priority.
    pub fn reorder_task(
        &self,
        task_id: TaskId,
        new_priority: TaskPriority,
    ) -> Result<(), OrchestratorError> {
        let agent_id = crate::sync_lock::rw_read(&self.task_assignments)
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;

        let agents = crate::sync_lock::rw_read(&self.agents);
        let queue_lock = agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        let mut queue = crate::sync_lock::rw_write(queue_lock);

        if queue.reorder(task_id, new_priority) {
            tracing::info!(
                "Reordered task {} to priority {:?} on agent {}",
                task_id,
                new_priority,
                agent_id
            );
            Ok(())
        } else {
            Err(OrchestratorError::TaskNotFound(task_id))
        }
    }

    /// Drain all queued tasks from an agent without retiring it.
    pub fn drain_agent(&self, agent_id: AgentId) -> Result<Vec<AgentTask>, OrchestratorError> {
        let agents = crate::sync_lock::rw_read(&self.agents);
        let queue_lock = agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        let mut queue = crate::sync_lock::rw_write(queue_lock);

        let remaining = queue.drain_tasks();
        let mut assignments = crate::sync_lock::rw_write(&self.task_assignments);
        for task in &remaining {
            assignments.remove(&task.id);
        }

        tracing::info!("Drained {} tasks from agent {}", remaining.len(), agent_id);
        Ok(remaining)
    }

    /// Update the heartbeat for an agent and emit an event.
    pub fn heartbeat(&self, agent_id: AgentId, activity: crate::events::AgentActivity) {
        let active_skill = {
            let agents = crate::sync_lock::rw_read(&self.agents);
            agents.get(&agent_id).and_then(|q| {
                crate::sync_lock::rw_read(&**q)
                    .current_task()
                    .and_then(|t| t.active_skill.clone())
            })
        };
        crate::sync_lock::rw_write(&*self.heartbeat_monitor).heartbeat(agent_id, activity);
        self.event_bus
            .emit(crate::events::AgentEventKind::AgentHeartbeat {
                agent_id,
                activity,
                active_skill,
            });
        if activity != crate::events::AgentActivity::Idle {
            self.record_activity();
        }
    }

    /// Pause an agent's dequeue loop.
    pub fn pause_agent(&self, agent_id: AgentId) -> Result<(), OrchestratorError> {
        let agents = crate::sync_lock::rw_read(&self.agents);
        let queue_lock = agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        crate::sync_lock::rw_write(queue_lock).pause();
        tracing::info!("Agent {} paused", agent_id);
        Ok(())
    }

    /// Resume an agent's dequeue loop.
    pub fn resume_agent(&self, agent_id: AgentId) -> Result<(), OrchestratorError> {
        let agents = crate::sync_lock::rw_read(&self.agents);
        let queue_lock = agents
            .get(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        crate::sync_lock::rw_write(queue_lock).resume();
        tracing::info!("Agent {} resumed", agent_id);
        Ok(())
    }
}
