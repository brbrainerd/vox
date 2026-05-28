use std::collections::VecDeque;

use crate::types::{AgentId, AgentTask, TaskId, TaskPriority, TaskStatus};

use super::AgentQueue;

impl AgentQueue {
    /// Create a new empty queue for the given agent.
    pub fn new(id: AgentId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            tasks: std::collections::VecDeque::new(),
            in_progress: None,
            completed: Vec::new(),
            paused: false,
            last_active: std::time::Instant::now(),
            agent_session_id: None,
            capabilities: crate::contract::TaskCapabilityHints::default(),
            active_skills: std::collections::HashMap::new(),
            workflow_context: None,
            endpoint_reliability_key: None,
            recent_shard_validation_failures: 0,
            reducer_cooldown_until_ms: None,
        }
    }

    /// Link an AI agent session to this queue.
    pub fn set_agent_session(&mut self, session_id: String) {
        self.agent_session_id = Some(session_id);
    }

    /// Enqueue a task, inserting it in priority order.
    /// Higher priority tasks go before lower priority tasks.
    /// Within the same priority, new tasks go to the end (FIFO).
    pub fn enqueue(&mut self, mut task: AgentTask) {
        // Supervisor Arbitration: block infinite A2A handoff loops
        if task.handoff_count > crate::types::MAX_A2A_BOUNCE {
            tracing::error!(
                task_id = %task.id,
                count = task.handoff_count,
                "Infinite A2A handoff loop detected; failing task"
            );
            task.status = TaskStatus::Failed(format!(
                "Infinite A2A handoff loop detected (max bounce exceeded: {})",
                crate::types::MAX_A2A_BOUNCE
            ));
        }

        // Find the insertion point: higher priority tasks first.
        // Within the same priority, order chronologically by created_at_ms.
        let pos = self
            .tasks
            .iter()
            .position(|t| {
                if t.priority < task.priority {
                    true
                } else if t.priority == task.priority {
                    t.created_at_ms > task.created_at_ms
                } else {
                    false
                }
            })
            .unwrap_or(self.tasks.len());
        self.tasks.insert(pos, task);
        self.last_active = std::time::Instant::now();
    }

    /// Change the priority of a queued task and reorder.
    pub fn reorder(&mut self, task_id: TaskId, new_priority: TaskPriority) -> bool {
        if let Some(pos) = self.tasks.iter().position(|t| t.id == task_id) {
            let mut task = self.tasks.remove(pos).expect("position was valid");
            task.priority = new_priority;
            self.enqueue(task);
            true
        } else {
            false
        }
    }

    /// Cancel a queued task and return it.
    pub fn cancel(&mut self, task_id: TaskId) -> Option<AgentTask> {
        if let Some(pos) = self.tasks.iter().position(|t| t.id == task_id) {
            self.tasks.remove(pos)
        } else {
            None
        }
    }

    /// Total number of pending tasks (not including in-progress).
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Number of tasks at a specific priority level.
    pub fn depth_by_priority(&self, priority: TaskPriority) -> usize {
        self.tasks.iter().filter(|t| t.priority == priority).count()
    }

    /// Whether the queue is empty (no pending tasks).
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Calculate the weighted load of this queue.
    /// Factors in task count, priority, and complexity.
    pub fn weighted_load(&self) -> f64 {
        let mut load = 0.0;

        // Factor in the in-progress task (it's actively consuming resources)
        if let Some(task) = &self.in_progress {
            load += self.task_weight(task);
        }

        // Factor in queued tasks
        for task in &self.tasks {
            load += self.task_weight(task);
        }

        load
    }

    fn task_weight(&self, task: &crate::types::AgentTask) -> f64 {
        let priority_multiplier = match task.priority {
            crate::types::TaskPriority::Urgent => 3.0,
            crate::types::TaskPriority::Normal => 1.0,
            crate::types::TaskPriority::Background => 0.4,
        };

        // Complexity (1-10) normalized to a reasonable scalar
        let complexity_factor = task.estimated_complexity as f64 / 5.0;

        priority_multiplier * complexity_factor
    }

    /// List all pending tasks in the queue.
    pub fn tasks(&self) -> &std::collections::VecDeque<AgentTask> {
        &self.tasks
    }

    /// Attach planning metadata to a queued or in-progress task.
    pub fn attach_planning_meta(
        &mut self,
        task_id: TaskId,
        meta: &crate::planning::PlanningTaskMeta,
    ) -> bool {
        let active_skill = meta
            .execution_policy_json
            .as_ref()
            .and_then(|json| {
                serde_json::from_str::<crate::planning::ExecutionPolicy>(json)
                    .ok()
                    .and_then(|p| p.allowed_skills.first().cloned())
            });

        if let Some(t) = self.in_progress.as_mut()
            && t.id == task_id
        {
            t.plan_session_id = Some(meta.plan_session_id.clone());
            t.plan_node_id = Some(meta.plan_node_id.clone());
            t.plan_version = Some(meta.plan_version);
            t.execution_policy_json = meta.execution_policy_json.clone();
            if t.active_skill.is_none() {
                t.active_skill = active_skill.clone();
            }
            return true;
        }
        for t in self.tasks.iter_mut() {
            if t.id == task_id {
                t.plan_session_id = Some(meta.plan_session_id.clone());
                t.plan_node_id = Some(meta.plan_node_id.clone());
                t.plan_version = Some(meta.plan_version);
                t.execution_policy_json = meta.execution_policy_json.clone();
                if t.active_skill.is_none() {
                    t.active_skill = active_skill.clone();
                }
                return true;
            }
        }
        false
    }

    /// Attach Socrates evidence context to a queued or in-progress task.
    pub fn attach_socrates_context(
        &mut self,
        task_id: TaskId,
        ctx: crate::socrates::SocratesTaskContext,
    ) -> bool {
        if let Some(t) = self.in_progress.as_mut()
            && t.id == task_id
        {
            t.socrates = Some(ctx);
            return true;
        }
        for t in self.tasks.iter_mut() {
            if t.id == task_id {
                t.socrates = Some(ctx);
                return true;
            }
        }
        false
    }

    /// Generate a markdown checklist of all queued tasks.
    pub fn to_markdown(&self) -> String {
        let mut md = format!("## Agent {} — {}\n\n", self.id, self.name);

        if let Some(ref task) = self.in_progress {
            md.push_str(&format!(
                "- [/] **[{}]** {} ({})\n",
                task.id, task.description, task.priority
            ));
        }

        for task in &self.tasks {
            let checkbox = match &task.status {
                TaskStatus::Completed => "[x]",
                TaskStatus::InProgress => "[/]",
                TaskStatus::Blocked(_) => "[ ] ⏳",
                _ => "[ ]",
            };
            md.push_str(&format!(
                "- {} **[{}]** {} ({})\n",
                checkbox, task.id, task.description, task.priority
            ));
        }

        if self.tasks.is_empty() && self.in_progress.is_none() {
            md.push_str("_No pending tasks._\n");
        }

        md.push_str(&format!(
            "\n> Completed: {} | Pending: {} | Paused: {}\n",
            self.completed.len(),
            self.tasks.len(),
            self.paused,
        ));

        md
    }

    /// Serialize the queue state to JSON.
    pub fn to_json(&self) -> String {
        // We serialize just the task list for state persistence
        serde_json::to_string_pretty(&self.tasks).unwrap_or_else(|_| "[]".to_string())
    }

    // ── Deduplication queue enhancements (enqueue_dedup) ────────────────

    /// Attempt to enqueue, deduplicating by description.
    /// Returns `true` if inserted, `false` if a task with the same description
    /// (case-insensitive) already exists in the queue.
    pub fn enqueue_dedup(&mut self, task: AgentTask) -> bool {
        let desc_lc = task.description.to_lowercase();
        let exists = self
            .tasks
            .iter()
            .any(|t| t.description.to_lowercase() == desc_lc);
        if exists {
            return false;
        }
        // Also check in-progress
        if let Some(ref ip) = self.in_progress {
            if ip.description.to_lowercase() == desc_lc {
                return false;
            }
        }
        self.enqueue(task);
        true
    }

    /// Take all pending tasks for route replay; does not touch [`Self::in_progress`].
    pub(crate) fn take_pending_tasks(&mut self) -> VecDeque<AgentTask> {
        std::mem::take(&mut self.tasks)
    }

    /// Restore pending tasks after route replay (replaces the pending deque).
    pub(crate) fn restore_pending_tasks(&mut self, tasks: VecDeque<AgentTask>) {
        self.tasks = tasks;
    }

    /// Returns an iterator over all pending and in-progress tasks.
    pub fn all_tasks(&self) -> impl Iterator<Item = &AgentTask> {
        self.in_progress
            .as_ref()
            .into_iter()
            .chain(self.tasks.iter())
    }

    /// Returns a mutable iterator over all pending and in-progress tasks.
    pub fn all_tasks_mut(&mut self) -> impl Iterator<Item = &mut AgentTask> {
        self.in_progress
            .as_mut()
            .into_iter()
            .chain(self.tasks.iter_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentId, AgentTask, TaskId, TaskStatus};

    #[test]
    fn test_loop_blocking_enforcement() {
        let mut queue = AgentQueue::new(AgentId(1), "TestAgent");

        // Task within limits — uses TaskPriority::Normal and empty manifest
        let mut t1 = AgentTask::new(TaskId(101), "Normal Task", TaskPriority::Normal, vec![]);
        t1.handoff_count = crate::types::MAX_A2A_BOUNCE;
        queue.enqueue(t1);
        assert!(matches!(
            queue.tasks().front().unwrap().status,
            TaskStatus::Queued
        ));

        // Task exceeding limits
        let mut t2 = AgentTask::new(TaskId(102), "Looping Task", TaskPriority::Normal, vec![]);
        t2.handoff_count = crate::types::MAX_A2A_BOUNCE + 1;
        queue.enqueue(t2);

        let queued_t2 = queue.tasks().iter().find(|t| t.id == TaskId(102)).unwrap();
        match &queued_t2.status {
            TaskStatus::Failed(msg) => {
                assert!(msg.contains("Infinite A2A handoff loop detected"));
            }
            _ => panic!("Task should have failed"),
        }
    }
}
