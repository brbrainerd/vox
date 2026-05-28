//! Re-run routing for queued tasks after Populi federation capacity drops.

use std::collections::VecDeque;

impl super::Orchestrator {
    /// Call `Self::resolve_route` for each pending task (not [`crate::types::AgentTask::populi_remote_delegate`])
    /// and move work when the resolved agent changed. Skips tasks delegated to remote Populi execution.
    pub async fn replay_queued_routes_after_populi_schedulable_drop(&self) -> usize {
        let agent_ids: Vec<crate::types::AgentId> = crate::sync_lock::rw_read(&*self.agents)
            .keys()
            .copied()
            .collect();
        let mut moved: usize = 0;
        for from_id in agent_ids {
            let pending: VecDeque<_> = {
                let agents = crate::sync_lock::rw_read(&*self.agents);
                let Some(lock) = agents.get(&from_id) else {
                    continue;
                };
                crate::sync_lock::rw_write(&**lock).take_pending_tasks()
            };
            if pending.is_empty() {
                continue;
            }
            let mut keep = VecDeque::new();
            for task in pending {
                if task.populi_remote_delegate.is_some() {
                    keep.push_back(task);
                    continue;
                }
                let target = match self
                    .resolve_route(
                        &task.file_manifest,
                        None,
                        task.capability_requirements.as_ref(),
                        Some(task.description.as_str()),
                        Some(task.id),
                    )
                    .await
                {
                    Ok(id) => id,
                    Err(e) => {
                        tracing::debug!(
                            task_id = task.id.0,
                            error = %e,
                            "populi route replay: resolve_route failed; keeping task on agent"
                        );
                        keep.push_back(task);
                        continue;
                    }
                };
                if target == from_id {
                    keep.push_back(task);
                    continue;
                }
                let tid = task.id;
                if self.transfer_queued_task_between_agents(task, from_id, target) {
                    moved += 1;
                    tracing::info!(
                        target: "vox.orchestrator.routing",
                        decision = "queued_route_replay_move",
                        task_id = tid.0,
                        from_agent = from_id.0,
                        to_agent = target.0,
                        "populi federation: moved queued task during route replay"
                    );
                }
            }
            let agents = crate::sync_lock::rw_read(&*self.agents);
            let Some(lock) = agents.get(&from_id) else {
                continue;
            };
            crate::sync_lock::rw_write(&**lock).restore_pending_tasks(keep);
        }
        if moved > 0 {
            tracing::info!(
                target: "vox.orchestrator.routing",
                decision = "populi_remote_drop_queued_route_replay",
                tasks_moved = moved,
                "populi federation: replayed routes for queued tasks after schedulable drop"
            );
        }
        moved
    }
}
