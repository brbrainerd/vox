use std::collections::VecDeque;

use crate::types::{AgentId, AgentTask, TaskId, TaskPriority, TaskStatus};

/// Per-agent priority task queue.
///
/// Tasks are stored in priority order (Urgent > Normal > Background).
/// Within the same priority level, tasks are FIFO.
#[derive(Debug)]
pub struct AgentQueue {
    /// The agent that owns this queue.
    pub id: AgentId,
    /// Human-readable name for this agent/queue.
    pub name: String,
    /// Ordered queue of pending tasks.
    tasks: VecDeque<AgentTask>,
    /// The task currently being executed (if any).
    in_progress: Option<AgentTask>,
    /// IDs of completed tasks (for dependency resolution).
    completed: Vec<TaskId>,
    /// Whether this queue is paused (no dequeue).
    pub paused: bool,
    /// Last time this agent was active (enqueued, dequeued, or completed a task).
    pub last_active: std::time::SystemTime,
    /// ID of the AI agent session mapped to this queue, if any.
    pub agent_session_id: Option<String>,
}

impl AgentQueue {
    /// Create a new empty queue for the given agent.
    pub fn new(id: AgentId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            tasks: VecDeque::new(),
            in_progress: None,
            completed: Vec::new(),
            paused: false,
            last_active: std::time::SystemTime::now(),
            agent_session_id: None,
        }
    }

    /// Link an AI agent session to this queue.
    pub fn set_agent_session(&mut self, session_id: String) {
        self.agent_session_id = Some(session_id);
    }

    /// Enqueue a task, inserting it in priority order.
    /// Higher priority tasks go before lower priority tasks.
    /// Within the same priority, new tasks go to the end (FIFO).
    pub fn enqueue(&mut self, task: AgentTask) {
        // Find the insertion point: after all tasks of equal or higher priority
        let pos = self
            .tasks
            .iter()
            .position(|t| t.priority < task.priority)
            .unwrap_or(self.tasks.len());
        self.tasks.insert(pos, task);
        self.last_active = std::time::SystemTime::now();
    }

    /// Dequeue the highest-priority ready task.
    /// Returns `None` if the queue is empty, paused, or all tasks are blocked.
    pub fn dequeue(&mut self) -> Option<AgentTask> {
        if self.paused {
            return None;
        }
        // Find the first task that is ready (not blocked)
        let pos = self.tasks.iter().position(|t| match &t.status {
            TaskStatus::Queued => t.is_ready(&self.completed),
            _ => false,
        })?;
        let mut task = self.tasks.remove(pos)?;
        task.status = TaskStatus::InProgress;
        self.in_progress = Some(task.clone());
        self.last_active = std::time::SystemTime::now();
        Some(task)
    }

    /// Peek at the next task without removing it.
    pub fn peek(&self) -> Option<&AgentTask> {
        self.tasks.front()
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

    /// Mark the current in-progress task as completed.
    pub fn mark_complete(&mut self, task_id: TaskId) -> bool {
        if let Some(ref task) = self.in_progress {
            if task.id == task_id {
                self.completed.push(task_id);
                self.in_progress = None;
                // Unblock tasks that depended on this one
                self.unblock(task_id);
                self.last_active = std::time::SystemTime::now();
                return true;
            }
        }
        false
    }

    /// Mark the current in-progress task as failed.
    pub fn mark_failed(&mut self, task_id: TaskId, reason: String) -> bool {
        if let Some(ref mut task) = self.in_progress {
            if task.id == task_id {
                task.status = TaskStatus::Failed(reason);
                self.in_progress = None;
                return true;
            }
        }
        false
    }

    /// Unblock all queued tasks that depended on the given task.
    pub fn unblock(&mut self, completed_task_id: TaskId) {
        for task in self.tasks.iter_mut() {
            task.depends_on.retain(|dep| *dep != completed_task_id);
            if task.depends_on.is_empty() {
                if let TaskStatus::Blocked(_) = &task.status {
                    task.status = TaskStatus::Queued;
                }
            }
        }
    }

    /// Check if a specific task is blocked on unmet dependencies.
    pub fn is_blocked(&self, task_id: TaskId) -> bool {
        self.tasks
            .iter()
            .find(|t| t.id == task_id)
            .map(|t| matches!(&t.status, TaskStatus::Blocked(_)))
            .unwrap_or(false)
    }

    /// Pause this queue — no tasks will be dequeued.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume this queue.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Whether this queue is paused.
    pub fn is_paused(&self) -> bool {
        self.paused
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

    /// Number of completed tasks.
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    /// Whether there is a task currently in progress.
    pub fn has_in_progress(&self) -> bool {
        self.in_progress.is_some()
    }

    /// Number of tasks in progress (0 or 1).
    pub fn in_progress_count(&self) -> usize {
        if self.in_progress.is_some() {
            1
        } else {
            0
        }
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

    /// Get the currently in-progress task.
    pub fn current_task(&self) -> Option<&AgentTask> {
        self.in_progress.as_ref()
    }

    /// List of completed task IDs.
    pub fn completed_ids(&self) -> &[TaskId] {
        &self.completed
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

    /// Drain all pending tasks out of this queue (for redistribution).
    pub fn drain_tasks(&mut self) -> Vec<AgentTask> {
        self.tasks.drain(..).collect()
    }

    // ── Phase 5.1 additions ──────────────────────────────────────────────

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

    /// Time out any in-progress task that has been running longer than `timeout`.
    /// Returns the timed-out task if one was found.
    pub fn timeout_in_progress(&mut self, timeout: std::time::Duration) -> Option<AgentTask> {
        let now = std::time::Instant::now();
        if let Some(ref task) = self.in_progress {
            if let Some(created) = task.created_at {
                if now.duration_since(created) >= timeout {
                    return self.in_progress.take();
                }
            }
        }
        None
    }

    /// Requeue a failed or timed-out task with an incremented retry count.
    /// Returns `false` if `max_retries` was already reached.
    pub fn retry_task(&mut self, mut task: AgentTask, max_retries: u32) -> bool {
        if task.retry_count >= max_retries {
            return false;
        }
        task.retry_count += 1;
        // Exponential backoff: store next-eligible time in task metadata
        task.status = TaskStatus::Queued;
        self.enqueue(task);
        true
    }

    /// Drain all tasks that have exceeded a timeout (for external rescheduling).
    pub fn drain_timed_out(&mut self, timeout: std::time::Duration) -> Vec<AgentTask> {
        let now = std::time::Instant::now();
        let mut timed_out = Vec::new();
        let mut i = 0;
        while i < self.tasks.len() {
            let expired = if let Some(created) = self.tasks[i].created_at {
                now.duration_since(created) >= timeout
            } else {
                false
            };
            if expired {
                if let Some(t) = self.tasks.remove(i) {
                    timed_out.push(t);
                }
            } else {
                i += 1;
            }
        }
        timed_out
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: u64, priority: TaskPriority) -> AgentTask {
        AgentTask::new(TaskId(id), format!("task-{}", id), priority, vec![])
    }

    #[test]
    fn enqueue_respects_priority() {
        let mut q = AgentQueue::new(AgentId(1), "test");
        q.enqueue(make_task(1, TaskPriority::Normal));
        q.enqueue(make_task(2, TaskPriority::Urgent));
        q.enqueue(make_task(3, TaskPriority::Background));

        // Urgent should be first
        let first = q.dequeue().expect("should have task");
        assert_eq!(first.id, TaskId(2));
        let second = q.dequeue().expect("should have task");
        assert_eq!(second.id, TaskId(1));
    }

    #[test]
    fn fifo_within_same_priority() {
        let mut q = AgentQueue::new(AgentId(1), "test");
        q.enqueue(make_task(1, TaskPriority::Normal));
        q.enqueue(make_task(2, TaskPriority::Normal));
        q.enqueue(make_task(3, TaskPriority::Normal));

        assert_eq!(q.dequeue().unwrap().id, TaskId(1));
    }

    #[test]
    fn cancel_task() {
        let mut q = AgentQueue::new(AgentId(1), "test");
        q.enqueue(make_task(1, TaskPriority::Normal));
        q.enqueue(make_task(2, TaskPriority::Normal));

        let cancelled = q.cancel(TaskId(1));
        assert!(cancelled.is_some());
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn mark_complete_and_unblock() {
        let mut q = AgentQueue::new(AgentId(1), "test");

        // Task 2 depends on task 1
        let t1 = make_task(1, TaskPriority::Normal);
        let t2 = make_task(2, TaskPriority::Normal).depends_on(TaskId(1));

        q.enqueue(t1);
        q.enqueue(t2);

        // Dequeue task 1 (task 2 is blocked)
        let active = q.dequeue().expect("task 1 ready");
        assert_eq!(active.id, TaskId(1));

        // Task 2 should still be blocked
        assert!(q.is_blocked(TaskId(2)));

        // Complete task 1 — should unblock task 2
        q.mark_complete(TaskId(1));
        assert!(!q.is_blocked(TaskId(2)));

        // Now task 2 should be dequeue-able
        let next = q.dequeue().expect("task 2 should be unblocked");
        assert_eq!(next.id, TaskId(2));
    }

    #[test]
    fn paused_queue_blocks_dequeue() {
        let mut q = AgentQueue::new(AgentId(1), "test");
        q.enqueue(make_task(1, TaskPriority::Normal));
        q.pause();
        assert!(q.dequeue().is_none());
        q.resume();
        assert!(q.dequeue().is_some());
    }

    #[test]
    fn reorder_changes_position() {
        let mut q = AgentQueue::new(AgentId(1), "test");
        q.enqueue(make_task(1, TaskPriority::Background));
        q.enqueue(make_task(2, TaskPriority::Normal));

        // Promote task 1 to urgent
        q.reorder(TaskId(1), TaskPriority::Urgent);

        let first = q.dequeue().unwrap();
        assert_eq!(first.id, TaskId(1));
    }

    #[test]
    fn markdown_output() {
        let mut q = AgentQueue::new(AgentId(1), "parser");
        q.enqueue(make_task(1, TaskPriority::Normal));
        q.enqueue(make_task(2, TaskPriority::Urgent));

        let md = q.to_markdown();
        assert!(md.contains("Agent A-01"));
        assert!(md.contains("parser"));
        assert!(md.contains("task-1"));
        assert!(md.contains("task-2"));
    }

    #[test]
    fn drain_empties_queue() {
        let mut q = AgentQueue::new(AgentId(1), "test");
        q.enqueue(make_task(1, TaskPriority::Normal));
        q.enqueue(make_task(2, TaskPriority::Normal));

        let drained = q.drain_tasks();
        assert_eq!(drained.len(), 2);
        assert!(q.is_empty());
    }

    #[test]
    fn enqueue_dedup_prevents_duplicate() {
        let mut q = AgentQueue::new(AgentId(1), "test");
        let t1 = make_task(1, TaskPriority::Normal);
        let t2 = AgentTask::new(TaskId(2), "task-1", TaskPriority::Urgent, vec![]); // same desc as t1
        assert!(q.enqueue_dedup(t1));
        assert!(
            !q.enqueue_dedup(t2),
            "duplicate description should be rejected"
        );
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn enqueue_dedup_case_insensitive() {
        let mut q = AgentQueue::new(AgentId(1), "test");
        let t1 = AgentTask::new(TaskId(1), "Fix Parser", TaskPriority::Normal, vec![]);
        let t2 = AgentTask::new(TaskId(2), "fix parser", TaskPriority::Urgent, vec![]);
        assert!(q.enqueue_dedup(t1));
        assert!(
            !q.enqueue_dedup(t2),
            "case-insensitive duplicate should be rejected"
        );
    }

    #[test]
    fn retry_task_increments_count() {
        let mut q = AgentQueue::new(AgentId(1), "test");
        let task = make_task(1, TaskPriority::Normal);
        let ok = q.retry_task(task, 3);
        assert!(ok);
        assert_eq!(q.len(), 1);
        let dequeued = q.dequeue().unwrap();
        assert_eq!(dequeued.retry_count, 1);
    }

    #[test]
    fn retry_task_respects_max_retries() {
        let mut q = AgentQueue::new(AgentId(1), "test");
        let mut task = make_task(42, TaskPriority::Normal);
        task.retry_count = 3;
        let ok = q.retry_task(task, 3);
        assert!(!ok, "should refuse when retry_count >= max_retries");
        assert!(q.is_empty());
    }
}
