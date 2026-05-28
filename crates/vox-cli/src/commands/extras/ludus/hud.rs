//! In-process orchestrator HUD for Ludus companions (`vox ludus hud`; needs `ludus-hud` feature).

use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use tokio::time::{Duration, sleep};
use vox_gamify::companion::{Companion, Interaction, render_multi_agent_status};
use vox_gamify::db::canonical_user_id;
use vox_orchestrator::types::AgentMessage;
use vox_orchestrator::{OrchestratorConfig, build_repo_scoped_orchestrator};

pub async fn run() -> Result<()> {
    let config = OrchestratorConfig::default();
    let orch = build_repo_scoped_orchestrator(config, None).orchestrator;
    let mut rx = orch.bulletin().subscribe();
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
                let msg = match result {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                match msg {
                    AgentMessage::AgentSpawned { agent_id, name } => {
                        let c = Companion::new(
                            format!("agent-{}", agent_id.0),
                            &uid,
                            name,
                            "vox",
                        );
                        companions.insert(agent_id.0, c);
                    }
                    AgentMessage::TaskAssigned { agent_id, .. } => {
                        if let Some(c) = companions.get_mut(&agent_id.0) {
                            c.interact(Interaction::TaskAssigned);
                        }
                    }
                    AgentMessage::TaskCompleted { agent_id, .. } => {
                        if let Some(c) = companions.get_mut(&agent_id.0) {
                            c.interact(Interaction::TaskCompleted);
                        }
                    }
                    AgentMessage::TaskFailed { agent_id, .. } => {
                        if let Some(c) = companions.get_mut(&agent_id.0) {
                            c.interact(Interaction::TaskFailed);
                            tracing::debug!(
                                agent_id = agent_id.0,
                                "ludus hud: task failed (companion mood updated)"
                            );
                        }
                    }
                    AgentMessage::TaskDoubted { agent_id, .. } => {
                        if let Some(c) = companions.get_mut(&agent_id.0) {
                            c.interact(Interaction::TaskDoubted);
                            tracing::debug!(
                                agent_id = agent_id.0,
                                "ludus hud: task doubted (companion mood updated)"
                            );
                        }
                    }
                    AgentMessage::LockAcquired { agent_id, .. } => {
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
}
