//! `vox live` — real-time Matrix-style EventBus dashboard.
//!
//! Subscribes to orchestrator [`AgentEvent`](vox_orchestrator::AgentEvent)s and renders a live,
//! updating dashboard in the terminal. Press Ctrl+C to quit.
//!
//! ## Shared bus with MCP
//!
//! When **`VOX_ORCHESTRATOR_EVENT_LOG`** is set to a file path, `vox-mcp` appends one JSON object
//! per line (serialized [`vox_orchestrator::AgentEvent`]). High-volume token stream events are omitted
//! from that file sink to reduce log noise, so in file-tail mode the token counter may stay at zero
//! even while tasks run (use in-process mode without the env var for full token stats).
//! The same env var makes **this** command tail that file instead of starting an in-process demo
//! orchestrator, so `vox live` and MCP observe the same task/VCS/cost stream.

use anyhow::Result;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use tokio::time::{Duration, sleep};
use vox_orchestrator::events::AgentEventKind;
use vox_orchestrator::{AgentEvent, OrchestratorConfig, build_repo_scoped_orchestrator};

const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const MAGENTA: &str = "\x1b[35m";
const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";
const CLEAR_SCREEN: &str = "\x1b[2J\x1b[H";

#[derive(Default, Clone)]
struct LiveStats {
    tasks_submitted: u64,
    tasks_completed: u64,
    tasks_failed: u64,
    tokens_total_chars: u64,
    snapshots_captured: u64,
    conflicts_detected: u64,
    total_cost_usd: f64,
    cost_events: u64,
    rebalances: u64,
    recent_events: Vec<String>,
}

const MAX_RECENT: usize = 12;

impl LiveStats {
    fn push_event(&mut self, msg: impl Into<String>) {
        if self.recent_events.len() >= MAX_RECENT {
            self.recent_events.remove(0);
        }
        self.recent_events.push(msg.into());
    }
}

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn render(stats: &LiveStats, tick: u64) {
    let spin = SPINNER[(tick as usize) % SPINNER.len()];
    print!("{CLEAR_SCREEN}");
    println!(
        "{BOLD}{CYAN}  {spin} VOX LIVE DASHBOARD{RESET}  {DIM}— real-time event stream (Ctrl+C to exit){RESET}"
    );
    println!("  {DIM}─────────────────────────────────────────────────────────────{RESET}");
    println!();
    println!(
        "  {BOLD}{GREEN}Tasks{RESET}     submitted {YELLOW}{:>6}{RESET}   completed {GREEN}{:>6}{RESET}   failed {RED}{:>5}{RESET}",
        stats.tasks_submitted, stats.tasks_completed, stats.tasks_failed,
    );
    println!(
        "  {BOLD}{MAGENTA}LLM{RESET}       tokens    {CYAN}{:>8}{RESET}   cost today {YELLOW}${:.4}{RESET}  ({} calls)",
        stats.tokens_total_chars, stats.total_cost_usd, stats.cost_events,
    );
    println!(
        "  {BOLD}VCS{RESET}       snapshots {DIM}{:>6}{RESET}   conflicts  {RED}{:>5}{RESET}   rebalances {DIM}{:>4}{RESET}",
        stats.snapshots_captured, stats.conflicts_detected, stats.rebalances,
    );
    println!();
    println!("  {DIM}─────────────────────────────────────────────────────────────{RESET}");
    println!("  {BOLD}Recent Events{RESET}");
    println!();
    let pad = MAX_RECENT.saturating_sub(stats.recent_events.len());
    for _ in 0..pad {
        println!("  {DIM}  ·{RESET}");
    }
    for line in &stats.recent_events {
        println!("  {line}");
    }
}

fn merge_agent_event(stats: &mut LiveStats, event: &AgentEvent) {
    match &event.kind {
        AgentEventKind::TaskSubmitted {
            task_id,
            agent_id,
            description,
            ..
        } => {
            stats.tasks_submitted += 1;
            let short: String = description.chars().take(36).collect();
            stats.push_event(format!(
                "{YELLOW}▶ submitted{RESET}  #{task_id}  agent={a}  {short}",
                a = agent_id.0
            ));
        }
        AgentEventKind::TaskCompleted {
            task_id, agent_id, ..
        } => {
            stats.tasks_completed += 1;
            stats.push_event(format!(
                "{GREEN}✓ completed{RESET}  #{task_id}  agent={}",
                agent_id.0
            ));
        }
        AgentEventKind::TaskFailed {
            task_id,
            agent_id,
            error,
            ..
        } => {
            stats.tasks_failed += 1;
            let short: String = error.chars().take(38).collect();
            stats.push_event(format!(
                "{RED}✗ failed{RESET}     #{task_id}  agent={}  {short}",
                agent_id.0
            ));
        }
        AgentEventKind::TokenStreamed { text, .. } => {
            stats.tokens_total_chars += text.chars().count() as u64;
        }
        AgentEventKind::CostIncurred {
            provider,
            model,
            input_tokens,
            output_tokens,
            cost_usd,
            ..
        } => {
            stats.cost_events += 1;
            stats.total_cost_usd += cost_usd;
            stats.push_event(format!(
                "{MAGENTA}$ cost{RESET}       {provider}/{model}  {input_tokens}+{output_tokens}tok  ${cost_usd:.6}"
            ));
        }
        AgentEventKind::SnapshotCaptured {
            agent_id,
            file_count,
            description,
            ..
        } => {
            stats.snapshots_captured += 1;
            let short: String = description.chars().take(34).collect();
            stats.push_event(format!(
                "{CYAN}📸 snapshot{RESET}  agent={}  files={file_count}  {short}",
                agent_id.0
            ));
        }
        AgentEventKind::ConflictDetected {
            path, conflict_id, ..
        } => {
            stats.conflicts_detected += 1;
            stats.push_event(format!(
                "{RED}⚡ conflict{RESET}  id={conflict_id}  {}",
                path.display()
            ));
        }
        AgentEventKind::ConflictResolved {
            conflict_id,
            resolution_strategy,
        } => {
            stats.push_event(format!(
                "{GREEN}✔ resolved{RESET}   id={conflict_id}  via={resolution_strategy}"
            ));
        }
        AgentEventKind::UrgentRebalanceTriggered { moved } => {
            stats.rebalances += 1;
            stats.push_event(format!("{YELLOW}⇄ rebalance{RESET} moved={moved} tasks"));
        }
        AgentEventKind::OperationUndone { operation_id, .. } => {
            stats.push_event(format!("{DIM}↩ undo{RESET}       op={operation_id}"));
        }
        AgentEventKind::OperationRedone { operation_id, .. } => {
            stats.push_event(format!("{DIM}↪ redo{RESET}       op={operation_id}"));
        }
        AgentEventKind::AgentSpawned { agent_id, name } => {
            stats.push_event(format!(
                "{CYAN}+ spawn{RESET}      agent={}  name={name}",
                agent_id.0
            ));
        }
        AgentEventKind::AgentRetired { agent_id } => {
            stats.push_event(format!("{DIM}− retire{RESET}     agent={}", agent_id.0));
        }
        _ => {}
    }
}

/// Validate env path and ensure we can append (creates an empty file if missing, like `vox-mcp`).
fn validate_event_log_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        anyhow::bail!(
            "VOX_ORCHESTRATOR_EVENT_LOG is set but empty; set it to a JSONL file path (e.g. /tmp/vox-events.jsonl)."
        );
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            anyhow::bail!(
                "VOX_ORCHESTRATOR_EVENT_LOG parent directory does not exist: {} (create it or fix the path).",
                parent.display()
            );
        }
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| {
            anyhow::anyhow!(
                "Cannot open or create VOX_ORCHESTRATOR_EVENT_LOG at {}: {e}",
                path.display()
            )
        })?;
    f.flush().ok();
    Ok(())
}

async fn run_event_log_tail(path: PathBuf) -> Result<()> {
    let mut stats = LiveStats::default();
    let mut tick: u64 = 0;
    let mut pos: u64 = 0;
    let mut unreadable_logged = false;
    render(&stats, tick);
    loop {
        sleep(vox_config::timeouts::D_250MS).await;
        tick = tick.wrapping_add(1);
        if let Ok(meta) = std::fs::metadata(&path) {
            let len = meta.len();
            if len < pos {
                pos = 0;
            }
            if len > pos {
                if let Ok(mut f) = std::fs::File::open(&path) {
                    let _ = f.seek(SeekFrom::Start(pos));
                    let mut buf = String::new();
                    if f.read_to_string(&mut buf).is_ok() {
                        pos = len;
                        for line in buf.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }
                            if let Ok(ev) = serde_json::from_str::<AgentEvent>(line) {
                                merge_agent_event(&mut stats, &ev);
                            }
                        }
                    }
                }
            }
        } else if tick.is_multiple_of(12) && !unreadable_logged {
            eprintln!(
                "{RED}Cannot read {} (VOX_ORCHESTRATOR_EVENT_LOG); fix path or permissions.{RESET}",
                path.display()
            );
            unreadable_logged = true;
        }
        render(&stats, tick);
    }
}

pub async fn run() -> Result<()> {
    let event_log_resolved =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOrchestratorEventLog);
    if let Some(raw) = event_log_resolved.expose() {
        let path = PathBuf::from(raw.trim());
        validate_event_log_path(&path)?;
        tracing::info!(
            path = %path.display(),
            "vox live tailing VOX_ORCHESTRATOR_EVENT_LOG (shared with vox-mcp)"
        );
        return run_event_log_tail(path).await;
    }

    let config = OrchestratorConfig::default();
    let orch = build_repo_scoped_orchestrator(config, None).orchestrator;
    let mut rx = orch.event_bus().subscribe();

    let mut stats = LiveStats::default();
    let mut tick: u64 = 0;
    render(&stats, tick);

    loop {
        tokio::select! {
            Ok(event) = rx.recv() => {
                merge_agent_event(&mut stats, &event);
                render(&stats, tick);
            }
            _ = sleep(vox_config::timeouts::D_250MS) => {
                tick = tick.wrapping_add(1);
                render(&stats, tick);
            }
        }
    }
}
