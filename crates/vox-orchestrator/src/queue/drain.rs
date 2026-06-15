use crate::types::{AgentTask, TaskId, TaskStatus};

use super::{AgentQueue, PopuliRemoteHoldError};

impl AgentQueue {
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
        task.start(); // record wall-clock start for temporal context injection
        self.in_progress = Some(task.clone());
        self.last_active = std::time::Instant::now();
        Some(task)
    }

    /// Dequeue a task that is currently in Doubted status.
    pub fn dequeue_doubted(&mut self) -> Option<AgentTask> {
        if self.paused {
            return None;
        }
        let pos = self
            .tasks
            .iter()
            .position(|t| matches!(t.status, TaskStatus::Doubted(_)))?;
        let mut task = self.tasks.remove(pos)?;
        task.status = TaskStatus::InProgress;
        task.start();
        self.in_progress = Some(task.clone());
        self.last_active = std::time::Instant::now();
        Some(task)
    }

    /// Peek at the next task without removing it.
    pub fn peek(&self) -> Option<&AgentTask> {
        self.tasks.front()
    }

    /// Mark the current in-progress task as completed.
    pub fn mark_complete(&mut self, task_id: TaskId) -> bool {
        if let Some(ref task) = self.in_progress {
            if task.id == task_id {
                self.completed.push(task_id);
                self.in_progress = None;
                // Unblock tasks that depended on this one
                self.unblock(task_id);
                self.last_active = std::time::Instant::now();
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

    /// Number of completed tasks.
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    /// Number of tasks in Doubted state.
    pub fn doubted_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Doubted(_)))
            .count()
    }

    /// Whether there is a task currently in progress.
    pub fn has_in_progress(&self) -> bool {
        self.in_progress.is_some()
    }

    /// Number of tasks in progress (0 or 1).
    pub fn in_progress_count(&self) -> usize {
        if self.in_progress.is_some() { 1 } else { 0 }
    }

    /// Get the currently in-progress task.
    pub fn current_task(&self) -> Option<&AgentTask> {
        self.in_progress.as_ref()
    }

    /// Get a mutable reference to the currently in-progress task.
    pub fn current_task_mut(&mut self) -> Option<&mut AgentTask> {
        self.in_progress.as_mut()
    }

    /// Find a task by ID in the queue and return a mutable reference.
    pub fn find_task_mut(&mut self, task_id: TaskId) -> Option<&mut AgentTask> {
        self.tasks.iter_mut().find(|t| t.id == task_id)
    }

    /// Hold a task as in-progress for Populi remote execution without dequeuing from [`Self::tasks`].
    ///
    /// Fails when another task is already in progress for this agent.
    pub fn hold_for_populi_remote(
        &mut self,
        mut task: AgentTask,
    ) -> Result<(), PopuliRemoteHoldError> {
        if self.in_progress.is_some() {
            return Err(PopuliRemoteHoldError::AgentBusy);
        }
        task.status = TaskStatus::InProgress;
        task.start();
        self.in_progress = Some(task);
        self.last_active = std::time::Instant::now();
        Ok(())
    }

    /// Remove the in-progress task when it matches `task_id` (cancel / external transition).
    pub fn take_in_progress_if(&mut self, task_id: TaskId) -> Option<AgentTask> {
        if self.in_progress.as_ref().is_some_and(|t| t.id == task_id) {
            self.in_progress.take()
        } else {
            None
        }
    }

    /// Remove a queued task if it matches `task_id`.
    pub fn take_queued(&mut self, task_id: TaskId) -> Option<AgentTask> {
        if let Some(pos) = self.tasks.iter().position(|t| t.id == task_id) {
            self.tasks.remove(pos)
        } else {
            None
        }
    }

    /// List of completed task IDs.
    pub fn completed_ids(&self) -> &[TaskId] {
        &self.completed
    }

    /// Drain all pending tasks out of this queue (for redistribution).
    pub fn drain_tasks(&mut self) -> Vec<AgentTask> {
        self.tasks.drain(..).collect()
    }

    /// Time out any in-progress task that has been running longer than `timeout`.
    /// Returns the timed-out task if one was found.
    pub fn timeout_in_progress(&mut self, timeout: std::time::Duration) -> Option<AgentTask> {
        let now = std::time::Instant::now();
        let now_ms = crate::types::now_unix_ms();
        if let Some(ref task) = self.in_progress {
            let expired = if let Some(created) = task.created_at {
                now.duration_since(created) >= timeout
            } else {
                now_ms.saturating_sub(task.created_at_ms) >= timeout.as_millis() as u64
            };
            if expired {
                return self.in_progress.take();
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
        let now_ms = crate::types::now_unix_ms();
        let mut timed_out = Vec::new();
        let mut i = 0;
        let timeout_ms = timeout.as_millis() as u64;
        while i < self.tasks.len() {
            let task = &self.tasks[i];
            let expired = if let Some(created) = task.created_at {
                now.duration_since(created) >= timeout
            } else {
                now_ms.saturating_sub(task.created_at_ms) >= timeout_ms
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

    /// Calculate the maximum handoff count observed in any active or queued task.
    pub fn max_handoff_count(&self) -> u8 {
        let mut max = self
            .in_progress
            .as_ref()
            .map(|t| t.handoff_count)
            .unwrap_or(0);
        for task in &self.tasks {
            if task.handoff_count > max {
                max = task.handoff_count;
            }
        }
        max
    }
}

#[cfg(test)]
mod semcov_drain_tests {
    use super::*;
    use crate::types::{AgentId, AgentTask, TaskId, TaskPriority, TaskStatus};

    fn task(id: u64) -> AgentTask {
        AgentTask::new(TaskId(id), format!("t{id}"), TaskPriority::Normal, vec![])
    }

    // Catches: dequeue() handing out a task whose dependencies are NOT yet in the
    // completed set (is_ready bypass), violating DAG ordering.
    #[test]
    fn dequeue_skips_task_blocked_on_unmet_dependency() {
        let mut q = AgentQueue::new(AgentId(1), "a");
        // t2 depends on t1; depends_on() flips status to Blocked.
        let t2 = task(2).depends_on(TaskId(1));
        q.enqueue(t2);
        // Nothing is dequeueable because t1 is not completed.
        assert!(q.dequeue().is_none());
    }

    // Catches: mark_complete failing to move the id into `completed` and failing to
    // unblock dependents, which would deadlock a dependency chain.
    #[test]
    fn mark_complete_records_id_and_unblocks_dependents() {
        let mut q = AgentQueue::new(AgentId(1), "a");
        q.enqueue(task(1));
        let t2 = task(2).depends_on(TaskId(1));
        q.enqueue(t2);

        let first = q.dequeue().expect("t1 ready");
        assert_eq!(first.id, TaskId(1));
        assert!(q.mark_complete(TaskId(1)));
        assert_eq!(q.completed_count(), 1);
        assert_eq!(q.completed_ids(), &[TaskId(1)]);

        // t2 was Blocked; unblock should have promoted it to Queued and now dequeueable.
        let second = q.dequeue().expect("t2 now ready");
        assert_eq!(second.id, TaskId(2));
    }

    // Catches: mark_complete returning true for a task id that is not the current
    // in-progress task (wrong-id acknowledgement corrupting completion ledger).
    #[test]
    fn mark_complete_rejects_wrong_id() {
        let mut q = AgentQueue::new(AgentId(1), "a");
        q.enqueue(task(1));
        q.dequeue();
        assert!(!q.mark_complete(TaskId(999)));
        assert_eq!(q.completed_count(), 0);
    }

    // Catches: max_handoff_count ignoring queued tasks (or the in-progress task),
    // under-reporting bounce depth used by the A2A loop breaker.
    #[test]
    fn max_handoff_count_takes_max_over_queued_and_in_progress() {
        let mut q = AgentQueue::new(AgentId(1), "a");
        let mut hi = task(1);
        hi.handoff_count = 4;
        q.enqueue(hi);
        let mut lo = task(2);
        lo.handoff_count = 1;
        q.enqueue(lo);
        assert_eq!(q.max_handoff_count(), 4);
    }
}
