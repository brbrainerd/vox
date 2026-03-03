//! `vox status --live` — Real-time Matrix-style EventBus dashboard.
//!
//! Subscribes to orchestrator `AgentEvent`s and renders a live, updating
//! dashboard in the terminal. Press Ctrl+C to quit.

use anyhow::Result;
use tokio::time::{sleep, Duration};
use vox_orchestrator::events::AgentEventKind;
use vox_orchestrator::{Orchestrator, OrchestratorConfig};

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
    println!("{BOLD}{CYAN}  {spin} VOX LIVE DASHBOARD{RESET}  {DIM}— real-time event stream (Ctrl+C to exit){RESET}");
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

pub async fn run() -> Result<()> {
    let config = OrchestratorConfig::default();
    let orch = Orchestrator::new(config);
    let mut rx = orch.event_bus().subscribe();

    let mut stats = LiveStats::default();
    let mut tick: u64 = 0;
    render(&stats, tick);

    loop {
        tokio::select! {
            Ok(event) = rx.recv() => {
                match &event.kind {
                    AgentEventKind::TaskSubmitted { task_id, agent_id, description } => {
                        stats.tasks_submitted += 1;
                        let short: String = description.chars().take(36).collect();
                        stats.push_event(format!(
                            "{YELLOW}▶ submitted{RESET}  #{task_id}  agent={a}  {short}", a = agent_id.0
                        ));
                    }
                    AgentEventKind::TaskCompleted { task_id, agent_id } => {
                        stats.tasks_completed += 1;
                        stats.push_event(format!(
                            "{GREEN}✓ completed{RESET}  #{task_id}  agent={}", agent_id.0
                        ));
                    }
                    AgentEventKind::TaskFailed { task_id, agent_id, error } => {
                        stats.tasks_failed += 1;
                        let short: String = error.chars().take(38).collect();
                        stats.push_event(format!(
                            "{RED}✗ failed{RESET}     #{task_id}  agent={}  {short}", agent_id.0
                        ));
                    }
                    AgentEventKind::TokenStreamed { text, .. } => {
                        stats.tokens_total_chars += text.chars().count() as u64;
                    }
                    AgentEventKind::CostIncurred { provider, model, input_tokens, output_tokens, cost_usd, .. } => {
                        stats.cost_events += 1;
                        stats.total_cost_usd += cost_usd;
                        stats.push_event(format!(
                            "{MAGENTA}$ cost{RESET}       {provider}/{model}  {input_tokens}+{output_tokens}tok  ${cost_usd:.6}"
                        ));
                    }
                    AgentEventKind::SnapshotCaptured { agent_id, file_count, description, .. } => {
                        stats.snapshots_captured += 1;
                        let short: String = description.chars().take(34).collect();
                        stats.push_event(format!(
                            "{CYAN}📸 snapshot{RESET}  agent={}  files={file_count}  {short}", agent_id.0
                        ));
                    }
                    AgentEventKind::ConflictDetected { path, conflict_id, .. } => {
                        stats.conflicts_detected += 1;
                        stats.push_event(format!(
                            "{RED}⚡ conflict{RESET}  id={conflict_id}  {}", path.display()
                        ));
                    }
                    AgentEventKind::ConflictResolved { conflict_id, resolution_strategy } => {
                        stats.push_event(format!(
                            "{GREEN}✔ resolved{RESET}   id={conflict_id}  via={resolution_strategy}"
                        ));
                    }
                    AgentEventKind::UrgentRebalanceTriggered { moved } => {
                        stats.rebalances += 1;
                        stats.push_event(format!(
                            "{YELLOW}⇄ rebalance{RESET} moved={moved} tasks"
                        ));
                    }
                    AgentEventKind::OperationUndone { operation_id, .. } => {
                        stats.push_event(format!("{DIM}↩ undo{RESET}       op={operation_id}"));
                    }
                    AgentEventKind::OperationRedone { operation_id, .. } => {
                        stats.push_event(format!("{DIM}↪ redo{RESET}       op={operation_id}"));
                    }
                    AgentEventKind::AgentSpawned { agent_id, name } => {
                        stats.push_event(format!(
                            "{CYAN}+ spawn{RESET}      agent={}  name={name}", agent_id.0
                        ));
                    }
                    AgentEventKind::AgentRetired { agent_id } => {
                        stats.push_event(format!(
                            "{DIM}− retire{RESET}     agent={}", agent_id.0
                        ));
                    }
                    _ => {}
                }
                render(&stats, tick);
            }
            _ = sleep(Duration::from_millis(250)) => {
                tick = tick.wrapping_add(1);
                render(&stats, tick);
            }
        }
    }
}
