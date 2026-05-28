//! Tokio/`vox-actor-runtime` bridge: actor agents, task processors, and fleet scaling hooks.
//!
//! [`AgentFleet`](crate::runtime::AgentFleet) keeps [`ProcessHandle`](vox_actor_runtime::ProcessHandle) values aligned with [`Orchestrator`](crate::orchestrator::Orchestrator) registrations
//! and applies [`ScalingAction`](crate::services::ScalingAction) decisions from the scaling service.

use std::sync::{Arc, Mutex};

use vox_actor_runtime::{
    ProcessHandle, RegistryError, mailbox::MessagePayload, process::ProcessContext,
    scheduler::Scheduler, supervisor::ChildSpec, supervisor::RestartStrategy,
    supervisor::Supervisor,
};

use crate::events::AgentEventKind;
use crate::models::{ModelRouteBackend, route_backend_for_model};
use crate::orchestrator::Orchestrator;
use crate::planning::prompts::SUPERPOWERS_PROMPT;
use crate::services::{ScalingAction, ScalingService};
use crate::types::AgentId;
use crate::types::TaskId;
use futures_util::StreamExt;
use std::time::Instant;

/// Returns the first hyphen-delimited segment of `s`, or the first 8 bytes if
/// there are no hyphens.  Never panics.
fn short_id_from_str(s: &str) -> &str {
    if let Some(pos) = s.find('-') {
        &s[..pos]
    } else {
        &s[..s.len().min(8)]
    }
}

/// Message type sent to the ActorAgent to trigger task processing.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum AgentCommand {
    /// Drain the agent's queue once (used by supervisor ticks).
    ProcessQueue,
    /// Pause dequeueing new tasks.
    Pause,
    /// Resume after [`AgentCommand::Pause`].
    Resume,
    /// Remove a specific pending task id from the local queue.
    CancelTask(TaskId),
}

/// Pluggable executor invoked by [`ActorAgent`] for each dequeued [`AgentTask`](crate::types::AgentTask).
#[async_trait::async_trait]
pub trait TaskProcessor: Send + Sync {
    /// Runs `task` on behalf of `agent_id` and returns the finished task id on success.
    async fn process(
        &self,
        agent_id: crate::types::AgentId,
        task: crate::types::AgentTask,
    ) -> anyhow::Result<crate::types::TaskId>;
}

/// No-op processor for tests and dry runs: completes immediately without calling external AI.
pub struct StubTaskProcessor;

#[async_trait::async_trait]
impl TaskProcessor for StubTaskProcessor {
    async fn process(
        &self,
        _agent_id: crate::types::AgentId,
        task: crate::types::AgentTask,
    ) -> anyhow::Result<crate::types::TaskId> {
        Ok(task.id)
    }
}

/// A real AI-powered task processor that streams tokens back to the event bus.
pub struct AiTaskProcessor {
    client: vox_gamify::ai::FreeAiClient,
    event_bus: crate::events::EventBus,
    orchestrator: Arc<Orchestrator>,
    /// Provider name stored at construction time (e.g. "ollama", "google").
    provider: String,
    /// Model identifier stored at construction time.
    model: String,
}

// TaskPhase moved to types/tasks.rs

impl AiTaskProcessor {
    /// Create a new AI processor that auto-discovers providers.
    pub async fn new(event_bus: crate::events::EventBus, orchestrator: Arc<Orchestrator>) -> Self {
        let client = vox_gamify::ai::FreeAiClient::auto_discover().await;
        // Reflect the active provider in costs/logs
        let (provider, model) = client.active_provider_info();
        Self {
            client,
            event_bus,
            orchestrator,
            provider,
            model,
        }
    }

    async fn run_phase_stream(
        &self,
        client: &vox_gamify::ai::FreeAiClient,
        agent_id: crate::types::AgentId,
        task: &crate::types::AgentTask,
        phase: crate::types::TaskPhase,
        usage_model: &str,
        prior_notes: &str,
        route: vox_gamify::StreamRoute<'_>,
    ) -> String {
        let mut history_block = String::new();
        if !task.transcript.is_empty() {
            history_block.push_str("### Prior Agent Turns (Context)\n");
            // Inject a max of 3 relevant history turns (Surgical Injection phase 1).
            for turn in task.transcript.iter().rev().take(3).rev() {
                history_block.push_str(&format!(
                    "[{}] (Agent: {}):\n{}\n\n",
                    turn.agent_id, turn.agent_name, turn.message
                ));
            }
        }

        let mut skill_block = String::new();
        if let Some(ref skill) = task.active_skill {
            skill_block.push_str("\n### Procedural Skill (Active Methodology)\n");
            skill_block.push_str(&format!("ACTIVE_SKILL: {}\n", skill));
            skill_block.push_str(SUPERPOWERS_PROMPT);
            skill_block.push_str("\n\n");
        }

        let prompt = format!(
            "Task: {}\n\n{}{}\nPhase: {}\nCategory: {:?}\nRouting model hint: {}\n\nKnown notes:\n{}\n\nAction contract:\n- Think step-by-step for this phase only.\n- If proposing tool usage, emit one line starting with `@tool` and a concrete tool name.\n- Keep output concise and executable.",
            task.description,
            history_block,
            skill_block,
            phase.as_str(),
            task.task_category,
            usage_model,
            prior_notes
        );

        let mut stream = client.generate_stream_routed(&prompt, route).await;
        let mut phase_text = String::new();
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(text) => {
                    phase_text.push_str(&text);
                    self.event_bus
                        .emit(AgentEventKind::TokenStreamed { agent_id, text });
                }
                Err(e) => tracing::error!("AI stream error [{}]: {}", phase.as_str(), e),
            }
        }
        phase_text
    }
}

#[async_trait::async_trait]
impl TaskProcessor for AiTaskProcessor {
    async fn process(
        &self,
        agent_id: crate::types::AgentId,
        task: crate::types::AgentTask,
    ) -> anyhow::Result<crate::types::TaskId> {
        let cost_pref = crate::sync_lock::rw_read(&*self.orchestrator.config).cost_preference;
        let mut allowed_providers = std::collections::HashSet::new();
        if let Some(db) = self.orchestrator.db() {
            let tracker = crate::usage::UsageTracker::new_ref(&db);
            if let Ok(budgets) = tracker.remaining_all().await {
                for b in budgets {
                    if b.remaining > 0 && !b.rate_limited {
                        allowed_providers.insert(b.provider.clone());
                    }
                }
            }
        }

        let models_handle = self.orchestrator.models_handle();
        let routed = {
            let registry = crate::sync_lock::rw_read(&*models_handle);
            let exploration_spent = crate::sync_lock::rw_read(&*self.orchestrator.budget_manager).global_exploration_cost_usd();
            let exploration_limit = vox_config::load_model_routing_config().exploration.budget_usd_per_day;

            if allowed_providers.is_empty() {
                registry.best_for_task_with_filter(&task, cost_pref, |m| {
                    if m.pricing_source == crate::models::spec::PricingSource::Unknown && exploration_spent >= exploration_limit {
                        return false;
                    }
                    true
                })
            } else {
                registry.best_for_task_with_filter(&task, cost_pref, |m| {
                    if m.pricing_source == crate::models::spec::PricingSource::Unknown && exploration_spent >= exploration_limit {
                        return false;
                    }
                    let provider_str = match m.provider_type {
                        crate::models::ProviderType::OpenRouter => "openrouter",
                        crate::models::ProviderType::Ollama => "ollama",
                        crate::models::ProviderType::GoogleDirect => "google",
                        crate::models::ProviderType::Groq => "groq",
                        crate::models::ProviderType::Cerebras => "cerebras",
                        crate::models::ProviderType::Mistral => "mistral",
                        crate::models::ProviderType::DeepSeek => "deepseek",
                        crate::models::ProviderType::SambaNova => "sambanova",
                        crate::models::ProviderType::Anthropic => "anthropic",
                        crate::models::ProviderType::PopuliMesh => "populimesh",
                        crate::models::ProviderType::HuggingFaceRouter => "huggingface",
                        crate::models::ProviderType::Custom(_) => "custom",
                        crate::models::ProviderType::VoxLocal => "vox_local",
                    };
                    allowed_providers.contains(provider_str)
                })
            }
        };
        let (usage_provider, usage_model) = if let Some(ref mo) = task.model_override {
            ("task_override".to_string(), mo.clone())
        } else if let Some(m) = routed.as_ref() {
            (m.provider.clone(), m.id.clone())
        } else {
            (self.provider.clone(), self.model.clone())
        };

        let route = if let Some(mo) = task
            .model_override
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            vox_gamify::StreamRoute::UserModelOverride(mo)
        } else if let Some(m) = routed.as_ref() {
            match route_backend_for_model(m) {
                ModelRouteBackend::Ollama => vox_gamify::StreamRoute::Registry {
                    backend: vox_gamify::LudusStreamBackend::Ollama,
                    model: m.id.as_str(),
                },
                ModelRouteBackend::GeminiDirect => vox_gamify::StreamRoute::Registry {
                    backend: vox_gamify::LudusStreamBackend::Gemini,
                    model: m.id.as_str(),
                },
                ModelRouteBackend::OpenRouter => vox_gamify::StreamRoute::Registry {
                    backend: vox_gamify::LudusStreamBackend::OpenRouter,
                    model: m.id.as_str(),
                },
                ModelRouteBackend::CascadeFallback => vox_gamify::StreamRoute::Cascade,
                ModelRouteBackend::PopuliMesh => vox_gamify::StreamRoute::Cascade,
                ModelRouteBackend::VoxLocal => vox_gamify::StreamRoute::Cascade,
            }
        } else {
            vox_gamify::StreamRoute::Cascade
        };

        if let Some(db) = self.orchestrator.db() {
            let repo = crate::lineage::repository_id();
            let has_model_override = task
                .model_override
                .as_deref()
                .map(str::trim)
                .is_some_and(|s| !s.is_empty());
            let ludus_fallback = !has_model_override && routed.is_none();
            let reason = vox_actor_runtime::routing_telemetry::OrchestratorTaskRoutingReasonV1::new(
                format!("{:?}", task.task_category),
                task.estimated_complexity,
                usage_provider.clone(),
                usage_model.clone(),
                routed.is_some(),
                format!("{:?}", cost_pref),
                ludus_fallback,
                vox_actor_runtime::routing_telemetry::unified_routing_rollout_enabled(),
                vox_actor_runtime::route_capability_policy::RouteCapabilityPolicySnapshot::from_env()
                    .profile
                    .clone(),
                Vec::new(),
                task.id.0,
            );
            let reason_s = reason.to_json_bounded(
                vox_actor_runtime::routing_telemetry::ROUTING_REASON_JSON_MAX_BYTES,
            );
            if let Err(e) = db
                .record_routing_decision(
                    None::<&str>,
                    repo.as_str(),
                    task.session_id.as_deref(),
                    "orchestrator_ai_task",
                    Some(usage_model.as_str()),
                    Some(reason_s.as_str()),
                )
                .await
            {
                tracing::debug!(error = %e, "record_routing_decision (orchestrator_ai_task) skipped");
            }
        }

        let reconciled_cost = Arc::new(Mutex::new(0.0));
        let client = {
            let reconciled_cost = reconciled_cost.clone();
            self.client
                .clone()
                .with_cost_reporter(Arc::new(move |cost| {
                    if let Ok(mut lock) = reconciled_cost.lock() {
                        *lock += cost;
                    }
                }))
        };

        let mut notes = String::new();
        let phases = [
            crate::types::TaskPhase::Inspect,
            crate::types::TaskPhase::Localize,
            crate::types::TaskPhase::Hypothesize,
            crate::types::TaskPhase::Act,
            crate::types::TaskPhase::Verify,
            crate::types::TaskPhase::Decide,
        ];
        // Keep execution bounded: no infinite self-reflection or uncontrolled loops.
        for phase in phases {
            // Update the task's current phase in the orchestrator state for observability.
            if let Some(queue_lock) = self.orchestrator.agent_queue(agent_id) {
                let mut queue = crate::sync_lock::rw_write(&*queue_lock);
                if let Some(t) = queue.find_task_mut(task.id) {
                    t.current_phase = Some(phase);
                } else if let Some(t) = queue.current_task_mut() {
                    if t.id == task.id {
                        t.current_phase = Some(phase);
                    }
                }
            }
            self.orchestrator.record_workflow_phase_change(task.id, phase).await;
            self.event_bus.emit(AgentEventKind::TaskPhaseChanged {
                task_id: task.id,
                agent_id,
                phase,
            });

            let phase_out = self
                .run_phase_stream(
                    &client,
                    agent_id,
                    &task,
                    phase,
                    usage_model.as_str(),
                    notes.as_str(),
                    route,
                )
                .await;

            // Drift detection (Doom-loop protection)
            let drift_decision = self.orchestrator.record_agent_iteration(
                agent_id,
                &phase_out,
                phase_out.contains("@tool"),
            );
            match drift_decision {
                crate::budget::DriftDecision::HaltAgent { reason } => {
                    tracing::error!(agent_id = agent_id.0, %reason, "halted agent due to semantic drift");
                    self.event_bus.emit(AgentEventKind::DoubtReported {
                        agent_id,
                        task_id: task.id,
                        reason: reason.clone(),
                    });
                    return Err(anyhow::anyhow!("Safety Halt: {}", reason));
                }
                crate::budget::DriftDecision::WarnUser {
                    iterations,
                    cost_usd,
                } => {
                    tracing::warn!(
                        agent_id = agent_id.0,
                        iterations,
                        cost_usd,
                        "agent showing early signs of semantic drift"
                    );
                }
                crate::budget::DriftDecision::Continue => {}
            }
            if !notes.is_empty() {
                notes.push_str("\n\n");
            }
            notes.push_str(&format!("[{}]\n{}", phase.as_str(), phase_out));
            // Lightweight tool intent tracing: explicit breadcrumbs for future bridge adapters.
            if let Some(tool_line) = phase_out
                .lines()
                .map(str::trim)
                .find(|line| line.starts_with("@tool "))
            {
                tracing::info!(
                    agent_id = agent_id.0,
                    task_id = task.id.0,
                    phase = phase.as_str(),
                    tool_intent = %tool_line,
                    "bounded executor emitted tool intent"
                );
            }
        }
        let full_text = notes;

        let input_tokens =
            crate::compaction::CompactionEngine::estimate_tokens(&task.description) as u32;
        let output_tokens = crate::compaction::CompactionEngine::estimate_tokens(&full_text) as u32;

        let cost_usd = if let Some(m) = routed.as_ref() {
            let input_cost = (input_tokens as f64 / 1000.0) * m.cost_per_1k_input;
            let output_cost = (output_tokens as f64 / 1000.0) * m.cost_per_1k_output;
            input_cost + output_cost
        } else {
            (input_tokens + output_tokens) as f64 * 0.000_001
        };

        // Record usage through the unified pipeline (event bus + budget + oplog)
        self.orchestrator
            .record_ai_usage(
                agent_id,
                usage_provider.as_str(),
                usage_model.as_str(),
                input_tokens,
                output_tokens,
                cost_usd,
                reconciled_cost
                    .lock()
                    .ok()
                    .and_then(|lock| if *lock > 0.0 { Some(*lock) } else { None }),
                routed.as_ref().map(|m| m.pricing_source.clone()),
            )
            .await;

        // Record the final condensed summary back into the task's transcript for future handoffs.
        let agent_name = {
            if let Some(queue_lock) = self.orchestrator.agent_queue(agent_id) {
                crate::sync_lock::rw_read(&*queue_lock).name.clone()
            } else {
                "unknown".to_string()
            }
        };

        // We update the task transcript in the orchestrator's state so it persists
        // across handoffs if the task is re-queued or migrated.
        let turn_opt = {
            if let Some(queue_lock) = self.orchestrator.agent_queue(agent_id) {
                let mut queue = crate::sync_lock::rw_write(&*queue_lock);
                if let Some(t) = queue.find_task_mut(task.id) {
                    t.append_turn(agent_id, agent_name.clone(), full_text.clone());
                    t.transcript.last().cloned()
                } else if let Some(t) = queue.current_task_mut() {
                    if t.id == task.id {
                        t.append_turn(agent_id, agent_name, full_text.clone());
                        t.transcript.last().cloned()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(turn) = turn_opt {
            self.orchestrator.record_workflow_turn(task.id, &turn).await;
        }

        Ok(task.id)
    }
}

/// Actor process wrapping an `AgentQueue`.
///
/// Converts a reactive orchestrator queue into an active background worker
/// using `vox-actor-runtime` actor primitives.
pub struct ActorAgent {
    /// Agent id managed by this process.
    pub agent_id: AgentId,
    /// Human-readable process/agent name.
    pub name: String,
}

impl ActorAgent {
    /// Spawn an active agent process from an `AgentQueue`.
    pub fn spawn(
        scheduler: &Scheduler,
        agent_id: AgentId,
        name: String,
        orchestrator: Arc<Orchestrator>,
        processor: Arc<dyn TaskProcessor>,
    ) -> Result<ProcessHandle, RegistryError> {
        let process_name = format!("agent-{}", name);

        scheduler.spawn_named(&process_name, move |mut ctx: ProcessContext| async move {
            tracing::info!("Agent {} ({}) process started", agent_id, name);

            loop {
                // Wait for commands
                let msg = ctx.receive().await;
                if let Some(envelope) = msg {
                    if let vox_actor_runtime::mailbox::Envelope::Message(msg) = envelope {
                        if let MessagePayload::Json(json_data) = msg.payload {
                            if let Ok(cmd) = serde_json::from_slice::<AgentCommand>(&json_data) {
                                Self::handle_command(cmd, agent_id, &orchestrator, &processor)
                                    .await;
                            }
                        }
                    }
                } else {
                    // Channel closed
                    break;
                }
            }
            tracing::info!("Agent {} ({}) process shutting down", agent_id, name);
        })
    }

    /// Handle a command sent to this agent process.
    async fn handle_command(
        cmd: AgentCommand,
        agent_id: AgentId,
        orchestrator_ref: &Arc<Orchestrator>,
        processor: &Arc<dyn TaskProcessor>,
    ) {
        match cmd {
            AgentCommand::ProcessQueue => {
                let task_to_run = {
                    let dequeued = if let Some(queue_lock) = orchestrator_ref.agent_queue(agent_id)
                    {
                        let mut queue = crate::sync_lock::rw_write(&queue_lock);
                        if !queue.is_paused() {
                            let t = queue.dequeue();
                            if t.is_some() {
                                orchestrator_ref
                                    .heartbeat(agent_id, crate::events::AgentActivity::Thinking);
                            } else {
                                orchestrator_ref
                                    .heartbeat(agent_id, crate::events::AgentActivity::Idle);
                            }
                            t
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some(ref task) = dequeued {
                        orchestrator_ref
                            .event_bus()
                            .emit(AgentEventKind::TaskStarted {
                                task_id: task.id,
                                agent_id,
                                session_id: task.session_id.clone(),
                            });
                    }
                    dequeued
                };

                if let Some(task) = task_to_run {
                    let task_id = task.id;
                    tracing::info!("Agent {} processing task {}", agent_id, task_id);

                    match processor.process(agent_id, task).await {
                        Ok(completed_id) => {
                            let _ = orchestrator_ref.complete_task(completed_id).await;
                            orchestrator_ref
                                .heartbeat(agent_id, crate::events::AgentActivity::Idle);
                        }
                        Err(e) => {
                            tracing::error!("Agent {} failed task {}: {}", agent_id, task_id, e);
                            if let Err(err) =
                                orchestrator_ref.fail_task(task_id, e.to_string()).await
                            {
                                tracing::error!(
                                    "fail_task after processor error: {} (task {})",
                                    err,
                                    task_id
                                );
                            }
                            orchestrator_ref
                                .heartbeat(agent_id, crate::events::AgentActivity::Idle);
                        }
                    }
                }
            }
            AgentCommand::Pause => {
                orchestrator_ref.heartbeat(agent_id, crate::events::AgentActivity::Idle);
                let _ = orchestrator_ref.pause_agent(agent_id);
            }
            AgentCommand::Resume => {
                orchestrator_ref.heartbeat(agent_id, crate::events::AgentActivity::Idle);
                let _ = orchestrator_ref.resume_agent(agent_id);
            }
            AgentCommand::CancelTask(task_id) => {
                orchestrator_ref.heartbeat(agent_id, crate::events::AgentActivity::Idle);
                if let Some(q_lock) = orchestrator_ref.agent_queue(agent_id) {
                    crate::sync_lock::rw_write(&q_lock).cancel(task_id);
                }
            }
        }
    }
}

/// A fleet supervisor that manages multiple agent processes.
pub struct AgentFleet {
    supervisor: Supervisor,
    scheduler: Arc<Scheduler>,
    orchestrator: Arc<Orchestrator>,
    processor: Arc<dyn TaskProcessor>,
    /// Last time we performed a scale-up (for cooldown).
    last_scale_up: std::sync::RwLock<Option<Instant>>,
    /// Number of agents spawned in the current tick (reset at start of check_scaling).
    spawns_this_tick: std::sync::atomic::AtomicUsize,
}

impl AgentFleet {
    /// Wires the shared scheduler and shared [`Arc<Orchestrator>`] with a task processor implementation.
    pub fn new(
        scheduler: Arc<Scheduler>,
        orchestrator: Arc<Orchestrator>,
        processor: Arc<dyn TaskProcessor>,
    ) -> Self {
        Self {
            supervisor: Supervisor::new(RestartStrategy::RestForOne),
            scheduler,
            orchestrator,
            processor,
            last_scale_up: std::sync::RwLock::new(None),
            spawns_this_tick: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Watch the orchestrator state and ensure an actor exists for every
    /// agent registered in the orchestrator. Also stops processes for retired agents.
    pub async fn sync_fleet(&self) {
        let agent_info: Vec<(AgentId, String)> = {
            let ids = self.orchestrator.agent_ids();
            ids.iter()
                .map(|id| {
                    (
                        *id,
                        crate::sync_lock::rw_read(
                            &*self.orchestrator.agent_queue(*id).expect("agent queue"),
                        )
                        .name
                        .clone(),
                    )
                })
                .collect()
        };
        let active_agent_ids: std::collections::HashSet<AgentId> =
            agent_info.iter().map(|(id, _)| *id).collect();

        // 1. Ensure all active agents have actors
        for (agent_id, name) in agent_info {
            let proc_name = format!("agent-{}", name);

            // Check if process is already running in the global registry
            let already_running = match self.scheduler.registry().lookup_name(&proc_name) {
                Ok(opt) => opt.is_some(),
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        proc_name = %proc_name,
                        "process registry poisoned during fleet sync; aborting sync_fleet"
                    );
                    return;
                }
            };
            if !already_running {
                // Not running, add it to supervisor
                let orchestrator_clone = self.orchestrator.clone();
                let scheduler_clone = self.scheduler.clone();
                let processor_clone = self.processor.clone();

                let spec = ChildSpec {
                    name: proc_name.clone(),
                    start: Box::new(move || {
                        let h = ActorAgent::spawn(
                            &scheduler_clone,
                            agent_id,
                            name.clone(),
                            orchestrator_clone.clone(),
                            processor_clone.clone(),
                        )?;
                        orchestrator_clone.register_agent_handle(agent_id, h.clone());
                        Ok(h)
                    }),
                };

                self.supervisor.add_child(spec).await;
            }
        }

        // 2. Prune stale handles for retired agents so runtime state converges.
        let mut handles = crate::sync_lock::rw_write(&*self.orchestrator.agent_handles);
        let stale_ids: Vec<AgentId> = handles
            .keys()
            .copied()
            .filter(|id| !active_agent_ids.contains(id))
            .collect();
        for id in stale_ids {
            handles.remove(&id);
            tracing::debug!("Removed stale runtime handle for retired agent {}", id);
        }
        drop(handles);
    }

    /// Check if agents need to be spawned or retired using ScalingService and profile limits.
    pub async fn check_scaling(&self) {
        // Reset spawn counter at the start of each scaling cycle so each tick
        // gets a clean budget — avoids stale carry-over from concurrent paths.
        self.spawns_this_tick
            .store(0, std::sync::atomic::Ordering::Relaxed);

        let (status, idle_dynamic, config, budget_manager, remote_gpu_capacity) = {
            let orch = &*self.orchestrator;
            let config_arc = orch.config_handle();
            let config = crate::sync_lock::rw_read(&config_arc).clone();
            if !config.scaling_enabled {
                return;
            }
            let status = orch.status();
            let idle_dynamic: Vec<_> = status
                .agents
                .iter()
                .filter(|a| a.dynamic && a.queued == 0 && !a.in_progress)
                .filter_map(|a| {
                    orch.agent_queue(a.id)
                        .map(|q| (a.id, crate::sync_lock::rw_read(&*q).last_active))
                })
                .collect();
            let budget_manager = orch.budget_manager_handle();
            let remote_gpu_capacity = crate::sync_lock::rw_read(&*orch.remote_populi_routing_hints)
                .iter()
                .filter(|h| {
                    h.capabilities.gpu_cuda
                        || h.capabilities.gpu_metal
                        || h.capabilities.gpu_vulkan
                        || h.capabilities.gpu_webgpu
                        || h.capabilities.npu
                })
                .count();
            (
                status,
                idle_dynamic,
                config,
                budget_manager,
                remote_gpu_capacity,
            )
        };

        let load_history: Vec<f64> = crate::sync_lock::rw_read(&*self.orchestrator.load_history)
            .iter()
            .copied()
            .collect();
        let action = ScalingService::decide_scaling(
            &status,
            &config,
            &load_history,
            remote_gpu_capacity,
            &idle_dynamic,
            &crate::sync_lock::rw_read(&budget_manager),
        );

        match action {
            ScalingAction::NoOp => {}
            ScalingAction::ScaleUp { name_prefix, count } => {
                let max_per_tick = config.max_spawn_per_tick;
                let cooldown_ms = config.scaling_cooldown_ms;
                let spawns = self
                    .spawns_this_tick
                    .load(std::sync::atomic::Ordering::Relaxed);
                let cooldown_ok = crate::sync_lock::rw_read(&self.last_scale_up)
                    .as_ref()
                    .map(|t| t.elapsed() >= std::time::Duration::from_millis(cooldown_ms))
                    .unwrap_or(true);

                if spawns < max_per_tick && cooldown_ok {
                    let limit = std::cmp::min(count, max_per_tick - spawns);
                    for _ in 0..limit {
                        let uuid_str = uuid::Uuid::new_v4().to_string();
                        let name = format!("{}-{}", name_prefix, short_id_from_str(&uuid_str));
                        let _ = self.orchestrator.spawn_dynamic_agent_with_parent(
                            &name,
                            None,
                            Some("scaling_load"),
                            None,
                            None,
                        );
                        self.spawns_this_tick
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    *crate::sync_lock::rw_write(&self.last_scale_up) =
                        Some(std::time::Instant::now());
                    tracing::info!(
                        "Scaling up: spawned {} dynamic agents (load: {:.2}, profile: {:?})",
                        limit,
                        status.total_weighted_load,
                        config.scaling_profile
                    );
                }
            }
            ScalingAction::ScaleDown { agent_ids } => {
                if !agent_ids.is_empty() {
                    tracing::info!(
                        "Scaling down: retiring {} idle dynamic agent(s)",
                        agent_ids.len()
                    );
                }
                for id in agent_ids {
                    if let Ok(remaining) = self.orchestrator.retire_agent(id).await {
                        for task in remaining {
                            let _ = self.orchestrator.submit_existing_task(task).await;
                        }
                    }
                }
            }
        }
    }

    /// Start the main orchestrator loop: rebalancing, maintenance, and fleet syncing.
    pub async fn run(&self) {
        loop {
            // 1. Scaling checks
            self.check_scaling().await;

            // 2. Sync fleet (ensure all agents have actors)
            self.sync_fleet().await;

            // 3. Perform orchestrator maintenance (rebalance and tick)
            {
                self.orchestrator.rebalance();
                self.orchestrator.tick().await;
            }

            // 4. Wait until next tick
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}

/// When truthy (default if unset), MCP / `vox-orchestrator-d` spawn [`AgentFleet`] with [`AiTaskProcessor`].
///
/// Disable with **`VOX_MCP_AGENT_FLEET`**=`0`, `false`, `no`, or `off`.
#[must_use]
pub fn agent_fleet_env_enabled() -> bool {
    match vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpAgentFleet).expose() {
        Some(v) => {
            let v = v.trim();
            if v.is_empty() {
                return true;
            }
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("no")
                || v.eq_ignore_ascii_case("off"))
        }
        None => true,
    }
}

pub fn spawn_agent_fleet_if_enabled(orchestrator: Arc<Orchestrator>) {
    if !agent_fleet_env_enabled() {
        tracing::info!(
            target: "vox_orchestrator::runtime",
            "VOX_MCP_AGENT_FLEET disabled: task queues will not auto-drain via AgentFleet"
        );
        return;
    }
    let scheduler = Arc::new(Scheduler::new());
    tokio::spawn(async move {
        let processor = Arc::new(
            AiTaskProcessor::new(orchestrator.event_bus.clone(), orchestrator.clone()).await,
        );
        let fleet = AgentFleet::new(scheduler, orchestrator, processor);
        tracing::info!(
            target: "vox_orchestrator::runtime",
            "AgentFleet loop running (AiTaskProcessor; MCP / orchestrator-d)"
        );
        fleet.run().await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_from_standard_uuid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let result = short_id_from_str(uuid_str);
        assert_eq!(result, "550e8400");
    }

    #[test]
    fn short_id_from_hyphen_free_string() {
        // If format ever lacks hyphens, must not panic — returns first 8 chars
        let s = "1234567890abcdef1234567890abcdef";
        let result = short_id_from_str(s);
        assert_eq!(result, "12345678");
    }
}
