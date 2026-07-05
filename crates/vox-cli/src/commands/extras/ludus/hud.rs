//! Orchestrator HUD for Ludus companions (`vox ludus hud`; needs `ludus-hud` feature).
//!
//! T2.3: subscribes to the shared `vox-orchestrator-d` daemon's agent-event
//! stream (spawning it if absent) instead of a private, throwaway in-process
//! `Orchestrator`'s isolated bulletin bus — the old in-process mode's bus
//! never received anything from other clients (GUI, `vox dei submit`, MCP),
//! so the HUD could only ever show its own dead orchestrator.

use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use tokio::time::sleep;
use vox_cli_core::daemon_ipc::orchestrator_daemon_ensure::OrchestratorDaemonEnsure;
use vox_gamify::companion::{Companion, Interaction, render_multi_agent_status};
use vox_gamify::db::canonical_user_id;
use vox_orchestrator::AgentEvent;
use vox_orchestrator::events::AgentEventKind;

pub async fn run() -> Result<()> {
    let daemon = OrchestratorDaemonEnsure::default();
    let client = daemon
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("could not reach or spawn vox-orchestrator-d: {e}"))?;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(256);
    let producer = tokio::spawn(async move { client.subscribe_events(tx).await });
    let uid = canonical_user_id();

    println!(
        "{}",
        "Starting Ludus HUD. Listening for orchestrator events…".cyan()
    );
    sleep(vox_config::timeouts::D_1S).await;

    let mut companions: HashMap<u64, Companion> = HashMap::new();

    loop {
        tokio::select! {
            result = rx.recv() => {
                let Some(raw) = result else {
                    // Daemon stream ended; stop polling further frames but
                    // keep rendering the last-known companion state below.
                    break;
                };
                let Ok(event) = serde_json::from_value::<AgentEvent>(raw) else {
                    continue;
                };

                match event.kind {
                    AgentEventKind::AgentSpawned { agent_id, name } => {
                        let c = Companion::new(
                            format!("agent-{}", agent_id.0),
                            &uid,
                            name,
                            "vox",
                        );
                        companions.insert(agent_id.0, c);
                    }
                    // AgentMessage::TaskAssigned had no AgentEventKind equivalent by
                    // that name; TaskStarted is the closest semantic match (agent
                    // begins working a task) — same Interaction as before.
                    AgentEventKind::TaskStarted { agent_id, .. } => {
                        if let Some(c) = companions.get_mut(&agent_id.0) {
                            c.interact(Interaction::TaskAssigned);
                        }
                    }
                    AgentEventKind::TaskCompleted { agent_id, .. } => {
                        if let Some(c) = companions.get_mut(&agent_id.0) {
                            c.interact(Interaction::TaskCompleted);
                        }
                    }
                    AgentEventKind::TaskFailed { agent_id, .. } => {
                        if let Some(c) = companions.get_mut(&agent_id.0) {
                            c.interact(Interaction::TaskFailed);
                            tracing::debug!(
                                agent_id = agent_id.0,
                                "ludus hud: task failed (companion mood updated)"
                            );
                        }
                    }
                    AgentEventKind::TaskDoubted { agent_id, .. } => {
                        if let Some(c) = companions.get_mut(&agent_id.0) {
                            c.interact(Interaction::TaskDoubted);
                            tracing::debug!(
                                agent_id = agent_id.0,
                                "ludus hud: task doubted (companion mood updated)"
                            );
                        }
                    }
                    AgentEventKind::LockAcquired { agent_id, .. } => {
                        if let Some(c) = companions.get_mut(&agent_id.0) {
                            c.interact(Interaction::LockAcquired);
                        }
                    }
                    _ => {}
                }
            }
            _ = sleep(vox_config::timeouts::D_3S) => {}
        }

        let mut refs: Vec<&Companion> = companions.values().collect();
        refs.sort_by_key(|c| c.id.clone());

        println!("{}", render_multi_agent_status(&refs));

        for c in &refs {
            let ascii = vox_gamify::sprite::generate_deterministic(&c.name, c.mood);
            println!("{}\n", ascii.cyan());
        }
    }

    let _ = producer.await;
    Ok(())
}
