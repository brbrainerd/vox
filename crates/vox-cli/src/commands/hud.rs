use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use vox_gamify::companion::{render_multi_agent_status, Companion, Interaction};
use vox_orchestrator::types::AgentMessage;
use vox_orchestrator::{Orchestrator, OrchestratorConfig};

pub async fn run() -> Result<()> {
    let config = OrchestratorConfig::default();
    let orch = Orchestrator::new(config);
    let mut rx = orch.bulletin().subscribe();

    println!(
        "{}",
        "Starting Gamified Vox HUD. Listening for events...".cyan()
    );
    sleep(Duration::from_secs(1)).await;

    let mut companions: HashMap<u64, Companion> = HashMap::new();

    loop {
        tokio::select! {
            result = rx.recv() => {
                let msg = match result {
                    Ok(m) => m,
                    Err(_) => continue, // Disconnected or lagged
                };

                match msg {
                    AgentMessage::AgentSpawned { agent_id, name } => {
                        let c = Companion::new(format!("agent-{}", agent_id.0), "user", name, "vox");
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
                            println!("{} BUG BATTLE TRIGGERED for agent {}! {}", "⚔️".red(), agent_id.0, "⚔️".red());
                            sleep(Duration::from_millis(500)).await;
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
            _ = sleep(Duration::from_millis(500)) => {
                // Periodically render
            }
        }

        // Render to terminal
        print!("{esc}c", esc = 27 as char); // Clear screen

        let mut refs: Vec<&Companion> = companions.values().collect();
        // Sort by ID to keep the display stable
        refs.sort_by_key(|c| c.id.clone());

        println!("{}", render_multi_agent_status(&refs));

        for c in &refs {
            let ascii = vox_gamify::sprite::generate_deterministic(&c.name, c.mood);
            println!("{}\n", ascii.cyan());
        }
    }
}
