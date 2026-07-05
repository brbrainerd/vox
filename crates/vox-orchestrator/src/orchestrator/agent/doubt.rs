use crate::feedback::FeedbackId;
use crate::orchestrator::OrchestratorError;
use crate::types::{AgentId, AgentTask, TaskId, TaskStatus};

/// State-mutation result of [`Orchestrator::doubt_task`], carrying everything
/// needed to durably record the transition *before* broadcasting it (T1.2:
/// two-tier append-before-broadcast). `doubt_task` itself stays sync (it holds
/// a queue write-lock across the mutation) so it cannot `.await` the durable
/// oplog write; callers are expected to call `record_operation` with this
/// outcome's fields and only then call [`Orchestrator::emit_doubt_events`].
#[derive(Debug, Clone)]
pub struct DoubtOutcome {
    pub agent_id: AgentId,
    pub feedback_id: FeedbackId,
    pub reason: Option<String>,
}

/// State-mutation result of [`Orchestrator::overrule_task`], carrying everything
/// needed to durably record the transition *before* broadcasting it (T1.2:
/// two-tier append-before-broadcast). `overrule_task` itself stays sync (it holds
/// a queue write-lock across the mutation) so it cannot `.await` the durable
/// oplog write; callers are expected to call `record_operation` with this
/// outcome's fields and only then call [`Orchestrator::emit_overrule_events`].
#[derive(Debug, Clone)]
pub struct OverruleOutcome {
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub session_id: Option<String>,
    pub audit_report: Option<String>,
}

impl crate::orchestrator::Orchestrator {
    /// Flag a task as "Suspect" by the user, triggering a resolution loop.
    ///
    /// Does **not** broadcast `TaskDoubted`/`FeedbackRequested` on the event bus —
    /// callers must durably record the transition first via `record_operation`,
    /// then call [`Orchestrator::emit_doubt_events`] with the returned
    /// [`DoubtOutcome`] (T1.2: Tier-A durable-before-broadcast).
    pub fn doubt_task(
        &self,
        task_id: TaskId,
        reason: Option<String>,
    ) -> Result<DoubtOutcome, OrchestratorError> {
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

        self.bulletin
            .publish(crate::types::AgentMessage::TaskDoubted {
                task_id,
                agent_id,
                reason: reason.clone(),
            });

        // Surface the doubt in the unified "Needs You" feedback inbox so the user can
        // Overrule (force-validate) or let the Verifier pass run. Non-gating (gates: [])
        // because the task is re-enqueued in place and the agent keeps working;
        // `doubted_task_id` carries the Overrule target for `resolve_feedback`.
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let prompt = match reason.as_deref() {
            Some(r) if !r.is_empty() => format!("Agent flagged this task as suspect: {r}"),
            _ => "Agent flagged this task as suspect and is re-verifying.".to_string(),
        };
        let fid = self.feedback().register(
            crate::feedback::FeedbackKind::Doubt,
            prompt,
            Vec::new(),
            Vec::new(),
            Some(task_id),
            0.0,
            0,
            crate::feedback::Surface::NeedsYou,
            task.session_id.clone(),
            Some(agent_id),
            ts,
            None,
        );

        // The Implementation Plan requires that we re-enqueue it and let explicitly-enforced
        // terminal checks clear the Verification mode before it can be marked complete.
        queue.enqueue(task);

        tracing::info!(
            "Task {} doubted by human for agent {}: {:?}",
            task_id,
            agent_id,
            reason
        );
        Ok(DoubtOutcome {
            agent_id,
            feedback_id: fid,
            reason,
        })
    }

    /// Broadcast the `TaskDoubted` + `FeedbackRequested` bus events for a
    /// [`DoubtOutcome`] previously returned by [`Orchestrator::doubt_task`].
    /// Callers MUST call this only *after* durably recording the transition
    /// (via `record_operation`) — see the T1.2 tier-A contract on
    /// [`crate::events::is_tier_a`].
    pub fn emit_doubt_events(&self, task_id: TaskId, outcome: &DoubtOutcome) {
        self.event_bus
            .emit(crate::events::AgentEventKind::TaskDoubted {
                task_id,
                agent_id: outcome.agent_id,
                reason: outcome.reason.clone(),
            });
        self.event_bus
            .emit(crate::events::AgentEventKind::FeedbackRequested {
                feedback_id: outcome.feedback_id.0.clone(),
                kind: "doubt".into(),
                gates: Vec::new(),
                surface: "needs_you".into(),
            });
    }

    /// Dequeue a task in Doubted status for a specific agent.
    pub fn dequeue_doubted(&self, agent_id: AgentId) -> Option<AgentTask> {
        let agents = crate::sync_lock::rw_read(&self.agents);
        let queue_lock = agents.get(&agent_id)?;
        let mut queue = crate::sync_lock::rw_write(queue_lock);
        queue.dequeue_doubted()
    }

    /// Overrule a human doubt or agent failure, force-validating the result.
    ///
    /// Does **not** broadcast `TaskCompleted` on the event bus — callers must
    /// durably record the transition first via `record_operation`, then call
    /// [`Orchestrator::emit_overrule_events`] with the returned
    /// [`OverruleOutcome`] (T1.2: Tier-A durable-before-broadcast).
    pub fn overrule_task(
        &self,
        task_id: TaskId,
        reason: Option<String>,
    ) -> Result<OverruleOutcome, OrchestratorError> {
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
        task.audit_report = Some(format!(
            "OVERRULED: {}",
            reason.unwrap_or_else(|| "No reason provided".into())
        ));

        tracing::info!(
            target: "vox.orchestrator.tasks",
            %task_id,
            %agent_id,
            "Task overruled by human: moving to Completed status."
        );

        // Since it's completed, we don't re-enqueue. Callers durably record this as a
        // completion attestation (via `record_operation`) before broadcasting it with
        // `emit_overrule_events` — see the T1.2 tier-A contract on
        // [`crate::events::is_tier_a`].
        let session_id = task.session_id.clone();
        let audit_report = task.audit_report.clone();

        crate::sync_lock::rw_write(&self.task_assignments).remove(&task_id);

        Ok(OverruleOutcome {
            task_id,
            agent_id,
            session_id,
            audit_report,
        })
    }

    /// Broadcast the `TaskCompleted` bus event for an [`OverruleOutcome`]
    /// previously returned by [`Orchestrator::overrule_task`]. Callers MUST
    /// call this only *after* durably recording the transition (via
    /// `record_operation`) — see the T1.2 tier-A contract on
    /// [`crate::events::is_tier_a`].
    pub fn emit_overrule_events(&self, outcome: &OverruleOutcome) {
        self.event_bus
            .emit(crate::events::AgentEventKind::TaskCompleted {
                task_id: outcome.task_id,
                agent_id: outcome.agent_id,
                session_id: outcome.session_id.clone(),
                audit_report: outcome.audit_report.clone(),
            });
    }
}
