#![cfg(feature = "dei")]
use anyhow::Result;
use owo_colors::OwoColorize;
use vox_orchestrator::{
    AgentId, FileAffinity, Orchestrator, OrchestratorConfig, TaskPriority,
    build_repo_scoped_orchestrator, discover_repository_from_cwd, json_vcs_facade,
};

/// `vox orchestrator status` — show all agents, queues, and file assignments.
pub async fn status() -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);
    orch.clone().spawn_background_tasks();
    let status = orch.status();

    println!("{}", "╔══════════════════════════════════════╗".cyan());
    println!("{}", "║   Vox DEI Status                     ║".cyan());
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
    }
    
    println!(
        "  {} Visualization available via {}",
        "Tip:".bold().cyan(),
        "vox gui".bold().yellow()
    );

    println!();
    Ok(())
}

/// `vox orchestrator submit` — manually submit a task.
pub async fn submit(
    description: &str,
    files: &[String],
    priority: Option<&str>,
    session_id: Option<String>,
) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);

    let file_manifest: Vec<FileAffinity> = files.iter().map(FileAffinity::write).collect();

    let priority = match priority {
        Some("urgent") => Some(TaskPriority::Urgent),
        Some("background") => Some(TaskPriority::Background),
        _ => None,
    };

    match orch
        .submit_task(description, file_manifest, priority, session_id)
        .await
    {
        Ok(task_id) => {
            let id_str = task_id.to_string();
            println!(
                "  {} Task {} submitted successfully",
                "✓".green().bold(),
                id_str.bold()
            );
        }
        Err(e) => {
            println!("  {} Failed to submit task: {}", "✗".red().bold(), e);
        }
    }

    Ok(())
}

/// Read stdin lines (until EOF or empty line) and submit each as a task under a shared session id.
pub async fn assistant(session_id: String, files: &[String], priority: Option<&str>) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let sid = session_id.trim().to_string();
    if sid.is_empty() {
        anyhow::bail!("session_id must be non-empty");
    }
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);
    orch.clone().spawn_background_tasks();

    let file_list: Vec<String> = if files.is_empty() {
        vec![".".to_string()]
    } else {
        files.to_vec()
    };
    println!(
        "{}",
        format!(
            "Vox orchestrator assistant — session `{}`. Enter tasks (empty line to finish).",
            sid
        )
        .cyan()
    );
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    while let Some(line) = lines.next_line().await? {
        let t = line.trim();
        if t.is_empty() {
            break;
        }
        submit(t, &file_list, priority, Some(sid.clone())).await?;
    }
    Ok(())
}

/// `vox orchestrator queue` — show a specific agent's queue.
pub async fn queue(agent_id: u64) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);

    let id = AgentId(agent_id);
    match orch.agent_queue(id) {
        Some(q) => {
            let q_lock = q.read().unwrap();
            println!("{}", q_lock.to_markdown());
        }
        None => {
            let id_str = id.to_string();
            println!("  {} Agent {} not found", "✗".red().bold(), id_str.bold());
        }
    }

    Ok(())
}

/// `vox orchestrator rebalance` — trigger manual rebalancing.
pub async fn rebalance() -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);

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
    println!("{}", "║   DEI Configuration                  ║".cyan());
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
    let orch = build_repo_scoped_orchestrator_cli(config);
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
    let orch = build_repo_scoped_orchestrator_cli(config);
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
    let orch = build_repo_scoped_orchestrator_cli(config.clone());
    let store = vox_db::VoxDb::open_default().await?;
    let state = vox_orchestrator::state::OrchestratorState::from_status(&orch.status(), &config);

    match state.save_to_db(&store).await {
        Ok(_) => println!(
            "  {} DEI state saved to DB successfully",
            "✓".green().bold()
        ),
        Err(e) => println!("  {} Failed to save state to DB: {}", "✗".red().bold(), e),
    }

    Ok(())
}

/// `vox stop` — trigger early stop.
pub async fn stop(reason: Option<String>) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);
    orch.emergency_stop(reason.clone());
    println!(
        "  {} Orchestrator emergency stop requested",
        "✓".green().bold()
    );
    Ok(())
}

/// `vox orchestrator load` — manually load orchestrator state.
pub async fn load() -> Result<()> {
    let config = load_config();
    let _orch = build_repo_scoped_orchestrator_cli(config);
    let store = vox_db::VoxDb::open_default().await?;

    match vox_orchestrator::state::OrchestratorState::load_from_db(&store).await {
        Ok(Some(_)) => println!(
            "  {} DEI state loaded from DB successfully",
            "✓".green().bold()
        ),
        Ok(None) => println!("  {} No saved state found in DB", "ℹ".blue().bold()),
        Err(e) => println!("  {} Failed to load state from DB: {}", "✗".red().bold(), e),
    }

    Ok(())
}

/// `vox orchestrator undo` — undo the last N operations.
pub async fn undo(count: usize) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);

    if let Ok(store) = vox_db::VoxDb::open_default().await {
        let _ = orch.init_db(std::sync::Arc::new(store)).await;
    }

    let mut successful = 0;
    for _ in 0..count {
        // Find the last NON-UNDONE operation from the history (newest first)
        if let Some(op) = vox_orchestrator::sync_lock::rw_read(&*orch.oplog)
            .history()
            .iter()
            .rev()
            .find(|e| !e.undone)
        {
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
                    println!(
                        "  {} Failed to undo operation {}: {}",
                        "✗".red().bold(),
                        id,
                        e
                    );
                    break;
                }
            }
        } else {
            println!("  {} No more operations to undo", "ℹ".blue().bold());
            break;
        }
    }

    if successful > 0 {
        println!(
            "\n  {} successfully undid {} operations",
            "✓".green(),
            successful
        );
    }

    Ok(())
}

/// `vox orchestrator redo` — redo the last N undone operations.
pub async fn redo(count: usize) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);

    if let Ok(store) = vox_db::VoxDb::open_default().await {
        let _ = orch.init_db(std::sync::Arc::new(store)).await;
    }

    let mut successful = 0;
    for _ in 0..count {
        // Find the last operation that was undone (redo-able)
        if let Some(op) = vox_orchestrator::sync_lock::rw_read(&*orch.oplog)
            .history()
            .iter()
            .rev()
            .find(|e| e.undone)
        {
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
                    println!(
                        "  {} Failed to redo operation {}: {}",
                        "✗".red().bold(),
                        id,
                        e
                    );
                    break;
                }
            }
        } else {
            println!("  {} No more operations to redo", "ℹ".blue().bold());
            break;
        }
    }

    if successful > 0 {
        println!(
            "\n  {} successfully redid {} operations",
            "✓".green(),
            successful
        );
    }

    Ok(())
}

/// DEI (Distributed Execution Intelligence) command CLI.
#[derive(clap::Subcommand, Debug)]
pub enum DeiCli {
    /// Show all agents, queues, and file assignments.
    Status,
    /// Manually submit a task.
    Submit {
        /// Task description.
        description: String,
        /// Optional: file paths (for affinity).
        #[arg(short, long)]
        files: Vec<String>,
        /// Optional: priority (urgent, background).
        #[arg(short, long)]
        priority: Option<String>,
        /// Optional session id (context envelope / Socrates grouping; same as MCP `session_id`).
        #[arg(long)]
        session_id: Option<String>,
    },
    /// Multi-line interactive submit loop with a stable session id (developer pair-programming path).
    Assistant {
        /// Stable session key for all tasks in this loop (default `cli-assistant`).
        #[arg(long, default_value = "cli-assistant")]
        session_id: String,
        #[arg(short, long)]
        files: Vec<String>,
        #[arg(short, long)]
        priority: Option<String>,
    },
    /// Show a specific agent's queue.
    Queue {
        /// Agent numeric ID.
        agent_id: u64,
    },
    /// Trigger manual agent task rebalancing.
    Rebalance,
    /// Show current orchestrator configuration.
    Config,
    /// Pause an agent.
    Pause {
        /// Agent numeric ID.
        agent_id: u64,
    },
    /// Resume a paused agent.
    Resume {
        /// Agent numeric ID.
        agent_id: u64,
    },
    /// Manually save orchestrator state.
    Save,
    /// Manually load orchestrator state.
    Load,
    /// Undo the last N operations.
    Undo {
        /// Number of operations to undo (default 1).
        #[arg(default_value_t = 1)]
        count: usize,
    },
    /// Redo the last N undone operations.
    Redo {
        /// Number of operations to redo (default 1).
        #[arg(default_value_t = 1)]
        count: usize,
    },
    /// Agent workspace lifecycle (parity with MCP `vox_workspace_*`).
    Workspace {
        /// Subcommand.
        #[command(subcommand)]
        cmd: DeiWorkspaceCmd,
    },
    /// Filesystem snapshots (parity with MCP `vox_snapshot_*`).
    Snapshot {
        /// Subcommand.
        #[command(subcommand)]
        cmd: DeiSnapshotCmd,
    },
    /// Operation log inspection (parity with MCP `vox_oplog`).
    Oplog {
        /// Subcommand.
        #[command(subcommand)]
        cmd: DeiOplogCmd,
    },
    /// Aggregated repo + workspace + snapshot/oplog tails for human handoff (JSON stdout).
    #[command(name = "takeover-status")]
    TakeoverStatus {
        /// Agent scope for workspace/snapshot/oplog tails.
        #[arg(long, default_value_t = 0)]
        agent_id: u64,
        /// Print a short human summary before the JSON blob.
        #[arg(long)]
        human: bool,
    },
    /// Flag a task as "suspect" to trigger a verifier resolution loop.
    Doubt {
        /// Task numeric ID.
        task_id: u64,
        /// Optional reason for doubt.
        #[arg(short, long)]
        reason: Option<String>,
    },
    /// Overrule a doubted or failed task, force-marking it as completed.
    Overrule {
        /// Task numeric ID.
        task_id: u64,
        /// Required justification for the overrule.
        #[arg(short, long)]
        reason: String,
    },
    /*
    /// Analyze a Vox file for diagnostic errors and suggest repairs using HIR hints.
    Analyze {
        /// Path to the Vox file.
        path: String,
        /// If true, automatically apply the first suggested fix.
        #[arg(long)]
        apply: bool,
    },
    */
}

/// `vox dei workspace …`
#[derive(clap::Subcommand, Debug)]
pub enum DeiWorkspaceCmd {
    /// Create a workspace for an agent (captures a base snapshot).
    Create {
        /// Agent numeric ID.
        agent_id: u64,
    },
    /// Show modified files and base snapshot for an agent workspace.
    Status {
        /// Agent numeric ID.
        agent_id: u64,
    },
    /// Merge workspace changes and drop the workspace record.
    Merge {
        /// Agent numeric ID.
        agent_id: u64,
    },
}

/// `vox dei snapshot …`
#[derive(clap::Subcommand, Debug)]
pub enum DeiSnapshotCmd {
    /// List recent snapshots, optionally filtered by agent.
    List {
        #[arg(long)]
        agent_id: Option<u64>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Diff two snapshots by numeric id (see `list` output).
    Diff { before: u64, after: u64 },
    /// Restore tracked files from a snapshot (`S-123` or numeric).
    Restore { snapshot_id: String },
}

/// `vox dei oplog …`
#[derive(clap::Subcommand, Debug)]
pub enum DeiOplogCmd {
    /// List recent oplog entries.
    List {
        #[arg(long)]
        agent_id: Option<u64>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
}

/// Dispatch DEI subcommands.
pub async fn run(cli: DeiCli) -> Result<()> {
    match cli {
        DeiCli::Status => status().await,
        DeiCli::Submit {
            description,
            files,
            priority,
            session_id,
        } => {
            submit(
                &description,
                &files,
                priority.as_deref(),
                session_id.filter(|s| !s.trim().is_empty()),
            )
            .await
        }
        DeiCli::Assistant {
            session_id,
            files,
            priority,
        } => assistant(session_id, &files, priority.as_deref()).await,
        DeiCli::Queue { agent_id } => queue(agent_id).await,
        DeiCli::Rebalance => rebalance().await,
        DeiCli::Config => config().await,
        DeiCli::Pause { agent_id } => pause(agent_id).await,
        DeiCli::Resume { agent_id } => resume(agent_id).await,
        DeiCli::Save => save().await,
        DeiCli::Load => load().await,
        DeiCli::Undo { count } => undo(count).await,
        DeiCli::Redo { count } => redo(count).await,
        DeiCli::Workspace { cmd } => run_dei_workspace(cmd).await,
        DeiCli::Snapshot { cmd } => run_dei_snapshot(cmd).await,
        DeiCli::Oplog { cmd } => run_dei_oplog(cmd).await,
        DeiCli::TakeoverStatus { agent_id, human } => {
            run_dei_takeover_status(agent_id, human).await
        }
        DeiCli::Doubt { task_id, reason } => doubt(task_id, reason).await,
        DeiCli::Overrule { task_id, reason } => overrule(task_id, reason).await,
        // DeiCli::Analyze { path, apply } => run_dei_analyze(&path, apply).await,
    }
}

fn print_dei_json(v: &serde_json::Value) -> Result<()> {
    let _ = std::hint::black_box(v.is_null());
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

async fn run_dei_workspace(cmd: DeiWorkspaceCmd) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);
    let v = match cmd {
        DeiWorkspaceCmd::Create { agent_id } => {
            json_vcs_facade::workspace_create_json(&orch, agent_id)
        }
        DeiWorkspaceCmd::Status { agent_id } => {
            json_vcs_facade::workspace_status_json(&orch, agent_id)
        }
        DeiWorkspaceCmd::Merge { agent_id } => {
            json_vcs_facade::workspace_merge_json(&orch, agent_id)
        }
    };
    if v.get("merged") == Some(&serde_json::Value::Bool(false)) {
        anyhow::bail!(
            "no active workspace for this agent (same condition as MCP `vox_workspace_merge`)"
        );
    }
    print_dei_json(&v)?;
    Ok(())
}

async fn run_dei_snapshot(cmd: DeiSnapshotCmd) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);
    match cmd {
        DeiSnapshotCmd::List { agent_id, limit } => {
            let v = json_vcs_facade::snapshot_list_json(&orch, agent_id, limit);
            print_dei_json(&v)?;
        }
        DeiSnapshotCmd::Diff { before, after } => {
            let v = json_vcs_facade::snapshot_diff_json(&orch, before, after);
            if v.get("error").is_some() {
                anyhow::bail!("snapshot diff: one or both snapshot ids not found");
            }
            print_dei_json(&v)?;
        }
        DeiSnapshotCmd::Restore { snapshot_id } => {
            let v = json_vcs_facade::snapshot_restore_json(&orch, &snapshot_id)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            print_dei_json(&v)?;
        }
    }
    Ok(())
}

async fn run_dei_oplog(cmd: DeiOplogCmd) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);
    match cmd {
        DeiOplogCmd::List { agent_id, limit } => {
            let v = json_vcs_facade::oplog_list_json(&orch, agent_id, limit).await;
            print_dei_json(&v)?;
        }
    }
    Ok(())
}

async fn run_dei_takeover_status(agent_id: u64, human: bool) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);
    let repo = discover_repository_from_cwd(None);
    let v = json_vcs_facade::takeover_handoff_json(
        &orch,
        &repo.root.display().to_string(),
        &repo.repository_id,
        agent_id,
    )
    .await;
    if human {
        print_takeover_human_summary(&v);
        println!();
    }
    print_dei_json(&v)?;
    Ok(())
}

fn print_takeover_human_summary(v: &serde_json::Value) {
    println!("{}", "Takeover handoff (summary)".cyan().bold());
    if let Some(repo) = v.get("repository").and_then(|x| x.as_object()) {
        if let Some(id) = repo.get("repository_id").and_then(|x| x.as_str()) {
            println!("  {} {}", "repository_id:".bold(), id);
        }
        if let Some(root) = repo.get("root").and_then(|x| x.as_str()) {
            println!("  {} {}", "root:".bold(), root);
        }
    }
    let agent_id = v.get("agent_id").and_then(|x| x.as_u64()).unwrap_or(0);
    println!("  {} {}", "agent_id:".bold(), agent_id);
    if let Some(ws) = v.get("workspace").and_then(|x| x.as_object()) {
        let has = ws
            .get("has_workspace")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        if has {
            let n = ws
                .get("modified_count")
                .and_then(|x| x.as_u64())
                .or_else(|| {
                    ws.get("modified_files")
                        .and_then(|x| x.as_array())
                        .map(|a| a.len() as u64)
                })
                .unwrap_or(0);
            let base = ws
                .get("base_snapshot")
                .and_then(|x| x.as_str())
                .unwrap_or("—");
            println!(
                "  {} active workspace; {} modified file(s); base_snapshot {}",
                "workspace:".bold(),
                n,
                base
            );
        } else {
            println!("  {} none", "workspace:".bold());
        }
    }
    let snap_n = v
        .get("snapshots")
        .and_then(|x| x.get("snapshots"))
        .and_then(|x| x.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    println!(
        "  {} {} recent snapshot(s) in bundle",
        "snapshots:".bold(),
        snap_n
    );
    let op_n = v
        .get("oplog")
        .and_then(|x| x.get("operations"))
        .and_then(|x| x.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    println!(
        "  {} {} recent oplog entr{} in bundle",
        "oplog:".bold(),
        op_n,
        if op_n == 1 { "y" } else { "ies" }
    );
}

/*
async fn run_dei_analyze(path: &str, apply: bool) -> Result<()> {
    let config = load_config();
    let _orch = build_repo_scoped_orchestrator_cli(config);
    let path_buf = std::path::PathBuf::from(path);

    if !path_buf.exists() {
        anyhow::bail!("File not found: {}", path);
    }

    println!(
        "{} Analyzing {} for structural issues...",
        "ℹ".blue().bold(),
        path.bold()
    );

    // 1. Run vox check with IR emission to get the HIR diagnostics
    let result = vox_compiler::v0_api::check_file_v0(&path_buf, true);

    if result.diagnostics.is_empty() {
        println!("{} No diagnostics found. File is structurally sound.", "✓".green().bold());
        return Ok(());
    }

    let mut fixable_count = 0;
    for diag in &result.diagnostics {
        println!(
            "  {} [line {}]: {}",
            if diag.severity == vox_compiler::diagnostics::Severity::Error {
                "✗".red().bold()
            } else {
                "⚠".yellow().bold()
            },
            diag.line,
            diag.message
        );

        if let Some(hint) = &diag.correction_hint {
            println!("    {} {}", "Hint:".cyan().bold(), hint);
            fixable_count += 1;

            if apply && fixable_count == 1 {
                println!(
                    "    {} Automatically applying fix (dummy placeholder)...",
                    "→".magenta().bold()
                );
                // In a real implementation, we would apply the fix to the source here.
            }
        }
    }

    if fixable_count > 0 && !apply {
        println!();
        println!(
            "  {} found {} fixable issues. Run with --apply to remediate.",
            "ℹ".blue().bold(),
            fixable_count
        );
    }

    Ok(())
}
*/

/// Load orchestrator config from Vox.toml or defaults.
fn load_config() -> OrchestratorConfig {
    // Try to load from Vox.toml, fall back to defaults
    let mut config: OrchestratorConfig = std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            let toml_path = cwd.join("Vox.toml");
            OrchestratorConfig::load_from_toml(&toml_path).ok()
        })
        .unwrap_or_default();
    config.merge_env_overrides();
    config
}

fn build_repo_scoped_orchestrator_cli(config: OrchestratorConfig) -> std::sync::Arc<Orchestrator> {
    std::sync::Arc::new(build_repo_scoped_orchestrator(config, None).orchestrator)
}

async fn doubt(task_id: u64, reason: Option<String>) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);
    orch.doubt_task(TaskId(task_id), reason)
        .map_err(|e| anyhow::anyhow!(e))?;
    println!("{} Task {} flagged as suspect.", "✓".green().bold(), task_id);
    Ok(())
}

async fn overrule(task_id: u64, reason: String) -> Result<()> {
    let config = load_config();
    let orch = build_repo_scoped_orchestrator_cli(config);
    orch.overrule_task(TaskId(task_id), Some(reason))
        .map_err(|e| anyhow::anyhow!(e))?;
    println!("{} Task {} overruled and marked as completed.", "✓".green().bold(), task_id);
    Ok(())
}
