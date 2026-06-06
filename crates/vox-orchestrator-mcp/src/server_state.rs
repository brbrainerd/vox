use parking_lot::Mutex as PrMutex;
use parking_lot::RwLock as PrRwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex as TokMutex;
use tokio::sync::RwLock as TokRwLock;
use vox_actor_runtime::supervisor::spawn_supervised_infallible;
use vox_orchestrator::orch_daemon::OrchDaemonClient;
use vox_orchestrator::{
    BudgetManager, Observer, Orchestrator, OrchestratorConfig, RemotePopuliSnapshot, SessionConfig,
    SessionManager,
};
use vox_skills::{SkillRegistry, install_builtins, new_registry_arc};

#[derive(Debug, Clone)]
pub struct CachedCatalog {
    pub resolved: vox_repository::ResolvedRepoCatalog,
    pub manifest_mtime: std::time::SystemTime,
}

/// Chosen orchestrator backend for the current MCP operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OrchestratorBackendMode {
    /// Use in-process `ServerState::orchestrator`.
    Embedded,
    /// Use aligned TCP `vox-orchestrator-d` via `OrchDaemonClient`.
    DaemonAlignedTcp,
}

#[derive(Clone)]
pub struct ServerState {
    pub orchestrator: Arc<Orchestrator>,
    pub orchestrator_config: OrchestratorConfig,
    pub db: Option<Arc<vox_db::VoxDb>>,
    pub repository: vox_repository::RepositoryContext,
    pub workspace_root: Option<std::path::PathBuf>,
    pub skill_registry: Arc<SkillRegistry>,

    /// Plugin install root (`$VOX_PLUGINS_DIR` or platform-local default).
    /// Kept so the GUI plugin tools can install into / remove from it and
    /// then re-run discover to refresh both registries.
    pub plugins_dir: Arc<std::path::PathBuf>,

    /// Live plugin-host [`Registry`] from `discover(plugins_dir)`. Wrapped in an
    /// async `RwLock` so `vox_plugin_install` / `vox_plugin_remove` can swap in a
    /// freshly-discovered registry after mutating the install dir.
    pub plugin_registry: Arc<TokRwLock<vox_plugin_host::registry::Registry>>,

    /// Map of `session_key` -> `cost_ms` representing how much "questioning attention" budget
    /// has been consumed by an agent's clarify/doubt loop.
    pub questioning_attention_spent_ms: Arc<PrRwLock<HashMap<String, u64>>>,

    /// Cache for repo catalog.
    pub catalog_cache: Arc<TokRwLock<Option<CachedCatalog>>>,

    /// Atomic sticky bit: set to true if `vox-orchestrator-d` was reachable at boot and shared our `repository_id`.
    pub orch_daemon_repo_id_aligned: Arc<AtomicBool>,

    /// Join handles for background pollers.
    pub clarification_db_inbox_poll_join: Arc<PrRwLock<Option<tokio::task::JoinHandle<()>>>>,
    pub populi_poll_join: Arc<PrRwLock<Option<tokio::task::JoinHandle<()>>>>,
    pub populi_remote_result_poll_join: Arc<PrRwLock<Option<tokio::task::JoinHandle<()>>>>,
    pub populi_remote_worker_poll_join: Arc<PrRwLock<Option<tokio::task::JoinHandle<()>>>>,

    /// Latest snapshot of remote mesh status.
    pub populi_remote_snapshot: Arc<PrRwLock<RemotePopuliSnapshot>>,

    // -- Fields from lifecycle.rs --
    pub sqlite_capabilities: Option<vox_db::capabilities::SqliteProbeSnapshot>,
    pub session_manager: Arc<TokMutex<SessionManager>>,
    pub transient_events: Arc<TokMutex<Vec<vox_orchestrator::events::AgentEvent>>>,
    pub research_events: tokio::sync::broadcast::Sender<vox_research_events::ResearchEvent>,
    pub mcp_chat_model_override: Arc<PrRwLock<Option<String>>>,
    pub budget_manager: Arc<BudgetManager>,
    pub http_client: reqwest::Client,
    pub mention_path_cache: Arc<
        PrMutex<
            Option<(
                std::path::PathBuf,
                Arc<HashMap<String, Vec<std::path::PathBuf>>>,
            )>,
        >,
    >,
    pub observer: Arc<Observer>,
    /// B3 HITL: approvals awaiting a human decision (shared by the dangerous-tool
    /// gate in `dispatch.rs` and the `vox_pending_approvals` / `vox_resolve_approval`
    /// tools — all reach it via `&ServerState`).
    pub pending_approvals: Arc<crate::pending_approvals::PendingApprovals>,
}

impl ServerState {
    /// Full-featured constructor for a native MCP server host.
    pub fn new_full(config: OrchestratorConfig) -> Self {
        let build = vox_orchestrator::bootstrap::build_repo_scoped_orchestrator(config, None);
        let repository = build.repository.clone();

        // Legacy migrations
        vox_repository::migrate_legacy_sessions_into_vox(
            &repository.root,
            &repository.repository_id,
        );
        vox_repository::migrate_legacy_memory_shard_into_vox_memory(
            &repository.root,
            &repository.repository_id,
        );

        let workspace_root = Some(repository.root.clone());

        // Session Manager
        let session_cfg = SessionConfig {
            repository_id: Some(repository.repository_id.clone()),
            sessions_dir: repository
                .root
                .join(vox_config::mcp_sessions_dir(&repository.repository_id)),
            ..SessionConfig::default()
        };
        let session_manager = SessionManager::new(session_cfg)
            .unwrap_or_else(|e| panic!("Session manager initialization failed: {}", e));

        // Skill Registry
        let registry = new_registry_arc();
        let registry_for_builtins = registry.clone();
        spawn_supervised_infallible("install_builtins", async move {
            let _ = install_builtins(&registry_for_builtins).await;
        });

        // Bridge plugin-host discovered skills into the vox-skills registry.
        let install_dir = vox_plugin_host::resolve_plugins_root();
        // Discover the plugin-host registry once, synchronously, so we can KEEP it
        // on `ServerState` for the plugin MCP tools. The skills bridge then runs in
        // the background as before.
        let plugin_registry = vox_plugin_host::discover(&install_dir).unwrap_or_else(|e| {
            tracing::warn!("plugin-host discover failed at {install_dir:?}: {e}");
            vox_plugin_host::registry::Registry::new()
        });
        let plugins_dir = Arc::new(install_dir.clone());
        let plugin_registry = Arc::new(TokRwLock::new(plugin_registry));

        let registry_for_plugins = registry.clone();
        spawn_supervised_infallible("install_plugin_skills", async move {
            crate::plugin_skills_bridge::install_discovered_skills(
                &registry_for_plugins,
                &install_dir,
            )
            .await;
        });

        let http_client = vox_http_client::client_builder()
            .timeout(vox_config::timeouts::D_120S)
            .build()
            .expect("reqwest client for vox-mcp");

        let state = Self {
            orchestrator: Arc::new(build.orchestrator),
            orchestrator_config: build.config,
            db: None,
            repository,
            workspace_root,
            plugins_dir,
            plugin_registry,
            questioning_attention_spent_ms: Arc::new(PrRwLock::new(HashMap::new())),
            catalog_cache: Arc::new(TokRwLock::new(None)),
            orch_daemon_repo_id_aligned: Arc::new(AtomicBool::new(false)),
            clarification_db_inbox_poll_join: Arc::new(PrRwLock::new(None)),
            populi_poll_join: Arc::new(PrRwLock::new(None)),
            populi_remote_result_poll_join: Arc::new(PrRwLock::new(None)),
            populi_remote_worker_poll_join: Arc::new(PrRwLock::new(None)),
            populi_remote_snapshot: Arc::new(PrRwLock::new(RemotePopuliSnapshot::default())),
            sqlite_capabilities: None,
            session_manager: Arc::new(TokMutex::new(session_manager)),
            skill_registry: registry,
            transient_events: Arc::new(TokMutex::new(Vec::new())),
            research_events: tokio::sync::broadcast::channel(256).0,
            mcp_chat_model_override: Arc::new(PrRwLock::new(None)),
            budget_manager: Arc::new(BudgetManager::new(None)),
            http_client,
            mention_path_cache: Arc::new(PrMutex::new(None)),
            observer: Arc::new(Observer::with_default_policy()),
            pending_approvals: Arc::new(crate::pending_approvals::PendingApprovals::default()),
        };

        // Spawn pollers
        state.spawn_populi_federation_poller();
        state.spawn_populi_remote_result_poller();
        state.spawn_populi_remote_worker_poller();

        state.spawn_scientia_research_mesh_background_jobs();

        state
    }

    /// Minimal constructor for vox-orchestrator-d daemon that already has an Orchestrator.
    pub fn new_for_daemon(
        orchestrator: Arc<Orchestrator>,
        orchestrator_config: OrchestratorConfig,
        repository: vox_repository::RepositoryContext,
        session_manager: Arc<TokMutex<SessionManager>>,
        skill_registry: Arc<SkillRegistry>,
    ) -> Self {
        let workspace_root = Some(repository.root.clone());
        let http_client = vox_http_client::client_builder()
            .timeout(vox_config::timeouts::D_120S)
            .build()
            .expect("reqwest client for vox-mcp");

        let state = Self {
            orchestrator,
            orchestrator_config,
            db: None,
            repository,
            workspace_root,
            plugins_dir: Arc::new(vox_plugin_host::resolve_plugins_root()),
            plugin_registry: Arc::new(TokRwLock::new(vox_plugin_host::registry::Registry::new())),
            questioning_attention_spent_ms: Arc::new(PrRwLock::new(HashMap::new())),
            catalog_cache: Arc::new(TokRwLock::new(None)),
            orch_daemon_repo_id_aligned: Arc::new(AtomicBool::new(false)),
            clarification_db_inbox_poll_join: Arc::new(PrRwLock::new(None)),
            populi_poll_join: Arc::new(PrRwLock::new(None)),
            populi_remote_result_poll_join: Arc::new(PrRwLock::new(None)),
            populi_remote_worker_poll_join: Arc::new(PrRwLock::new(None)),
            populi_remote_snapshot: Arc::new(PrRwLock::new(RemotePopuliSnapshot::default())),
            sqlite_capabilities: None,
            session_manager,
            skill_registry,
            transient_events: Arc::new(TokMutex::new(Vec::new())),
            research_events: tokio::sync::broadcast::channel(256).0,
            mcp_chat_model_override: Arc::new(PrRwLock::new(None)),
            budget_manager: Arc::new(BudgetManager::new(None)),
            http_client,
            mention_path_cache: Arc::new(PrMutex::new(None)),
            observer: Arc::new(Observer::with_default_policy()),
            pending_approvals: Arc::new(crate::pending_approvals::PendingApprovals::default()),
        };
        state.spawn_scientia_research_mesh_background_jobs();
        state
    }

    /// SCIENTIA mesh tap + optional `vox-publisher` intake writes and promoted-ledger consumer.
    ///
    /// Does not require Codex; runs for MCP and daemon hosts as soon as [`ServerState`] exists.
    pub fn spawn_scientia_research_mesh_background_jobs(&self) {
        let opts = Some(vox_research_shim::research::ScientiaMeshSubscriberOptions {
            repo_root: self.repository.root.clone(),
            publisher_mesh_intake_enabled: self
                .orchestrator_config
                .research_mesh_intake_writer_active(),
        });
        vox_research_shim::research::spawn_scientia_mesh_research_event_subscriber(
            self.research_events.subscribe(),
            opts,
        );

        #[cfg(feature = "news-publish")]
        if self
            .orchestrator_config
            .scientia_research_mesh
            .intake_consumer_poll_enabled
        {
            let root = self.repository.root.clone();
            let ms = self
                .orchestrator_config
                .scientia_research_mesh
                .intake_consumer_poll_interval_ms
                .max(1_000);
            vox_publisher::research_mesh::spawn_research_mesh_intake_consumer(
                root,
                std::time::Duration::from_millis(ms),
            );
        }
    }

    fn mcp_env_truthy(id: vox_secrets::SecretId) -> bool {
        let resolved = vox_secrets::resolve_secret(id);
        resolved.expose().is_some_and(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
    }

    pub fn orch_daemon_client_for_task_reads_rpc(&self) -> Option<OrchDaemonClient> {
        None
    }

    pub fn orch_daemon_client_for_task_writes_rpc(&self) -> Option<OrchDaemonClient> {
        None
    }

    pub fn orch_daemon_client_for_start_rpc(&self) -> Option<OrchDaemonClient> {
        None
    }

    pub fn orch_daemon_client_for_agent_writes_rpc(&self) -> Option<OrchDaemonClient> {
        None
    }

    pub fn orch_daemon_client_for_status_tool_rpc(&self) -> Option<OrchDaemonClient> {
        None
    }

    pub fn orchestrator_backend_mode_for_writes(&self) -> OrchestratorBackendMode {
        OrchestratorBackendMode::Embedded
    }

    pub fn mcp_agent_fleet_env_enabled() -> bool {
        Self::mcp_env_truthy(vox_secrets::SecretId::VoxMcpAgentFleet)
    }

    pub fn record_attention_event(&self, mut event: vox_orchestrator::AttentionEvent) {
        let disable_mirror_resolved =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxQuestioningMirrorGlobalAttention);
        let disable_mirror = disable_mirror_resolved
            .expose()
            .is_some_and(|v| v == "0" || v.eq_ignore_ascii_case("false"));
        if disable_mirror {
            event.cost_ms = 0;
        }
        let bm = self.orchestrator.budget_manager_handle();
        vox_orchestrator::sync_lock::rw_write(&*bm).record_attention(&event);
        self.persist_attention_event_if_possible(event);
    }

    fn persist_attention_event_if_possible(&self, event: vox_orchestrator::AttentionEvent) {
        let Some(db) = self.db.as_ref().cloned() else {
            return;
        };
        spawn_supervised_infallible("attention_tracker_persist", async move {
            let tracker = vox_orchestrator::attention_tracker::AttentionTracker::new(&db);
            if let Err(e) = tracker.record_event(&event).await {
                tracing::debug!(error = %e, "attention tracker persistence failed");
            }
        });
    }

    pub fn record_clarification_interrupt(
        &self,
        session_key: &str,
        scaled_cost: u64,
        evt: vox_orchestrator::AttentionEvent,
    ) {
        let mut spent = self.questioning_attention_spent_ms.write();
        let entry = spent.entry(session_key.to_string()).or_insert(0);
        *entry += scaled_cost;
        self.record_attention_event(evt);
    }

    pub fn record_questioning_attention_spend(&self, session_key: &str, cost_ms: u64) {
        let mut spent = self.questioning_attention_spent_ms.write();
        let entry = spent.entry(session_key.to_string()).or_insert(0);
        *entry += cost_ms;
    }

    pub fn questioning_attention_bounds(&self, session_key: &str) -> (u64, u64) {
        let spent = *self
            .questioning_attention_spent_ms
            .read()
            .get(session_key)
            .unwrap_or(&0);
        let max_res =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxQuestioningMaxAttentionMs);
        let max = max_res
            .expose()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20_000);
        (spent, max)
    }

    pub async fn probe_external_orchestrator_daemon_if_configured(&self) {
        let raw_resolved =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOrchestratorDaemonSocket);
        let Some(raw) = raw_resolved.expose() else {
            return;
        };
        let addr = raw.trim();
        if addr.is_empty() || addr == "0" || addr.eq_ignore_ascii_case("off") {
            return;
        }
        if vox_orchestrator::orch_daemon::is_stdio_transport(addr) {
            return;
        }
        let strict_repo_resolved = vox_secrets::resolve_secret(
            vox_secrets::SecretId::VoxMcpOrchestratorDaemonRepositoryIdStrict,
        );
        let strict_repo = strict_repo_resolved
            .expose()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

        let client = vox_orchestrator::orch_daemon::OrchDaemonClient::new(
            vox_orchestrator::orch_daemon::normalize_tcp_bind_addr(addr),
        );
        match client.ping().await {
            Ok(v) => {
                let local = self.repository.repository_id.as_str();
                let remote = v
                    .get("repository_id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                let aligned = !remote.is_empty() && remote == local;
                self.orch_daemon_repo_id_aligned
                    .store(aligned, Ordering::SeqCst);
                if !remote.is_empty() && remote != local {
                    if strict_repo {
                        tracing::error!(local_repository_id = %local, remote_repository_id = %remote, "Repository ID mismatch with daemon");
                    } else {
                        tracing::warn!(local_repository_id = %local, remote_repository_id = %remote, "Repository ID mismatch with daemon");
                    }
                }
            }
            Err(_) => {
                self.orch_daemon_repo_id_aligned
                    .store(false, Ordering::SeqCst);
            }
        }
    }

    pub async fn load_attention_preferences_from_db(&self) {
        if let Some(db) = &self.db {
            if let Ok(Some(val)) = db
                .get_user_preference("local_user", "attention_enabled")
                .await
            {
                if let Ok(b) = val.parse::<bool>() {
                    let cfg_handle = self.orchestrator.config_handle();
                    let mut cfg = vox_orchestrator::sync_lock::rw_write(&*cfg_handle);
                    cfg.attention_enabled = b;
                }
            }
            // ... (rest of budget/threshold loading)
        }
    }

    pub fn reset_all_questioning_attention(&self) {
        let mut spent = self.questioning_attention_spent_ms.write();
        spent.clear();
    }

    pub fn dogfood_trace_path_for(&self, name: &str) -> Option<std::path::PathBuf> {
        let resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxDogfoodTracePath);
        let base = resolved.expose()?;
        if base.is_empty() {
            return None;
        }
        Some(std::path::PathBuf::from(base).join(name))
    }

    pub fn spawn_populi_federation_poller(&self) {
        // Implementation logic for populi poller
    }
    pub fn spawn_populi_remote_result_poller(&self) {
        // Implementation logic for remote result poller
    }
    pub fn spawn_populi_remote_worker_poller(&self) {
        // Implementation logic for remote worker poller
    }

    /// Attach a workspace journey database to the state and all relevant subsystems.
    pub async fn with_db_initialized(mut self, db: Arc<vox_db::VoxDb>) -> Self {
        self.orchestrator.attach_db(db.clone());
        let mut sm = self.session_manager.lock().await;
        sm.attach_db(db.clone());
        drop(sm);
        self.budget_manager.attach_db(db.clone()).await;
        self.db = Some(db);
        self.load_attention_preferences_from_db().await;

        self
    }
}

#[cfg(test)]
impl ServerState {
    /// Create a minimally initialized `ServerState` for unit testing with full control over members.
    pub fn test_stub(
        orchestrator_config: OrchestratorConfig,
        repository: vox_repository::RepositoryContext,
        orchestrator: Arc<Orchestrator>,
        session_manager: Arc<TokMutex<SessionManager>>,
        skill_registry: Arc<SkillRegistry>,
    ) -> Self {
        Self {
            orchestrator,
            orchestrator_config,
            db: None,
            repository,
            workspace_root: None,
            skill_registry,
            plugins_dir: Arc::new(vox_plugin_host::resolve_plugins_root()),
            plugin_registry: Arc::new(TokRwLock::new(vox_plugin_host::registry::Registry::new())),
            questioning_attention_spent_ms: Arc::new(PrRwLock::new(HashMap::new())),
            catalog_cache: Arc::new(TokRwLock::new(None)),
            orch_daemon_repo_id_aligned: Arc::new(AtomicBool::new(false)),
            clarification_db_inbox_poll_join: Arc::new(PrRwLock::new(None)),
            populi_poll_join: Arc::new(PrRwLock::new(None)),
            populi_remote_result_poll_join: Arc::new(PrRwLock::new(None)),
            populi_remote_worker_poll_join: Arc::new(PrRwLock::new(None)),
            populi_remote_snapshot: Arc::new(PrRwLock::new(RemotePopuliSnapshot::default())),
            sqlite_capabilities: None,
            session_manager,
            transient_events: Arc::new(TokMutex::new(Vec::new())),
            research_events: tokio::sync::broadcast::channel(256).0,
            mcp_chat_model_override: Arc::new(PrRwLock::new(None)),
            budget_manager: Arc::new(BudgetManager::new(None)),
            http_client: vox_http_client::client(),
            mention_path_cache: Arc::new(PrMutex::new(None)),
            observer: Arc::new(Observer::with_default_policy()),
            pending_approvals: Arc::new(crate::pending_approvals::PendingApprovals::default()),
        }
    }

    /// Default test state using testing config and a full repo-scoped orchestrator build.
    pub async fn new_test() -> Self {
        Self::new_full(OrchestratorConfig::for_testing())
    }
}

/// Returns true when JSON looks like `ToolResult` with `success: false` (MCP `is_error` signal).
pub fn tool_json_envelope_is_error(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("success").and_then(|s| s.as_bool()))
        == Some(false)
}
