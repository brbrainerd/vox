//! Long-lived orchestrator owner: TCP JSON-line RPC (ADR 022 Phase B).
//!
//! Requires **`VOX_ORCHESTRATOR_DAEMON_SOCKET`**: TCP bind (`127.0.0.1:9745`) or **`stdio`** / **`-`** for line JSON on stdin/stdout.

use anyhow::Context as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use vox_orchestrator::runtime;
use vox_orchestrator::{
    OrchestratorConfig, RemotePopuliSnapshot, a2a, build_repo_scoped_orchestrator,
    clarification_db_inbox_poll, mesh_federation_poll, orch_daemon,
};

fn load_config() -> OrchestratorConfig {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut candidates = Vec::new();
    if let Some(root) = vox_repository::find_project_manifest_root(&cwd) {
        candidates.push(root.join("Vox.toml"));
    }
    candidates.push(PathBuf::from("Vox.toml"));

    let mut config = OrchestratorConfig::default();
    let mut loaded = false;
    for toml_path in candidates {
        if toml_path.is_file() {
            match OrchestratorConfig::load_from_toml(&toml_path) {
                Ok(cfg) => {
                    tracing::info!(path = %toml_path.display(), "loaded orchestrator config from Vox.toml");
                    config = cfg;
                    loaded = true;
                    break;
                }
                Err(e) => tracing::warn!(
                    path = %toml_path.display(),
                    "failed to load Vox.toml: {e}, trying next candidate"
                ),
            }
        }
    }
    if !loaded {
        tracing::info!("no readable Vox.toml found, using defaults");
    }
    config.merge_env_overrides();
    config
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    vox_foundation::tracing::try_init_from_default_env();

    let bind_raw = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOrchestratorDaemonSocket)
        .expose()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "VOX_ORCHESTRATOR_DAEMON_SOCKET is required (e.g. 127.0.0.1:9745 or stdio)"
            )
        })?
        .to_string();

    let cfg = load_config();
    let build = build_repo_scoped_orchestrator(cfg, None);
    let orch_config = build.config.clone();
    let repository_id = build.repository.repository_id.clone();
    let orch = Arc::new(build.orchestrator);

    let mut db_holder: Option<Arc<vox_db::VoxDb>> = None;
    if let Some(db) =
        vox_db::connect_workspace_journey_optional(vox_db::DbConnectSurface::Runtime, false).await
    {
        let db = Arc::new(db);
        db_holder = Some(db.clone());
        if let Err(e) = orch.init_db(db).await {
            tracing::warn!(error = %e, "orchestrator init_db failed; continuing without persisted Codex");
            db_holder = None;
        } else {
            tracing::info!("Codex attached and orchestrator schema synced");
        }
    }

    runtime::spawn_agent_fleet_if_enabled(orch.clone());

    // MCP parity: mesh federation snapshot, remote task pollers, event log, clarification inbox.
    let populi_remote_snapshot = Arc::new(RwLock::new(RemotePopuliSnapshot::default()));
    let populi_poll_join = Arc::new(Mutex::new(None));
    mesh_federation_poll::spawn_populi_federation_poller(
        &orch_config,
        repository_id.clone(),
        db_holder.clone(),
        orch.clone(),
        Arc::clone(&populi_remote_snapshot),
        Arc::clone(&populi_poll_join),
    );
    a2a::spawn_populi_remote_result_poller(orch.clone(), Arc::new(Mutex::new(None)));
    a2a::spawn_populi_remote_worker_poller(orch.clone(), Arc::new(Mutex::new(None)));

    if let Some(db) = db_holder.as_ref() {
        clarification_db_inbox_poll::spawn_clarification_db_inbox_poller(
            db.clone(),
            repository_id.clone(),
            Arc::new(Mutex::new(None)),
        );
    }
    vox_orchestrator::socrates::spawn_socrates_research_poller(orch.clone());

    // Flywheel automation: Monitor diversity and trigger training
    let flywheel = vox_orchestrator::services::flywheel::FlywheelMonitor::new(orch.clone());
    flywheel.spawn().await;

    // HTTP Gateway requires a ServerState
    let session_cfg = vox_orchestrator::SessionConfig {
        repository_id: Some(repository_id.clone()),
        sessions_dir: build
            .repository
            .root
            .join(vox_config::mcp_sessions_dir(&repository_id)),
        ..vox_orchestrator::SessionConfig::default()
    };
    let session_manager = vox_orchestrator::SessionManager::new(session_cfg)
        .context("session manager initialization failed")?;

    // Skills are discovered from plugins below; install_builtins is a no-op and removed.
    let registry = vox_skills::new_registry_arc();

    // Bridge plugin-host discovered skills into the vox-skills registry.
    let install_dir = std::env::var("VOX_PLUGINS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::data_local_dir()
                .map(|p| p.join("vox").join("plugins"))
                .unwrap_or_else(|| std::path::PathBuf::from("./vox-plugins"))
        });
    {
        let registry_for_plugins = registry.clone();
        vox_orchestrator_mcp::plugin_skills_bridge::install_discovered_skills(
            &registry_for_plugins,
            &install_dir,
        )
        .await;
    }

    let mut state = vox_orchestrator_mcp::server_state::ServerState::new_for_daemon(
        orch.clone(),
        orch_config.clone(),
        build.repository.clone(),
        Arc::new(tokio::sync::Mutex::new(session_manager)),
        registry,
    );
    if let Some(db) = db_holder.clone() {
        state = state.with_db_initialized(db).await;
    }

    if let Err(e) = vox_orchestrator_mcp::http_gateway::spawn_http_gateway_if_enabled(state) {
        tracing::error!(error = %e, "Failed to spawn HTTP gateway");
    }

    if orch_daemon::is_stdio_transport(&bind_raw) {
        return orch_daemon::run_stdio_server(repository_id, orch).await;
    }

    let bind = orch_daemon::normalize_tcp_bind_addr(&bind_raw);
    if bind.is_empty() {
        anyhow::bail!("VOX_ORCHESTRATOR_DAEMON_SOCKET is empty after normalization");
    }

    orch_daemon::run_tcp_server(&bind, repository_id, orch).await
}
