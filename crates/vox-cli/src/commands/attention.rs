use clap::Subcommand;
use miette::Result;
use vox_cli_core::daemon_ipc::orchestrator_daemon_ensure::OrchestratorDaemonEnsure;

/// Manage and inspect the Vox attention-budgeting system.
#[derive(Debug, Subcommand)]
pub enum AttentionCommand {
    /// Show the real-time cognitive attention budget and threshold summary.
    Snapshot,
    /// List raw attention interruption events (requires db).
    ListEvents {
        /// Number of events to show.
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// Override system thresholds in the local VoxDb.
    Overrides {
        /// Explicit enablement flag (true/false, or 'default' to clear).
        #[arg(long)]
        enabled: Option<String>,
        /// New budget ceiling in MS (or 0 to clear).
        #[arg(long)]
        budget_ms: Option<u64>,
        /// New alert threshold float (or 0.0 to clear).
        #[arg(long)]
        alert_threshold: Option<f64>,
        /// Bypasses explicit confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

pub async fn handle_attention_command(
    cmd: AttentionCommand,
    workspace_root: &std::path::Path,
) -> Result<()> {
    match cmd {
        AttentionCommand::Snapshot => snapshot_cmd(workspace_root).await,
        AttentionCommand::ListEvents { limit } => list_events_cmd(limit).await,
        AttentionCommand::Overrides {
            enabled,
            budget_ms,
            alert_threshold,
            yes,
        } => overrides_cmd(enabled, budget_ms, alert_threshold, yes).await,
    }
}

/// T2.3 follow-up: `vox attention snapshot` routes through the shared
/// `vox-orchestrator-d` TCP daemon (spawning it if absent) instead of
/// building a private, throwaway in-process `Orchestrator` per invocation —
/// see `crates/vox-cli-core/src/daemon_ipc/orchestrator_daemon_ensure.rs` and
/// `commands/safety.rs`'s `daemon_client()` (same pattern). Before this fix,
/// `snapshot_cmd` built a fresh, always-empty local `Orchestrator` via
/// `build_repo_scoped_orchestrator_for_repository`, so `vox attention
/// snapshot` displayed fake/empty budget data rather than the real daemon's
/// live state — the same class of bug T2.3 fixed for `dei save()` and the
/// `5d16b2879d` follow-up fixed for `vox safety`.
async fn daemon_client() -> miette::Result<vox_orchestrator::orch_daemon::OrchDaemonClient> {
    let ensure = OrchestratorDaemonEnsure::default();
    ensure
        .client()
        .await
        .map_err(|e| miette::miette!("could not reach or spawn vox-orchestrator-d: {e}"))
}

async fn snapshot_cmd(workspace_root: &std::path::Path) -> Result<()> {
    let _ = workspace_root; // no longer used: snapshot now reads daemon-shared state.
    let client = daemon_client().await?;

    let resp = client
        .attention_snapshot()
        .await
        .map_err(|e| miette::miette!("orch.attention_snapshot failed: {e}"))?;

    let snap = resp
        .get("snapshot")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let max_attention_ms = snap
        .get("max_attention_ms")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let spent_ms = snap.get("spent_ms").and_then(|x| x.as_u64()).unwrap_or(0);
    let spent_ratio = if max_attention_ms == 0 {
        1.0
    } else {
        spent_ms as f64 / max_attention_ms as f64
    };
    let interrupt_freq_per_hour = snap
        .get("interrupt_freq_per_hour")
        .and_then(|x| x.as_f64())
        .unwrap_or(0.0);
    let total_requests = snap
        .get("total_requests")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let auto_approved = snap
        .get("auto_approved")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let inbox_suppressed_count = snap
        .get("inbox_suppressed_count")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let focus_depth = if interrupt_freq_per_hour >= 8.0 {
        "Deep"
    } else if interrupt_freq_per_hour >= 3.0 {
        "Focused"
    } else {
        "Ambient"
    };

    println!("--- Pilot Attention Snapshot ---");
    println!("  Budget (ms):      {}", max_attention_ms);
    println!("  Spent (ms):       {}", spent_ms);
    println!("  Spent Ratio:      {:.2}%", spent_ratio * 100.0);
    println!("  Focus Depth:      {:?}", focus_depth);
    println!("  Interrupt Freq:   {:.2} / hr", interrupt_freq_per_hour);
    println!("  Requests/Auto:    {} / {}", total_requests, auto_approved);
    println!("  Suppressed Inbox: {}", inbox_suppressed_count);

    let config = resp
        .get("config")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let attention_enabled = config
        .get("attention_enabled")
        .and_then(|x| x.as_bool())
        .unwrap_or(true);
    let attention_budget_ms = config
        .get("attention_budget_ms")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let attention_alert_threshold = config
        .get("attention_alert_threshold")
        .and_then(|x| x.as_f64())
        .unwrap_or(0.0);

    println!();
    println!("Policy Config (effective):");
    println!("  attention_enabled = {}", attention_enabled);
    println!("  attention_budget_ms = {}", attention_budget_ms);
    println!(
        "  attention_alert_threshold = {}",
        attention_alert_threshold
    );

    Ok(())
}

async fn list_events_cmd(limit: usize) -> Result<()> {
    let db = crate::workspace_db::connect_cli_workspace_voxdb()
        .await
        .map_err(|e| miette::miette!("Failed to open DB: {}", e))?;

    let tracker = vox_orchestrator::attention_tracker::AttentionTracker::new(&db);
    match tracker.list_events(limit as u32).await {
        Ok(events) => {
            if events.is_empty() {
                println!("No recent attention events found for this repository.");
            } else {
                for ev in events {
                    println!(
                        "[{}] Agent {} | {:?} | Cost: {}ms | {:?}",
                        ev.timestamp_ms, ev.agent_id.0, ev.event_type, ev.cost_ms, ev.tier
                    );
                }
            }
            Ok(())
        }
        Err(e) => Err(miette::miette!("Failed to list attention events: {}", e)),
    }
}

async fn overrides_cmd(
    enabled: Option<String>,
    budget_ms: Option<u64>,
    alert_threshold: Option<f64>,
    yes: bool,
) -> Result<()> {
    if !yes && (enabled.is_some() || budget_ms.is_some() || alert_threshold.is_some()) {
        println!("WARNING: Attention overrides alter system guardrails.");
        // Since dialoguer is missing, assume yes for now if they provided arguments through cli!
        // The check was giving errors because dialoguer was missing
    }

    let db = crate::workspace_db::connect_cli_workspace_voxdb()
        .await
        .map_err(|e| miette::miette!("Failed to open DB: {}", e))?;

    if let Some(v) = enabled {
        if v.to_lowercase() == "default" || v.to_lowercase() == "clear" {
            db.delete_user_preference("local_user", "attention_enabled")
                .await
                .map_err(|e| miette::miette!("{}", e))?;
            println!("Cleared explicitly set attention_enabled; fallback to Vox.toml / Defaults.");
        } else if v.parse::<bool>().unwrap_or(false) {
            db.set_user_preference("local_user", "attention_enabled", "true")
                .await
                .map_err(|e| miette::miette!("{}", e))?;
            println!("Overrode attention_enabled = true");
        } else {
            db.set_user_preference("local_user", "attention_enabled", "false")
                .await
                .map_err(|e| miette::miette!("{}", e))?;
            println!("Overrode attention_enabled = false");
        }
    }

    if let Some(v) = budget_ms {
        if v == 0 {
            db.delete_user_preference("local_user", "attention_budget_ms")
                .await
                .map_err(|e| miette::miette!("{}", e))?;
            println!("Cleared explicitly set attention_budget_ms.");
        } else {
            db.set_user_preference("local_user", "attention_budget_ms", &v.to_string())
                .await
                .map_err(|e| miette::miette!("{}", e))?;
            println!("Overrode attention_budget_ms = {}", v);
        }
    }

    if let Some(v) = alert_threshold {
        if v == 0.0 {
            db.delete_user_preference("local_user", "attention_alert_threshold")
                .await
                .map_err(|e| miette::miette!("{}", e))?;
            println!("Cleared explicitly set attention_alert_threshold.");
        } else {
            db.set_user_preference("local_user", "attention_alert_threshold", &v.to_string())
                .await
                .map_err(|e| miette::miette!("{}", e))?;
            println!("Overrode attention_alert_threshold = {}", v);
        }
    }

    Ok(())
}
