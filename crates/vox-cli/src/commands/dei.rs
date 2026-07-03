#![cfg(feature = "dei")]
use anyhow::Result;
use owo_colors::OwoColorize;
use serde::Deserialize;
use vox_cli_core::daemon_ipc::orchestrator_daemon_ensure::OrchestratorDaemonEnsure;
use vox_foundation::protocol::orch_daemon_method;
use vox_orchestrator::{FileAffinity, OrchestratorConfig, TaskPriority};

/// Deserializable mirror of the fields `dei.rs` reads from
/// [`vox_orchestrator::orchestrator::types::OrchestratorStatus`]'s JSON
/// (that type only derives `Serialize`, since it's the daemon's *outgoing*
/// wire shape — this is the CLI's read-side view over the same JSON).
#[derive(Debug, Deserialize)]
struct DaemonOrchestratorStatus {
    enabled: bool,
    agent_count: usize,
    total_queued: usize,
    total_in_progress: usize,
    total_completed: usize,
    locked_files: usize,
    total_weighted_load: f64,
    predicted_load: f64,
    reserved_agents: usize,
    dynamic_agents: usize,
    agents: Vec<DaemonAgentSummary>,
}

#[derive(Debug, Deserialize)]
struct DaemonAgentSummary {
    id: u64,
    name: String,
    queued: usize,
    urgent_count: usize,
    normal_count: usize,
    background_count: usize,
    in_progress: bool,
    completed: usize,
    paused: bool,
    owned_files: usize,
    dynamic: bool,
    weighted_load: f64,
}

/// T2.3: `vox dei` subcommands route through the shared `vox-orchestrator-d`
/// TCP daemon (spawning it if absent) instead of building a private,
/// throwaway in-process `Orchestrator` per invocation — see
/// `crates/vox-cli-core/src/daemon_ipc/orchestrator_daemon_ensure.rs`. This
/// closes the split-brain gap T2.1 fixed for the GUI: a `vox dei doubt`/`vox
/// dei submit` run from a terminal is now visible to the GUI's Approvals/DEI
/// views (and vice versa), rather than mutating a state the daemon never
/// sees.
async fn daemon_client() -> Result<vox_orchestrator::orch_daemon::OrchDaemonClient> {
    let ensure = OrchestratorDaemonEnsure::default();
    ensure
        .client()
        .await
        .map_err(|e| anyhow::anyhow!("could not reach or spawn vox-orchestrator-d: {e}"))
}

/// Call an MCP tool through the daemon's `orch.tool_call` RPC and unwrap the
/// `ToolResult<T>` envelope (`{"success": bool, "data": .., "error": ..}`)
/// into a plain `Result`. Mirrors `vox-orchestrator-mcp::daemon_route::call_tool_via_daemon`
/// (T2.2's stdio-MCP pattern) but returns the already-parsed `data` payload
/// since every `dei.rs` call site wants the inner value, not the raw envelope.
async fn tool_call(
    client: &vox_orchestrator::orch_daemon::OrchDaemonClient,
    name: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value> {
    let envelope = client
        .call(
            orch_daemon_method::TOOL_CALL,
            serde_json::json!({ "name": name, "args": args }),
        )
        .await?;
    match envelope.get("success").and_then(|s| s.as_bool()) {
        Some(true) => Ok(envelope.get("data").cloned().unwrap_or(serde_json::Value::Null)),
        Some(false) => {
            let msg = envelope
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("tool call failed");
            anyhow::bail!("{msg}")
        }
        // Tools that don't use the ToolResult envelope (rare) — pass through raw.
        None => Ok(envelope),
    }
}

/// `vox orchestrator status` — show all agents, queues, and file assignments.
pub async fn status() -> Result<()> {
    let client = daemon_client().await?;
    let raw = client.orchestrator_status().await?;
    let status: DaemonOrchestratorStatus = serde_json::from_value(raw)
        .map_err(|e| anyhow::anyhow!("daemon returned malformed status: {e}"))?;
    // scaling_threshold/scaling_profile are static Vox.toml-derived config, not
    // daemon-shared mutable state — reading them locally (like `config()`
    // below) is not a split-brain risk, unlike the agent/task counts above.
    let config = load_config();

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
    let client = daemon_client().await?;

    let file_manifest: Vec<FileAffinity> = files.iter().map(FileAffinity::write).collect();

    let priority = match priority {
        Some("urgent") => Some(TaskPriority::Urgent),
        Some("background") => Some(TaskPriority::Background),
        _ => None,
    };

    // The daemon's SUBMIT_TASK handler treats an explicit JSON `null` for
    // "priority" as "params.get(..) returned Some(Null)" and tries to
    // deserialize that as a (non-Option) TaskPriority, which fails — the
    // field must be OMITTED entirely (not present as null) to mean "use the
    // default priority", same contract as "session_id" below.
    let mut params = serde_json::json!({
        "description": description,
        "file_manifest": file_manifest,
    });
    let obj = params.as_object_mut().expect("json!({...}) is an object");
    if let Some(p) = priority {
        obj.insert("priority".to_string(), serde_json::to_value(p)?);
    }
    if let Some(sid) = session_id {
        obj.insert("session_id".to_string(), serde_json::Value::String(sid));
    }

    match client.submit_task(params).await {
        Ok(v) => {
            if let Some(task_id) = v.get("task_id").and_then(|x| x.as_u64()) {
                println!(
                    "  {} Task {} submitted successfully",
                    "✓".green().bold(),
                    task_id.to_string().bold()
                );
            } else if let Some(dup) = v.get("duplicate_of").and_then(|x| x.as_u64()) {
                println!(
                    "  {} Near-duplicate of task {}; not submitted",
                    "ℹ".blue().bold(),
                    dup
                );
            } else {
                println!("  {} Task submitted: {}", "✓".green().bold(), v);
            }
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
    // Background task scheduling is the shared daemon's responsibility now
    // (it runs its own spawn_background_tasks loop); this loop just submits.
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
///
/// T2.3: the daemon does not expose a dedicated "one agent's queue as
/// markdown" RPC (that rendering lived on the in-process `PriorityQueue`
/// type). Reimplemented as a markdown-equivalent view over
/// [`orch_daemon_method::LIST_TASKS`] filtered to `agent_id`, so this stays
/// daemon-routed rather than reintroducing a private orchestrator.
pub async fn queue(agent_id: u64) -> Result<()> {
    let client = daemon_client().await?;
    let v = client
        .call(orch_daemon_method::LIST_TASKS, serde_json::json!({}))
        .await?;
    let tasks = v.get("tasks").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let mine: Vec<&serde_json::Value> = tasks
        .iter()
        .filter(|t| t.get("agent_id").and_then(|a| a.as_u64()) == Some(agent_id))
        .collect();

    if mine.is_empty() {
        println!(
            "  {} Agent {} has no tasks (or does not exist)",
            "ℹ".blue().bold(),
            agent_id
        );
        return Ok(());
    }

    println!("## Agent {agent_id}\n");
    for t in mine {
        let id = t.get("id").and_then(|x| x.as_u64()).unwrap_or(0);
        let desc = t.get("description").and_then(|x| x.as_str()).unwrap_or("");
        let priority = t.get("priority").and_then(|x| x.as_str()).unwrap_or("Normal");
        let lifecycle = t.get("lifecycle").and_then(|x| x.as_str()).unwrap_or("Queued");
        let marker = if lifecycle == "InProgress" { "/" } else { " " };
        println!("- [{marker}] **[{id}]** {desc} ({priority})");
    }
    println!();

    Ok(())
}

/// `vox orchestrator rebalance` — trigger manual rebalancing.
pub async fn rebalance() -> Result<()> {
    let client = daemon_client().await?;
    let v = client.rebalance().await?;
    let moved = v.get("rebalanced").and_then(|x| x.as_u64()).unwrap_or(0);
    if moved > 0 {
        println!("  {} Rebalanced: {} tasks moved", "✓".green().bold(), moved);
    } else {
        println!("  {} No rebalancing needed", "ℹ".blue().bold());
    }

    Ok(())
}

/// `vox orchestrator config` — show current orchestrator configuration.
///
/// T2.3: kept local (not routed through the daemon). This reads static
/// `Vox.toml`/defaults via [`load_config`] — it is not shared, mutable
/// daemon state (no task queue, no approvals), so there is no split-brain
/// risk in reading it locally, and the daemon's own `config.get` RPC
/// (`orch_daemon_method::CONFIG_GET`/`ai.` `config.get`) exposes a narrower
/// subset (`max_agents`/`planning_enabled`/...) than this command's full
/// scaling/queue/cost-preference dump.
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
    let client = daemon_client().await?;
    match client.pause_agent(agent_id).await {
        Ok(_) => println!("  {} Agent {} paused", "✓".green().bold(), agent_id),
        Err(e) => println!("  {} {}", "✗".red().bold(), e),
    }

    Ok(())
}

/// `vox orchestrator resume` — resume an agent.
pub async fn resume(agent_id: u64) -> Result<()> {
    let client = daemon_client().await?;
    match client.resume_agent(agent_id).await {
        Ok(_) => println!("  {} Agent {} resumed", "✓".green().bold(), agent_id),
        Err(e) => println!("  {} {}", "✗".red().bold(), e),
    }

    Ok(())
}

/// `vox orchestrator save` — manually save orchestrator state.
///
/// T2.3: previously snapshotted a freshly-constructed, always-empty local
/// `Orchestrator`'s `.status()` — i.e. it never actually persisted the real
/// (daemon-owned) state. Now snapshots the shared daemon's live status.
pub async fn save() -> Result<()> {
    let config = load_config();
    let client = daemon_client().await?;
    let raw = client.orchestrator_status().await?;
    let status: DaemonOrchestratorStatus = serde_json::from_value(raw)
        .map_err(|e| anyhow::anyhow!("daemon returned malformed status: {e}"))?;
    let store = vox_db::VoxDb::open_default().await?;
    // Built directly from the daemon's JSON status rather than
    // `OrchestratorState::from_status` (which requires the real, non-`Deserialize`
    // `OrchestratorStatus` struct — see `DaemonOrchestratorStatus`'s doc comment).
    // `context_entries` isn't part of this CLI's status view (STATUS's daemon
    // JSON does include it, but `dei.rs` never rendered it before either) — an
    // empty map here doesn't regress anything the old local-orchestrator path
    // captured, since that path's orchestrator was always freshly-constructed
    // and empty to begin with.
    let state = vox_orchestrator::state::OrchestratorState {
        version: 1,
        config: config.clone(),
        agents: status
            .agents
            .iter()
            .map(|a| vox_orchestrator::state::SavedAgentState {
                id: a.id,
                name: a.name.clone(),
                queued_count: a.queued,
                urgent_count: a.urgent_count,
                normal_count: a.normal_count,
                background_count: a.background_count,
                completed_count: a.completed,
                paused: a.paused,
            })
            .collect(),
        total_completed: status.total_completed,
        saved_at: chrono::Utc::now().to_rfc3339(),
        context_entries: Default::default(),
        plugin_states: Default::default(),
    };

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
///
/// T2.3: no daemon RPC for emergency-stop exists yet
/// (`orch_daemon_method` has no `EMERGENCY_STOP`/equivalent). Adding one is a
/// backend change outside this task's scope — scoped down to an explicit
/// follow-up rather than silently left broken: this still calls
/// `emergency_stop` on a freshly-constructed **local** orchestrator, which
/// does NOT stop the shared daemon's agents (pre-existing gap, now
/// documented rather than silently carried forward).
pub async fn stop(reason: Option<String>) -> Result<()> {
    eprintln!(
        "  {} `vox stop` does not yet reach the shared vox-orchestrator-d daemon \
         (no orch_daemon_method::EMERGENCY_STOP exists); this only stops a \
         throwaway local orchestrator instance. Follow-up: add a daemon RPC \
         (T2.4 candidate).",
        "⚠".yellow().bold()
    );
    let config = load_config();
    let orch = vox_orchestrator::build_repo_scoped_orchestrator(config, None).orchestrator;
    orch.emergency_stop(reason.clone());
    println!(
        "  {} Local orchestrator emergency stop requested (daemon unaffected)",
        "✓".green().bold()
    );
    Ok(())
}

/// `vox orchestrator load` — manually load orchestrator state.
///
/// T2.3: reads only `VoxDb` (a persistence read, not shared orchestrator
/// state); the previous local `Orchestrator` construction here was unused
/// dead weight (bound to `_orch`, never applied) — dropped rather than
/// routed through the daemon, since there is nothing to route.
pub async fn load() -> Result<()> {
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

/// Shared body for [`undo`]/[`redo`]: list the daemon's oplog via the
/// `vox_oplog` MCP tool (`orch.tool_call`), find the last entry matching
/// `wants_undone` (mirrors the previous local `.find(|e| e.undone == ...)`
/// scan), then apply `apply_method` (`vox_undo`/`vox_redo`) to it. Loops up
/// to `count` times, stopping early on the first "nothing left" or error —
/// same semantics as the original local-oplog-scan implementation.
async fn undo_redo_via_daemon(
    count: usize,
    wants_undone: bool,
    apply_tool: &str,
    verb: &str,
) -> Result<()> {
    let client = daemon_client().await?;
    let mut successful = 0usize;

    for _ in 0..count {
        let list = tool_call(&client, "vox_oplog", serde_json::json!({ "limit": 50 })).await?;
        let ops = list.get("operations").and_then(|o| o.as_array()).cloned().unwrap_or_default();
        // History is newest-first per json_vcs_facade::oplog_list_json's ordering.
        let Some(op) = ops
            .iter()
            .find(|e| e.get("undone").and_then(|u| u.as_bool()) == Some(wants_undone))
        else {
            println!("  {} No more operations to {}", "ℹ".blue().bold(), verb);
            break;
        };
        let id = op.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let desc = op
            .get("description")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();

        match tool_call(&client, apply_tool, serde_json::json!({ "operation_id": id })).await {
            Ok(_) => {
                successful += 1;
                let past_tense = if verb == "undo" { "Undid" } else { "Redid" };
                println!(
                    "  {} {} operation {} ({})",
                    "✓".green().bold(),
                    past_tense,
                    id.bold(),
                    desc
                );
            }
            Err(e) => {
                println!("  {} Failed to {} operation {}: {}", "✗".red().bold(), verb, id, e);
                break;
            }
        }
    }

    if successful > 0 {
        println!(
            "\n  {} successfully {}d {} operations",
            "✓".green(),
            verb,
            successful
        );
    }

    Ok(())
}

/// `vox orchestrator undo` — undo the last N operations.
pub async fn undo(count: usize) -> Result<()> {
    undo_redo_via_daemon(count, false, "vox_undo", "undo").await
}

/// `vox orchestrator redo` — redo the last N undone operations.
pub async fn redo(count: usize) -> Result<()> {
    undo_redo_via_daemon(count, true, "vox_redo", "redo").await
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
    let client = daemon_client().await?;
    let v = match cmd {
        DeiWorkspaceCmd::Create { agent_id } => {
            tool_call(&client, "vox_workspace_create", serde_json::json!({ "agent_id": agent_id }))
                .await?
        }
        DeiWorkspaceCmd::Status { agent_id } => {
            tool_call(&client, "vox_workspace_status", serde_json::json!({ "agent_id": agent_id }))
                .await?
        }
        DeiWorkspaceCmd::Merge { agent_id } => {
            // vox_workspace_merge's ToolResult is itself an error (not a
            // {"merged":false} success payload) when there's no active
            // workspace — tool_call already turns that into an Err, so the
            // explicit "merged == false" bail! from the old local-orchestrator
            // path is redundant here; propagate via `?` instead.
            tool_call(&client, "vox_workspace_merge", serde_json::json!({ "agent_id": agent_id }))
                .await?
        }
    };
    print_dei_json(&v)?;
    Ok(())
}

async fn run_dei_snapshot(cmd: DeiSnapshotCmd) -> Result<()> {
    let client = daemon_client().await?;
    match cmd {
        DeiSnapshotCmd::List { agent_id, limit } => {
            let v = tool_call(
                &client,
                "vox_snapshot_list",
                serde_json::json!({ "agent_id": agent_id, "limit": limit }),
            )
            .await?;
            print_dei_json(&v)?;
        }
        DeiSnapshotCmd::Diff { before, after } => {
            let v = tool_call(
                &client,
                "vox_snapshot_diff",
                serde_json::json!({ "before": before, "after": after }),
            )
            .await?;
            print_dei_json(&v)?;
        }
        DeiSnapshotCmd::Restore { snapshot_id } => {
            let v = tool_call(
                &client,
                "vox_snapshot_restore",
                serde_json::json!({ "snapshot_id": snapshot_id }),
            )
            .await?;
            print_dei_json(&v)?;
        }
    }
    Ok(())
}

async fn run_dei_oplog(cmd: DeiOplogCmd) -> Result<()> {
    let client = daemon_client().await?;
    match cmd {
        DeiOplogCmd::List { agent_id, limit } => {
            let v = tool_call(
                &client,
                "vox_oplog",
                serde_json::json!({ "agent_id": agent_id, "limit": limit }),
            )
            .await?;
            print_dei_json(&v)?;
        }
    }
    Ok(())
}

/// `vox dei takeover-status` — aggregated repo + workspace + snapshot/oplog
/// tails for human handoff.
///
/// T2.3: no single daemon RPC/MCP tool bundles this exact
/// repo-identity+workspace+snapshots+oplog shape
/// (`json_vcs_facade::takeover_handoff_json`). Reassembled client-side from
/// three daemon-routed calls (workspace/snapshot/oplog, all already migrated
/// above) plus local repo-identity discovery (`discover_repository_from_cwd`
/// is a pure filesystem read, not orchestrator state) — same external JSON
/// shape as before, now sourced from the shared daemon instead of a private
/// throwaway orchestrator.
async fn run_dei_takeover_status(agent_id: u64, human: bool) -> Result<()> {
    let client = daemon_client().await?;
    let repo = vox_orchestrator::discover_repository_from_cwd(None);

    let workspace = tool_call(&client, "vox_workspace_status", serde_json::json!({ "agent_id": agent_id }))
        .await
        .unwrap_or_else(|e| serde_json::json!({ "has_workspace": false, "error": e.to_string() }));
    let snapshots = tool_call(
        &client,
        "vox_snapshot_list",
        serde_json::json!({ "agent_id": agent_id, "limit": 5 }),
    )
    .await
    .unwrap_or_else(|_| serde_json::json!({ "snapshots": [] }));
    let oplog = tool_call(
        &client,
        "vox_oplog",
        serde_json::json!({ "agent_id": agent_id, "limit": 5 }),
    )
    .await
    .unwrap_or_else(|_| serde_json::json!({ "operations": [] }));

    let v = serde_json::json!({
        "schema": "vox_takeover_handoff_v1",
        "schema_version": 1,
        "repository": {
            "root": repo.root.display().to_string(),
            "repository_id": repo.repository_id,
        },
        "agent_id": agent_id.to_string(),
        "workspace": workspace,
        "snapshots": snapshots,
        "oplog": oplog,
    });
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

fn load_config() -> OrchestratorConfig {
    vox_orchestrator_driver::build_embedded_orchestrator_config()
}

/// `vox dei doubt` — flag a task as suspect via the daemon's
/// [`orch_daemon_method::DOUBT_TASK`] RPC.
///
/// T2.3: previously ran against a private local `Orchestrator`, and its own
/// comment noted this CLI path did NOT go through the MCP/daemon oplog wiring
/// (a pre-existing T1.1 gap it had to work around with an immediate manual
/// `emit_doubt_events` broadcast — see the removed comment). The daemon's
/// `DOUBT_TASK` handler (`orch_daemon/mod.rs`) already does the durable
/// oplog-record-then-broadcast sequence correctly, so routing through it
/// fixes that gap rather than reintroducing the workaround.
pub async fn doubt(task_id: u64, reason: Option<String>) -> Result<()> {
    let client = daemon_client().await?;
    client
        .call(
            orch_daemon_method::DOUBT_TASK,
            serde_json::json!({ "task_id": task_id, "reason": reason }),
        )
        .await?;
    println!(
        "{} Task {} flagged as suspect.",
        "✓".green().bold(),
        task_id
    );
    Ok(())
}

/// `vox dei overrule` — force-complete a doubted/failed task via the
/// daemon's [`orch_daemon_method::OVERRULE_TASK`] RPC.
///
/// T2.3: same rationale as [`doubt`] — the daemon's `OVERRULE_TASK` handler
/// already performs the durable `TaskComplete` oplog record before
/// broadcasting, so the manual `record_operation` + `emit_overrule_events`
/// this CLI command used to do against a private local orchestrator is no
/// longer needed client-side.
async fn overrule(task_id: u64, reason: String) -> Result<()> {
    let client = daemon_client().await?;
    client
        .call(
            orch_daemon_method::OVERRULE_TASK,
            serde_json::json!({ "task_id": task_id, "reason": reason }),
        )
        .await?;
    println!(
        "{} Task {} overruled and marked as completed.",
        "✓".green().bold(),
        task_id
    );
    Ok(())
}
