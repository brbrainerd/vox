//! [`ServerState`] construction, Populi polling, orchestrator event sinks, and optional DB wiring.

use std::sync::Arc;

use crate::server_state::ServerState;
use vox_orchestrator::{Orchestrator, OrchestratorConfig};
// No longer used in this module after refactor

/// When truthy (default if unset), MCP spawns [`vox_orchestrator::runtime::AgentFleet`] so queued
/// tasks receive `ProcessQueue` wakes from registered worker actors.
#[inline]
pub fn mcp_agent_fleet_env_enabled() -> bool {
    vox_orchestrator::runtime::agent_fleet_env_enabled()
}

fn spawn_embedded_agent_fleet_if_enabled(orchestrator: Arc<Orchestrator>) {
    vox_orchestrator::runtime::spawn_agent_fleet_if_enabled(orchestrator.clone());

    // NOTE: ResolutionAgent (formerly vox_dei) functionality is now integrated
    // into the main orchestrator dispatch and verification loops.
}

pub fn load_config() -> OrchestratorConfig {
    vox_orchestrator_driver::build_embedded_orchestrator_config()
}

pub async fn run_stdio_server_blocking() -> anyhow::Result<()> {
    tracing::info!("vox native mcp server starting...");

    // Load configuration
    let config = load_config();
    tracing::info!(?config, "orchestrator config loaded");

    // Create shared state and server
    let mut state = ServerState::new_full(config);
    spawn_embedded_agent_fleet_if_enabled(state.orchestrator.clone());

    // Degraded optional: MCP must keep serving stdio even if Codex is misconfigured
    if let Some(db) =
        vox_db::connect_workspace_journey_optional(vox_db::DbConnectSurface::Mcp, false).await
    {
        state = state.with_db_initialized(Arc::new(db)).await;
        tracing::info!(
            "workspace journey database connected and linked to state (orchestrator schema synced)"
        );
    }

    #[cfg(feature = "populi-transport")]
    crate::populi_startup::publish_mesh_on_mcp_start(&state).await;

    state
        .probe_external_orchestrator_daemon_if_configured()
        .await;

    // Optional remote/mobile control plane (HTTP + WebSocket).
    let _http_gateway = crate::http_gateway::spawn_http_gateway_if_enabled(state.clone())?;

    // Flywheel automation: Monitor diversity and trigger training
    let flywheel =
        vox_orchestrator::services::flywheel::FlywheelMonitor::new(state.orchestrator.clone());
    flywheel.spawn().await;

    let server = crate::server::VoxMcpServer::new(state);
    tracing::info!("server state initialized, starting stdio transport...");

    // Start the MCP server on stdio via RMCP
    let service = rmcp::ServiceExt::serve(server, rmcp::transport::stdio())
        .await
        .inspect_err(|e| {
            tracing::error!("failed to start MCP server: {e}");
        })?;

    tracing::info!("vox native mcp server running on stdio");

    // Block until the service shuts down
    service.waiting().await?;

    tracing::info!("vox native mcp server shutting down");
    Ok(())
}
