use crate::orchestrator::OrchestratorError;
use crate::types::{AgentId, AgentTask, TaskId, TaskStatus};

impl crate::orchestrator::Orchestrator {
    /// Flag a task as "Suspect" by the user, triggering a resolution loop.
    pub fn doubt_task(
        &self,
        task_id: TaskId,
        reason: Option<String>,
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

        // If in progress, take it out
        let mut task = if let Some(t) = queue.take_in_progress_if(task_id) {
            t
        } else {
            // Check if it's in the queue
            queue
                .take_queued(task_id)
                .ok_or(OrchestratorError::TaskNotFound(task_id))?
        };

        // Update context envelope to explicitly force Verification mode for the agent's prompt
        if let Some(ref sid) = task.session_id {
            let key = crate::socrates::session_context_envelope_key(sid);
            let env_opt = crate::sync_lock::rw_read(&*self.context_store).get(&key);
            if let Some(env_json) = env_opt {
                if let Ok(mut env) = serde_json::from_str::<crate::ContextEnvelope>(&env_json) {
                    env.operating_mode =
                        Some(crate::context_envelope::OperatingMode::Verification {
                            reason: reason.clone(),
                        });
                    if let Ok(new_json) = serde_json::to_string(&env) {
                        crate::sync_lock::rw_write(&*self.context_store)
                            .set(agent_id, key, new_json, 3600);
                    }
                }
            }
        }

        // Change the role to explicitly focus on verification
        task.execution_role = Some(crate::reconstruction::AgentExecutionRole::Verifier);
        task.status = TaskStatus::Doubted(reason.clone());

        tracing::info!(
            target: "vox.orchestrator.tasks",
            %task_id,
            %agent_id,
            reason = ?reason,
            "Task doubted: enforcing rigid Second Pass compilation/validation compliance."
        );

        // Emit event for hud and ludus
        self.event_bus
            .emit(crate::events::AgentEventKind::TaskDoubted {
                task_id,
                agent_id,
                reason: reason.clone(),
            });

        self.bulletin
            .publish(crate::types::AgentMessage::TaskDoubted {
                task_id,
                agent_id,
                reason: reason.clone(),
            });

        // The Implementation Plan requires that we re-enqueue it and let explicitly-enforced
        // terminal checks clear the Verification mode before it can be marked complete.
        queue.enqueue(task);

        tracing::info!(
            "Task {} doubted by human for agent {}: {:?}",
            task_id,
            agent_id,
            reason
        );
        Ok(())
    }

    /// Dequeue a task in Doubted status for a specific agent.
    pub fn dequeue_doubted(&self, agent_id: AgentId) -> Option<AgentTask> {
        let agents = crate::sync_lock::rw_read(&self.agents);
        let queue_lock = agents.get(&agent_id)?;
        let mut queue = crate::sync_lock::rw_write(queue_lock);
        queue.dequeue_doubted()
    }

    /// Overrule a human doubt or agent failure, force-validating the result.
    pub fn overrule_task(
        &self,
        task_id: TaskId,
        reason: Option<String>,
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

        // Find the task in the queue or in progress
        let mut task = if let Some(t) = queue.take_in_progress_if(task_id) {
            t
        } else if let Some(t) = queue.take_queued(task_id) {
            t
        } else {
            return Err(OrchestratorError::TaskNotFound(task_id));
        };

        // Clear verification modes in socrates context if present
        if let Some(ref sid) = task.session_id {
            let key = crate::socrates::session_context_envelope_key(sid);
            let store = crate::sync_lock::rw_write(&*self.context_store);
            if let Some(env_json) = store.get(&key) {
                if let Ok(mut env) = serde_json::from_str::<crate::ContextEnvelope>(&env_json) {
                    env.operating_mode = None; // Clear Verification mode
                    if let Ok(new_json) = serde_json::to_string(&env) {
                        store.set(agent_id, key, new_json, 3600);
                    }
                }
            }
        }

        task.status = TaskStatus::Completed;
        task.audit_report = Some(format!("OVERRULED: {}", reason.unwrap_or_else(|| "No reason provided".into())));
        
        tracing::info!(
            target: "vox.orchestrator.tasks",
            %task_id,
            %agent_id,
            "Task overruled by human: moving to Completed status."
        );

        // Since it's completed, we don't re-enqueue. We record it as a completion attestation event.
        self.event_bus
            .emit(crate::events::AgentEventKind::TaskCompleted {
                task_id,
                agent_id,
                session_id: task.session_id.clone(),
                audit_report: task.audit_report.clone(),
            });

        crate::sync_lock::rw_write(&self.task_assignments).remove(&task_id);
        
        Ok(())
    }
}
