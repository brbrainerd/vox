use anyhow::Result;
use owo_colors::OwoColorize;
use vox_orchestrator::{AgentId, FileAffinity, Orchestrator, OrchestratorConfig, TaskPriority};

/// `vox orchestrator status` — show all agents, queues, and file assignments.
pub async fn status() -> Result<()> {
    let config = load_config();
    let orch = Orchestrator::new(config);
    let status = orch.status();

    println!("{}", "╔══════════════════════════════════════╗".cyan());
    println!("{}", "║   Vox Orchestrator Status            ║".cyan());
    println!("{}", "╚══════════════════════════════════════╝".cyan());
    println!();

    println!(
        "  {} {}",
        "Enabled:".bold(),
        if status.enabled {
            "yes".green().to_string()
        } else {
            "no".red().to_string()
        }
    );
    println!(
        "  {} {} ({} reserved, {} dynamic)",
        "Agents:".bold(),
        status.agent_count,
        status.reserved_agents,
        status.dynamic_agents
    );
    println!(
        "  {} {:.2}",
        "Weighted load:".bold(),
        status.total_weighted_load
    );
    println!(
        "  {} {:.2}",
        "Predicted load:".bold(),
        status.predicted_load
    );
    let config = load_config();
    let effective_threshold =
        config.scaling_threshold as f64 * config.scaling_profile.threshold_multiplier();
    println!(
        "  {} {:?} (effective scale-up threshold: {:.1})",
        "Scaling profile:".bold(),
        config.scaling_profile,
        effective_threshold
    );
    println!("  {} {}", "Queued tasks:".bold(), status.total_queued);
    println!("  {} {}", "In progress:".bold(), status.total_in_progress);
    println!("  {} {}", "Completed:".bold(), status.total_completed);
    println!("  {} {}", "Locked files:".bold(), status.locked_files);

    if !status.agents.is_empty() {
        println!();
        println!("  {}", "Agents:".bold().underline());
        for agent in &status.agents {
            let state = if agent.paused {
                "⏸ paused".yellow().to_string()
            } else if agent.in_progress {
                "▶ working".green().to_string()
            } else {
                "● idle".dimmed().to_string()
            };
            let dynamic_tag = if agent.dynamic {
                "[dynamic]".magenta().to_string()
            } else {
                "[reserved]".blue().to_string()
            };
            println!(
                "    {} ({}) {} — {} | load: {:.2} | queued: {} ({} {} {}) | done: {} | files: {}",
                agent.id.to_string().bold(),
                agent.name,
                dynamic_tag,
                state,
                agent.weighted_load,
                agent.queued,
                format!("U:{}", agent.urgent_count).red(),
                format!("N:{}", agent.normal_count).blue(),
                format!("B:{}", agent.background_count).dimmed(),
                agent.completed,
                agent.owned_files,
            );
        }
    } else {
        println!();
        println!("  {}", "No agents spawned yet.".dimmed());
    }

    println!();
    Ok(())
}

/// `vox orchestrator submit` — manually submit a task.
pub async fn submit(description: &str, files: &[String], priority: Option<&str>) -> Result<()> {
    let config = load_config();
    let mut orch = Orchestrator::new(config);

    let file_manifest: Vec<FileAffinity> = files.iter().map(FileAffinity::write).collect();

    let priority = match priority {
        Some("urgent") => Some(TaskPriority::Urgent),
        Some("background") => Some(TaskPriority::Background),
        _ => None,
    };

    match orch.submit_task(description, file_manifest, priority).await {
        Ok(task_id) => {
            println!(
                "  {} Task {} submitted successfully",
                "✓".green().bold(),
                task_id.to_string().bold()
            );
        }
        Err(e) => {
            println!("  {} Failed to submit task: {}", "✗".red().bold(), e);
        }
    }

    Ok(())
}

/// `vox orchestrator queue` — show a specific agent's queue.
pub async fn queue(agent_id: u64) -> Result<()> {
    let config = load_config();
    let orch = Orchestrator::new(config);

    let id = AgentId(agent_id);
    match orch.agent_queue(id) {
        Some(q) => {
            println!("{}", q.to_markdown());
        }
        None => {
            println!(
                "  {} Agent {} not found",
                "✗".red().bold(),
                id.to_string().bold()
            );
        }
    }

    Ok(())
}

/// `vox orchestrator rebalance` — trigger manual rebalancing.
pub async fn rebalance() -> Result<()> {
    let config = load_config();
    let mut orch = Orchestrator::new(config);

    let moved = orch.rebalance();
    if moved > 0 {
        println!("  {} Rebalanced: {} tasks moved", "✓".green().bold(), moved);
    } else {
        println!("  {} No rebalancing needed", "ℹ".blue().bold());
    }

    Ok(())
}

/// `vox orchestrator config` — show current orchestrator configuration.
pub async fn config() -> Result<()> {
    let cfg = load_config();

    println!("{}", "╔══════════════════════════════════════╗".cyan());
    println!("{}", "║   Orchestrator Configuration         ║".cyan());
    println!("{}", "╚══════════════════════════════════════╝".cyan());
    println!();

    println!("  {} {}", "enabled:".bold(), cfg.enabled);
    println!("  {} {}", "max_agents:".bold(), cfg.max_agents);
    println!("  {} {}", "default_priority:".bold(), cfg.default_priority);
    println!(
        "  {} {:?}",
        "queue_overflow_strategy:".bold(),
        cfg.queue_overflow_strategy
    );
    println!("  {} {}ms", "lock_timeout:".bold(), cfg.lock_timeout_ms);
    println!("  {} {}", "scaling_enabled:".bold(), cfg.scaling_enabled);
    println!("  {} {}", "min_agents:".bold(), cfg.min_agents);
    println!("  {} {}", "max_agents:".bold(), cfg.max_agents);
    println!(
        "  {} {}",
        "scaling_threshold:".bold(),
        cfg.scaling_threshold
    );
    println!(
        "  {} {}ms",
        "idle_retirement_ms:".bold(),
        cfg.idle_retirement_ms
    );
    println!("  {} {:?}", "scaling_profile:".bold(), cfg.scaling_profile);
    println!(
        "  {} {} (per tick)",
        "max_spawn_per_tick:".bold(),
        cfg.max_spawn_per_tick
    );
    println!(
        "  {} {}ms",
        "scaling_cooldown_ms:".bold(),
        cfg.scaling_cooldown_ms
    );
    println!("  {} {:?}", "cost_preference:".bold(), cfg.cost_preference);
    println!("  {} {}", "toestub_gate:".bold(), cfg.toestub_gate);
    println!("  {} {}", "log_level:".bold(), cfg.log_level);

    println!();
    Ok(())
}

/// `vox orchestrator pause` — pause an agent.
pub async fn pause(agent_id: u64) -> Result<()> {
    let config = load_config();
    let mut orch = Orchestrator::new(config);
    let id = AgentId(agent_id);

    match orch.pause_agent(id) {
        Ok(()) => println!("  {} Agent {} paused", "✓".green().bold(), id),
        Err(e) => println!("  {} {}", "✗".red().bold(), e),
    }

    Ok(())
}

/// `vox orchestrator resume` — resume an agent.
pub async fn resume(agent_id: u64) -> Result<()> {
    let config = load_config();
    let mut orch = Orchestrator::new(config);
    let id = AgentId(agent_id);

    match orch.resume_agent(id) {
        Ok(()) => println!("  {} Agent {} resumed", "✓".green().bold(), id),
        Err(e) => println!("  {} {}", "✗".red().bold(), e),
    }

    Ok(())
}

/// `vox orchestrator save` — manually save orchestrator state.
pub async fn save() -> Result<()> {
    let config = load_config();
    let orch = Orchestrator::new(config.clone());
    let state = vox_orchestrator::state::OrchestratorState::from_status(&orch.status(), &config);

    match state.save(std::path::Path::new(".vox_orch_state.json")) {
        Ok(_) => println!(
            "  {} Orchestrator state saved successfully",
            "✓".green().bold()
        ),
        Err(e) => println!("  {} Failed to save state: {}", "✗".red().bold(), e),
    }

    Ok(())
}

/// `vox orchestrator load` — manually load orchestrator state.
pub async fn load() -> Result<()> {
    let config = load_config();
    let _orch = Orchestrator::new(config);

    match vox_orchestrator::state::OrchestratorState::load(std::path::Path::new(
        ".vox_orch_state.json",
    )) {
        Ok(Some(_)) => println!(
            "  {} Orchestrator state loaded successfully",
            "✓".green().bold()
        ),
        Ok(None) => println!("  {} No saved state found", "ℹ".blue().bold()),
        Err(e) => println!("  {} Failed to load state: {}", "✗".red().bold(), e),
    }

    Ok(())
}

/// `vox orchestrator undo` — undo the last N operations.
pub async fn undo(count: usize) -> Result<()> {
    let config = load_config();
    let mut orch = Orchestrator::new(config);

    // Ensure CodeStore is connected for DB undo
    if let Ok(store) = vox_pm::CodeStore::open_default().await {
        orch.set_code_store(store);
    }

    let mut successful = 0;
    for _ in 0..count {
        // Find the last NON-UNDONE operation from the history (newest first)
        if let Some(op) = orch.oplog().history().iter().rev().find(|e| !e.undone) {
            let id = op.id;
            // op.description is &String here, need to clone before it's borrowed mutably
            let desc = op.description.clone();
            match orch.undo_operation(id).await {
                Ok(_) => {
                    successful += 1;
                    println!(
                        "  {} Undid operation {} ({})",
                        "✓".green().bold(),
                        id.to_string().bold(),
                        desc
                    );
                }
                Err(e) => {
                    println!("  {} Failed to undo operation {}: {}", "✗".red().bold(), id, e);
                    break;
                }
            }
        } else {
            println!("  {} No more operations to undo", "ℹ".blue().bold());
            break;
        }
    }

    if successful > 0 {
        println!("\n  {} successfully undid {} operations", "✓".green(), successful);
    }

    Ok(())
}

/// `vox orchestrator redo` — redo the last N undone operations.
pub async fn redo(count: usize) -> Result<()> {
    let config = load_config();
    let mut orch = Orchestrator::new(config);

    // Ensure CodeStore is connected for DB redo
    if let Ok(store) = vox_pm::CodeStore::open_default().await {
        orch.set_code_store(store);
    }

    let mut successful = 0;
    for _ in 0..count {
        // Find the last operation that was undone (redo-able)
        if let Some(op) = orch.oplog().history().iter().rev().find(|e| e.undone) {
            let id = op.id;
            let desc = op.description.clone();
            match orch.redo_operation(id).await {
                Ok(_) => {
                    successful += 1;
                    println!(
                        "  {} Redid operation {} ({})",
                        "✓".green().bold(),
                        id.to_string().bold(),
                        desc
                    );
                }
                Err(e) => {
                    println!("  {} Failed to redo operation {}: {}", "✗".red().bold(), id, e);
                    break;
                }
            }
        } else {
            println!("  {} No more operations to redo", "ℹ".blue().bold());
            break;
        }
    }

    if successful > 0 {
        println!("\n  {} successfully redid {} operations", "✓".green(), successful);
    }

    Ok(())
}

/// Load orchestrator config from Vox.toml or defaults.
fn load_config() -> OrchestratorConfig {
    // Try to load from Vox.toml, fall back to defaults
    let mut config = std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            let toml_path = cwd.join("Vox.toml");
            OrchestratorConfig::load_from_toml(&toml_path).ok()
        })
        .unwrap_or_default();
    config.merge_env_overrides();
    config
}
