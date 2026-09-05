use crate::orchestrator::OrchestratorError;
use crate::services::MessageGateway;
use crate::types::{AgentId, TaskId};

impl crate::orchestrator::Orchestrator {
    /// Spawn a new named agent with specific capability requirements.
    pub fn spawn_agent_with_hints(
        &self,
        name: &str,
        hints: Option<crate::contract::TaskCapabilityHints>,
    ) -> Result<AgentId, OrchestratorError> {
        let config = crate::sync_lock::rw_read(&*self.config);
        if crate::sync_lock::rw_read(&*self.agents).len() >= config.max_agents {
            return Err(OrchestratorError::MaxAgentsReached {
                max: config.max_agents,
            });
        }
        let mut caps = config.default_agent_capabilities.clone();
        drop(config);

        if let Some(h) = hints {
            caps = crate::capability_probe::merge_agent_capabilities(&caps, h);
        }

        let agent_id = self.agent_id_gen.next();
        let mut queue = crate::queue::AgentQueue::new(agent_id, name);
        let probed = crate::capability_probe::probe_host_capabilities();
        queue.capabilities = crate::capability_probe::merge_agent_capabilities(&caps, probed);
        crate::sync_lock::rw_write(&*self.agents)
            .insert(agent_id, std::sync::Arc::new(std::sync::RwLock::new(queue)));
        crate::sync_lock::rw_write(&*self.heartbeat_monitor).register(agent_id);
        MessageGateway::publish_agent_spawned(
            &self.bulletin,
            &self.event_bus,
            agent_id,
            name.to_string(),
        );

        let bm = crate::sync_lock::rw_read(&*self.budget_manager).clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                bm.load_user_configured_budget(agent_id).await;
            });
        }

        tracing::info!("Spawned agent {} (name: {})", agent_id, name);
        Ok(agent_id)
    }

    /// Spawn a new named agent using default capabilities.
    pub fn spawn_agent(&self, name: &str) -> Result<AgentId, OrchestratorError> {
        self.spawn_agent_with_hints(name, None)
    }

    /// Spawn a transient (dynamic) agent, marking it for automatic retirement when idle.
    pub fn spawn_dynamic_agent(&self, name: &str) -> Result<AgentId, OrchestratorError> {
        self.spawn_dynamic_agent_with_parent(name, None, None, None, None, None, None)
    }

    /// Spawn a transient agent with an optional explicit parent binding.
    ///
    /// `chat_session_id`/`origin_turn_id` carry chat-harness delegation lineage
    /// (Phase D Task D1): when the spawn was caused by a `vox_spawn_agent` /
    /// `vox_submit_task` tool call inside a chat turn, these identify which chat
    /// session and which provider tool-call id originated it. They are stored on
    /// the in-memory [`crate::topology::AgentDelegationBinding`] AND persisted via
    /// [`Self::record_lineage_event`] (`kind = "task_delegated"`) so the edge
    /// survives a daemon restart even though the in-memory binding does not —
    /// `chat_session_id` becomes the lineage row's `session_id` column,
    /// `origin_turn_id` rides in the JSON payload.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_dynamic_agent_with_parent(
        &self,
        name: &str,
        parent_agent_id: Option<AgentId>,
        reason: Option<&str>,
        source_task_id: Option<TaskId>,
        hints: Option<crate::contract::TaskCapabilityHints>,
        chat_session_id: Option<String>,
        origin_turn_id: Option<String>,
    ) -> Result<AgentId, OrchestratorError> {
        if let Some(parent) = parent_agent_id {
            let parent_exists = crate::sync_lock::rw_read(&*self.agents).contains_key(&parent);
            if !parent_exists {
                return Err(OrchestratorError::DelegationParentNotFound(parent));
            }
        }
        let agent_id = self.spawn_agent_with_hints(name, hints)?;
        crate::sync_lock::rw_write(&*self.dynamic_agents).insert(agent_id);
        // Phase D Task D3: reuse the same `agent_session_id` link the primary
        // chat agent uses (`map_agent_session`, set via `queue.set_agent_session`)
        // so a delegated agent is also discoverable by chat session — this is
        // what lets a GUI-side `agentsForSession` filter find it instead of
        // guessing from the fleet-wide agent list.
        if let Some(ref session) = chat_session_id {
            if let Err(e) = self.map_agent_session(agent_id, session.clone()) {
                tracing::debug!(
                    error = %e,
                    "failed to link chat_session_id to newly spawned agent"
                );
            }
        }
        let spawn_reason = reason
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("dynamic_spawn")
            .to_string();
        crate::sync_lock::rw_write(&*self.dynamic_spawn_context).insert(
            agent_id,
            crate::topology::DynamicSpawnContext {
                source_task_id,
                reason: spawn_reason.clone(),
            },
        );
        if let Some(parent) = parent_agent_id {
            let binding = crate::topology::AgentDelegationBinding {
                parent_agent_id: parent,
                source_task_id,
                reason: spawn_reason.clone(),
                chat_session_id: chat_session_id.clone(),
                origin_turn_id: origin_turn_id.clone(),
            };
            crate::sync_lock::rw_write(&*self.agent_delegations).insert(agent_id, binding);

            self.record_lineage_event(
                "task_delegated",
                source_task_id,
                Some(agent_id),
                chat_session_id.clone(),
                None,
                None,
                None,
                Some(serde_json::json!({
                    "reason": spawn_reason,
                    "is_dynamic": true,
                    "origin_turn_id": origin_turn_id,
                })),
            );
        }
        tracing::info!("Agent {} marked as dynamic", agent_id,);
        Ok(agent_id)
    }
}
