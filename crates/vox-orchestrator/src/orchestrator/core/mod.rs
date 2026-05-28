use std::collections::{HashMap, VecDeque};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, AtomicU64, AtomicUsize},
};

use crate::affinity::FileAffinityMap;
use crate::bulletin::BulletinBoard;
use crate::config::OrchestratorConfig;
use crate::groups::AffinityGroupRegistry;
use crate::locks::FileLockManager;
use crate::scope::ScopeGuard;
use crate::types::{AgentIdGenerator, TaskIdGenerator};

impl crate::orchestrator::Orchestrator {
    pub fn new(config: OrchestratorConfig) -> Self {
        let bulletin = BulletinBoard::new(config.bulletin_capacity);
        let skill_registry = vox_skills::new_registry_arc();
        Self {
            config: Arc::new(RwLock::new(config.clone())),
            affinity_map: FileAffinityMap::new(),
            lock_manager: FileLockManager::new(),
            context_store: Arc::new(RwLock::new(crate::context::ContextStore::new())),
            budget_manager: Arc::new(RwLock::new({
                let bm = crate::budget::BudgetManager::new(None);
                bm.init_holistic_budgets(
                    config.attention_budget_ms,
                    config.financial_cost_budget_micros,
                    config.execution_time_budget_multiplier,
                );
                bm
            })),
            summary_manager: Arc::new(RwLock::new(crate::summary::SummaryManager::new())),
            models: Arc::new(RwLock::new(crate::models::ModelRegistry::new())),
            bulletin,
            agents: Arc::new(RwLock::new(HashMap::new())),
            groups: Arc::new(RwLock::new(AffinityGroupRegistry::defaults())),
            task_id_gen: TaskIdGenerator::new(),
            agent_id_gen: AgentIdGenerator::new(),
            task_assignments: Arc::new(RwLock::new(HashMap::new())),
            qa_router: Arc::new(RwLock::new(crate::qa::QARouter::new())),
            monitor: Arc::new(RwLock::new(crate::monitor::AiMonitor::new(
                config.continuation_cooldown_ms,
                config.max_auto_continuations,
                config.stale_threshold_ms,
                skill_registry.clone(),
            ))),
            event_bus: crate::events::EventBus::new(1024),
            message_bus: crate::a2a::MessageBus::new(100),
            dynamic_agents: Arc::new(RwLock::new(std::collections::HashSet::new())),
            agent_delegations: Arc::new(RwLock::new(HashMap::new())),
            dynamic_spawn_context: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "runtime")]
            agent_handles: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_monitor: Arc::new(RwLock::new(crate::heartbeat::HeartbeatMonitor::new(
                config.stale_threshold_ms,
            ))),
            #[cfg(feature = "system-metrics")]
            sys: Arc::new(RwLock::new(sysinfo::System::new_all())),
            load_history: Arc::new(RwLock::new(VecDeque::with_capacity(
                config.scaling_lookback_ticks,
            ))),
            scope_guard: Arc::new(RwLock::new(ScopeGuard::new(config.scope_enforcement))),
            task_traces: Arc::new(RwLock::new(HashMap::new())),
            snapshot_store: Arc::new(RwLock::new(crate::snapshot::SnapshotStore::default())),
            oplog: Arc::new(RwLock::new(crate::oplog::OpLog::default())),
            conflict_manager: Arc::new(RwLock::new(crate::conflicts::ConflictManager::new())),
            workspace_manager: Arc::new(RwLock::new(crate::workspace::WorkspaceManager::new())),
            db: Arc::new(RwLock::new(None)),
            last_rebalance_at: Arc::new(RwLock::new(None)),
            last_activity_ms: AtomicU64::new(crate::types::now_unix_ms()),
            tavily_credits_used: Arc::new(AtomicUsize::new(0)),
            remote_populi_routing_hints: Arc::new(RwLock::new(Vec::new())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            tool_ledger: Arc::new(RwLock::new(
                crate::tool_receipt::ToolReceiptLedger::from_config(&config),
            )),
            resource_locks: crate::locks::ResourceLockManager::new(),
            privacy_router: Arc::new(RwLock::new(crate::privacy_router::PrivacyRouter::new(
                crate::privacy_router::PrivacyRoutingPolicy::default(),
            ))),
            judge_model: Arc::new(RwLock::new(crate::judge_model::JudgeModel::new(
                crate::judge_model::JudgePolicy::Never,
            ))),
            agentos_policy_ledger: crate::agentos::policy_runtime::AgentosPolicyLedger::shared(),
            skill_registry,
            tenant_budget_gate: Arc::new(RwLock::new(
                crate::budget_gate::OrchestratorBudgetGate::new(
                    config.budget_gate_config.clone().unwrap_or_default(),
                ),
            )),
        }
    }

    pub fn with_groups(config: OrchestratorConfig, groups: AffinityGroupRegistry) -> Self {
        let bulletin = BulletinBoard::new(config.bulletin_capacity);
        let skill_registry = vox_skills::new_registry_arc();
        Self {
            config: Arc::new(RwLock::new(config.clone())),
            affinity_map: FileAffinityMap::new(),
            lock_manager: FileLockManager::new(),
            context_store: Arc::new(RwLock::new(crate::context::ContextStore::new())),
            budget_manager: Arc::new(RwLock::new({
                let bm = crate::budget::BudgetManager::new(None);
                bm.init_holistic_budgets(
                    config.attention_budget_ms,
                    config.financial_cost_budget_micros,
                    config.execution_time_budget_multiplier,
                );
                bm
            })),
            summary_manager: Arc::new(RwLock::new(crate::summary::SummaryManager::new())),
            models: Arc::new(RwLock::new(crate::models::ModelRegistry::new())),
            bulletin,
            agents: Arc::new(RwLock::new(HashMap::new())),
            groups: Arc::new(RwLock::new(groups)),
            task_id_gen: TaskIdGenerator::new(),
            agent_id_gen: AgentIdGenerator::new(),
            task_assignments: Arc::new(RwLock::new(HashMap::new())),
            qa_router: Arc::new(RwLock::new(crate::qa::QARouter::new())),
            monitor: Arc::new(RwLock::new(crate::monitor::AiMonitor::new(
                config.continuation_cooldown_ms,
                config.max_auto_continuations,
                config.stale_threshold_ms,
                skill_registry.clone(),
            ))),
            event_bus: crate::events::EventBus::new(1024),
            message_bus: crate::a2a::MessageBus::new(100),
            dynamic_agents: Arc::new(RwLock::new(std::collections::HashSet::new())),
            agent_delegations: Arc::new(RwLock::new(HashMap::new())),
            dynamic_spawn_context: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "runtime")]
            agent_handles: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_monitor: Arc::new(RwLock::new(crate::heartbeat::HeartbeatMonitor::new(
                config.stale_threshold_ms,
            ))),
            #[cfg(feature = "system-metrics")]
            sys: Arc::new(RwLock::new(sysinfo::System::new_all())),
            load_history: Arc::new(RwLock::new(VecDeque::with_capacity(
                config.scaling_lookback_ticks,
            ))),
            scope_guard: Arc::new(RwLock::new(ScopeGuard::new(config.scope_enforcement))),
            task_traces: Arc::new(RwLock::new(HashMap::new())),
            snapshot_store: Arc::new(RwLock::new(crate::snapshot::SnapshotStore::default())),
            oplog: Arc::new(RwLock::new(crate::oplog::OpLog::default())),
            conflict_manager: Arc::new(RwLock::new(crate::conflicts::ConflictManager::new())),
            workspace_manager: Arc::new(RwLock::new(crate::workspace::WorkspaceManager::new())),
            db: Arc::new(RwLock::new(None)),
            last_rebalance_at: Arc::new(RwLock::new(None)),
            last_activity_ms: AtomicU64::new(crate::types::now_unix_ms()),
            tavily_credits_used: Arc::new(AtomicUsize::new(0)),
            remote_populi_routing_hints: Arc::new(RwLock::new(Vec::new())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            tool_ledger: Arc::new(RwLock::new(
                crate::tool_receipt::ToolReceiptLedger::from_config(&config),
            )),
            resource_locks: crate::locks::ResourceLockManager::new(),
            privacy_router: Arc::new(RwLock::new(crate::privacy_router::PrivacyRouter::new(
                crate::privacy_router::PrivacyRoutingPolicy::default(),
            ))),
            judge_model: Arc::new(RwLock::new(crate::judge_model::JudgeModel::new(
                crate::judge_model::JudgePolicy::Never,
            ))),
            agentos_policy_ledger: crate::agentos::policy_runtime::AgentosPolicyLedger::shared(),
            skill_registry,
            tenant_budget_gate: Arc::new(RwLock::new(
                crate::budget_gate::OrchestratorBudgetGate::new(
                    config.budget_gate_config.clone().unwrap_or_default(),
                ),
            )),
        }
    }

    /// Spawns background tasks (observer loop, telemetry, catalog refresh) into the current Tokio runtime.
    pub fn spawn_background_tasks(self: Arc<Self>) {
        // Observer loop
        let orch = self.clone();
        tokio::spawn(async move {
            crate::orchestrator::observer_loop::run_observer_loop(orch).await;
        });

        // Catalog refresh loop — fetches OpenRouter + LiteLLM every 6 h (±20 min jitter).
        let orch2 = self.clone();
        tokio::spawn(async move {
            crate::orchestrator::catalog_refresh::run_catalog_refresh_loop(orch2).await;
        });
    }

    /// Hot-reloads configuration from Vox.toml (discovers from CWD or repo root)
    pub fn reload_config(&self) {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut candidates = Vec::new();
        if let Some(root) = vox_repository::find_project_manifest_root(&cwd) {
            candidates.push(root.join("Vox.toml"));
        }
        candidates.push(std::path::PathBuf::from("Vox.toml"));

        for toml_path in candidates {
            if toml_path.is_file() {
                if let Ok(mut new_cfg) = OrchestratorConfig::load_from_toml(&toml_path) {
                    new_cfg.merge_env_overrides();
                    if let Ok(mut guard) = self.config.write() {
                        *guard = new_cfg;
                    }
                    self.event_bus.emit(crate::events::AgentEventKind::AttentionConfigReloaded);
                    tracing::info!(path = %toml_path.display(), "hot-reloaded orchestrator config from Vox.toml");
                    return;
                }
            }
        }
        tracing::warn!("hot-reload failed: could not load Vox.toml from current dir or repository root");
    }
}

mod accessors;
mod init;
mod lineage;
mod telemetry;
mod temporal;
mod usage;
