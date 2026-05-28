//! Load-based agent scaling and periodic orchestrator maintenance.
//!
//! - [`Orchestrator::rebalance`] — work-stealing across imbalanced agent queues.
//! - [`Orchestrator::tick`] — one maintenance cycle: stale lock release, load
//!   history, zombie detection, auto-continuation, and urgent-queue rebalance.

use crate::types::AgentId;

impl crate::orchestrator::Orchestrator {
    /// Move one queued task from `from` to `to`, updating locks, affinity, scope, and task assignment.
    /// Returns `true` if the task was enqueued on `to`. If `to` is missing, re-queues on `from` and returns `false`.
    pub(crate) fn transfer_queued_task_between_agents(
        &self,
        task: crate::types::AgentTask,
        from: AgentId,
        to: AgentId,
    ) -> bool {
        if from == to {
            return false;
        }
        let task_id = task.id;
        for path in task.write_files() {
            self.lock_manager.release(path, from);
            self.affinity_map.assign(path, to);
            let mut scope = crate::sync_lock::rw_write(&*self.scope_guard);
            scope.revoke_file(from, path);
            scope.assign_file(to, path.clone());
            let _ = self
                .lock_manager
                .try_acquire(path, to, crate::locks::LockKind::Exclusive);
        }
        let agents = crate::sync_lock::rw_read(&*self.agents);
        if let Some(target_lock) = agents.get(&to) {
            crate::sync_lock::rw_write(&**target_lock).enqueue(task);
            crate::sync_lock::rw_write(&*self.task_assignments).insert(task_id, to);
            true
        } else if let Some(source_lock) = agents.get(&from) {
            crate::sync_lock::rw_write(&**source_lock).enqueue(task);
            false
        } else {
            false
        }
    }

    /// Rebalance tasks across agents using work-stealing.
    ///
    /// Moves tasks from overloaded agents to underloaded ones, respecting file
    /// affinity (only moves tasks whose write-files are not locked by a third agent).
    pub fn rebalance(&self) -> usize {
        let agents = crate::sync_lock::rw_read(&*self.agents);
        let loads: Vec<(AgentId, f64)> = agents
            .iter()
            .map(|(id, queue_lock)| {
                let queue = crate::sync_lock::rw_read(&**queue_lock);
                (*id, queue.weighted_load())
            })
            .collect();

        if loads.len() < 2 {
            return 0;
        }

        let total_load: f64 = loads.iter().map(|(_, l)| l).sum();
        let avg = total_load / loads.len() as f64;
        let mut moved = 0;

        let overloaded: Vec<AgentId> = loads
            .iter()
            .filter(|(_, l)| *l > avg + 2.0)
            .map(|(id, _)| *id)
            .collect();
        let mut underloaded: Vec<AgentId> = loads
            .iter()
            .filter(|(_, l)| *l < avg)
            .map(|(id, _)| *id)
            .collect();

        let config = crate::sync_lock::rw_read(&*self.config);
        if config.cost_preference == crate::config::CostPreference::Economy {
            let models_lock = crate::sync_lock::rw_read(&*self.models);
            underloaded.sort_by(|a, b| {
                let cost_a = models_lock
                    .get_override(a.0)
                    .and_then(|id| models_lock.get(&id))
                    .map(|m| m.cost_per_1k)
                    .unwrap_or(0.003);
                let cost_b = models_lock
                    .get_override(b.0)
                    .and_then(|id| models_lock.get(&id))
                    .map(|m| m.cost_per_1k)
                    .unwrap_or(0.003);
                cost_a
                    .partial_cmp(&cost_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        drop(config);

        for over_id in &overloaded {
            for under_id in &underloaded {
                if let Some(queue_lock) = agents.get(over_id) {
                    let mut queue = crate::sync_lock::rw_write(&**queue_lock);
                    let mut tasks = queue.drain_tasks();
                    tasks.sort_by_key(|t| match t.priority {
                        crate::types::TaskPriority::Background => 0u8,
                        crate::types::TaskPriority::Normal => 1,
                        crate::types::TaskPriority::Urgent => 2,
                    });
                    let steal_idx = tasks.iter().position(|t| {
                        t.write_files().iter().all(|path| {
                            match self.lock_manager.holder(path.as_path()) {
                                None => true,
                                Some((holder, _)) => holder == *over_id,
                            }
                        })
                    });
                    let stolen = steal_idx.map(|i| tasks.remove(i));
                    // Re-enqueue the rest
                    for task in tasks {
                        queue.enqueue(task);
                    }
                    drop(queue);

                    if let Some(task) = stolen {
                        if let Some(target_lock) = agents.get(under_id) {
                            let task_id = task.id;
                            for path in task.write_files() {
                                // Transfer ownership invariants for moved work.
                                self.lock_manager.release(path, *over_id);
                                self.affinity_map.assign(path, *under_id);
                                let mut scope = crate::sync_lock::rw_write(&*self.scope_guard);
                                scope.revoke_file(*over_id, path);
                                scope.assign_file(*under_id, path.clone());
                                let _ = self.lock_manager.try_acquire(
                                    path,
                                    *under_id,
                                    crate::locks::LockKind::Exclusive,
                                );
                            }
                            crate::sync_lock::rw_write(&**target_lock).enqueue(task);
                            crate::sync_lock::rw_write(&*self.task_assignments)
                                .insert(task_id, *under_id);
                            moved += 1;
                        } else if let Some(source_lock) = agents.get(over_id) {
                            // Fail-closed: if target queue vanished, put task back on source.
                            crate::sync_lock::rw_write(&**source_lock).enqueue(task);
                        }
                    }
                }
            }
        }

        if moved > 0 {
            tracing::info!("Rebalanced: moved {} tasks", moved);
            *crate::sync_lock::rw_write(&*self.last_rebalance_at) = Some(std::time::Instant::now());
            crate::sync_lock::rw_write(&*self.oplog).record(
                AgentId(0),
                crate::oplog::OperationKind::Rebalance,
                format!("Rebalanced {} tasks", moved),
                None,
                None,
                None,
                None,
                None,
                None,
            );
        }
        moved
    }

    /// Run one periodic maintenance cycle.
    ///
    /// - Refreshes system metrics (when `system-metrics` feature is on).
    /// - Force-releases stale locks older than `config.lock_timeout_ms`.
    /// - Records current weighted load for predictive scaling history.
    /// - Checks heartbeats and retires zombie dynamic agents.
    /// - Issues auto-continuation tasks for idle agents.
    /// - Triggers urgent-queue rebalance when a single agent exceeds the threshold.
    /// - Runs persistence degradation outbox prune/retry accounting.
    pub async fn tick(&self) {
        #[cfg(feature = "system-metrics")]
        {
            let mut sys = crate::sync_lock::rw_write(&*self.sys);
            sys.refresh_cpu_all();
            sys.refresh_memory();
        }

        let (
            timeout,
            auto_continue,
            urgent_threshold,
            cooldown_ms,
            task_timeout_ms,
            idle_timeout_ms,
            scaling_lookback,
        ) = {
            let config = crate::sync_lock::rw_read(&*self.config);
            (
                config.lock_timeout_ms as u128,
                config.auto_continue_enabled,
                config.urgent_rebalance_threshold,
                config.scaling_cooldown_ms,
                config.task_timeout_ms,
                config.idle_timeout_ms,
                config.scaling_lookback_ticks,
            )
        };

        let released = self.lock_manager.force_release_stale(timeout);
        if released > 0 {
            tracing::warn!(
                "Tick: forcefully released {} stale orphaned lock(s) older than {}ms",
                released,
                timeout
            );
        }

        let current_load = self.status().total_weighted_load;
        {
            let mut history = crate::sync_lock::rw_write(&*self.load_history);
            history.push_back(current_load);
            if history.len() > scaling_lookback {
                history.pop_front();
            }
        }

        // Global Idle Check
        let now_ms = crate::types::now_unix_ms();
        let global_idle_ms = now_ms.saturating_sub(self.last_activity_ms());
        if global_idle_ms >= idle_timeout_ms {
            self.event_bus
                .emit(crate::events::AgentEventKind::OrchestratorIdle {
                    idle_ms: global_idle_ms,
                });
            // We only log once at the threshold to avoid spamming every tick
            if global_idle_ms < idle_timeout_ms + 10_000 {
                tracing::info!("Orchestrator has been idle for {}ms", global_idle_ms);
            }
        }

        // Task Expiration Check
        let task_timeout = std::time::Duration::from_millis(task_timeout_ms);
        {
            let agents = crate::sync_lock::rw_read(&*self.agents);
            for (&agent_id, queue_lock) in agents.iter() {
                let mut queue = crate::sync_lock::rw_write(&**queue_lock);
                let expired = queue.drain_timed_out(task_timeout);
                let has_remaining_writer =
                    |path: &std::path::Path, queue: &crate::queue::AgentQueue| -> bool {
                        queue
                            .current_task()
                            .is_some_and(|t| t.write_files().iter().any(|p| p.as_path() == path))
                            || queue
                                .tasks()
                                .iter()
                                .any(|t| t.write_files().iter().any(|p| p.as_path() == path))
                    };
                for task in expired {
                    let age = now_ms.saturating_sub(task.created_at_ms);
                    crate::sync_lock::rw_write(&*self.task_assignments).remove(&task.id);
                    for path in task.write_files() {
                        if !has_remaining_writer(path, &queue) {
                            self.lock_manager.release(path, agent_id);
                            self.affinity_map.release(path);
                            crate::sync_lock::rw_write(&*self.scope_guard)
                                .revoke_file(agent_id, path);
                        }
                    }
                    self.event_bus
                        .emit(crate::events::AgentEventKind::TaskExpired {
                            task_id: task.id,
                            agent_id,
                            age_ms: age,
                        });
                    tracing::warn!(
                        "Task {} on agent {} expired (age: {}ms)",
                        task.id,
                        agent_id,
                        age
                    );
                }
            }
        }

        let stale_ids =
            crate::sync_lock::rw_write(&*self.heartbeat_monitor).check_stale(&self.event_bus);
        for (id, level) in stale_ids {
            let is_dynamic = crate::sync_lock::rw_read(&*self.dynamic_agents).contains(&id);
            if is_dynamic {
                tracing::warn!(
                    "Tick: retiring zombie dynamic agent {} (level: {})",
                    id,
                    level
                );
                let _ = self.retire_agent(id);
            } else {
                tracing::error!(
                    "Tick: reserved agent {} is unresponsive at level {}! Immediate attention required.",
                    id,
                    level
                );
            }
        }

        if auto_continue {
            let stall_threshold_ms = (task_timeout_ms / 2).max(60_000);
            let active_agents: Vec<(AgentId, usize, bool, u64, Option<String>)> = {
                let agents = crate::sync_lock::rw_read(&*self.agents);
                agents
                    .iter()
                    .map(|(id, queue_lock)| {
                        let queue = crate::sync_lock::rw_read(&**queue_lock);
                        let queued = queue.len();
                        let has_in_progress = queue.has_in_progress();
                        let pending_total = queued + queue.in_progress_count();
                        let stalled_in_progress_ms = queue
                            .current_task()
                            .and_then(|t| t.started_at_ms)
                            .map(|started| now_ms.saturating_sub(started))
                            .filter(|elapsed| has_in_progress && *elapsed >= stall_threshold_ms)
                            .unwrap_or(0);
                        let active_skill = queue.current_task().and_then(|t| t.active_skill.clone());
                        (*id, pending_total, has_in_progress, stalled_in_progress_ms, active_skill)
                    })
                    .collect()
            };
            let intents = crate::sync_lock::rw_write(&*self.monitor)
                .check_idle_agents(&active_agents, &self.event_bus);

            for (agent_id, prompt) in intents {
                let agent_name = {
                    let agents = crate::sync_lock::rw_read(&*self.agents);
                    agents
                        .get(&agent_id)
                        .map(|q| crate::sync_lock::rw_read(&**q).name.clone())
                        .unwrap_or_default()
                };
                let _ = self
                    .submit_task_with_agent(
                        format!("[Auto-Continuation] {}", prompt),
                        vec![],
                        Some(crate::types::TaskPriority::Background),
                        Some(agent_name),
                        None,
                        None,
                        None,
                        None,
                    )
                    .await;
            }
        }

        if urgent_threshold > 0 {
            let agents_count = crate::sync_lock::rw_read(&*self.agents).len();
            if agents_count >= 2 {
                let can_rebalance = {
                    let last = crate::sync_lock::rw_read(&*self.last_rebalance_at);
                    last.map(|t| t.elapsed().as_millis() >= cooldown_ms as u128)
                        .unwrap_or(true)
                };

                if can_rebalance {
                    let overloaded_urgent: Vec<(AgentId, usize)> = {
                        let agents = crate::sync_lock::rw_read(&*self.agents);
                        agents
                            .iter()
                            .map(|(id, q_lock)| {
                                (
                                    *id,
                                    crate::sync_lock::rw_read(&**q_lock)
                                        .depth_by_priority(crate::types::TaskPriority::Urgent),
                                )
                            })
                            .filter(|(_, depth)| *depth > urgent_threshold)
                            .collect()
                    };

                    if !overloaded_urgent.is_empty() {
                        for (agent_id, depth) in &overloaded_urgent {
                            tracing::warn!(
                                "Tick: agent {} has {} urgent tasks (threshold {}), triggering urgent rebalance",
                                agent_id,
                                depth,
                                urgent_threshold
                            );
                        }
                        let moved = self.rebalance();
                        if moved > 0 {
                            self.event_bus.emit(
                                crate::events::AgentEventKind::UrgentRebalanceTriggered { moved },
                            );
                        }
                    }
                }
            }
        }

        // 7. Persistence degradation outbox: prune / retry accounting
        self.tick_persistence_outbox_lifecycle().await;

        // 7b. Populi remote lease watchdog: authoritative recovery
        self.tick_populi_remote_lease_watchdog();

        // 8. News Syndication check
        #[cfg(feature = "news-publish")]
        if let Err(e) = crate::services::news::NewsService::tick(self).await {
            tracing::error!("NewsService tick failed: {}", e);
        }
    }
}
