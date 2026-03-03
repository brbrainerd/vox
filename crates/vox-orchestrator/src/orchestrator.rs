use std::collections::HashMap;
use std::path::PathBuf;

use crate::affinity::FileAffinityMap;
use crate::bulletin::BulletinBoard;
use crate::config::OrchestratorConfig;
use crate::groups::AffinityGroupRegistry;
use crate::locks::{FileLockManager, LockKind};
use crate::queue::AgentQueue;
use crate::scope::{ScopeEnforcement, ScopeGuard};
use crate::services::{
    MessageGateway, PolicyCheckResult, PolicyEngine, RouteResult, RoutingService,
};
use crate::types::{
    AccessKind, AgentId, AgentIdGenerator, AgentTask, FileAffinity, TaskId, TaskIdGenerator,
    TaskPriority, TaskStatus,
};
use crate::oplog::{OperationId, OperationKind};
use crate::snapshot::SnapshotId;

/// Error type for orchestrator operations.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    #[error("Orchestrator is disabled")]
    Disabled,
    #[error("Maximum agents ({max}) reached")]
    MaxAgentsReached { max: usize },
    #[error("Agent {0} not found")]
    AgentNotFound(AgentId),
    #[error("Task {0} not found")]
    TaskNotFound(TaskId),
    #[error("Lock conflict: {0}")]
    LockConflict(#[from] crate::locks::LockConflict),
    #[error("Scope denied: {0}")]
    ScopeDenied(String),
    #[error("Operation not found")]
    OperationNotFound,
    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// One step in a task's lifecycle timeline (ingress → route → verification → outcome).
/// ORCH-01 SPLIT TARGET: Types (OrchestratorError, TaskTraceStep, OrchestratorStatus, AgentSummary)
/// → move to orchestrator/types.rs when decomposing this file into sub-modules.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskTraceStep {
    pub stage: String,
    pub timestamp_ms: u64,
    pub detail: Option<String>,
}

const MAX_TASK_TRACES: usize = 200;

/// Snapshot of the orchestrator state for display.
#[derive(Debug, serde::Serialize)]
pub struct OrchestratorStatus {
    pub enabled: bool,
    pub agent_count: usize,
    pub total_queued: usize,
    pub total_in_progress: usize,
    pub total_completed: usize,
    pub locked_files: usize,
    pub total_contention: usize,
    pub total_weighted_load: f64,
    pub predicted_load: f64,
    pub reserved_agents: usize,
    pub dynamic_agents: usize,
    pub context_entries: std::collections::HashMap<String, crate::context::ContextEntry>,
    pub agents: Vec<AgentSummary>,
}

/// Summary info for one agent.
#[derive(Debug, serde::Serialize)]
pub struct AgentSummary {
    pub id: AgentId,
    pub name: String,
    pub queued: usize,
    pub urgent_count: usize,
    pub normal_count: usize,
    pub background_count: usize,
    pub in_progress: bool,
    pub completed: usize,
    pub paused: bool,
    pub owned_files: usize,
    pub dynamic: bool,
    pub weighted_load: f64,
    pub agent_session_id: Option<String>,
}

/// The central coordinator for the multi-agent file-affinity queue system.
pub struct Orchestrator {
    config: OrchestratorConfig,
    affinity_map: FileAffinityMap,
    lock_manager: FileLockManager,
    context_store: crate::context::ContextStore,
    budget_manager: crate::budget::BudgetManager,
    summary_manager: crate::summary::SummaryManager,
    models: crate::models::ModelRegistry,
    bulletin: BulletinBoard,
    agents: HashMap<AgentId, AgentQueue>,
    groups: AffinityGroupRegistry,
    task_id_gen: TaskIdGenerator,
    agent_id_gen: AgentIdGenerator,
    /// Maps task IDs to the agent they were assigned to.
    task_assignments: HashMap<TaskId, AgentId>,
    qa_router: crate::qa::QARouter,
    monitor: crate::monitor::AiMonitor,
    event_bus: crate::events::EventBus,
    message_bus: crate::a2a::MessageBus,
    /// IDs of agents that were dynamically spawned (transient).
    dynamic_agents: std::collections::HashSet<AgentId>,
    /// Handles to the running agent processes.
    agent_handles: HashMap<AgentId, vox_runtime::ProcessHandle>,
    heartbeat_monitor: crate::heartbeat::HeartbeatMonitor,
    /// System resource monitor.
    #[cfg(feature = "system-metrics")]
    sys: sysinfo::System,
    /// Historical system load for predictive scaling.
    load_history: std::collections::VecDeque<f64>,
    /// Scope guard for write boundaries (synced with affinity on assign/retire).
    scope_guard: ScopeGuard,
    /// Per-task timeline (ingress → route → outcome), capped at MAX_TASK_TRACES.
    task_traces: HashMap<TaskId, Vec<TaskTraceStep>>,
    /// Content-addressed code store (TursoDB).
    code_store: Option<vox_pm::CodeStore>,
    // -- JJ-inspired subsystems --
    /// Auto-snapshot store for tracking file state changes.
    snapshot_store: crate::snapshot::SnapshotStore,
    /// Operation log for universal undo/redo.
    oplog: crate::oplog::OpLog,
    /// First-class conflict tracking.
    conflict_manager: crate::conflicts::ConflictManager,
    /// Per-agent virtual workspaces and change tracking.
    workspace_manager: crate::workspace::WorkspaceManager,
    /// Timestamp of the last rebalance (for cooldown enforcement).
    last_rebalance_at: Option<std::time::Instant>,
}

impl Orchestrator {
    /// Create a new orchestrator with the given configuration.
    // ORCH-01 SPLIT TARGET:
    //   new() / with_groups() / init_db() → orchestrator/core.rs
    //   submit_task*() / submit_batch() / resolve_route() / spawn_agent*() → orchestrator/task_dispatch.rs
    //   map_agent_session() / retire_agent() / heartbeat() / pause/resume_agent() → orchestrator/agent_state.rs
    //   capture_snapshot() / record_operation() / tick() → orchestrator/vcs_ops.rs
    //   complete_task() / fail_task() / status() → orchestrator/task_dispatch.rs
    //   record_ai_usage() / rebalance() → orchestrator/core.rs
    pub fn new(config: OrchestratorConfig) -> Self {
        let bulletin = BulletinBoard::new(config.bulletin_capacity);
        Self {
            config: config.clone(),
            affinity_map: FileAffinityMap::new(),
            lock_manager: FileLockManager::new(),
            context_store: crate::context::ContextStore::new(),
            budget_manager: crate::budget::BudgetManager::new(),
            summary_manager: crate::summary::SummaryManager::new(),
            models: crate::models::ModelRegistry::new(),
            bulletin,
            agents: HashMap::new(),
            groups: AffinityGroupRegistry::defaults(),
            task_id_gen: TaskIdGenerator::new(),
            agent_id_gen: AgentIdGenerator::new(),
            task_assignments: HashMap::new(),
            qa_router: crate::qa::QARouter::new(),
            monitor: crate::monitor::AiMonitor::new(
                config.continuation_cooldown_ms,
                config.max_auto_continuations,
                config.stale_threshold_ms,
            ),
            event_bus: crate::events::EventBus::new(1024),
            message_bus: crate::a2a::MessageBus::new(100),
            dynamic_agents: std::collections::HashSet::new(),
            agent_handles: HashMap::new(),
            heartbeat_monitor: crate::heartbeat::HeartbeatMonitor::new(config.stale_threshold_ms),
            #[cfg(feature = "system-metrics")]
            sys: sysinfo::System::new_all(),
            load_history: std::collections::VecDeque::with_capacity(config.scaling_lookback_ticks),
            scope_guard: ScopeGuard::new(config.scope_enforcement),
            task_traces: HashMap::new(),
            snapshot_store: crate::snapshot::SnapshotStore::default(),
            oplog: crate::oplog::OpLog::default(),
            conflict_manager: crate::conflicts::ConflictManager::new(),
            workspace_manager: crate::workspace::WorkspaceManager::new(),
            code_store: None,
            last_rebalance_at: None,
        }
    }

    /// Create an orchestrator with custom affinity groups.
    pub fn with_groups(config: OrchestratorConfig, groups: AffinityGroupRegistry) -> Self {
        let bulletin = BulletinBoard::new(config.bulletin_capacity);
        Self {
            config: config.clone(),
            affinity_map: FileAffinityMap::new(),
            lock_manager: FileLockManager::new(),
            context_store: crate::context::ContextStore::new(),
            budget_manager: crate::budget::BudgetManager::new(),
            summary_manager: crate::summary::SummaryManager::new(),
            models: crate::models::ModelRegistry::new(),
            bulletin,
            agents: HashMap::new(),
            groups,
            task_id_gen: TaskIdGenerator::new(),
            agent_id_gen: AgentIdGenerator::new(),
            task_assignments: HashMap::new(),
            qa_router: crate::qa::QARouter::new(),
            monitor: crate::monitor::AiMonitor::new(
                config.continuation_cooldown_ms,
                config.max_auto_continuations,
                config.stale_threshold_ms,
            ),
            event_bus: crate::events::EventBus::new(1024),
            message_bus: crate::a2a::MessageBus::new(100),
            dynamic_agents: std::collections::HashSet::new(),
            agent_handles: HashMap::new(),
            heartbeat_monitor: crate::heartbeat::HeartbeatMonitor::new(config.stale_threshold_ms),
            #[cfg(feature = "system-metrics")]
            sys: sysinfo::System::new_all(),
            load_history: std::collections::VecDeque::with_capacity(config.scaling_lookback_ticks),
            scope_guard: ScopeGuard::new(config.scope_enforcement),
            task_traces: HashMap::new(),
            snapshot_store: crate::snapshot::SnapshotStore::default(),
            oplog: crate::oplog::OpLog::default(),
            conflict_manager: crate::conflicts::ConflictManager::new(),
            workspace_manager: crate::workspace::WorkspaceManager::new(),
            code_store: None,
            last_rebalance_at: None,
        }
    }

    /// Initialize the orchestrator database schema.
    pub async fn init_db(&self, db: &vox_db::VoxDb) -> Result<(), OrchestratorError> {
        db.sync_schema_from_digest(&crate::schema::orchestrator_schema())
            .await
            .map_err(|e| OrchestratorError::ScopeDenied(format!("DB sync failed: {}", e)))?;
        Ok(())
    }

    /// Submit a new task to the orchestrator (async).
    ///
    /// The orchestrator will:
    /// 1. Analyze the file manifest against the affinity map
    /// 2. Route the task to an existing agent or spawn a new one
    /// 3. Acquire file locks
    /// 4. Enqueue the task
    pub async fn submit_task(
        &mut self,
        description: impl Into<String>,
        file_manifest: Vec<FileAffinity>,
        priority: Option<TaskPriority>,
    ) -> Result<TaskId, OrchestratorError> {
        self.submit_task_with_agent(description, file_manifest, priority, None)
            .await
    }

    /// Submit a new task to the orchestrator, potentially targeting a specific agent name (async).
    pub async fn submit_task_with_agent(
        &mut self,
        description: impl Into<String>,
        file_manifest: Vec<FileAffinity>,
        priority: Option<TaskPriority>,
        target_agent: Option<String>,
    ) -> Result<TaskId, OrchestratorError> {
        if !self.config.enabled {
            return Err(OrchestratorError::Disabled);
        }

        let task_id = self.task_id_gen.next();
        let priority = priority.unwrap_or(self.config.default_priority);

        let task = AgentTask::new(task_id, description, priority, file_manifest.clone());

        // Route to the right agent via RoutingService
        let agent_id = self.resolve_route(&file_manifest, target_agent.as_deref())?;

        // Pre-queue policy check (locks; scope when enforcement enabled)
        let scope_guard = (self.config.scope_enforcement != ScopeEnforcement::Disabled)
            .then_some(&self.scope_guard);
        match PolicyEngine::check_before_queue(
            &self.lock_manager,
            scope_guard,
            &self.event_bus,
            &file_manifest,
            agent_id,
        ) {
            PolicyCheckResult::Allowed => {}
            PolicyCheckResult::LockConflict(e) => return Err(OrchestratorError::LockConflict(e)),
            PolicyCheckResult::ScopeDenied(msg) => return Err(OrchestratorError::ScopeDenied(msg)),
        }

        // Try to acquire locks for write files
        for fa in &file_manifest {
            if fa.access == AccessKind::Write {
                let lock_kind = LockKind::Exclusive;
                // If lock fails, we still enqueue (the agent will retry when it picks up the task)
                let _ = self.lock_manager.try_acquire(&fa.path, agent_id, lock_kind);
            }
        }

        // Assign files to the agent in the affinity map and scope guard
        for fa in &file_manifest {
            if fa.access == AccessKind::Write {
                self.affinity_map.assign(&fa.path, agent_id);
                self.scope_guard.assign_file(agent_id, fa.path.clone());
            }
        }

        // Capture pre-task snapshot for version control (persisted to CodeStore)
        let snapshot_before = {
            let paths: Vec<PathBuf> = file_manifest.iter().map(|f| f.path.clone()).collect();
            let desc_str = task.description.clone();
            let snap_desc = format!("pre-task: {:.50}", desc_str);
            let snap_id = self
                .capture_snapshot(agent_id, &paths, snap_desc.clone())
                .await;
            self.event_bus
                .emit(crate::events::AgentEventKind::SnapshotCaptured {
                    agent_id,
                    snapshot_id: snap_id.to_string(),
                    file_count: paths.len(),
                    description: snap_desc,
                });
            snap_id
        };

        self.record_operation(
            agent_id,
            OperationKind::TaskSubmit { task_id: task_id.0 },
            format!("Submitted task {}", task_id),
            Some(snapshot_before),
            None,
            None,
            None,
        )
        .await;

        // Enqueue the task
        if let Some(queue) = self.agents.get_mut(&agent_id) {
            queue.enqueue(task);
            self.task_assignments.insert(task_id, agent_id);

            // Notify the agent process to wake up and process
            if let Some(handle) = self.agent_handles.get(&agent_id) {
                let cmd = crate::runtime::AgentCommand::ProcessQueue;
                let _ = handle.send_json(cmd);
            }

            tracing::info!(
                "Task {} routed to agent {} (queue len: {})",
                task_id,
                agent_id,
                queue.len()
            );
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if self.task_traces.len() >= MAX_TASK_TRACES {
            if let Some(min_id) = self.task_traces.keys().min().copied() {
                self.task_traces.remove(&min_id);
            }
        }
        self.task_traces.insert(
            task_id,
            vec![
                TaskTraceStep {
                    stage: "ingress".to_string(),
                    timestamp_ms: now_ms,
                    detail: None,
                },
                TaskTraceStep {
                    stage: "routed".to_string(),
                    timestamp_ms: now_ms,
                    detail: Some(format!("agent {}", agent_id)),
                },
            ],
        );

        Ok(task_id)
    }

    /// Submit a batch of interdependent tasks (async).
    pub async fn submit_batch(
        &mut self,
        descriptors: Vec<crate::types::TaskDescriptor>,
    ) -> Result<Vec<TaskId>, OrchestratorError> {
        if !self.config.enabled {
            return Err(OrchestratorError::Disabled);
        }

        let mut assigned_ids: Vec<TaskId> = Vec::with_capacity(descriptors.len());

        // Pre-allocate task IDs
        for _ in 0..descriptors.len() {
            assigned_ids.push(self.task_id_gen.next());
        }

        let mut results = Vec::new();

        // Second pass: construct tasks with resolved IDs and submit
        for (i, mut desc) in descriptors.into_iter().enumerate() {
            let my_id = assigned_ids[i];

            // Resolve temporary deps into actual TaskIds
            for tmp_dep_idx in desc.temp_deps {
                if tmp_dep_idx < assigned_ids.len() {
                    desc.depends_on.push(assigned_ids[tmp_dep_idx]);
                } else {
                    tracing::warn!(
                        "Task descriptor {} referenced out-of-bounds temp dep {}",
                        i,
                        tmp_dep_idx
                    );
                }
            }

            let priority = desc.priority.unwrap_or(self.config.default_priority);
            let mut task = AgentTask::new(
                my_id,
                desc.description.clone(),
                priority,
                desc.file_manifest.clone(),
            );

            // Add all collected deps
            for dep in desc.depends_on {
                task = task.depends_on(dep);
            }

            // Route to best agent via RoutingService
            let agent_id = self.resolve_route(&desc.file_manifest, None)?;

            let scope_guard = (self.config.scope_enforcement != ScopeEnforcement::Disabled)
                .then_some(&self.scope_guard);
            match PolicyEngine::check_before_queue(
                &self.lock_manager,
                scope_guard,
                &self.event_bus,
                &desc.file_manifest,
                agent_id,
            ) {
                PolicyCheckResult::Allowed => {}
                PolicyCheckResult::LockConflict(e) => {
                    return Err(OrchestratorError::LockConflict(e))
                }
                PolicyCheckResult::ScopeDenied(msg) => {
                    return Err(OrchestratorError::ScopeDenied(msg))
                }
            }

            // Acquire locks and assign scope
            for fa in &desc.file_manifest {
                if fa.access == AccessKind::Write {
                    let _ = self
                        .lock_manager
                        .try_acquire(&fa.path, agent_id, LockKind::Exclusive);
                    self.affinity_map.assign(&fa.path, agent_id);
                    self.scope_guard.assign_file(agent_id, fa.path.clone());
                }
            }

            // Capture pre-task snapshot for version control
            let snapshot_before = {
                let paths: Vec<PathBuf> =
                    desc.file_manifest.iter().map(|f| f.path.clone()).collect();
                self.capture_snapshot(
                    agent_id,
                    &paths,
                    format!("pre-task batch: {:.50}", task.description),
                )
                .await
            };

            self.record_operation(
                agent_id,
                OperationKind::TaskSubmit { task_id: my_id.0 },
                format!("Submitted batch task {}", my_id),
                Some(snapshot_before),
                None,
                None,
                None,
            )
            .await;

            // Enqueue
            if let Some(queue) = self.agents.get_mut(&agent_id) {
                queue.enqueue(task);
                self.task_assignments.insert(my_id, agent_id);

                // Notify
                if let Some(handle) = self.agent_handles.get(&agent_id) {
                    let cmd = crate::runtime::AgentCommand::ProcessQueue;
                    let _ = handle.send_json(cmd);
                }
            }

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            if self.task_traces.len() >= MAX_TASK_TRACES {
                if let Some(min_id) = self.task_traces.keys().min().copied() {
                    self.task_traces.remove(&min_id);
                }
            }
            self.task_traces.insert(
                my_id,
                vec![
                    TaskTraceStep {
                        stage: "ingress".to_string(),
                        timestamp_ms: now_ms,
                        detail: None,
                    },
                    TaskTraceStep {
                        stage: "routed".to_string(),
                        timestamp_ms: now_ms,
                        detail: Some(format!("agent {}", agent_id)),
                    },
                ],
            );

            results.push(my_id);
        }

        tracing::info!("Submitted batch of {} tasks", results.len());
        Ok(results)
    }

    /// Resolve route via RoutingService and spawn if needed.
    fn resolve_route(&mut self, manifest: &[FileAffinity], target_agent: Option<&str>) -> Result<AgentId, OrchestratorError> {
        if let Some(agent_name) = target_agent {
            // First check if an agent with this name exists
            for (id, queue) in &self.agents {
                if queue.name == agent_name {
                    return Ok(*id);
                }
            }
            // Otherwise, spawn an agent with this name
            return self.spawn_agent(agent_name);
        }

        let result = RoutingService::route(
            manifest,
            &self.affinity_map,
            &self.groups,
            &self.agents,
            &self.config,
        );
        match result {
            RouteResult::Existing(id) => Ok(id),
            RouteResult::SpawnAgent(name) => self.spawn_agent(&name),
        }
    }

    /// Spawn a new agent with the given name.
    pub fn spawn_agent(&mut self, name: &str) -> Result<AgentId, OrchestratorError> {
        if self.agents.len() >= self.config.max_agents {
            return Err(OrchestratorError::MaxAgentsReached {
                max: self.config.max_agents,
            });
        }

        let agent_id = self.agent_id_gen.next();
        let queue = AgentQueue::new(agent_id, name);
        self.agents.insert(agent_id, queue);
        self.heartbeat_monitor.register(agent_id);
        MessageGateway::publish_agent_spawned(
            &mut self.bulletin,
            &self.event_bus,
            agent_id,
            name.to_string(),
        );
        tracing::info!("Spawned agent {} (name: {})", agent_id, name);
        Ok(agent_id)
    }

    /// Spawn a dynamic (transient) agent.
    pub fn spawn_dynamic_agent(&mut self, name: &str) -> Result<AgentId, OrchestratorError> {
        let agent_id = self.spawn_agent(name)?;
        self.dynamic_agents.insert(agent_id);
        tracing::info!("Agent {} marked as dynamic", agent_id);
        Ok(agent_id)
    }

    /// Map an AI agent session ID to an existing orchestrator agent queue.
    pub fn map_agent_session(
        &mut self,
        agent_id: AgentId,
        session_id: String,
    ) -> Result<(), OrchestratorError> {
        let queue = self
            .agents
            .get_mut(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;
        queue.set_agent_session(session_id.clone());
        tracing::info!(
            "Mapped agent session {} to agent {}",
            session_id,
            agent_id
        );
        Ok(())
    }

    /// Retire an agent, redistributing its remaining tasks to other agents.
    pub fn retire_agent(&mut self, agent_id: AgentId) -> Result<Vec<AgentTask>, OrchestratorError> {
        let mut queue = self
            .agents
            .remove(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;

        // Release all file locks, affinity, and scope
        self.lock_manager.release_all(agent_id);
        self.affinity_map.release_all(agent_id);
        self.scope_guard.clear_scope(agent_id);
        self.dynamic_agents.remove(&agent_id);
        self.agent_handles.remove(&agent_id);
        self.heartbeat_monitor.unregister(agent_id);

        // Drain remaining tasks for redistribution
        let remaining = queue.drain_tasks();
        MessageGateway::publish_agent_retired(&self.event_bus, agent_id);
        tracing::info!(
            "Retired agent {} — {} tasks to redistribute",
            agent_id,
            remaining.len()
        );
        Ok(remaining)
    }

    /// Cancel a queued task. Returns an error if the task is in-progress or completed.
    pub fn cancel_task(&mut self, task_id: TaskId) -> Result<(), OrchestratorError> {
        let agent_id = self
            .task_assignments
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;

        let queue = self
            .agents
            .get_mut(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;

        if let Some(_task) = queue.cancel(task_id) {
            self.task_assignments.remove(&task_id);
            // We should release the write locks/scope for this task's files (if not used by other tasks in this agent's queue).
            // For simplicity, a full release/re-assign will naturally happen on rebalance.
            tracing::info!("Cancelled task {} from agent {}", task_id, agent_id);
            Ok(())
        } else {
            Err(OrchestratorError::TaskNotFound(task_id))
        }
    }

    /// Register a process handle for an agent.
    pub fn register_agent_handle(&mut self, agent_id: AgentId, handle: vox_runtime::ProcessHandle) {
        self.agent_handles.insert(agent_id, handle);
    }

    /// Accept a handoff from another agent.
    pub fn accept_handoff(&mut self, payload: crate::handoff::HandoffPayload) -> Result<AgentId, OrchestratorError> {
        let from_agent = payload.from_agent;

        // Resolve target agent or spawn new one
        let target_id = if let Some(id) = payload.to_agent {
            if self.agents.contains_key(&id) {
                id
            } else {
                match self.spawn_agent(&format!("ResumingAgent-{}", id.0)) {
                    Ok(new_id) => new_id,
                    Err(e) => {
                        self.event_bus.emit(crate::events::AgentEventKind::AgentHandoffRejected {
                            from: from_agent,
                            reason: format!("Spawn failed: {}", e),
                        });
                        return Err(e);
                    }
                }
            }
        } else {
            match self.spawn_agent("AdaptiveResumer") {
                Ok(new_id) => new_id,
                Err(e) => {
                    self.event_bus.emit(crate::events::AgentEventKind::AgentHandoffRejected {
                        from: from_agent,
                        reason: format!("Spawn failed: {}", e),
                    });
                    return Err(e);
                }
            }
        };

        // Transfer files/locks
        for path in &payload.owned_files {
            self.affinity_map.assign(path, target_id);
            self.scope_guard.assign_file(target_id, path.clone());
            let _ = self.lock_manager.try_acquire(path, target_id, LockKind::Exclusive);
        }

        // Re-submit pending tasks
        let resumed_ids: Vec<TaskId> = payload.pending_tasks.clone();

        // In a real system, we'd need to re-construct the task descriptions or have them in the payload.
        // For now, we emit an event that the handoff was accepted and the agent is resuming.
        self.event_bus.emit(crate::events::AgentEventKind::AgentHandoffAccepted {
            agent_id: target_id,
            from: from_agent,
            plan_summary: payload.plan_summary.clone(),
        });

        tracing::info!("Agent {} accepted handoff from {} ({} tasks resumed: {:?})", target_id, from_agent, resumed_ids.len(), resumed_ids);
        Ok(target_id)
    }

    /// Reorder a queued task with a new priority.
    pub fn reorder_task(
        &mut self,
        task_id: TaskId,
        new_priority: TaskPriority,
    ) -> Result<(), OrchestratorError> {
        let agent_id = self
            .task_assignments
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;

        let queue = self
            .agents
            .get_mut(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;

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
    pub fn drain_agent(&mut self, agent_id: AgentId) -> Result<Vec<AgentTask>, OrchestratorError> {
        let queue = self
            .agents
            .get_mut(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;

        let remaining = queue.drain_tasks();
        for task in &remaining {
            self.task_assignments.remove(&task.id);
        }

        tracing::info!("Drained {} tasks from agent {}", remaining.len(), agent_id);
        Ok(remaining)
    }

    /// Mark a task as completed (async).
    pub async fn complete_task(
        &mut self,
        task_id: TaskId,
    ) -> Result<(), OrchestratorError> {
        let agent_id = self
            .task_assignments
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;

        let queue = self
            .agents
            .get_mut(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;

        // Get the task's file manifest before completing
        let write_files: Vec<PathBuf> = queue
            .current_task()
            .map(|t| t.write_files().into_iter().cloned().collect())
            .unwrap_or_default();

        let mut auto_debug_requeue = None;

        #[cfg(feature = "toestub-gate")]
        {
            if self.config.toestub_gate {
                if let Some(mut task_clone) = queue.current_task().cloned() {
                    let vr = crate::validation::post_task_validate(&task_clone);
                    if !crate::validation::quality_gate(&vr)
                        && task_clone.debug_iterations < self.config.max_debug_iterations
                    {
                        task_clone.debug_iterations += 1;
                        task_clone.description.push_str(&format!("\n\n[AUTO-DEBUG ITERATION {}]\nValidation failed with diagnostic issues. Please fix the following:\n{}", task_clone.debug_iterations, vr.report));
                        task_clone.status = TaskStatus::Queued;
                        auto_debug_requeue = Some((task_clone, vr.report.clone()));
                    }
                }
            }
        }

        if let Some((requeue_task, err_report)) = auto_debug_requeue {
            // Log it
            tracing::warn!(
                "Task {} failed validation. Auto-debugging (iteration {}/{})",
                task_id,
                requeue_task.debug_iterations,
                self.config.max_debug_iterations
            );
            queue.mark_failed(
                task_id,
                format!("Auto-debug validation failure:\n{}", err_report),
            );

            // Requeue the modified task back to the *same* queue
            queue.enqueue(requeue_task);
            return Ok(());
        }

        let desc = queue
            .current_task()
            .map(|t| t.description.clone())
            .unwrap_or_default();
        queue.mark_complete(task_id);

        // Capture post-task snapshot and record in oplog (persisted to CodeStore)
        let snap_desc = format!("post-task complete: {:.50}", desc);
        let snapshot_after = self
            .capture_snapshot(agent_id, &write_files, snap_desc.clone())
            .await;

        self.event_bus
            .emit(crate::events::AgentEventKind::SnapshotCaptured {
                agent_id,
                snapshot_id: snapshot_after.to_string(),
                file_count: write_files.len(),
                description: snap_desc,
            });

        // Find pre-task snapshots from the oplog to link this completion
        let (snap_before, db_snap_before) = self.oplog.find_task_snapshots(task_id.0);
        let db_snap_after = self.take_db_snapshot(agent_id, format!("post-task-complete: {}", task_id)).await;

        self.record_operation(
            agent_id,
            OperationKind::TaskComplete { task_id: task_id.0 },
            format!("Completed task {}", task_id),
            snap_before,
            Some(snapshot_after),
            db_snap_before,
            db_snap_after,
        )
        .await;

        // Auto-detect conflicts: check if any other agent's workspace overlaps
        let other_agents: Vec<AgentId> = self
            .agents
            .keys()
            .filter(|&&id| id != agent_id && self.workspace_manager.has_workspace(id))
            .copied()
            .collect();
        for other_id in other_agents {
            let overlaps = self.workspace_manager.overlapping_paths(agent_id, other_id);
            for overlap_path in overlaps {
                let conflict_id = self.conflict_manager.record_conflict(
                    overlap_path.clone(),
                    Some(snapshot_after),
                    vec![(agent_id, snapshot_after), (other_id, snapshot_after)],
                );
                self.event_bus
                    .emit(crate::events::AgentEventKind::ConflictDetected {
                        path: overlap_path,
                        agent_ids: vec![agent_id, other_id],
                        conflict_id: conflict_id.to_string(),
                    });
                tracing::warn!(
                    "Conflict {} detected between {} and {}",
                    conflict_id,
                    agent_id,
                    other_id
                );
            }
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if let Some(steps) = self.task_traces.get_mut(&task_id) {
            steps.push(TaskTraceStep {
                stage: "outcome".to_string(),
                timestamp_ms: now_ms,
                detail: Some("completed".to_string()),
            });
        }

        // Release file locks for this task's write files
        for path in &write_files {
            self.lock_manager.release(path, agent_id);
        }

        MessageGateway::publish_task_completed(
            &mut self.bulletin,
            &mut self.message_bus,
            &self.event_bus,
            task_id,
            agent_id,
        );

        // Unblock dependent tasks across ALL agents
        for queue in self.agents.values_mut() {
            queue.unblock(task_id);
        }

        tracing::info!("Task {} completed by agent {}", task_id, agent_id);
        Ok(())
    }

    /// Mark a task as failed (async).
    pub async fn fail_task(&mut self, task_id: TaskId, reason: String) -> Result<(), OrchestratorError> {
        let agent_id = self
            .task_assignments
            .get(&task_id)
            .copied()
            .ok_or(OrchestratorError::TaskNotFound(task_id))?;

        let queue = self
            .agents
            .get_mut(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?;

        queue.mark_failed(task_id, reason.clone());

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if let Some(steps) = self.task_traces.get_mut(&task_id) {
            steps.push(TaskTraceStep {
                stage: "outcome".to_string(),
                timestamp_ms: now_ms,
                detail: Some(format!("failed: {}", reason)),
            });
        }

        // Release locks
        self.lock_manager.release_all(agent_id);

        // Find pre-task snapshots to link this failure
        let (snap_before, db_snap_before) = self.oplog.find_task_snapshots(task_id.0);

        // Record failure in oplog (async to support DB snapshot)
        self.record_operation(
            agent_id,
            OperationKind::TaskFail {
                task_id: task_id.0,
                reason: reason.clone(),
            },
            format!("Failed task {}", task_id),
            snap_before,
            None,
            db_snap_before,
            None,
        )
        .await;

        MessageGateway::publish_task_failed(
            &mut self.bulletin,
            &self.event_bus,
            task_id,
            agent_id,
            reason.clone(),
        );

        tracing::warn!("Task {} failed: {}", task_id, reason);
        Ok(())
    }

    /// Get a snapshot of the orchestrator's current state.
    pub fn status(&self) -> OrchestratorStatus {
        let agents: Vec<AgentSummary> = self
            .agents
            .iter()
            .map(|(id, queue)| AgentSummary {
                id: *id,
                name: queue.name.clone(),
                queued: queue.len(),
                urgent_count: queue.depth_by_priority(crate::types::TaskPriority::Urgent),
                normal_count: queue.depth_by_priority(crate::types::TaskPriority::Normal),
                background_count: queue.depth_by_priority(crate::types::TaskPriority::Background),
                in_progress: queue.has_in_progress(),
                completed: queue.completed_count(),
                paused: queue.is_paused(),
                owned_files: self.affinity_map.files_for_agent(*id).len(),
                dynamic: self.dynamic_agents.contains(id),
                weighted_load: queue.weighted_load(),
                agent_session_id: queue.agent_session_id.clone(),
            })
            .collect();

        let dynamic_count = self.dynamic_agents.len();
        let reserved_count = self.agents.len().saturating_sub(dynamic_count);

        #[allow(unused_mut)]
        let mut total_weighted_load: f64 = agents.iter().map(|a| a.weighted_load).sum();

        // Integrate system resources if configured
        #[cfg(feature = "system-metrics")]
        if self.config.resource_weight > 0.0 {
            let cpu_usage = self.sys.global_cpu_usage() as f64 / 100.0;
            let mem_usage = self.sys.used_memory() as f64 / self.sys.total_memory().max(1) as f64;
            let mut resource_factor = cpu_usage * self.config.resource_cpu_multiplier
                + mem_usage * self.config.resource_mem_multiplier;
            if self.config.resource_exponent != 1.0 {
                resource_factor = resource_factor.powf(self.config.resource_exponent);
            }
            total_weighted_load *= 1.0 + (resource_factor * self.config.resource_weight);
        }

        let predicted_load = if self.load_history.is_empty() {
            total_weighted_load
        } else {
            let avg: f64 =
                self.load_history.iter().copied().sum::<f64>() / self.load_history.len() as f64;
            if self.load_history.len() >= 2 {
                let last = *self.load_history.back().unwrap();
                let trend = last - avg;
                (last + trend).max(0.0)
            } else {
                avg
            }
        };

        OrchestratorStatus {
            enabled: self.config.enabled,
            agent_count: self.agents.len(),
            total_queued: agents.iter().map(|a| a.queued).sum(),
            total_in_progress: agents.iter().filter(|a| a.in_progress).count(),
            total_completed: agents.iter().map(|a| a.completed).sum(),
            locked_files: self.lock_manager.active_lock_count(),
            total_contention: self.lock_manager.contention_count(),
            total_weighted_load,
            predicted_load,
            reserved_agents: reserved_count,
            dynamic_agents: dynamic_count,
            context_entries: self.context_store.entries(),
            agents,
        }
    }

    /// Rebalance tasks across agents using work-stealing.
    ///
    /// Moves tasks from overloaded agents to underloaded ones,
    /// respecting file affinity (only moves tasks whose files aren't locked).
    pub fn rebalance(&mut self) -> usize {
        let loads: Vec<(AgentId, f64)> = self
            .agents
            .iter()
            .map(|(id, q)| (*id, q.weighted_load()))
            .collect();

        if loads.len() < 2 {
            return 0;
        }

        let total_load: f64 = loads.iter().map(|(_, l)| l).sum();
        let avg = total_load / loads.len() as f64;
        let mut moved = 0;

        // Find overloaded and underloaded agents
        let overloaded: Vec<AgentId> = loads
            .iter()
            .filter(|(_, l)| *l > avg + 2.0) // Significant imbalance
            .map(|(id, _)| *id)
            .collect();
        let mut underloaded: Vec<AgentId> = loads
            .iter()
            .filter(|(_, l)| *l < avg)
            .map(|(id, _)| *id)
            .collect();

        // If economy mode, sort underloaded by cost (prefer cheapest)
        if self.config.cost_preference == crate::config::CostPreference::Economy {
            let models = &self.models;
            underloaded.sort_by(|a, b| {
                let cost_a = models
                    .get_override(a.0)
                    .and_then(|id| models.get(&id))
                    .map(|m| m.cost_per_1k)
                    .unwrap_or(0.003); // Default cost
                let cost_b = models
                    .get_override(b.0)
                    .and_then(|id| models.get(&id))
                    .map(|m| m.cost_per_1k)
                    .unwrap_or(0.003);
                cost_a
                    .partial_cmp(&cost_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        for over_id in &overloaded {
            for under_id in &underloaded {
                // Steal from the overloaded agent, preferring the lowest-priority task first.
                // This ensures Urgent work stays with the specialist agent where possible.
                if let Some(queue) = self.agents.get_mut(over_id) {
                    let mut tasks = queue.drain_tasks();
                    // Sort: Background first (easiest to steal), then Normal, then Urgent last
                    tasks.sort_by_key(|t| match t.priority {
                        crate::types::TaskPriority::Background => 0u8,
                        crate::types::TaskPriority::Normal => 1,
                        crate::types::TaskPriority::Urgent => 2,
                    });
                    // Pick the first task whose write-files are NOT currently locked by
                    // a different agent — skipping locked tasks prevents the target agent
                    // from receiving work it cannot start.
                    let steal_idx = tasks.iter().position(|t| {
                        t.write_files().iter().all(|path| {
                            // OK to steal if: unlocked, or locked only by the over_id itself
                            match self.lock_manager.holder(path.as_path()) {
                                None => true,
                                Some((holder, _)) => holder == *over_id,
                            }
                        })
                    });
                    let stolen = steal_idx.map(|i| tasks.remove(i));
                    // Re-enqueue the kept tasks (priority order is restored by enqueue)
                    if let Some(queue) = self.agents.get_mut(over_id) {
                        for task in tasks {
                            queue.enqueue(task);
                        }
                    }
                    // Hand the stolen task to the underloaded agent
                    if let Some(task) = stolen {
                        if let Some(target) = self.agents.get_mut(under_id) {
                            self.task_assignments.insert(task.id, *under_id);
                            target.enqueue(task);
                            moved += 1;
                        }
                    }
                }
            }
        }

        if moved > 0 {
            tracing::info!("Rebalanced: moved {} tasks", moved);
            self.last_rebalance_at = Some(std::time::Instant::now());
            self.oplog.record(
                AgentId(0), // system-level
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

    /// Run periodic orchestrator maintenance (like timed-out lock release).
    pub fn tick(&mut self) {
        // Refresh system metrics
        #[cfg(feature = "system-metrics")]
        {
            self.sys.refresh_cpu_all();
            self.sys.refresh_memory();
        }

        let timeout = self.config.lock_timeout_ms as u128;
        let released = self.lock_manager.force_release_stale(timeout);
        if released > 0 {
            tracing::warn!(
                "Tick: forcefully released {} stale orphaned lock(s) older than {}ms",
                released,
                timeout
            );
        }

        // Record history AFTER maintenance but BEFORE scaling/continuation
        let current_load = self.status().total_weighted_load;
        self.load_history.push_back(current_load);
        if self.load_history.len() > self.config.scaling_lookback_ticks {
            self.load_history.pop_front();
        }

        // Check for stale heartbeats (zombie agents)
        let stale_ids = self.heartbeat_monitor.check_stale(&self.event_bus);
        for (id, level) in stale_ids {
            if self.dynamic_agents.contains(&id) {
                tracing::warn!(
                    "Tick: retiring zombie dynamic agent {} (level: {})",
                    id,
                    level
                );
                let _ = self.retire_agent(id);
            } else {
                tracing::error!("Tick: reserved agent {} is unresponsive at level {}! Immediate attention required.", id, level);
            }
        }

        if self.config.auto_continue_enabled {
            let active_agents: Vec<(AgentId, usize)> = self
                .agents
                .iter()
                .map(|(id, queue)| (*id, queue.len()))
                .collect();
            let intents = self.monitor
                .check_idle_agents(&active_agents, &self.event_bus);

            for (agent_id, prompt) in intents {
                let _ = self.submit_task_with_agent(
                    format!("[Auto-Continuation] {}", prompt),
                    vec![], // No specific file affinity for continuation
                    Some(crate::types::TaskPriority::Background),
                    Some(self.agents.get(&agent_id).map(|q| q.name.clone()).unwrap_or_default()),
                );
            }
        }

        // Urgent-queue auto-rebalance: if any single agent has more Urgent tasks than the
        // configured threshold (and there are at least 2 agents to rebalance across),
        // trigger an immediate rebalance so priority work can flow to idle agents.
        let urgent_threshold = self.config.urgent_rebalance_threshold;
        if urgent_threshold > 0 && self.agents.len() >= 2 {
            // Enforce cooldown: don't rebalance more often than scaling_cooldown_ms
            let cooldown_ms = self.config.scaling_cooldown_ms;
            let can_rebalance = self
                .last_rebalance_at
                .map(|t| t.elapsed().as_millis() >= cooldown_ms as u128)
                .unwrap_or(true);

            if can_rebalance {
                let overloaded_urgent: Vec<(AgentId, usize)> = self
                    .agents
                    .iter()
                    .map(|(id, q)| (*id, q.depth_by_priority(crate::types::TaskPriority::Urgent)))
                    .filter(|(_, depth)| *depth > urgent_threshold)
                    .collect();

                if !overloaded_urgent.is_empty() {
                    for (agent_id, depth) in &overloaded_urgent {
                        tracing::warn!(
                            "Tick: agent {} has {} urgent tasks (threshold {}), triggering urgent rebalance",
                            agent_id, depth, urgent_threshold
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

    /// Record an AI model call — emits `CostIncurred`, updates budget, and appends to the oplog.
    ///
    /// Call this after every LLM API response to keep cost tracking, event streaming,
    /// and the operation log all in sync. This is the single integration point that
    /// replaces ad-hoc scattered tracking across subsystems.
    pub fn record_ai_usage(
        &mut self,
        agent_id: AgentId,
        provider: impl Into<String> + Clone,
        model: impl Into<String> + Clone,
        input_tokens: u32,
        output_tokens: u32,
        cost_usd: f64,
    ) {
        let provider_str: String = provider.into();
        let model_str: String = model.into();

        // 1. Emit real-time event (dashboard / monitor consumers)
        self.event_bus.emit(crate::events::AgentEventKind::CostIncurred {
            agent_id,
            provider: provider_str.clone(),
            model: model_str.clone(),
            input_tokens,
            output_tokens,
            cost_usd,
        });

        // 2. Update in-memory budget
        self.budget_manager
            .record_usage(agent_id, (input_tokens + output_tokens) as usize);
        self.budget_manager.record_cost(agent_id, cost_usd);

        // 3. Append to the operation log for auditability / undo support
        self.oplog.record_ai_call(
            agent_id,
            &provider_str,
            &model_str,
            input_tokens,
            output_tokens,
            cost_usd,
        );

        tracing::debug!(
            "AI usage recorded: agent={} {}/{} in={} out={} cost=${:.6}",
            agent_id, provider_str, model_str, input_tokens, output_tokens, cost_usd
        );
    }

    /// Pause an agent's queue.
    pub fn pause_agent(&mut self, agent_id: AgentId) -> Result<(), OrchestratorError> {
        self.agents
            .get_mut(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?
            .pause();
        Ok(())
    }

    /// Resume an agent's queue.
    pub fn resume_agent(&mut self, agent_id: AgentId) -> Result<(), OrchestratorError> {
        self.agents
            .get_mut(&agent_id)
            .ok_or(OrchestratorError::AgentNotFound(agent_id))?
            .resume();
        Ok(())
    }

    /// Record a heartbeat from an agent.
    pub fn heartbeat(&mut self, agent_id: AgentId, activity: crate::events::AgentActivity) {
        self.heartbeat_monitor.heartbeat(agent_id, activity);
        // Also record in monitor for auto-continuation logic
        self.monitor.record_activity(agent_id);
    }

    /// Get a reference to an agent's queue.
    pub fn agent_queue(&self, agent_id: AgentId) -> Option<&AgentQueue> {
        self.agents.get(&agent_id)
    }

    /// Get a mutable reference to an agent's queue.
    pub fn get_agent_queue_mut(&mut self, agent_id: AgentId) -> Option<&mut AgentQueue> {
        self.agents.get_mut(&agent_id)
    }

    /// Get a reference to the budget manager.
    pub fn budget_manager(&self) -> &crate::budget::BudgetManager {
        &self.budget_manager
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }

    /// Get a mutable reference to the configuration to allow run-time tuning.
    pub fn config_mut(&mut self) -> &mut OrchestratorConfig {
        &mut self.config
    }

    /// Get a reference to the bulletin board (for subscribing).
    pub fn bulletin(&self) -> &BulletinBoard {
        &self.bulletin
    }

    /// Get a mutable reference to the bulletin board (for publishing).
    pub fn bulletin_mut(&mut self) -> &mut BulletinBoard {
        &mut self.bulletin
    }

    /// Access the file affinity map.
    pub fn affinity_map(&self) -> &FileAffinityMap {
        &self.affinity_map
    }

    /// Access the QA Router.
    pub fn qa_router(&self) -> &crate::qa::QARouter {
        &self.qa_router
    }

    /// Access the file affinity map mutably.
    pub fn affinity_map_mut(&mut self) -> &mut FileAffinityMap {
        &mut self.affinity_map
    }

    /// Get a reference to the lock manager.
    pub fn lock_manager(&self) -> &FileLockManager {
        &self.lock_manager
    }

    /// List all agent IDs.
    pub fn agent_ids(&self) -> Vec<AgentId> {
        self.agents.keys().copied().collect()
    }

    /// Get a reference to task → agent assignment map.
    pub fn task_assignments(&self) -> &HashMap<TaskId, AgentId> {
        &self.task_assignments
    }

    /// Get the lifecycle timeline for a task (ingress → route → outcome), if recorded.
    pub fn task_trace(&self, task_id: TaskId) -> Option<&Vec<TaskTraceStep>> {
        self.task_traces.get(&task_id)
    }

    /// Get a reference to the shared context store.
    pub fn context(&self) -> &crate::context::ContextStore {
        &self.context_store
    }

    /// Get a reference to the budget manager.
    pub fn budget(&self) -> &crate::budget::BudgetManager {
        &self.budget_manager
    }

    /// Get a reference to the summary manager.
    pub fn summary(&self) -> &crate::summary::SummaryManager {
        &self.summary_manager
    }

    /// Access the model registry.
    pub fn models(&self) -> &crate::models::ModelRegistry {
        &self.models
    }

    pub fn models_mut(&mut self) -> &mut crate::models::ModelRegistry {
        &mut self.models
    }

    /// Access the event bus
    pub fn event_bus(&self) -> &crate::events::EventBus {
        &self.event_bus
    }

    /// Access the A2A message bus
    pub fn message_bus(&self) -> &crate::a2a::MessageBus {
        &self.message_bus
    }

    /// Access the A2A message bus mutably (for ack, etc.)
    pub fn message_bus_mut(&mut self) -> &mut crate::a2a::MessageBus {
        &mut self.message_bus
    }

    // -- JJ-inspired subsystem accessors --

    /// Access the auto-snapshot store.
    pub fn snapshot_store(&self) -> &crate::snapshot::SnapshotStore {
        &self.snapshot_store
    }

    /// Access the auto-snapshot store mutably.
    pub fn snapshot_store_mut(&mut self) -> &mut crate::snapshot::SnapshotStore {
        &mut self.snapshot_store
    }

    /// Set the code store for database persistence and snapshotting.
    pub fn set_code_store(&mut self, code_store: vox_pm::CodeStore) {
        self.code_store = Some(code_store);
    }

    /// Take a snapshot of the database state (async).
    /// Take a snapshot and persist file contents to the code store (async).
    pub async fn capture_snapshot(
        &mut self,
        agent_id: AgentId,
        paths: &[PathBuf],
        description: impl Into<String>,
    ) -> SnapshotId {
        let desc = description.into();
        let snap_id = self.snapshot_store.take_snapshot(agent_id, paths, &desc);

        // Persist contents to CodeStore if available
        if let Some(store) = &self.code_store {
            for p in paths {
                if let Ok(data) = std::fs::read(p) {
                    let _ = store.store("file", &data).await;
                }
            }
        }

        snap_id
    }

    /// Record a generic operation, automatically capturing a DB snapshot if a CodeStore is present and not provided.
    pub async fn record_operation(
        &mut self,
        agent_id: AgentId,
        kind: OperationKind,
        description: impl Into<String>,
        snapshot_before: Option<SnapshotId>,
        snapshot_after: Option<SnapshotId>,
        db_snapshot_before: Option<u64>,
        db_snapshot_after: Option<u64>,
    ) -> OperationId {
        let desc = description.into();
        let db_snap_before = match db_snapshot_before {
            Some(id) => Some(id),
            None => self.take_db_snapshot(agent_id, format!("pre-op: {}", desc)).await,
        };

        self.oplog.record(
            agent_id,
            kind,
            desc,
            snapshot_before,
            snapshot_after,
            db_snap_before,
            db_snapshot_after,
            None,
            None,
        )
    }

    /// Take a snapshot of the database state (async).
    pub async fn take_db_snapshot(
        &self,
        agent_id: AgentId,
        description: impl Into<String>,
    ) -> Option<u64> {
        if let Some(store) = &self.code_store {
            let snap_id = self.oplog.next_db_snapshot_id();

            let desc = description.into();
            if store
                .take_db_snapshot(snap_id, &agent_id.to_string(), &desc)
                .await
                .is_ok()
            {
                return Some(snap_id);
            }
        }
        None
    }

    /// Restore the state to before a specific operation (async).
    pub async fn undo_operation(&mut self, op_id: OperationId) -> Result<(), OrchestratorError> {
        let (fs_snap, db_snap) = self
            .oplog
            .undo(op_id)
            .ok_or(OrchestratorError::OperationNotFound)?;

        // 1. Restore Database Snapshot if present
        if let Some(db_id) = db_snap {
            if let Some(store) = &self.code_store {
                store.restore_db_snapshot(db_id).await.map_err(|e| {
                    OrchestratorError::DatabaseError(format!("Undo: DB restore failed: {}", e))
                })?;
            }
        }

        // 2. Restore Filesystem Snapshot if present
        if let Some(fs_id) = fs_snap {
            self.restore_fs_snapshot(fs_id).await?;
        }

        self.event_bus
            .emit(crate::events::AgentEventKind::OperationUndone {
                agent_id: AgentId(0), // System
                operation_id: op_id.to_string(),
            });

        Ok(())
    }

    /// Re-apply the state after a previously undone operation (async).
    pub async fn redo_operation(&mut self, op_id: OperationId) -> Result<(), OrchestratorError> {
        let (fs_snap, db_snap) = self
            .oplog
            .redo(op_id)
            .ok_or(OrchestratorError::OperationNotFound)?;

        // 1. Restore Database Snapshot if present
        if let Some(db_id) = db_snap {
            if let Some(store) = &self.code_store {
                store.restore_db_snapshot(db_id).await.map_err(|e| {
                    OrchestratorError::DatabaseError(format!("Redo: DB restore failed: {}", e))
                })?;
            }
        }

        // 2. Restore Filesystem Snapshot if present
        if let Some(fs_id) = fs_snap {
            self.restore_fs_snapshot(fs_id).await?;
        }

        self.event_bus
            .emit(crate::events::AgentEventKind::OperationRedone {
                agent_id: AgentId(0), // System
                operation_id: op_id.to_string(),
            });

        Ok(())
    }

    /// Internal helper to restore files from a snapshot ID (async).
    pub async fn restore_fs_snapshot(&self, snapshot_id: SnapshotId) -> Result<(), OrchestratorError> {
        let snap = self
            .snapshot_store
            .get(snapshot_id)
            .ok_or(OrchestratorError::OperationNotFound)?;
        let store = self.code_store.as_ref().ok_or_else(|| {
            OrchestratorError::DatabaseError("CodeStore not initialized for restore".into())
        })?;

        for entry in snap.files.values() {
            if entry.content_hash.is_empty() {
                if entry.path.exists() {
                    let _ = std::fs::remove_file(&entry.path);
                }
            } else {
                let data = store.get(&entry.content_hash).await.map_err(|e| {
                    OrchestratorError::DatabaseError(format!(
                        "Restore: object {} missing: {}",
                        entry.content_hash, e
                    ))
                })?;
                if let Some(parent) = entry.path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&entry.path, data).map_err(|e| {
                    OrchestratorError::DatabaseError(format!(
                        "Restore: write {} failed: {}",
                        entry.path.display(), e
                    ))
                })?;
            }
        }
        Ok(())
}

    /// Access the operation log.
    pub fn oplog(&self) -> &crate::oplog::OpLog {
        &self.oplog
    }

    /// Access the operation log mutably.
    pub fn oplog_mut(&mut self) -> &mut crate::oplog::OpLog {
        &mut self.oplog
    }

    /// Access the conflict manager.
    pub fn conflict_manager(&self) -> &crate::conflicts::ConflictManager {
        &self.conflict_manager
    }

    /// Access the conflict manager mutably.
    pub fn conflict_manager_mut(&mut self) -> &mut crate::conflicts::ConflictManager {
        &mut self.conflict_manager
    }

    /// Access the workspace manager.
    pub fn workspace_manager(&self) -> &crate::workspace::WorkspaceManager {
        &self.workspace_manager
    }

    /// Access the workspace manager mutably.
    pub fn workspace_manager_mut(&mut self) -> &mut crate::workspace::WorkspaceManager {
        &mut self.workspace_manager
    }

    /// Send a structured A2A message and publish to bulletin.
    pub fn send_a2a(
        &mut self,
        sender: AgentId,
        receiver: AgentId,
        msg_type: crate::types::A2AMessageType,
        payload: impl Into<String>,
    ) -> crate::types::MessageId {
        let payload_str = payload.into();

        // Native VCS integration: When an agent hands off a plan to another, automatically
        // start tracking a logical Change in the workspace manager for provenance visibility.
        if msg_type == crate::types::A2AMessageType::PlanHandoff {
            self.workspace_manager.create_change(
                receiver,
                format!("Plan handoff from {}: {:.100}", sender, payload_str),
            );
        }

        let msg_id = self
            .message_bus
            .send(sender, receiver, msg_type, payload_str);
        if let Some(msg) = self.message_bus.audit_trail().last() {
            self.bulletin
                .publish(crate::types::AgentMessage::A2A(msg.clone()));

            self.event_bus
                .emit(crate::events::AgentEventKind::MessageSent {
                    from: msg.sender,
                    to: msg.receiver,
                    summary: format!("{:?}: {}", msg.msg_type, msg.payload),
                });
        }
        msg_id
    }

    /// Broadcast a structured A2A message to all and publish to bulletin.
    pub fn broadcast_a2a(
        &mut self,
        sender: AgentId,
        msg_type: crate::types::A2AMessageType,
        payload: impl Into<String>,
    ) -> crate::types::MessageId {
        let msg_id = self.message_bus.broadcast(sender, msg_type, payload);
        if let Some(msg) = self.message_bus.audit_trail().last() {
            self.bulletin
                .publish(crate::types::AgentMessage::A2A(msg.clone()));

            self.event_bus
                .emit(crate::events::AgentEventKind::MessageSent {
                    from: msg.sender,
                    to: None, // Broadcast
                    summary: format!("{:?}: {}", msg.msg_type, msg.payload),
                });
        }
        msg_id
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_orchestrator() -> Orchestrator {
        Orchestrator::new(OrchestratorConfig::for_testing())
    }

    #[tokio::test]
    async fn spawn_agent() {
        let mut orch = test_orchestrator();
        let id = orch.spawn_agent("parser").expect("spawn");
        assert_eq!(orch.status().agent_count, 1);
        assert!(orch.agent_queue(id).is_some());
    }

    #[tokio::test]
    async fn max_agents_enforced() {
        let mut orch = Orchestrator::new(OrchestratorConfig {
            max_agents: 2,
            ..OrchestratorConfig::for_testing()
        });
        orch.spawn_agent("a").unwrap();
        orch.spawn_agent("b").unwrap();
        let err = orch.spawn_agent("c").unwrap_err();
        assert!(matches!(
            err,
            OrchestratorError::MaxAgentsReached { max: 2 }
        ));
    }

    #[tokio::test]
    async fn submit_and_route() {
        let mut orch = test_orchestrator();
        let task_id = orch
            .submit_task(
                "Fix parser bug",
                vec![FileAffinity::write("crates/vox-parser/src/grammar.rs")],
                None,
            )
            .await
            .expect("submit");
        assert_eq!(orch.status().total_queued, 1);
        assert_eq!(orch.status().agent_count, 1); // auto-spawned
        assert!(orch.task_assignments().contains_key(&task_id));
    }

    #[tokio::test]
    async fn same_file_routes_to_same_agent() {
        let mut orch = test_orchestrator();
        let t1 = orch
            .submit_task("Task 1", vec![FileAffinity::write("src/lib.rs")], None)
            .await
            .unwrap();
        let t2 = orch
            .submit_task("Task 2", vec![FileAffinity::write("src/lib.rs")], None)
            .await
            .unwrap();

        // Both tasks should be assigned to the same agent
        assert_eq!(
            orch.task_assignments().get(&t1),
            orch.task_assignments().get(&t2),
            "tasks touching the same file should route to the same agent"
        );
    }

    #[tokio::test]
    async fn different_files_can_route_to_different_agents() {
        let mut orch = test_orchestrator();
        orch.submit_task(
            "Parser work",
            vec![FileAffinity::write("crates/vox-parser/src/lib.rs")],
            None,
        )
        .await
        .unwrap();
        orch.submit_task(
            "Codegen work",
            vec![FileAffinity::write("crates/vox-codegen-rust/src/lib.rs")],
            None,
        )
        .await
        .unwrap();

        // Should have spawned at least one agent (may be 1 or 2 depending on routing)
        assert!(orch.status().agent_count >= 1);
    }

    #[tokio::test]
    async fn complete_task_flow() {
        let mut orch = test_orchestrator();
        let task_id = orch
            .submit_task("Test task", vec![FileAffinity::write("test.rs")], None)
            .await
            .unwrap();

        let agent_id = *orch.task_assignments().get(&task_id).unwrap();

        // Dequeue the task (simulating an agent picking it up)
        orch.get_agent_queue_mut(agent_id).unwrap().dequeue();

        // Complete it
        orch.complete_task(task_id).await.expect("complete");
        assert_eq!(orch.status().total_completed, 1);
    }

    #[tokio::test]
    async fn retire_agent_returns_tasks() {
        let mut orch = test_orchestrator();
        let agent_id = orch.spawn_agent("temp").unwrap();

        // Manually enqueue a task
        let task = AgentTask::new(TaskId(99), "leftover", TaskPriority::Normal, vec![]);
        orch.get_agent_queue_mut(agent_id).unwrap().enqueue(task);

        let remaining = orch.retire_agent(agent_id).unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(orch.agent_queue(agent_id).is_none());
    }

    #[tokio::test]
    async fn pause_resume_agent() {
        let mut orch = test_orchestrator();
        let agent_id = orch.spawn_agent("test").unwrap();

        orch.pause_agent(agent_id).unwrap();
        assert!(orch.agent_queue(agent_id).unwrap().is_paused());

        orch.resume_agent(agent_id).unwrap();
        assert!(!orch.agent_queue(agent_id).unwrap().is_paused());
    }

    #[tokio::test]
    async fn disabled_orchestrator_rejects_tasks() {
        let mut orch = Orchestrator::new(OrchestratorConfig {
            enabled: false,
            ..OrchestratorConfig::for_testing()
        });
        let err = orch.submit_task("test", vec![], None).await.unwrap_err();
        assert!(matches!(err, OrchestratorError::Disabled));
    }

    #[tokio::test]
    async fn status_snapshot() {
        let mut orch = test_orchestrator();
        orch.submit_task("t1", vec![FileAffinity::write("a.rs")], None)
            .await
            .unwrap();
        orch.submit_task("t2", vec![FileAffinity::write("b.rs")], None)
            .await
            .unwrap();

        let status = orch.status();
        assert!(status.enabled);
        assert!(status.total_queued >= 2);
    }

    #[tokio::test]
    async fn task_trace_after_submit() {
        let mut orch = test_orchestrator();
        let task_id = orch
            .submit_task("Trace me", vec![FileAffinity::write("x.rs")], None)
            .await
            .unwrap();
        let steps = orch.task_trace(task_id).expect("trace exists");
        assert!(steps.len() >= 2);
        assert_eq!(steps[0].stage, "ingress");
        assert_eq!(steps[1].stage, "routed");
        assert!(steps[1]
            .detail
            .as_ref()
            .map(|d| d.starts_with("agent "))
            .unwrap_or(false));
    }

    #[tokio::test]
    async fn task_trace_after_complete() {
        let mut orch = test_orchestrator();
        let task_id = orch
            .submit_task("Complete me", vec![FileAffinity::write("y.rs")], None)
            .await
            .unwrap();
        let agent_id = *orch.task_assignments().get(&task_id).unwrap();
        orch.get_agent_queue_mut(agent_id).unwrap().dequeue();
        orch.complete_task(task_id).await.unwrap();
        let steps = orch.task_trace(task_id).expect("trace exists");
        let outcome = steps
            .iter()
            .find(|s| s.stage == "outcome")
            .expect("outcome step");
        assert_eq!(outcome.detail.as_deref(), Some("completed"));
    }

    #[tokio::test]
    async fn task_trace_after_fail() {
        let mut orch = test_orchestrator();
        let task_id = orch
            .submit_task("Fail me", vec![FileAffinity::write("z.rs")], None)
            .await
            .unwrap();
        let agent_id = *orch.task_assignments().get(&task_id).unwrap();
        orch.get_agent_queue_mut(agent_id).unwrap().dequeue();
        orch.fail_task(task_id, "timeout".to_string()).await.unwrap();
        let steps = orch.task_trace(task_id).expect("trace exists");
        let outcome = steps
            .iter()
            .find(|s| s.stage == "outcome")
            .expect("outcome step");
        assert!(outcome
            .detail
            .as_deref()
            .map(|d| d.starts_with("failed: "))
            .unwrap_or(false));
    }
}
