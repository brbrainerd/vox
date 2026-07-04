use clap::Subcommand;
use miette::Result;
use owo_colors::OwoColorize;
use vox_cli_core::daemon_ipc::orchestrator_daemon_ensure::OrchestratorDaemonEnsure;

/// Manage and inspect the Vox safety and coherence systems.
#[derive(Debug, Subcommand)]
pub enum SafetyCommand {
    /// Show current safety, drift, and budget status for all agents.
    Status,
    /// Inspect the cryptographic tool receipt ledger.
    Ledger {
        /// Optional: filter by agent.
        #[arg(long)]
        agent_id: Option<u64>,
    },
    /// Inspect active generic resource locks.
    Locks,
}

/// T2.3 follow-up: `vox safety` subcommands route through the shared
/// `vox-orchestrator-d` TCP daemon (spawning it if absent) instead of
/// building a private, throwaway in-process `Orchestrator` per invocation —
/// see `crates/vox-cli-core/src/daemon_ipc/orchestrator_daemon_ensure.rs` and
/// `commands/dei.rs`'s `daemon_client()` (same pattern). Before this fix,
/// `status_cmd`/`ledger_cmd`/`locks_cmd` each built a fresh, always-empty
/// local `Orchestrator` via `build_repo_scoped_orchestrator_for_repository`,
/// so `vox safety status` displayed fake/empty budget, drift, and lock data
/// rather than the real daemon's live state — the same class of bug T2.3
/// fixed for `dei save()`.
async fn daemon_client() -> miette::Result<vox_orchestrator::orch_daemon::OrchDaemonClient> {
    let ensure = OrchestratorDaemonEnsure::default();
    ensure
        .client()
        .await
        .map_err(|e| miette::miette!("could not reach or spawn vox-orchestrator-d: {e}"))
}

pub async fn handle_safety_command(
    cmd: SafetyCommand,
    workspace_root: &std::path::Path,
) -> Result<()> {
    let _ = workspace_root; // no longer used: all three subcommands now read daemon-shared state.
    match cmd {
        SafetyCommand::Status => status_cmd().await,
        SafetyCommand::Ledger { agent_id } => ledger_cmd(agent_id).await,
        SafetyCommand::Locks => locks_cmd().await,
    }
}

async fn status_cmd() -> Result<()> {
    let client = daemon_client().await?;

    println!("{}", "╔══════════════════════════════════════╗".cyan());
    println!("{}", "║   Vox Safety & Coherence Status      ║".cyan());
    println!("{}", "╚══════════════════════════════════════╝".cyan());
    println!();

    let signals = client
        .safety_budget_signals()
        .await
        .map_err(|e| miette::miette!("orch.safety_budget_signals failed: {e}"))?;
    let agents = signals
        .get("agents")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    println!("{}", "Agent Budgets & Drift:".bold().underline());
    for agent in &agents {
        let id = agent.get("id").and_then(|x| x.as_u64()).unwrap_or(0);
        let name = agent.get("name").and_then(|x| x.as_str()).unwrap_or("?");
        let signal = agent
            .get("signal")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let signal_str = format_budget_signal(&signal);

        println!(
            "  Agent {} ({}): {}",
            id.to_string().bold(),
            name,
            signal_str
        );
    }

    let status = client
        .orchestrator_status()
        .await
        .map_err(|e| miette::miette!("orch.status failed: {e}"))?;
    let locked_files = status
        .get("locked_files")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    println!();
    println!("{}", "Active Locks:".bold().underline());

    let ledger = client
        .safety_ledger(None)
        .await
        .map_err(|e| miette::miette!("orch.safety_ledger failed: {e}"))?;
    let receipt_count = ledger
        .get("receipts")
        .and_then(|r| r.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    println!("  Tool Receipts:  {}", receipt_count);

    let locks = client
        .safety_locks()
        .await
        .map_err(|e| miette::miette!("orch.safety_locks failed: {e}"))?;
    let lock_count = locks
        .get("locks")
        .and_then(|l| l.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    println!(
        "  Resource Locks: {} (locked_files: {})",
        lock_count, locked_files
    );

    Ok(())
}

/// Render a JSON-serialized `vox_orchestrator::budget::BudgetSignal` the same
/// way `status_cmd`'s previous local-orchestrator match arm did.
fn format_budget_signal(signal: &serde_json::Value) -> String {
    let Some(obj) = signal.as_object() else {
        return "Unknown".to_string();
    };
    if let Some(v) = obj.get("Normal") {
        let ratio = v.get("usage_ratio").and_then(|x| x.as_f64()).unwrap_or(0.0);
        return format!("Normal ({:.1}%)", ratio * 100.0);
    }
    if let Some(v) = obj.get("HighLoad") {
        let ratio = v.get("usage_ratio").and_then(|x| x.as_f64()).unwrap_or(0.0);
        return format!("High Load ({:.1}%)", ratio * 100.0);
    }
    if obj.contains_key("Critical") {
        return "CRITICAL".to_string();
    }
    if obj.contains_key("CostExceeded") {
        return "COST EXCEEDED".to_string();
    }
    if let Some(v) = obj.get("HaltAgent") {
        let reason = v.get("reason").and_then(|x| x.as_str()).unwrap_or("");
        return format!("HALTED: {}", reason);
    }
    if let Some(v) = obj.get("DoomLoopSuspect") {
        let calls = v
            .get("consecutive_calls")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        return format!("DOOM LOOP SUSPECT ({} calls)", calls);
    }
    "Unknown".to_string()
}

async fn ledger_cmd(agent_id_opt: Option<u64>) -> Result<()> {
    let client = daemon_client().await?;

    let ledger = client
        .safety_ledger(agent_id_opt)
        .await
        .map_err(|e| miette::miette!("orch.safety_ledger failed: {e}"))?;
    let receipts = ledger
        .get("receipts")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();

    println!("{}", "Tool Receipt Ledger".bold().underline());
    if receipts.is_empty() {
        println!("  (No receipts issued in this session)");
    } else {
        for receipt in &receipts {
            let id = receipt
                .get("receipt_id")
                .and_then(|x| x.as_str())
                .unwrap_or("?");
            let aid = receipt
                .get("agent_id")
                .and_then(|x| x.as_u64())
                .unwrap_or(0);
            let tool = receipt
                .get("tool_name")
                .and_then(|x| x.as_str())
                .unwrap_or("?");
            println!("  [{}] Agent {} -> {}", id.dimmed(), aid, tool.cyan());
        }
    }
    Ok(())
}

async fn locks_cmd() -> Result<()> {
    let client = daemon_client().await?;

    let locks = client
        .safety_locks()
        .await
        .map_err(|e| miette::miette!("orch.safety_locks failed: {e}"))?;
    let snapshot = locks
        .get("locks")
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default();

    println!("{}", "Active Resource Locks".bold().underline());
    if snapshot.is_empty() {
        println!("  (No active resource locks)");
    } else {
        for lock in &snapshot {
            let resource_id = lock
                .get("resource_id")
                .and_then(|x| x.as_str())
                .unwrap_or("?");
            let holder = lock.get("holder").and_then(|x| x.as_u64()).unwrap_or(0);
            println!("  {:30} held by Agent {}", resource_id.cyan(), holder);
        }
    }
    Ok(())
}
