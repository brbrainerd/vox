//! [`ServerState`] construction (protocol-level surface only) and stdio transport startup.
//!
//! T2.2: `run_stdio_server_blocking` no longer boots a full, private
//! orchestrator stack. Tool *execution* is forwarded to the single shared
//! `vox-orchestrator-d` daemon (see `crate::daemon_route`,
//! `crate::server::VoxMcpServer::call_tool`), so this function only needs
//! enough local [`ServerState`] to serve protocol-level concerns that don't
//! touch live orchestrator state: tool-schema listing/advertisement,
//! resources, and prompts (`crate::registry`, `crate::server`). The agent
//! fleet, DB connection, `FlywheelMonitor`, and attention-calibration loop
//! that used to run here are now exclusively the daemon's job — running them
//! a second time in this process would be redundant/wasteful and would
//! recreate the exact split-brain (a private orchestrator disjoint from the
//! daemon's) that this task eliminates. See
//! docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md T2.2.

use crate::server_state::ServerState;
use vox_orchestrator::OrchestratorConfig;

/// When truthy (default if unset), MCP spawns [`vox_orchestrator::runtime::AgentFleet`] so queued
/// tasks receive `ProcessQueue` wakes from registered worker actors.
///
/// Retained for the daemon's own boot path (`vox-orchestrator-d` reads this
/// too); no longer called from [`run_stdio_server_blocking`], which does not
/// spawn its own agent fleet (see module doc comment).
#[inline]
pub fn mcp_agent_fleet_env_enabled() -> bool {
    vox_orchestrator::runtime::agent_fleet_env_enabled()
}

pub fn load_config() -> OrchestratorConfig {
    vox_orchestrator_driver::build_embedded_orchestrator_config()
}

pub async fn run_stdio_server_blocking() -> anyhow::Result<()> {
    tracing::info!("vox native mcp server starting...");

    // Load configuration
    let config = load_config();
    tracing::info!(?config, "orchestrator config loaded");

    // Local state backs protocol-level concerns only (tool-schema listing,
    // resources, prompts, skill-derived tool augmentation) — see the module
    // doc comment. No local agent fleet, DB connection, FlywheelMonitor, or
    // attention-calibration loop: those are the daemon's job. Tool execution
    // is forwarded to the daemon per-call (`crate::daemon_route`), so this
    // process does not need to probe for or align with an external daemon
    // itself the way the old full-stack path did.
    let state = ServerState::new_full(config);

    #[cfg(feature = "populi-transport")]
    crate::populi_startup::publish_mesh_on_mcp_start(&state).await;

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
