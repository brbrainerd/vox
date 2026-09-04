use crate::orchestrator::OrchestratorError;
use crate::types::AgentId;

impl crate::orchestrator::Orchestrator {
    /// Accept a structured handoff payload from another agent, spawning a target agent if needed.
    pub fn accept_handoff(
        &self,
        payload: crate::handoff::HandoffPayload,
    ) -> Result<AgentId, OrchestratorError> {
        let from_agent = payload.from_agent;
        if let Err(err) = crate::handoff::validate_handoff_invariants(&payload) {
            let reason = err.to_string();
            self.event_bus
                .emit(crate::events::AgentEventKind::AgentHandoffRejected {
                    from: from_agent,
                    reason: reason.clone(),
                });
            return Err(OrchestratorError::HandoffInvariant(reason));
        }
        let (has_context_envelope, has_harness_spec, handoff_session_id, handoff_thread_id) =
            crate::handoff::handoff_context_event_metadata(&payload);
        if let Some((_, context_json)) = payload
            .metadata
            .iter()
            .rev()
            .find(|(k, _)| k == crate::handoff::CONTEXT_ENVELOPE_JSON_METADATA_KEY)
        {
            if let Ok(env) = serde_json::from_str::<crate::ContextEnvelope>(context_json) {
                let cfg = crate::sync_lock::rw_read(&*self.config).clone();
                let repo = crate::lineage::repository_id();
                if let Some(session_id) = env
                    .subject
                    .session_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    let expectations = crate::context_lifecycle::ContextIngestExpectations {
                        repository_id: repo.as_str(),
                        session_id: Some(session_id),
                    };
                    if let Err(e) = crate::context_lifecycle::apply_context_lifecycle_policy(
                        &cfg,
                        &env,
                        expectations,
                        crate::context_lifecycle::ContextIngestSource::InternalHandoffAccept,
                    ) {
                        return Err(OrchestratorError::HandoffInvariant(e));
                    }
                    let key = crate::socrates::session_context_envelope_key(session_id);
                    let existing = crate::sync_lock::rw_read(&*self.context_store).get(&key);
                    let merged =
                        match crate::context_lifecycle::merge_context_envelope_for_session_store(
                            existing.as_deref(),
                            &env,
                            cfg.context_lifecycle_shadow,
                        ) {
                            Ok(m) => m,
                            Err(e) => return Err(OrchestratorError::HandoffInvariant(e)),
                        };
                    if let Err(e) = crate::context_lifecycle::apply_context_lifecycle_policy(
                        &cfg,
                        &merged,
                        expectations,
                        crate::context_lifecycle::ContextIngestSource::SessionStoreWrite,
                    ) {
                        return Err(OrchestratorError::HandoffInvariant(e));
                    }
                    let merged_json = match serde_json::to_string(&merged) {
                        Ok(s) => s,
                        Err(e) => {
                            return Err(OrchestratorError::HandoffInvariant(e.to_string()));
                        }
                    };
                    crate::sync_lock::rw_write(&*self.context_store).set(
                        from_agent,
                        key,
                        merged_json,
                        3600,
                    );
                }
            }
        }

        // Check for staleness/expiration
        let now = crate::types::now_unix_ms();
        let age_ms = now.saturating_sub(payload.created_at);
        let timeout = payload.timeout_ms.unwrap_or(3_600_000); // 1 hour default

        if age_ms > timeout {
            let reason = format!(
                "Handoff from {} is stale (age: {}s, timeout: {}s)",
                from_agent,
                age_ms / 1000,
                timeout / 1000
            );
            self.event_bus
                .emit(crate::events::AgentEventKind::AgentHandoffRejected {
                    from: from_agent,
                    reason: reason.clone(),
                });
            tracing::warn!("{}", reason);
            return Err(OrchestratorError::StaleHandoff {
                agent_id: from_agent,
                age_ms,
                timeout_ms: timeout,
            });
        }

        let hints = if let Some(ref manifest) = payload.attachment_manifest
            && manifest.has_vision_vitals()
        {
            Some(crate::contract::TaskCapabilityHints {
                visus_eligible: true,
                multi_modal: true,
                ..Default::default()
            })
        } else {
            None
        };

        let target_id = if let Some(id) = payload.to_agent {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            if agents.contains_key(&id) {
                id
            } else {
                drop(agents);
                match self.spawn_dynamic_agent_with_parent(
                    &format!("ResumingAgent-{}", id.0),
                    None,
                    Some("handoff_resume"),
                    None,
                    hints,
                    None,
                    None,
                ) {
                    Ok(new_id) => new_id,
                    Err(e) => {
                        self.event_bus
                            .emit(crate::events::AgentEventKind::AgentHandoffRejected {
                                from: from_agent,
                                reason: format!("Spawn failed: {}", e),
                            });
                        return Err(e);
                    }
                }
            }
        } else {
            match self.spawn_dynamic_agent_with_parent(
                "AdaptiveResumer",
                None,
                Some("handoff_resume"),
                None,
                hints,
                None,
                None,
            ) {
                Ok(new_id) => new_id,
                Err(e) => {
                    self.event_bus
                        .emit(crate::events::AgentEventKind::AgentHandoffRejected {
                            from: from_agent,
                            reason: format!("Spawn failed: {}", e),
                        });
                    return Err(e);
                }
            }
        };

        // Task migration: Move pending tasks from source agent to target agent and increment handoff_count.
        let mut moved_tasks = Vec::new();
        if let Some(from_queue_lock) = self.agent_queue(from_agent) {
            let mut from_queue = crate::sync_lock::rw_write(&from_queue_lock);
            for task_id in &payload.pending_tasks {
                if let Some(mut task) = from_queue.take_queued(*task_id) {
                    task.handoff_count += 1;
                    moved_tasks.push(task);
                } else if let Some(mut task) = from_queue.take_in_progress_if(*task_id) {
                    task.handoff_count += 1;
                    moved_tasks.push(task);
                }
            }
        }

        if !moved_tasks.is_empty() {
            if let Some(target_queue_lock) = self.agent_queue(target_id) {
                let mut target_queue = crate::sync_lock::rw_write(&target_queue_lock);
                for task in moved_tasks {
                    target_queue.enqueue(task);
                }
            }
        }

        for path in &payload.owned_files {
            self.affinity_map.assign(path, target_id);
            crate::sync_lock::rw_write(&self.scope_guard).assign_file(target_id, path.clone());
            let _ =
                self.lock_manager
                    .try_acquire(path, target_id, crate::locks::LockKind::Exclusive);
        }

        let resumed_ids: Vec<crate::types::TaskId> = payload.pending_tasks.clone();
        if target_id != from_agent {
            crate::sync_lock::rw_write(&*self.agent_delegations).insert(
                target_id,
                crate::topology::AgentDelegationBinding {
                    parent_agent_id: from_agent,
                    source_task_id: None,
                    reason: "handoff_accept".to_string(),
                    chat_session_id: None,
                    origin_turn_id: None,
                },
            );
            self.record_lineage_event(
                "task_delegated",
                None,
                Some(target_id),
                None,
                None,
                None,
                None,
                Some(serde_json::json!({
                    "reason": "handoff_accept",
                    "from_agent": from_agent
                })),
            );
        }

        self.event_bus
            .emit(crate::events::AgentEventKind::AgentHandoffAccepted {
                agent_id: target_id,
                from: from_agent,
                plan_summary: payload.plan_summary.clone(),
                has_context_envelope,
                has_harness_spec,
                session_id: handoff_session_id,
                thread_id: handoff_thread_id,
            });

        tracing::info!(
            "Agent {} accepted handoff from {} ({} tasks resumed: {:?})",
            target_id,
            from_agent,
            resumed_ids.len(),
            resumed_ids
        );
        Ok(target_id)
    }
}
