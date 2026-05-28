use crate::orchestrator::OrchestratorError;
use crate::services::MessageGateway;
use crate::types::{AgentId, AgentTask, TaskId, TaskPriority, TaskStatus};

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
            #[cfg(feature = "populi-transport")]
            if let (Some(key), Ok(handle)) =
                (idempotency_key, tokio::runtime::Handle::try_current())
            {
                let cfg = crate::sync_lock::rw_read(&*self.config).clone();
                if cfg.populi_remote_execute_experimental {
                    if let (Some(base), Some(recv_s)) = (
                        cfg.populi_control_url
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string()),
                        cfg.populi_remote_execute_receiver_agent
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty()),
                    ) {
                        if let Ok(recv_id) = recv_s.parse::<u64>() {
                            let send_id = cfg
                                .populi_remote_execute_sender_agent
                                .as_deref()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .and_then(|s| s.parse::<u64>().ok())
                                .unwrap_or(1);
                            let cancel = crate::a2a::RemoteTaskCancel {
                                idempotency_key: key,
                                task_id: task_id.0,
                                reason: Some("orchestrator_cancel".to_string()),
                            };
                            let timeout_ms = cfg.populi_http_timeout_ms.max(500);
                            let tid = task_id.0;
                            let lease_id = delegate.as_ref().and_then(|d| d.lease_id.clone());
                            let claimer_node_id =
                                delegate.as_ref().and_then(|d| d.claimer_node_id.clone());
                            handle.spawn(async move {
                                let client =
                                    vox_populi::http_client::PopuliHttpClient::new_with_timeout(
                                        &base,
                                        std::time::Duration::from_millis(timeout_ms),
                                    )
                                    .with_env_deliver_token();
                                if let Err(e) = crate::a2a::relay_remote_task_cancel(
                                    &client,
                                    crate::types::AgentId(send_id),
                                    crate::types::AgentId(recv_id),
                                    &cancel,
                                )
                                .await
                                {
                                    tracing::debug!(
                                        error = %e,
                                        task_id = tid,
                                        "populi remote_task_cancel relay failed (best-effort)"
                                    );
                                }
                                if let (Some(lease_id), Some(claimer_node_id)) =
                                    (lease_id, claimer_node_id)
                                {
                                    let _ = client
                                        .exec_lease_release(
                                            &vox_populi::transport::RemoteExecLeaseReleaseRequest {
                                                lease_id,
                                                claimer_node_id,
                                            },
                                        )
                                        .await;
                                }
                            });
                        }
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
            Ok(())
        } else {
            Err(OrchestratorError::TaskNotFound(task_id))
        }
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
            agents
                .get(&agent_id)
                .and_then(|q| {
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
