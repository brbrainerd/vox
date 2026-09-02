//! Tokio/`vox-actor-runtime` bridge: actor agents, task processors, and fleet scaling hooks.
//!
//! [`AgentFleet`](crate::runtime::AgentFleet) keeps [`ProcessHandle`](vox_actor_runtime::ProcessHandle) values aligned with [`Orchestrator`](crate::orchestrator::Orchestrator) registrations
//! and applies [`ScalingAction`](crate::services::ScalingAction) decisions from the scaling service.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use vox_actor_runtime::{
    ProcessHandle, RegistryError, mailbox::MessagePayload, process::ProcessContext,
    scheduler::Scheduler, supervisor::ChildSpec, supervisor::RestartStrategy,
    supervisor::Supervisor,
};

use crate::events::AgentEventKind;
use crate::models::{ModelRouteBackend, route_backend_for_model};
use crate::orchestrator::Orchestrator;
use crate::planning::prompts::SUPERPOWERS_PROMPT;
use crate::services::{ScalingAction, ScalingService};
use crate::types::AgentId;
use crate::types::TaskId;
use futures_util::StreamExt;
use std::time::Instant;

/// Compute the effective `(CostPreference, force_free_pool, RiskPosture)` for
/// one task — extracted from `AiTaskProcessor::process` so it's unit-testable
/// without a live orchestrator. `global_default` is
/// `OrchestratorConfig.cost_preference`, the fallback when no clutch policy
/// (explicit, category, or source) applies at all — preserving today's exact
/// behavior for a fully-unconfigured task. The returned `RiskPosture` is
/// always the full explicit>category>source>default resolution regardless of
/// whether any *clutch* policy applied, so callers get category/source risk
/// policy even for a task with no clutch override at all — `AgentTask::resolved_risk()`
/// only ever sees the explicit hint and must not be used for this instead.
fn resolve_task_cost_policy(
    task: &crate::types::AgentTask,
    overrides: &crate::config::TaskPolicyOverrides,
    global_default: crate::config::CostPreference,
) -> (
    crate::config::CostPreference,
    bool,
    crate::mode::RiskPosture,
) {
    let (category_clutch, category_risk) =
        crate::mode::effective_category_policy(overrides, task.task_category);
    let source = task
        .trigger_source
        .unwrap_or(crate::mode::TriggerSource::Interactive);
    let (source_clutch, source_risk) = crate::mode::effective_source_policy(overrides, source);
    let (clutch, risk) = crate::mode::resolve_task_policy(
        task.clutch_profile,
        task.risk_posture,
        category_clutch,
        category_risk,
        source_clutch,
        source_risk,
    );
    if task.clutch_profile.is_none() && category_clutch.is_none() && source_clutch.is_none() {
        return (global_default, false, risk);
    }
    let rc = clutch.resolve();
    (rc.cost_preference, rc.force_free_pool, risk)
}

/// Returns the first hyphen-delimited segment of `s`, or the first 8 bytes if
/// there are no hyphens.  Never panics.
fn short_id_from_str(s: &str) -> &str {
    if let Some(pos) = s.find('-') {
        &s[..pos]
    } else {
        &s[..s.len().min(8)]
    }
}

/// Parses an `@tool <name> [json args]` intent line (as emitted per the
/// `Action contract` in [`AiTaskProcessor::run_phase_stream_with_bus`]'s
/// prompt) into a tool name and a `serde_json::Value` args object.
///
/// - `<name>` is the first whitespace-delimited token after `@tool `.
/// - Everything after that, if present and it parses as a JSON object, is
///   used as-is for `args`. Any other trailing content (missing, not JSON, or
///   JSON that isn't an object) falls back to `{}` — the model is not
///   required to emit valid JSON args, and a parse failure must never panic
///   or block dispatch (the tool itself is responsible for rejecting
///   missing/invalid params).
///
/// `line` is expected already `str::trim`-med and to start with `"@tool "`
/// (callers filter for this); a defensive fallback still handles a bare
/// `"@tool"` with no trailing name.
fn parse_tool_intent_line(line: &str) -> (String, serde_json::Value) {
    let rest = line.strip_prefix("@tool").unwrap_or(line).trim_start();
    let rest = rest.trim();
    let (name, arg_str) = match rest.split_once(char::is_whitespace) {
        Some((n, a)) => (n.trim(), a.trim()),
        None => (rest, ""),
    };
    let args = if arg_str.is_empty() {
        serde_json::json!({})
    } else {
        match serde_json::from_str::<serde_json::Value>(arg_str) {
            Ok(v @ serde_json::Value::Object(_)) => v,
            _ => serde_json::json!({}),
        }
    };
    (name.to_string(), args)
}

/// RAII guard that removes a task's interrupt flag from the orchestrator's
/// `interrupt_flags` map when dropped — including on panic/unwind — so an
/// aborted or panicking task never leaves a stale flag behind (which would
/// otherwise let a later task reusing the same `TaskId` see a stale signal).
struct InterruptFlagGuard {
    flags: Arc<std::sync::RwLock<std::collections::HashMap<TaskId, Arc<AtomicBool>>>>,
    task_id: TaskId,
}

impl Drop for InterruptFlagGuard {
    fn drop(&mut self) {
        crate::sync_lock::rw_write(&self.flags).remove(&self.task_id);
    }
}

/// Message type sent to the ActorAgent to trigger task processing.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum AgentCommand {
    /// Drain the agent's queue once (used by supervisor ticks).
    ProcessQueue,
    /// Pause dequeueing new tasks.
    Pause,
    /// Resume after [`AgentCommand::Pause`].
    Resume,
    /// Remove a specific pending task id from the local queue.
    CancelTask(TaskId),
}

/// Pluggable executor invoked by [`ActorAgent`] for each dequeued [`AgentTask`](crate::types::AgentTask).
#[async_trait::async_trait]
pub trait TaskProcessor: Send + Sync {
    /// Runs `task` on behalf of `agent_id` and returns the finished task id on success.
    ///
    /// `cancel` is a per-task interrupt flag; implementations that loop or stream
    /// should poll it with [`AtomicBool::load`] and abort early when it is `true`.
    async fn process(
        &self,
        agent_id: crate::types::AgentId,
        task: crate::types::AgentTask,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<crate::types::TaskId>;
}

/// No-op processor for tests and dry runs: completes immediately without calling external AI.
pub struct StubTaskProcessor;

#[async_trait::async_trait]
impl TaskProcessor for StubTaskProcessor {
    async fn process(
        &self,
        _agent_id: crate::types::AgentId,
        task: crate::types::AgentTask,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<crate::types::TaskId> {
        // Honor a pre-set interrupt flag so the cancel path is testable even
        // without a real inference loop.
        if cancel.load(Ordering::Acquire) {
            return Err(anyhow::anyhow!("task interrupted"));
        }
        Ok(task.id)
    }
}

/// Out-of-band bridge for dispatching a tool call an *autonomous* agent
/// requested (via an `@tool` intent line in its own phase output) into the
/// real MCP dispatch gate (`handle_tool_call_with_mode` in
/// `vox-orchestrator-mcp`). Defined here (not in `vox-orchestrator-mcp`) for
/// the same reason as [`crate::orch_daemon::ExtraDispatch`]: `vox-orchestrator`
/// cannot depend on `vox-orchestrator-mcp` (dependency runs the other way), so
/// the heavy MCP layer supplies the impl and the library stays free of it.
///
/// T1.5 follow-up (harness reliability spec, 2026-07-02 /
/// `docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md`):
/// before this trait existed, `AiTaskProcessor::process` only logged a
/// detected `@tool` line as a tracing breadcrumb and never actually dispatched
/// it — see the (now historical) audit in
/// `crates/vox-orchestrator-queue/src/oplog/mod.rs`'s
/// `OperationKind::ApprovalRequested` doc comment. Wiring a
/// [`ToolDispatcher`] into [`AiTaskProcessor`] closes that gap.
#[async_trait::async_trait]
pub trait ToolDispatcher: Send + Sync {
    /// Dispatch `tool_name` with `args` on behalf of `task_id`/`agent_id`,
    /// through the same permission/approval gate every other tool-call caller
    /// (GUI, HTTP gateway, stdio MCP) goes through.
    ///
    /// `task_id` is an **explicit parameter**, not a field the caller writes
    /// into `args` — `args` is LLM-composed narration parsed out of the
    /// model's own phase output, and T0.1 (same spec) specifically killed a
    /// prior pattern where an `args`-controlled field could influence
    /// approval-gate behavior. The impl is responsible for threading
    /// `task_id` into the dispatch call by a path the LLM cannot spoof (e.g.
    /// as a genuine function parameter to `handle_tool_call_with_mode`'s
    /// caller, not by trusting an `args["task_id"]` the model might also try
    /// to set — see the impl in `vox-orchestrator-mcp` for how the two are
    /// kept separate).
    ///
    /// `permission_mode` mirrors `handle_tool_call_with_mode`'s parameter:
    /// `None` is the safe default (dangerous tools always park for human
    /// approval). Autonomous task execution has no authenticated operator
    /// session to read a mode from, so implementations should pass `None`
    /// unless a task explicitly carries a caller-supplied mode.
    async fn dispatch(
        &self,
        task_id: TaskId,
        agent_id: AgentId,
        tool_name: &str,
        args: serde_json::Value,
        permission_mode: Option<&str>,
    ) -> anyhow::Result<String>;
}

/// A real AI-powered task processor that streams tokens back to the event bus.
pub struct AiTaskProcessor {
    client: vox_gamify::ai::FreeAiClient,
    event_bus: crate::events::EventBus,
    orchestrator: Arc<Orchestrator>,
    /// Provider name stored at construction time (e.g. "ollama", "google").
    provider: String,
    /// Model identifier stored at construction time.
    model: String,
    /// Optional bridge into real MCP tool dispatch (T1.5 follow-up). `None`
    /// preserves the pre-existing breadcrumb-only behavior (e.g. in tests, or
    /// hosts that never wire an MCP `ServerState` in-process).
    tool_dispatcher: Option<Arc<dyn ToolDispatcher>>,
    /// Compacts the phase-loop `notes` scratchpad before it is resent as
    /// "Known notes" in each phase's prompt, so a long-running task's notes
    /// stop growing raw and unbounded (mirrors `session/state.rs`'s
    /// `compact_auto` use of the same engine over session history). A no-op
    /// below the engine's configured trigger threshold.
    compaction: crate::compaction::CompactionEngine,
}

// TaskPhase moved to types/tasks.rs

impl AiTaskProcessor {
    /// Create a new AI processor that auto-discovers providers. No tool
    /// dispatcher is wired — detected `@tool` lines are logged as breadcrumbs
    /// only. Use [`Self::with_tool_dispatcher`] to enable real dispatch.
    pub async fn new(event_bus: crate::events::EventBus, orchestrator: Arc<Orchestrator>) -> Self {
        let client = vox_gamify::ai::FreeAiClient::auto_discover().await;
        // Reflect the active provider in costs/logs
        let (provider, model) = client.active_provider_info();
        Self {
            client,
            event_bus,
            orchestrator,
            provider,
            model,
            tool_dispatcher: None,
            compaction: crate::compaction::CompactionEngine::new(scratchpad_compaction_config()),
        }
    }

    /// Same as [`Self::new`], but wires `dispatcher` so detected `@tool`
    /// intent lines are actually executed via [`ToolDispatcher::dispatch`]
    /// instead of only being logged.
    pub async fn with_tool_dispatcher(
        event_bus: crate::events::EventBus,
        orchestrator: Arc<Orchestrator>,
        dispatcher: Arc<dyn ToolDispatcher>,
    ) -> Self {
        let mut this = Self::new(event_bus, orchestrator).await;
        this.tool_dispatcher = Some(dispatcher);
        this
    }

    async fn run_phase_stream(
        &self,
        client: &vox_gamify::ai::FreeAiClient,
        agent_id: crate::types::AgentId,
        task: &crate::types::AgentTask,
        phase: crate::types::TaskPhase,
        usage_model: &str,
        prior_notes: &str,
        route: vox_gamify::StreamRoute<'_>,
        cancel: &Arc<AtomicBool>,
    ) -> anyhow::Result<String> {
        Self::run_phase_stream_with_bus(
            &self.event_bus,
            client,
            agent_id,
            task,
            phase,
            usage_model,
            prior_notes,
            route,
            cancel,
        )
        .await
    }

    /// Core of [`Self::run_phase_stream`], taking the event bus explicitly rather
    /// than through `&self` so it is testable without constructing a full
    /// [`Orchestrator`]. This is the single call chain that feeds `TokenStreamed`
    /// events from the consolidated `vox-gamify` streaming client (T4.1): every
    /// token delta yielded by `client.generate_stream_routed` is emitted here, and
    /// nowhere else in `vox-orchestrator` emits `TokenStreamed`.
    #[allow(clippy::too_many_arguments)]
    async fn run_phase_stream_with_bus(
        event_bus: &crate::events::EventBus,
        client: &vox_gamify::ai::FreeAiClient,
        agent_id: crate::types::AgentId,
        task: &crate::types::AgentTask,
        phase: crate::types::TaskPhase,
        usage_model: &str,
        prior_notes: &str,
        route: vox_gamify::StreamRoute<'_>,
        cancel: &Arc<AtomicBool>,
    ) -> anyhow::Result<String> {
        let mut history_block = String::new();
        if !task.transcript.is_empty() {
            history_block.push_str("### Prior Agent Turns (Context)\n");
            // Inject a max of 3 relevant history turns (Surgical Injection phase 1).
            for turn in task.transcript.iter().rev().take(3).rev() {
                history_block.push_str(&format!(
                    "[{}] (Agent: {}):\n{}\n\n",
                    turn.agent_id, turn.agent_name, turn.message
                ));
            }
        }

        let mut skill_block = String::new();
        if let Some(ref skill) = task.active_skill {
            skill_block.push_str("\n### Procedural Skill (Active Methodology)\n");
            skill_block.push_str(&format!("ACTIVE_SKILL: {}\n", skill));
            skill_block.push_str(SUPERPOWERS_PROMPT);
            skill_block.push_str("\n\n");
        }

        let prompt = format!(
            "Task: {}\n\n{}{}\nPhase: {}\nCategory: {:?}\nRouting model hint: {}\n\nKnown notes:\n{}\n\nAction contract:\n- Think step-by-step for this phase only.\n- If proposing tool usage, emit one line starting with `@tool` followed by a concrete tool name and, optionally, a single-line JSON object of arguments, e.g. `@tool vox_read_file {{\"path\": \"src/main.rs\"}}`. This line is actually executed (not just narrated) — the tool's real result is fed back into your next phase's notes, so only emit it when you intend the call to happen now. Omit the JSON object (or leave it invalid) to call the tool with no arguments.\n- Keep output concise and executable.",
            task.description,
            history_block,
            skill_block,
            phase.as_str(),
            task.task_category,
            usage_model,
            prior_notes
        );

        let mut stream = client.generate_stream_routed(&prompt, route).await;
        let mut phase_text = String::new();
        while let Some(chunk_result) = stream.next().await {
            // Poll the interrupt flag after every chunk so a user interrupt
            // stops streaming promptly.
            if cancel.load(Ordering::Acquire) {
                return Err(anyhow::anyhow!("task interrupted"));
            }
            match chunk_result {
                Ok(text) => {
                    phase_text.push_str(&text);
                    event_bus.emit(AgentEventKind::TokenStreamed {
                        agent_id,
                        text,
                        // Background AiTaskProcessor streaming has no chat
                        // session concept — see Task G1's doc comment on
                        // `AgentEventKind::TokenStreamed`.
                        session_id: None,
                    });
                }
                Err(e) => tracing::error!("AI stream error [{}]: {}", phase.as_str(), e),
            }
        }
        Ok(phase_text)
    }
}

/// [`crate::compaction::CompactionConfig`] scoped for the phase-loop `notes`
/// scratchpad (used to build [`AiTaskProcessor`]'s `compaction` engine).
///
/// `CompactionConfig::default()` (128K tokens / 0.80 threshold ⇒ trigger at
/// ~102,400 tokens) is calibrated for a full model context window, not this
/// internal scratchpad. A single task runs at most 6 phases plus a handful
/// of tool-result blocks; realistic accumulated `notes` size is in the low
/// thousands to tens of thousands of tokens (each `phase_out` is one LLM
/// response for a single narrow phase prompt, not a whole conversation), so
/// reusing the 128K default meant `compact_notes` almost never actually
/// compacted anything in practice — a silent no-op for the common case it
/// was meant to address (Task C1 review finding #2). Scope the budget to
/// the scratchpad itself: 12K tokens (roughly two-to-three long phase
/// outputs) with an 80% trigger fires compaction around ~9,600 tokens, and
/// `tail_preserve_tokens` is sized to keep the most recent phase's full
/// output intact rather than the 8K default tuned for a much larger window.
/// Shared by production construction and the test below so the two can
/// never drift apart.
fn scratchpad_compaction_config() -> crate::compaction::CompactionConfig {
    crate::compaction::CompactionConfig {
        max_context_tokens: 12_000,
        reserved_tokens: 1_000,
        compaction_threshold: 0.80,
        min_viable_tokens: 1_000,
        strategy: crate::compaction::CompactionStrategy::Balanced,
        head_preserve_tokens: 1_000,
        tail_preserve_tokens: 4_000,
        complexity_token_weight: 32,
    }
}

/// Compact the phase-loop `notes` scratchpad through `engine` before it's
/// resent as "Known notes" in the next phase's prompt.
///
/// `blocks` is the phase loop's own structured record of what went into
/// `notes` — one entry per `"[{phase}]\n{phase_out}"` block or
/// `"[tool_result: ...]\n..."` block, in the order they were appended.
/// **Do not** reconstruct this by delimiter-splitting a pre-joined `notes`
/// string on `"\n\n"`: `phase_out` text (an LLM response) and tool-result
/// JSON can themselves contain internal `"\n\n"`, so string-splitting can
/// fragment a single phase's output into several pseudo-`Turn`s. The
/// per-`Turn` trim logic in `compaction.rs` can then keep some fragments
/// and drop others, splicing together a truncated fragment of one phase
/// next to content from a different phase with no marker showing where the
/// cut happened (Task C1 review finding #1). Building `Turn`s directly from
/// the caller's own `(block)` list — one real `Turn` per actual phase/tool
/// entry — makes that impossible: a block is kept or dropped whole.
///
/// Role is `"assistant"` for all entries (all agent output; `compaction.rs`'s
/// trim strategies only special-case role `"system"`, never `"user"`, so no
/// "user turns protected from trimming" concern applies here). A no-op below
/// the engine's configured trigger threshold, mirroring
/// `session/state.rs::compact_auto`'s reassignment pattern (retained turns'
/// content rejoined back into the working representation) rather than
/// merely computing a value that's never applied.
///
/// Phase notes are an internal scratchpad the task discards on completion
/// (unlike session transcripts, which are user-facing history), so unlike
/// `Session::compact_auto` this deliberately does NOT durably archive
/// `dropped_turns` — losing them here is acceptable.
///
/// Known limitation (not fixed here): a single very large phase output can
/// itself exceed `CompactionConfig::tail_preserve_tokens` and get trimmed
/// even though it's the most recent, most relevant turn. Not a regression —
/// today's uncompacted behavior has no protection at all — but worth tuning
/// later.
fn compact_notes(engine: &crate::compaction::CompactionEngine, blocks: &[String]) -> Vec<String> {
    if blocks.is_empty() {
        return Vec::new();
    }
    let history_turns: Vec<crate::compaction::Turn> = blocks
        .iter()
        .map(|block| crate::compaction::Turn::new("assistant", block.as_str()))
        .collect();
    match engine.compact(&history_turns) {
        Ok(result) if result.compacted => result
            .retained_turns
            .into_iter()
            .map(|t| t.content)
            .collect(),
        _ => blocks.to_vec(),
    }
}

#[cfg(test)]
mod compact_notes_tests {
    use super::{compact_notes, scratchpad_compaction_config};
    use crate::compaction::{CompactionConfig, CompactionEngine, CompactionStrategy};

    /// Red-then-green proof for Task C1: with a small trigger threshold, a
    /// `notes` scratchpad that's grown past it must come back shorter than
    /// the raw concatenation would be. Before `compact_notes` was wired into
    /// the phase loop, `notes` was resent raw and unbounded every phase.
    #[test]
    fn compact_notes_shrinks_once_over_trigger() {
        let config = CompactionConfig {
            max_context_tokens: 200,
            reserved_tokens: 0,
            compaction_threshold: 0.5, // trigger_at() == 100 tokens
            min_viable_tokens: 10,
            strategy: CompactionStrategy::Balanced,
            head_preserve_tokens: 5,
            tail_preserve_tokens: 20,
            complexity_token_weight: 32,
        };
        let engine = CompactionEngine::new(config);

        let blocks: Vec<String> = (0..50)
            .map(|i| {
                format!(
                    "[phase{i}]\nsome moderately long phase output text accumulating tokens quickly across many phases"
                )
            })
            .collect();
        let raw_len: usize = blocks.iter().map(|b| b.len()).sum();

        let compacted = compact_notes(&engine, &blocks);
        let compacted_len: usize = compacted.iter().map(|b| b.len()).sum();

        assert!(
            compacted_len < raw_len,
            "compact_notes must shrink notes once past the configured trigger threshold \
             (raw len={raw_len}, compacted len={compacted_len})"
        );
    }

    /// Below the trigger threshold, compaction must be a complete no-op —
    /// short conversations/early phases are unaffected.
    #[test]
    fn compact_notes_is_noop_below_trigger() {
        let engine = CompactionEngine::new(CompactionConfig::default());
        let blocks = vec!["[inspect]\nshort note".to_string()];
        let compacted = compact_notes(&engine, &blocks);
        assert_eq!(compacted, blocks);
    }

    #[test]
    fn compact_notes_handles_empty() {
        let engine = CompactionEngine::new(CompactionConfig::default());
        assert_eq!(compact_notes(&engine, &[]), Vec::<String>::new());
    }

    /// Red-then-green proof for Task C1 finding #1: phase output (or
    /// tool-result JSON) can itself contain internal `"\n\n"` (e.g. a
    /// multi-paragraph LLM response). `compact_notes` must build one `Turn`
    /// per *caller-supplied block*, never by delimiter-splitting a
    /// pre-joined string — a block is kept or dropped whole, so the
    /// compacted output can never contain more blocks than went in, and any
    /// surviving `[act]` block must be the complete original text, never a
    /// spliced-together fragment.
    #[test]
    fn compact_notes_does_not_fragment_blocks_with_internal_blank_lines() {
        let config = CompactionConfig {
            max_context_tokens: 200,
            reserved_tokens: 0,
            compaction_threshold: 0.3, // trigger_at() == 60 tokens
            min_viable_tokens: 5,
            strategy: CompactionStrategy::Balanced,
            head_preserve_tokens: 5,
            tail_preserve_tokens: 20,
            complexity_token_weight: 32,
        };
        let engine = CompactionEngine::new(config);

        let multi_paragraph_phase_out = "Paragraph one of a single phase's response.\n\n\
             Paragraph two continues the very same phase's output, not a new phase.\n\n\
             Paragraph three finishes the same phase's output off with more detail.";
        let blocks = vec![
            "[inspect]\nfirst phase note".to_string(),
            format!("[act]\n{multi_paragraph_phase_out}"),
            "[verify]\nfinal short note".to_string(),
        ];

        let compacted = compact_notes(&engine, &blocks);

        // A delimiter-split implementation would fragment the multi-paragraph
        // [act] block into 3 extra pseudo-Turns (4 total from that one
        // block), any subset of which could then be independently kept or
        // dropped by the per-Turn trim logic — silently splicing a partial
        // fragment of [act] next to [inspect]/[verify] content. Building
        // Turns from the real block list forbids this: at most `blocks.len()`
        // Turns can ever exist.
        assert!(
            compacted.len() <= blocks.len(),
            "compacted block count ({}) must never exceed original block count ({}) — \
             fragmentation would let per-fragment trimming splice partial phases together",
            compacted.len(),
            blocks.len()
        );
        for b in &compacted {
            if b.starts_with("[act]") {
                assert_eq!(
                    b,
                    &format!("[act]\n{multi_paragraph_phase_out}"),
                    "a surviving [act] block must be the whole phase output, not a fragment"
                );
            }
        }
    }

    /// Red-then-green proof for Task C1 finding #2: the scratchpad
    /// compaction config must actually be reachable by a realistic
    /// multi-phase task, not just by artificially tiny thresholds rigged
    /// only for the other tests in this module.
    #[test]
    fn scratchpad_config_trigger_is_far_below_full_context_default() {
        let scratchpad_trigger = scratchpad_compaction_config().trigger_at();
        let full_context_trigger = CompactionConfig::default().trigger_at();
        assert!(
            scratchpad_trigger * 5 < full_context_trigger,
            "scratchpad trigger ({scratchpad_trigger}) must be meaningfully smaller than \
             the full-context default ({full_context_trigger}), or compact_notes remains a \
             no-op for realistic per-task notes sizes"
        );
    }

    /// Simulates a realistic multi-phase task: 6 phases plus 2 tool-result
    /// blocks, each a plausible several-paragraph LLM response. Under the
    /// old `CompactionConfig::default()` (trigger ~102,400 tokens) this
    /// accumulated notes size never triggers compaction — the C1 fix was a
    /// no-op for exactly the case it was meant to address. Under the new
    /// scratchpad-scoped config it must actually compact.
    #[test]
    fn realistic_multi_phase_notes_trigger_compaction_under_scratchpad_config_but_not_default() {
        // A plausible several-paragraph LLM response, repeated to a
        // realistic full-phase-output length (LLM phase outputs routinely
        // run several thousand tokens for Act/Verify-style phases).
        let paragraph = "Investigated the reported failure by reading the relevant \
            module and tracing the call path through the request handler.\n\n\
            Found that the validation step silently swallows a specific error case, \
            which explains the intermittent symptom reported by the user.\n\n\
            Proposing a fix that surfaces the error explicitly and adds a regression \
            test covering the previously-swallowed case, plus a short note on why the \
            original code path was structured that way.\n\n";
        let long_phase_out = paragraph.repeat(20);
        let phases = [
            "inspect",
            "localize",
            "hypothesize",
            "act",
            "verify",
            "decide",
        ];
        let mut blocks: Vec<String> = phases
            .iter()
            .map(|p| format!("[{p}]\n{long_phase_out}"))
            .collect();
        blocks.push(format!("[tool_result: read_file]\n{long_phase_out}"));
        blocks.push(format!("[tool_result: run_tests]\n{long_phase_out}"));

        let total_tokens: usize = blocks
            .iter()
            .map(|b| CompactionEngine::estimate_tokens(b))
            .sum();

        let scratchpad_engine = CompactionEngine::new(scratchpad_compaction_config());
        let default_engine = CompactionEngine::new(CompactionConfig::default());

        assert!(
            total_tokens > scratchpad_compaction_config().trigger_at(),
            "test fixture must realistically exceed the scratchpad trigger \
             (total_tokens={total_tokens}, scratchpad trigger={})",
            scratchpad_compaction_config().trigger_at()
        );
        assert!(
            total_tokens < CompactionConfig::default().trigger_at(),
            "test fixture must stay well under the old full-context default trigger, \
             proving the old default was a no-op here \
             (total_tokens={total_tokens}, default trigger={})",
            CompactionConfig::default().trigger_at()
        );

        let scratchpad_compacted = compact_notes(&scratchpad_engine, &blocks);
        let default_compacted = compact_notes(&default_engine, &blocks);

        let scratchpad_len: usize = scratchpad_compacted.iter().map(|b| b.len()).sum();
        let default_len: usize = default_compacted.iter().map(|b| b.len()).sum();
        let raw_len: usize = blocks.iter().map(|b| b.len()).sum();

        assert!(
            scratchpad_len < raw_len,
            "scratchpad-scoped config must actually compact a realistic multi-phase notes size \
             (raw_len={raw_len}, scratchpad_len={scratchpad_len})"
        );
        assert_eq!(
            default_len, raw_len,
            "sanity check: the old full-context default really was a no-op at this realistic size \
             (default_len={default_len}, raw_len={raw_len})"
        );
    }
}

#[async_trait::async_trait]
impl TaskProcessor for AiTaskProcessor {
    async fn process(
        &self,
        agent_id: crate::types::AgentId,
        task: crate::types::AgentTask,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<crate::types::TaskId> {
        // Drive Console: a task's clutch (explicit hint, or category/source policy)
        // overrides the global cost preference and can force the free-only model
        // pool; a Low-risk posture (model_lean=Intelligence) nudges selection toward
        // Performance. No policy anywhere ⇒ unchanged global behavior.
        let (overrides, global_default) = {
            let cfg = crate::sync_lock::rw_read(&*self.orchestrator.config);
            (cfg.task_policy.clone(), cfg.cost_preference)
        };
        let (mut cost_pref, force_free_pool, resolved_risk) =
            resolve_task_cost_policy(&task, &overrides, global_default);
        if matches!(
            resolved_risk.resolve().model_lean,
            crate::mode::ModelLean::Intelligence
        ) {
            cost_pref = crate::config::CostPreference::Performance;
        }
        let mut allowed_providers = std::collections::HashSet::new();
        if let Some(db) = self.orchestrator.db() {
            let tracker = crate::usage::UsageTracker::new_ref(&db);
            if let Ok(budgets) = tracker.remaining_all().await {
                for b in budgets {
                    if b.remaining > 0 && !b.rate_limited {
                        allowed_providers.insert(b.provider.clone());
                    }
                }
            }
        }

        let models_handle = self.orchestrator.models_handle();
        let routed = {
            let registry = crate::sync_lock::rw_read(&*models_handle);
            let exploration_spent = crate::sync_lock::rw_read(&*self.orchestrator.budget_manager)
                .global_exploration_cost_usd();
            let exploration_limit = vox_config::load_model_routing_config()
                .exploration
                .budget_usd_per_day;

            if allowed_providers.is_empty() {
                registry.best_for_task_with_filter(&task, cost_pref, |m| {
                    if m.pricing_source == crate::models::spec::PricingSource::Unknown
                        && exploration_spent >= exploration_limit
                    {
                        return false;
                    }
                    if force_free_pool && !m.is_free {
                        return false;
                    }
                    true
                })
            } else {
                registry.best_for_task_with_filter(&task, cost_pref, |m| {
                    if m.pricing_source == crate::models::spec::PricingSource::Unknown
                        && exploration_spent >= exploration_limit
                    {
                        return false;
                    }
                    if force_free_pool && !m.is_free {
                        return false;
                    }
                    let provider_str = match m.provider_type {
                        crate::models::ProviderType::OpenRouter => "openrouter",
                        crate::models::ProviderType::Ollama => "ollama",
                        crate::models::ProviderType::GoogleDirect => "google",
                        crate::models::ProviderType::Groq => "groq",
                        crate::models::ProviderType::Cerebras => "cerebras",
                        crate::models::ProviderType::Mistral => "mistral",
                        crate::models::ProviderType::DeepSeek => "deepseek",
                        crate::models::ProviderType::SambaNova => "sambanova",
                        crate::models::ProviderType::Anthropic => "anthropic",
                        crate::models::ProviderType::PopuliMesh => "populimesh",
                        crate::models::ProviderType::HuggingFaceRouter => "huggingface",
                        crate::models::ProviderType::Custom(_) => "custom",
                        crate::models::ProviderType::VoxLocal => "vox_local",
                    };
                    allowed_providers.contains(provider_str)
                })
            }
        };
        // Code-review fix: `routed == None` (no eligible model in the registry,
        // e.g. no local model registered under local_only privacy) used to fall
        // straight through to `StreamRoute::Cascade` below regardless of
        // privacy mode. `Cascade` streams from `self.client`'s provider list
        // (`FreeAiClient::auto_discover()`, built once at construction with no
        // privacy awareness — it always includes a cloud provider), which is
        // exactly the cloud egress `VOX_INFERENCE_PRIVACY=local_only` exists to
        // prevent. Fail closed instead of silently leaking to cloud: if there's
        // no explicit model_override to honor either, this is an unroutable
        // request under the current privacy mode.
        if routed.is_none()
            && task
                .model_override
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .is_none()
            && crate::route_policy::inference_privacy_local_only_from_env()
        {
            anyhow::bail!(
                "no local model available for task {} under VOX_INFERENCE_PRIVACY=local_only; \
                 refusing to fall back to a cloud-inclusive Cascade route",
                task.id.0
            );
        }

        let (usage_provider, usage_model) = if let Some(ref mo) = task.model_override {
            ("task_override".to_string(), mo.clone())
        } else if let Some(m) = routed.as_ref() {
            (m.provider.clone(), m.id.clone())
        } else {
            (self.provider.clone(), self.model.clone())
        };

        let route = if let Some(mo) = task
            .model_override
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            vox_gamify::StreamRoute::UserModelOverride(mo)
        } else if let Some(m) = routed.as_ref() {
            match route_backend_for_model(m) {
                ModelRouteBackend::Ollama => vox_gamify::StreamRoute::Registry {
                    backend: vox_gamify::LudusStreamBackend::Ollama,
                    model: m.id.as_str(),
                },
                ModelRouteBackend::GeminiDirect => vox_gamify::StreamRoute::Registry {
                    backend: vox_gamify::LudusStreamBackend::Gemini,
                    model: m.id.as_str(),
                },
                ModelRouteBackend::OpenRouter => vox_gamify::StreamRoute::Registry {
                    backend: vox_gamify::LudusStreamBackend::OpenRouter,
                    model: m.id.as_str(),
                },
                ModelRouteBackend::CascadeFallback => vox_gamify::StreamRoute::Cascade,
                ModelRouteBackend::PopuliMesh => vox_gamify::StreamRoute::Cascade,
                ModelRouteBackend::VoxLocal => vox_gamify::StreamRoute::Cascade,
            }
        } else {
            vox_gamify::StreamRoute::Cascade
        };

        if let Some(db) = self.orchestrator.db() {
            let repo = crate::lineage::repository_id();
            let has_model_override = task
                .model_override
                .as_deref()
                .map(str::trim)
                .is_some_and(|s| !s.is_empty());
            let ludus_fallback = !has_model_override && routed.is_none();
            let reason = vox_actor_runtime::routing_telemetry::OrchestratorTaskRoutingReasonV1::new(
                format!("{:?}", task.task_category),
                task.estimated_complexity,
                usage_provider.clone(),
                usage_model.clone(),
                routed.is_some(),
                format!("{:?}", cost_pref),
                ludus_fallback,
                vox_actor_runtime::routing_telemetry::unified_routing_rollout_enabled(),
                vox_actor_runtime::route_capability_policy::RouteCapabilityPolicySnapshot::from_env()
                    .profile
                    .clone(),
                Vec::new(),
                task.id.0,
            );
            let reason_s = reason.to_json_bounded(
                vox_actor_runtime::routing_telemetry::ROUTING_REASON_JSON_MAX_BYTES,
            );
            if let Err(e) = db
                .record_routing_decision(
                    None::<&str>,
                    repo.as_str(),
                    task.session_id.as_deref(),
                    "orchestrator_ai_task",
                    Some(usage_model.as_str()),
                    Some(reason_s.as_str()),
                )
                .await
            {
                tracing::debug!(error = %e, "record_routing_decision (orchestrator_ai_task) skipped");
            }
        }

        let reconciled_cost = Arc::new(Mutex::new(0.0));
        let client = {
            let reconciled_cost = reconciled_cost.clone();
            self.client
                .clone()
                .with_cost_reporter(Arc::new(move |cost| {
                    if let Ok(mut lock) = reconciled_cost.lock() {
                        *lock += cost;
                    }
                }))
        };

        // Capture attribution for the SelectedModelRecord that the completion handler
        // copies onto CompletionAttestation (so the GUI ModelBadge can show the real
        // model/provider instead of "model unknown"). Derived from the actual local
        // routing decision above — not a re-run of the scorer.
        let selected_model_id = usage_model.clone();
        let selected_provider = routed
            .as_ref()
            .map(|m| m.provider.clone())
            .unwrap_or_else(|| usage_provider.clone());
        let selection_reason = if task
            .model_override
            .as_deref()
            .map(str::trim)
            .is_some_and(|s| !s.is_empty())
        {
            "override".to_string()
        } else if routed.is_some() {
            "scored".to_string()
        } else {
            "fallback".to_string()
        };
        let dispatch_start = std::time::Instant::now();

        // Structured record of what's gone into the scratchpad so far — one
        // entry per phase output / tool-result block, in append order. This
        // (not a pre-joined `notes` string) is what `compact_notes` builds
        // its `Turn`s from, so a block containing internal `"\n\n"` can
        // never get silently fragmented into multiple pseudo-Turns (Task C1
        // review finding #1). `notes` (the joined string form the prompt
        // actually wants) is recomputed from `notes_blocks` each iteration.
        let mut notes_blocks: Vec<String> = Vec::new();
        let phases = [
            crate::types::TaskPhase::Inspect,
            crate::types::TaskPhase::Localize,
            crate::types::TaskPhase::Hypothesize,
            crate::types::TaskPhase::Act,
            crate::types::TaskPhase::Verify,
            crate::types::TaskPhase::Decide,
        ];
        // Keep execution bounded: no infinite self-reflection or uncontrolled loops.
        for phase in phases {
            // Poll the interrupt flag at the top of every phase so a user
            // interrupt aborts before kicking off the next inference round.
            if cancel.load(Ordering::Acquire) {
                self.orchestrator.abort_interrupted_task(task.id, agent_id);
                return Err(anyhow::anyhow!("task interrupted"));
            }
            // Update the task's current phase in the orchestrator state for observability.
            if let Some(queue_lock) = self.orchestrator.agent_queue(agent_id) {
                let mut queue = crate::sync_lock::rw_write(&*queue_lock);
                if let Some(t) = queue.find_task_mut(task.id) {
                    t.current_phase = Some(phase);
                } else if let Some(t) = queue.current_task_mut() {
                    if t.id == task.id {
                        t.current_phase = Some(phase);
                    }
                }
            }
            self.orchestrator
                .record_workflow_phase_change(task.id, phase)
                .await;
            self.event_bus.emit(AgentEventKind::TaskPhaseChanged {
                task_id: task.id,
                agent_id,
                phase,
            });

            // Compact the accumulated scratchpad's blocks before rejoining
            // them as "Known notes" in this phase's prompt (Task C1) - a
            // no-op below the engine's configured trigger threshold. Operates
            // on the real per-phase block list, not a delimiter-split string.
            notes_blocks = compact_notes(&self.compaction, &notes_blocks);
            let notes = notes_blocks.join("\n\n");

            let phase_out = match self
                .run_phase_stream(
                    &client,
                    agent_id,
                    &task,
                    phase,
                    usage_model.as_str(),
                    notes.as_str(),
                    route,
                    &cancel,
                )
                .await
            {
                Ok(text) => text,
                Err(e) => {
                    // Interrupt raised mid-stream: release locks and propagate.
                    self.orchestrator.abort_interrupted_task(task.id, agent_id);
                    return Err(e);
                }
            };

            // Drift detection (Doom-loop protection)
            let drift_decision = self.orchestrator.record_agent_iteration(
                agent_id,
                &phase_out,
                phase_out.contains("@tool"),
            );
            match drift_decision {
                crate::budget::DriftDecision::HaltAgent { reason } => {
                    tracing::error!(agent_id = agent_id.0, %reason, "halted agent due to semantic drift");
                    self.event_bus.emit(AgentEventKind::DoubtReported {
                        agent_id,
                        task_id: task.id,
                        reason: reason.clone(),
                    });
                    self.orchestrator.abort_interrupted_task(task.id, agent_id);
                    return Err(anyhow::anyhow!("Safety Halt: {}", reason));
                }
                crate::budget::DriftDecision::WarnUser {
                    iterations,
                    cost_usd,
                } => {
                    tracing::warn!(
                        agent_id = agent_id.0,
                        iterations,
                        cost_usd,
                        "agent showing early signs of semantic drift"
                    );
                }
                crate::budget::DriftDecision::Continue => {}
            }
            notes_blocks.push(format!("[{}]\n{}", phase.as_str(), phase_out));
            // Tool intent detection: an `@tool <name> [json args]` line in the
            // model's own phase output. Always logged as a breadcrumb; when a
            // `ToolDispatcher` is wired (T1.5 follow-up), also actually
            // dispatched through the real MCP gate so autonomous dangerous-tool
            // calls go through the same approval path as GUI-invoked ones, and
            // the tool's result is fed back into `notes` for the next phase.
            if let Some(tool_line) = phase_out
                .lines()
                .map(str::trim)
                .find(|line| line.starts_with("@tool "))
            {
                tracing::info!(
                    agent_id = agent_id.0,
                    task_id = task.id.0,
                    phase = phase.as_str(),
                    tool_intent = %tool_line,
                    "bounded executor emitted tool intent"
                );
                if let Some(dispatcher) = self.tool_dispatcher.as_ref() {
                    let (tool_name, tool_args) = parse_tool_intent_line(tool_line);
                    let dispatch_result = dispatcher
                        .dispatch(
                            task.id,
                            agent_id,
                            tool_name.as_str(),
                            tool_args,
                            None, // T0.3: autonomous execution has no operator-selected mode; safe default is `Ask`.
                        )
                        .await;
                    let ok = dispatch_result.is_ok();
                    self.event_bus.emit(AgentEventKind::ToolCallDispatched {
                        task_id: task.id,
                        agent_id,
                        tool_name: tool_name.clone(),
                        ok,
                    });
                    let result_block = match dispatch_result {
                        Ok(json) => format!("[tool_result: {tool_name}]\n{json}"),
                        Err(e) => {
                            tracing::warn!(
                                agent_id = agent_id.0,
                                task_id = task.id.0,
                                tool = %tool_name,
                                error = %e,
                                "autonomous tool dispatch failed"
                            );
                            format!("[tool_result: {tool_name}]\nERROR: {e}")
                        }
                    };
                    notes_blocks.push(result_block);
                }
            }
        }
        let full_text = notes_blocks.join("\n\n");
        let latency_ms = dispatch_start.elapsed().as_millis() as u64;

        let input_tokens =
            crate::compaction::CompactionEngine::estimate_tokens(&task.description) as u32;
        let output_tokens = crate::compaction::CompactionEngine::estimate_tokens(&full_text) as u32;

        let cost_usd = if let Some(m) = routed.as_ref() {
            let input_cost = (input_tokens as f64 / 1000.0) * m.cost_per_1k_input;
            let output_cost = (output_tokens as f64 / 1000.0) * m.cost_per_1k_output;
            input_cost + output_cost
        } else {
            (input_tokens + output_tokens) as f64 * 0.000_001
        };

        // Record usage through the unified pipeline (event bus + budget + oplog)
        self.orchestrator
            .record_ai_usage(
                agent_id,
                usage_provider.as_str(),
                usage_model.as_str(),
                input_tokens,
                output_tokens,
                cost_usd,
                reconciled_cost
                    .lock()
                    .ok()
                    .and_then(|lock| if *lock > 0.0 { Some(*lock) } else { None }),
                routed.as_ref().map(|m| m.pricing_source.clone()),
            )
            .await;

        // Record the final condensed summary back into the task's transcript for future handoffs.
        let agent_name = {
            if let Some(queue_lock) = self.orchestrator.agent_queue(agent_id) {
                crate::sync_lock::rw_read(&*queue_lock).name.clone()
            } else {
                "unknown".to_string()
            }
        };

        // We update the task transcript in the orchestrator's state so it persists
        // across handoffs if the task is re-queued or migrated.
        let selected_model_record = crate::types::SelectedModelRecord {
            model_id: selected_model_id,
            provider: selected_provider,
            selection_reason,
            request_tokens: Some(input_tokens as u64),
            latency_ms: Some(latency_ms),
        };
        let turn_opt = {
            if let Some(queue_lock) = self.orchestrator.agent_queue(agent_id) {
                let mut queue = crate::sync_lock::rw_write(&*queue_lock);
                if let Some(t) = queue.find_task_mut(task.id) {
                    t.selected_model_record = Some(selected_model_record.clone());
                    t.append_turn(agent_id, agent_name.clone(), full_text.clone());
                    t.transcript.last().cloned()
                } else if let Some(t) = queue.current_task_mut() {
                    if t.id == task.id {
                        t.selected_model_record = Some(selected_model_record.clone());
                        t.append_turn(agent_id, agent_name, full_text.clone());
                        t.transcript.last().cloned()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(turn) = turn_opt {
            self.orchestrator.record_workflow_turn(task.id, &turn).await;
        }

        Ok(task.id)
    }
}

/// Actor process wrapping an `AgentQueue`.
///
/// Converts a reactive orchestrator queue into an active background worker
/// using `vox-actor-runtime` actor primitives.
pub struct ActorAgent {
    /// Agent id managed by this process.
    pub agent_id: AgentId,
    /// Human-readable process/agent name.
    pub name: String,
}

impl ActorAgent {
    /// Spawn an active agent process from an `AgentQueue`.
    pub fn spawn(
        scheduler: &Scheduler,
        agent_id: AgentId,
        name: String,
        orchestrator: Arc<Orchestrator>,
        processor: Arc<dyn TaskProcessor>,
    ) -> Result<ProcessHandle, RegistryError> {
        let process_name = format!("agent-{}", name);

        scheduler.spawn_named(&process_name, move |mut ctx: ProcessContext| async move {
            tracing::info!("Agent {} ({}) process started", agent_id, name);

            loop {
                // Wait for commands
                let msg = ctx.receive().await;
                if let Some(envelope) = msg {
                    if let vox_actor_runtime::mailbox::Envelope::Message(msg) = envelope {
                        if let MessagePayload::Json(json_data) = msg.payload {
                            if let Ok(cmd) = serde_json::from_slice::<AgentCommand>(&json_data) {
                                Self::handle_command(cmd, agent_id, &orchestrator, &processor)
                                    .await;
                            }
                        }
                    }
                } else {
                    // Channel closed
                    break;
                }
            }
            tracing::info!("Agent {} ({}) process shutting down", agent_id, name);
        })
    }

    /// Handle a command sent to this agent process.
    async fn handle_command(
        cmd: AgentCommand,
        agent_id: AgentId,
        orchestrator_ref: &Arc<Orchestrator>,
        processor: &Arc<dyn TaskProcessor>,
    ) {
        match cmd {
            AgentCommand::ProcessQueue => {
                let dequeued = if let Some(queue_lock) = orchestrator_ref.agent_queue(agent_id) {
                    // Scope the write guard tightly: `dequeue`/`is_paused` need
                    // the write lock, but `heartbeat` below takes a *read* lock
                    // on this exact same per-agent `Arc<RwLock<AgentQueue>>`
                    // (see `Orchestrator::heartbeat` in
                    // `orchestrator/agent/lifecycle_ops.rs`, which resolves
                    // `agents.get(&agent_id)` to the identical Arc returned by
                    // `agent_queue`). `std::sync::RwLock` is not reentrant, so
                    // calling `heartbeat` while still holding this write guard
                    // deadlocks permanently. The guard must be dropped before
                    // `heartbeat` runs.
                    let (paused, t) = {
                        let mut queue = crate::sync_lock::rw_write(&queue_lock);
                        if !queue.is_paused() {
                            (false, queue.dequeue())
                        } else {
                            (true, None)
                        }
                    };
                    if !paused {
                        if t.is_some() {
                            orchestrator_ref
                                .heartbeat(agent_id, crate::events::AgentActivity::Thinking);
                        } else {
                            orchestrator_ref
                                .heartbeat(agent_id, crate::events::AgentActivity::Idle);
                        }
                    }
                    t
                } else {
                    None
                };

                let task_to_run = {
                    if let Some(ref task) = dequeued {
                        orchestrator_ref
                            .event_bus()
                            .emit(AgentEventKind::TaskStarted {
                                task_id: task.id,
                                agent_id,
                                session_id: task.session_id.clone(),
                            });
                    }
                    dequeued
                };

                if let Some(task) = task_to_run {
                    let task_id = task.id;
                    tracing::info!("Agent {} processing task {}", agent_id, task_id);

                    // Register a per-task interrupt flag so `interrupt_task` can
                    // signal this in-progress task to abort, then clean it up.
                    let cancel = Arc::new(AtomicBool::new(false));
                    crate::sync_lock::rw_write(&orchestrator_ref.interrupt_flags)
                        .insert(task_id, Arc::clone(&cancel));
                    // Guard removes the flag on EVERY exit path — normal return,
                    // early `?`, and panic/unwind — preventing a stale-flag leak.
                    let _flag_guard = InterruptFlagGuard {
                        flags: Arc::clone(&orchestrator_ref.interrupt_flags),
                        task_id,
                    };

                    let result = processor.process(agent_id, task, Arc::clone(&cancel)).await;

                    match result {
                        Ok(completed_id) => {
                            if let Err(err) = orchestrator_ref.complete_task(completed_id).await {
                                tracing::error!(
                                    "complete_task failed after processor success: {} (task {})",
                                    err,
                                    completed_id
                                );
                            }
                            orchestrator_ref
                                .heartbeat(agent_id, crate::events::AgentActivity::Idle);
                        }
                        Err(e) => {
                            tracing::error!("Agent {} failed task {}: {}", agent_id, task_id, e);
                            if let Err(err) =
                                orchestrator_ref.fail_task(task_id, e.to_string()).await
                            {
                                tracing::error!(
                                    "fail_task after processor error: {} (task {})",
                                    err,
                                    task_id
                                );
                            }
                            orchestrator_ref
                                .heartbeat(agent_id, crate::events::AgentActivity::Idle);
                        }
                    }
                }
            }
            AgentCommand::Pause => {
                orchestrator_ref.heartbeat(agent_id, crate::events::AgentActivity::Idle);
                let _ = orchestrator_ref.pause_agent(agent_id);
            }
            AgentCommand::Resume => {
                orchestrator_ref.heartbeat(agent_id, crate::events::AgentActivity::Idle);
                let _ = orchestrator_ref.resume_agent(agent_id);
            }
            AgentCommand::CancelTask(task_id) => {
                orchestrator_ref.heartbeat(agent_id, crate::events::AgentActivity::Idle);
                if let Some(q_lock) = orchestrator_ref.agent_queue(agent_id) {
                    crate::sync_lock::rw_write(&q_lock).cancel(task_id);
                }
            }
        }
    }
}

/// A fleet supervisor that manages multiple agent processes.
pub struct AgentFleet {
    supervisor: Supervisor,
    scheduler: Arc<Scheduler>,
    orchestrator: Arc<Orchestrator>,
    processor: Arc<dyn TaskProcessor>,
    /// Last time we performed a scale-up (for cooldown).
    last_scale_up: std::sync::RwLock<Option<Instant>>,
    /// Number of agents spawned in the current tick (reset at start of check_scaling).
    spawns_this_tick: std::sync::atomic::AtomicUsize,
}

impl AgentFleet {
    /// Wires the shared scheduler and shared [`Arc<Orchestrator>`] with a task processor implementation.
    pub fn new(
        scheduler: Arc<Scheduler>,
        orchestrator: Arc<Orchestrator>,
        processor: Arc<dyn TaskProcessor>,
    ) -> Self {
        Self {
            supervisor: Supervisor::new(RestartStrategy::RestForOne),
            scheduler,
            orchestrator,
            processor,
            last_scale_up: std::sync::RwLock::new(None),
            spawns_this_tick: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Watch the orchestrator state and ensure an actor exists for every
    /// agent registered in the orchestrator. Also stops processes for retired agents.
    pub async fn sync_fleet(&self) {
        let agent_info: Vec<(AgentId, String)> = {
            let ids = self.orchestrator.agent_ids();
            ids.iter()
                .map(|id| {
                    (
                        *id,
                        crate::sync_lock::rw_read(
                            &*self.orchestrator.agent_queue(*id).expect("agent queue"),
                        )
                        .name
                        .clone(),
                    )
                })
                .collect()
        };
        let active_agent_ids: std::collections::HashSet<AgentId> =
            agent_info.iter().map(|(id, _)| *id).collect();

        // 1. Ensure all active agents have actors
        for (agent_id, name) in agent_info {
            let proc_name = format!("agent-{}", name);

            // Check if process is already running in the global registry
            let already_running = match self.scheduler.registry().lookup_name(&proc_name) {
                Ok(opt) => opt.is_some(),
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        proc_name = %proc_name,
                        "process registry lookup failed for this agent during fleet sync; skipping it this tick"
                    );
                    continue;
                }
            };
            if !already_running {
                // Not running, add it to supervisor
                let orchestrator_clone = self.orchestrator.clone();
                let scheduler_clone = self.scheduler.clone();
                let processor_clone = self.processor.clone();

                let spec = ChildSpec {
                    name: proc_name.clone(),
                    start: Box::new(move || {
                        let h = ActorAgent::spawn(
                            &scheduler_clone,
                            agent_id,
                            name.clone(),
                            orchestrator_clone.clone(),
                            processor_clone.clone(),
                        )?;
                        orchestrator_clone.register_agent_handle(agent_id, h.clone());
                        Ok(h)
                    }),
                };

                self.supervisor.add_child(spec).await;
            }
        }

        // 2. Prune stale handles for retired agents so runtime state converges.
        let mut handles = crate::sync_lock::rw_write(&*self.orchestrator.agent_handles);
        let stale_ids: Vec<AgentId> = handles
            .keys()
            .copied()
            .filter(|id| !active_agent_ids.contains(id))
            .collect();
        for id in stale_ids {
            handles.remove(&id);
            tracing::debug!("Removed stale runtime handle for retired agent {}", id);
        }
        drop(handles);
    }

    /// Supervisor-tick nudge: send [`AgentCommand::ProcessQueue`] to every
    /// agent that has queued work and is not already processing a task.
    ///
    /// This is the tick the `ProcessQueue` doc comment always claimed existed.
    /// Without it, the ONLY notifies are sent at submit time
    /// (`task_submit.rs` / `batch.rs`), and those are silently skipped when
    /// the agent's actor handle does not exist yet — which is exactly the
    /// case when routing spawned the agent during the same submit (the handle
    /// is only registered by [`Self::sync_fleet`]'s next tick). The first
    /// such task queued forever, and because `handle_command` drains exactly
    /// one task per notify, every later submit's notify drained one OLDER
    /// task: permanently one behind.
    /// Fans sends out concurrently (Task C2) rather than one `agent_id` at a
    /// time: a single wedged mailbox needing the full send-timeout used to
    /// stall every later agent's nudge (and everything after this tick -
    /// `check_scaling`/`sync_fleet`/rebalance) by up to
    /// `N_agents * send-timeout`. Concurrent dispatch bounds this to one
    /// timeout window regardless of how many agents are nudged this tick.
    pub async fn nudge_queued_agents(&self) {
        let candidates: Vec<AgentId> = self
            .orchestrator
            .agent_ids()
            .into_iter()
            .filter(|&agent_id| match self.orchestrator.agent_queue(agent_id) {
                Some(queue_lock) => {
                    let queue = crate::sync_lock::rw_read(&*queue_lock);
                    // has_ready_task (not len>0): a queue holding only
                    // dependency-blocked/Doubted tasks would otherwise be
                    // nudged every tick forever, each a no-op dequeue.
                    queue.has_ready_task() && !queue.has_in_progress()
                }
                None => false,
            })
            .collect();

        let sends = candidates.into_iter().filter_map(|agent_id| {
            let handle = crate::sync_lock::rw_read(&*self.orchestrator.agent_handles)
                .get(&agent_id)
                .cloned();
            // Actor not created yet; sync_fleet will register it and the
            // next tick nudges again.
            let handle = handle?;
            Some(async move {
                let json = serde_json::to_string(&AgentCommand::ProcessQueue).unwrap_or_else(|e| {
                    tracing::warn!("serialize ProcessQueue: {e}");
                    "{}".to_string()
                });
                let env = vox_actor_runtime::mailbox::Envelope::Message(
                    vox_actor_runtime::mailbox::Message {
                        from: vox_actor_runtime::Pid::new(),
                        payload: MessagePayload::Json(json.into()),
                    },
                );
                match tokio::time::timeout(vox_config::timeouts::D_5S, handle.send(env)).await {
                    Ok(Err(e)) => tracing::warn!(
                        "fleet tick: ProcessQueue nudge to agent {agent_id} failed: {e:?}"
                    ),
                    Err(_) => tracing::warn!(
                        "fleet tick: ProcessQueue nudge to agent {agent_id} timed out"
                    ),
                    Ok(Ok(())) => {}
                }
            })
        });

        futures_util::future::join_all(sends).await;
    }

    /// Check if agents need to be spawned or retired using ScalingService and profile limits.
    pub async fn check_scaling(&self) {
        // Reset spawn counter at the start of each scaling cycle so each tick
        // gets a clean budget — avoids stale carry-over from concurrent paths.
        self.spawns_this_tick
            .store(0, std::sync::atomic::Ordering::Relaxed);

        let (status, idle_dynamic, config, budget_manager, remote_gpu_capacity) = {
            let orch = &*self.orchestrator;
            let config_arc = orch.config_handle();
            let config = crate::sync_lock::rw_read(&config_arc).clone();
            if !config.scaling_enabled {
                return;
            }
            let status = orch.status();
            let idle_dynamic: Vec<_> = status
                .agents
                .iter()
                .filter(|a| a.dynamic && a.queued == 0 && !a.in_progress)
                .filter_map(|a| {
                    orch.agent_queue(a.id)
                        .map(|q| (a.id, crate::sync_lock::rw_read(&*q).last_active))
                })
                .collect();
            let budget_manager = orch.budget_manager_handle();
            let remote_gpu_capacity = crate::sync_lock::rw_read(&*orch.remote_populi_routing_hints)
                .iter()
                .filter(|h| {
                    h.capabilities.gpu_cuda
                        || h.capabilities.gpu_metal
                        || h.capabilities.gpu_vulkan
                        || h.capabilities.gpu_webgpu
                        || h.capabilities.npu
                })
                .count();
            (
                status,
                idle_dynamic,
                config,
                budget_manager,
                remote_gpu_capacity,
            )
        };

        let load_history: Vec<f64> = crate::sync_lock::rw_read(&*self.orchestrator.load_history)
            .iter()
            .copied()
            .collect();
        let local_snapshot = crate::services::local_resources::snapshot();
        let action = ScalingService::decide_scaling(
            &status,
            &config,
            &load_history,
            remote_gpu_capacity,
            &idle_dynamic,
            &crate::sync_lock::rw_read(&budget_manager),
            local_snapshot.as_ref(),
        );

        match action {
            ScalingAction::NoOp => {}
            ScalingAction::ScaleUp { name_prefix, count } => {
                let max_per_tick = config.max_spawn_per_tick;
                let cooldown_ms = config.scaling_cooldown_ms;
                let spawns = self
                    .spawns_this_tick
                    .load(std::sync::atomic::Ordering::Relaxed);
                let cooldown_ok = crate::sync_lock::rw_read(&self.last_scale_up)
                    .as_ref()
                    .map(|t| t.elapsed() >= std::time::Duration::from_millis(cooldown_ms))
                    .unwrap_or(true);

                if spawns < max_per_tick && cooldown_ok {
                    let limit = std::cmp::min(count, max_per_tick - spawns);
                    for _ in 0..limit {
                        let uuid_str = uuid::Uuid::new_v4().to_string();
                        let name = format!("{}-{}", name_prefix, short_id_from_str(&uuid_str));
                        let _ = self.orchestrator.spawn_dynamic_agent_with_parent(
                            &name,
                            None,
                            Some("scaling_load"),
                            None,
                            None,
                            None,
                            None,
                        );
                        self.spawns_this_tick
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    *crate::sync_lock::rw_write(&self.last_scale_up) =
                        Some(std::time::Instant::now());
                    tracing::info!(
                        "Scaling up: spawned {} dynamic agents (load: {:.2}, profile: {:?})",
                        limit,
                        status.total_weighted_load,
                        config.scaling_profile
                    );
                }
            }
            ScalingAction::ScaleDown { agent_ids } => {
                if !agent_ids.is_empty() {
                    tracing::info!(
                        "Scaling down: retiring {} idle dynamic agent(s)",
                        agent_ids.len()
                    );
                }
                for id in agent_ids {
                    if let Ok(remaining) = self.orchestrator.retire_agent(id).await {
                        for task in remaining {
                            let task_id = task.id;
                            if let Err(e) = self.orchestrator.submit_existing_task(task).await {
                                tracing::error!(
                                    "failed to requeue task {} from retiring agent {}: {} (task is now untracked)",
                                    task_id,
                                    id,
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Start the main orchestrator loop: rebalancing, maintenance, and fleet syncing.
    pub async fn run(&self) {
        loop {
            // 1. Scaling checks
            self.check_scaling().await;

            // 2. Sync fleet (ensure all agents have actors)
            self.sync_fleet().await;

            // 2b. Nudge agents with queued work (closes the spawn-at-submit
            // notify race and the one-behind drain — see nudge_queued_agents).
            self.nudge_queued_agents().await;

            // 3. Perform orchestrator maintenance (rebalance and tick)
            {
                self.orchestrator.rebalance();
                self.orchestrator.tick().await;
            }

            // 4. Wait until next tick
            tokio::time::sleep(vox_config::timeouts::D_1S).await;
        }
    }
}

/// When truthy (default if unset), MCP / `vox-orchestrator-d` spawn [`AgentFleet`] with [`AiTaskProcessor`].
///
/// Disable with **`VOX_MCP_AGENT_FLEET`**=`0`, `false`, `no`, or `off`.
#[must_use]
pub fn agent_fleet_env_enabled() -> bool {
    match vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpAgentFleet).expose() {
        Some(v) => {
            let v = v.trim();
            if v.is_empty() {
                return true;
            }
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("no")
                || v.eq_ignore_ascii_case("off"))
        }
        None => true,
    }
}

pub fn spawn_agent_fleet_if_enabled(orchestrator: Arc<Orchestrator>) {
    spawn_agent_fleet_if_enabled_with_dispatcher(orchestrator, None);
}

/// Same as [`spawn_agent_fleet_if_enabled`], but wires `dispatcher` (if
/// given) into the spawned [`AiTaskProcessor`] so autonomous `@tool` intent
/// lines are actually dispatched (T1.5 follow-up) rather than only logged.
/// `None` preserves the pre-existing breadcrumb-only behavior — pass `None`
/// for hosts that never construct an MCP `ServerState` in-process (there is
/// nothing to bridge into).
pub fn spawn_agent_fleet_if_enabled_with_dispatcher(
    orchestrator: Arc<Orchestrator>,
    dispatcher: Option<Arc<dyn ToolDispatcher>>,
) {
    if !agent_fleet_env_enabled() {
        tracing::info!(
            target: "vox_orchestrator::runtime",
            "VOX_MCP_AGENT_FLEET disabled: task queues will not auto-drain via AgentFleet"
        );
        return;
    }
    let scheduler = Arc::new(Scheduler::new());
    tokio::spawn(async move {
        let agentic = Arc::new(match dispatcher {
            Some(d) => {
                AiTaskProcessor::with_tool_dispatcher(
                    orchestrator.event_bus.clone(),
                    orchestrator.clone(),
                    d,
                )
                .await
            }
            None => {
                AiTaskProcessor::new(orchestrator.event_bus.clone(), orchestrator.clone()).await
            }
        });
        // Fix Task 4 (gui-axis-chat-harness-fixes): `ChatTaskProcessor` (a
        // separate, single-shot, non-tool-calling processor for
        // `TaskCategory::Chat`) is deleted -- it had no tool-calling loop and
        // never read `task.active_skill`, and nothing produces
        // `TaskCategory::Chat` in a way that expects that special handling
        // anymore (the GUI's composer routes chat-category sends through the
        // synchronous `chat_send_message` path before a background task is
        // ever submitted; the "Background task" toggle position sends no
        // category at all, same as `/spawn`). `RoutingTaskProcessor` stays --
        // it is generic over its two processor types, not hardcoded to
        // `ChatTaskProcessor` -- but both its `agentic` and `chat` slots now
        // point at the same `AiTaskProcessor` instance, so a `Chat`-category
        // task (however it might arise, e.g. an MCP tool caller using
        // `task_category_from_mcp_str`) is handled by the real, tool-calling,
        // privacy-filtered pipeline like every other category, not silently
        // dropped or misrouted.
        let processor: Arc<dyn TaskProcessor> = Arc::new(
            crate::routing_processor::RoutingTaskProcessor::new(agentic.clone(), agentic),
        );
        let fleet = AgentFleet::new(scheduler, orchestrator, processor);
        tracing::info!(
            target: "vox_orchestrator::runtime",
            "AgentFleet loop running (RoutingTaskProcessor: AiTaskProcessor for all categories; MCP / orchestrator-d)"
        );
        fleet.run().await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_from_standard_uuid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let result = short_id_from_str(uuid_str);
        assert_eq!(result, "550e8400");
    }

    #[test]
    fn short_id_from_hyphen_free_string() {
        // If format ever lacks hyphens, must not panic — returns first 8 chars
        let s = "1234567890abcdef1234567890abcdef";
        let result = short_id_from_str(s);
        assert_eq!(result, "12345678");
    }

    #[test]
    fn parse_tool_intent_line_name_only() {
        let (name, args) = parse_tool_intent_line("@tool vox_git_status");
        assert_eq!(name, "vox_git_status");
        assert_eq!(args, serde_json::json!({}));
    }

    #[test]
    fn parse_tool_intent_line_with_json_args() {
        let (name, args) = parse_tool_intent_line(r#"@tool vox_read_file {"path": "src/main.rs"}"#);
        assert_eq!(name, "vox_read_file");
        assert_eq!(args, serde_json::json!({"path": "src/main.rs"}));
    }

    #[test]
    fn parse_tool_intent_line_invalid_json_falls_back_to_empty_object() {
        let (name, args) = parse_tool_intent_line("@tool vox_run_shell not valid json at all");
        assert_eq!(name, "vox_run_shell");
        assert_eq!(args, serde_json::json!({}));
    }

    #[test]
    fn parse_tool_intent_line_non_object_json_falls_back_to_empty_object() {
        // A bare JSON array/number/string is not a usable tool-args object.
        let (name, args) = parse_tool_intent_line(r#"@tool vox_git_status [1, 2, 3]"#);
        assert_eq!(name, "vox_git_status");
        assert_eq!(args, serde_json::json!({}));
    }

    #[test]
    fn parse_tool_intent_line_bare_at_tool_no_name() {
        // Defensive: must not panic even on a malformed line without a name.
        let (name, args) = parse_tool_intent_line("@tool");
        assert_eq!(name, "");
        assert_eq!(args, serde_json::json!({}));
    }

    #[tokio::test]
    async fn stub_processor_aborts_when_cancel_flag_preset() {
        let proc_ = StubTaskProcessor;
        let task = crate::types::AgentTask::new(
            crate::types::TaskId(42),
            "interruptible work",
            crate::types::TaskPriority::Normal,
            vec![],
        );
        let cancel = Arc::new(AtomicBool::new(true));
        let result = proc_.process(crate::types::AgentId(1), task, cancel).await;
        assert!(result.is_err(), "pre-set cancel flag must abort process");
        assert!(
            result.unwrap_err().to_string().contains("interrupted"),
            "error should report interruption"
        );
    }

    #[tokio::test]
    async fn stub_processor_completes_when_not_cancelled() {
        let proc_ = StubTaskProcessor;
        let task = crate::types::AgentTask::new(
            crate::types::TaskId(7),
            "normal work",
            crate::types::TaskPriority::Normal,
            vec![],
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let result = proc_.process(crate::types::AgentId(1), task, cancel).await;
        assert_eq!(result.unwrap(), crate::types::TaskId(7));
    }

    /// RED test 3: `TokenStreamed` events emitted by `run_phase_stream_with_bus` — the
    /// single call chain `AiTaskProcessor::process` uses to drive streaming — genuinely
    /// originate from the T4.1-consolidated stack: `vox_gamify::FreeAiClient::generate_stream_routed`
    /// routed at `StreamRoute::Registry { backend: OpenRouter, .. }`, which (per the
    /// vox-gamify migration) now goes through `vox_actor_runtime::execute_activity` +
    /// `vox_llm_egress::stream_once` rather than a bespoke HTTP client. Proven end-to-end:
    /// a mock OpenRouter SSE response reaches the orchestrator's `EventBus` as one or more
    /// `TokenStreamed { text, .. }` events whose concatenated text matches the mocked delta.
    #[tokio::test]
    async fn token_streamed_events_originate_from_consolidated_stream_stack() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"uni\"}}]}\n\
                   data: {\"choices\":[{\"delta\":{\"content\":\"fied\"}}]}\n\
                   data: [DONE]\n\n";
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse),
            )
            .mount(&server)
            .await;

        // Rust 2024 made std::env::{set_var,remove_var} unsafe; single-threaded mutation,
        // scoped tightly around the call under test.
        let prev = std::env::var("OPENROUTER_BASE_URL").ok();
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("OPENROUTER_BASE_URL", server.uri());
        }
        vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

        let event_bus = crate::events::EventBus::new(64);
        let mut rx = event_bus.subscribe();

        let client = vox_gamify::FreeAiClient::new(vec![vox_gamify::FreeAiProvider::OpenRouter {
            api_key: "test-api-key".to_string(),
            models: Vec::new(),
        }]);
        let task = crate::types::AgentTask::new(
            crate::types::TaskId(1),
            "unify the streaming stack",
            crate::types::TaskPriority::Normal,
            vec![],
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let route = vox_gamify::StreamRoute::Registry {
            backend: vox_gamify::LudusStreamBackend::OpenRouter,
            model: "test/model",
        };

        let phase_text = AiTaskProcessor::run_phase_stream_with_bus(
            &event_bus,
            &client,
            crate::types::AgentId(1),
            &task,
            crate::types::TaskPhase::Inspect,
            "test/model",
            "",
            route,
            &cancel,
        )
        .await
        .expect("stream must complete");

        #[allow(unsafe_code)]
        unsafe {
            match prev {
                Some(v) => std::env::set_var("OPENROUTER_BASE_URL", v),
                None => std::env::remove_var("OPENROUTER_BASE_URL"),
            }
        }
        vox_config::snapshot::bump(&["OPENROUTER_BASE_URL"]);

        assert_eq!(phase_text, "unified");

        // Drain the TokenStreamed events actually broadcast on the EventBus and confirm
        // they concatenate to the same text — proving the event path (not just the
        // function's return value) carries the consolidated stack's deltas.
        let mut streamed_text = String::new();
        while let Ok(evt) = rx.try_recv() {
            if let AgentEventKind::TokenStreamed { text, agent_id, .. } = evt.kind {
                assert_eq!(agent_id, crate::types::AgentId(1));
                streamed_text.push_str(&text);
            }
        }
        assert_eq!(
            streamed_text, "unified",
            "TokenStreamed events on the EventBus must carry the consolidated stack's deltas"
        );
    }

    /// T5.2 (harness reliability spec, Phase 5): `handle_command`'s
    /// `AgentCommand::ProcessQueue` arm used to discard the `Result` of
    /// `orchestrator_ref.complete_task(completed_id)` via `let _ = ...`,
    /// silently swallowing an `Err` (e.g. `TaskNotFound`) with no trace at
    /// all. This test drives the exact `Ok(completed_id) => { if let
    /// Err(err) = ... }` pattern now in `handle_command` against a real
    /// `Orchestrator` (not through `handle_command` itself, which — via
    /// `heartbeat()` reading the same per-agent queue lock `ProcessQueue`
    /// holds writable — has a pre-existing lock-reentrancy deadlock
    /// unrelated to this fix and out of scope for T5.2): submit a task, then
    /// strip its `task_assignments` entry (as would happen if assignment
    /// bookkeeping and completion ever raced) so `complete_task` hits its
    /// `TaskNotFound` error path, and assert the failure is now logged via
    /// `tracing::error!` instead of vanishing.
    ///
    /// Uses a scoped (thread-local, via `tracing::subscriber::with_default`)
    /// capturing subscriber rather than `#[tracing_test::traced_test]`,
    /// which races to install a *global* default dispatcher and can panic
    /// when run in the same test binary as another test that already set one
    /// (e.g. `orchestrator::tests::orch_smoke::complexity_based_routing_test`'s
    /// `tracing_subscriber::fmt::try_init()`).
    #[tokio::test]
    async fn process_queue_logs_complete_task_error_instead_of_discarding_it() {
        use tracing_subscriber::layer::SubscriberExt;

        let orchestrator = Arc::new(Orchestrator::new(
            crate::config::OrchestratorConfig::for_testing(),
        ));
        let _agent_id = orchestrator.spawn_agent("t5-2").expect("spawn agent");
        let task_id = orchestrator
            .submit_task_with_agent(
                "T5.2 discarded-result repro",
                vec![],
                None,
                Some("t5-2".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .expect("submit task");

        // Drop the assignment record out from under the task so
        // `complete_task` hits its `TaskNotFound` error path instead of
        // succeeding — reproducing exactly the `Err` that `handle_command`'s
        // `Ok(completed_id) => { ... }` arm now logs instead of discarding.
        orchestrator
            .task_assignments
            .write()
            .unwrap()
            .remove(&task_id);

        let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let make_writer = {
            let captured = Arc::clone(&captured);
            move || CapturingWriter {
                buf: Arc::clone(&captured),
            }
        };
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_writer(make_writer)
                .with_ansi(false),
        );

        // Mirror `handle_command`'s fixed branch verbatim, inside the scoped
        // (thread-local, not global) subscriber guard so the Err log is
        // captured. `#[tokio::test]` defaults to a current-thread runtime, so
        // the guard stays active across the `.await` below.
        let _guard = tracing::subscriber::set_default(subscriber);
        if let Err(err) = orchestrator.complete_task(task_id).await {
            tracing::error!(
                "complete_task failed after processor success: {} (task {})",
                err,
                task_id
            );
        } else {
            panic!("complete_task on an unassigned task must fail");
        }
        drop(_guard);

        let logs = String::from_utf8(captured.lock().unwrap().clone()).expect("utf8 logs");
        assert!(
            logs.contains("complete_task failed after processor success"),
            "discarded complete_task Err must now be logged, not silently dropped; captured logs: {logs}"
        );
    }

    /// Regression test for the deadlock documented on
    /// `process_queue_logs_complete_task_error_instead_of_discarding_it`:
    /// `handle_command`'s `AgentCommand::ProcessQueue` arm used to call
    /// `Orchestrator::heartbeat` (which takes a *read* lock on the agent's
    /// `Arc<RwLock<AgentQueue>>`) while still holding a *write* guard on that
    /// same lock. `std::sync::RwLock` is not reentrant, so any real
    /// `ProcessQueue` dispatch — the exact path task submission uses via
    /// `submit_task_with_agent` (see
    /// `orchestrator/task_dispatch/submit/task_submit.rs`) — would hang the
    /// agent process forever.
    ///
    /// Drives `handle_command` itself (not a hand-rolled mirror) with a real
    /// queued task so both the write-lock dequeue and the `heartbeat` read
    /// lock are exercised in the same call, wrapped in a short
    /// `tokio::time::timeout` so a reintroduced deadlock fails this test
    /// loudly instead of hanging CI.
    #[tokio::test]
    async fn process_queue_does_not_deadlock_on_heartbeat() {
        let orchestrator = Arc::new(Orchestrator::new(
            crate::config::OrchestratorConfig::for_testing(),
        ));
        let agent_id = orchestrator.spawn_agent("t-deadlock").expect("spawn agent");
        orchestrator
            .submit_task_with_agent(
                "ProcessQueue/heartbeat deadlock repro",
                vec![],
                None,
                Some("t-deadlock".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .expect("submit task");

        let processor: Arc<dyn TaskProcessor> = Arc::new(StubTaskProcessor);

        let outcome = tokio::time::timeout(vox_config::timeouts::D_5S, async {
            ActorAgent::handle_command(
                AgentCommand::ProcessQueue,
                agent_id,
                &orchestrator,
                &processor,
            )
            .await;
        })
        .await;

        assert!(
            outcome.is_ok(),
            "AgentCommand::ProcessQueue timed out — the write guard on the \
             agent's queue lock is likely still held when `heartbeat` tries \
             to read-lock the same Arc<RwLock<AgentQueue>>, reintroducing \
             the ProcessQueue/heartbeat lock-reentrancy deadlock"
        );
    }

    /// Proof for Task C2: `nudge_queued_agents` must send to multiple wedged
    /// agents concurrently, not one-at-a-time.
    ///
    /// `vox_actor_runtime::ProcessHandle` is a concrete struct (not a trait),
    /// so a unit-level fake whose `send()` itself increments a counter isn't
    /// directly pluggable (Step 1b's investigation). Instead this builds
    /// real, deliberately-wedged mailboxes: each agent's mailbox is
    /// constructed with capacity 1 and pre-filled with a dummy envelope, and
    /// the receiver is kept alive but never drained — so any further send to
    /// it blocks for the *entire* configured send-timeout
    /// (`vox_config::timeouts::D_5S`), exactly the pathological "wedged
    /// mailbox" scenario the design doc's motivation describes.
    ///
    /// With `N_AGENTS` such wedged agents, a serial per-agent `for` loop
    /// must accumulate `N_AGENTS * D_5S` before returning (each send only
    /// gives up after its own full timeout); concurrent dispatch bounds
    /// total time to ~1 timeout window regardless of `N_AGENTS`. This is a
    /// coarse threshold assertion, not a "close to one window" assertion:
    /// 2 agents means serial floor is ~10s and concurrent ceiling is ~5s, so
    /// asserting `elapsed < 8s` has multiple seconds of margin on both
    /// sides — ordinary scheduler jitter cannot flip the result.
    #[tokio::test]
    async fn nudge_sends_are_concurrent_not_serial() {
        let orchestrator = Arc::new(Orchestrator::new(
            crate::config::OrchestratorConfig::for_testing(),
        ));

        const N_AGENTS: usize = 2;
        // Keep every receiver alive for the whole test so the mailbox stays
        // open-but-full (a dropped receiver would make further sends fail
        // fast with a "channel closed" error instead of blocking).
        let mut _rx_keepalive = Vec::with_capacity(N_AGENTS);

        for i in 0..N_AGENTS {
            let agent_id = orchestrator
                .spawn_agent(&format!("t-c2-nudge-{i}"))
                .expect("spawn agent");
            orchestrator
                .submit_task_with_agent(
                    &format!("C2 concurrency repro {i}"),
                    vec![],
                    None,
                    Some(format!("t-c2-nudge-{i}")),
                    None,
                    None,
                    None,
                    None,
                )
                .await
                .expect("submit task");

            let (tx, rx) = vox_actor_runtime::mailbox::new_mailbox(1);
            let dummy = vox_actor_runtime::mailbox::Envelope::Message(
                vox_actor_runtime::mailbox::Message {
                    from: vox_actor_runtime::Pid::new(),
                    payload: MessagePayload::Json("{}".to_string().into()),
                },
            );
            tx.send(dummy).await.expect("prefill the size-1 mailbox");

            let handle = ProcessHandle {
                pid: vox_actor_runtime::Pid::new(),
                mailbox_tx: tx,
                task: None,
            };
            orchestrator.register_agent_handle(agent_id, handle);
            _rx_keepalive.push(rx);
        }

        let scheduler = Arc::new(Scheduler::new());
        let processor: Arc<dyn TaskProcessor> = Arc::new(StubTaskProcessor);
        let fleet = AgentFleet::new(scheduler, orchestrator.clone(), processor);

        let start = std::time::Instant::now();
        fleet.nudge_queued_agents().await;
        let elapsed = start.elapsed();

        assert!(
            elapsed < std::time::Duration::from_secs(8),
            "nudge_queued_agents took {elapsed:?} for {N_AGENTS} permanently-wedged agents \
             (each requiring the full send-timeout) - expected ~1 timeout window from \
             concurrent dispatch, not {N_AGENTS} windows from serial dispatch"
        );
    }

    /// Test-only [`std::io::Write`] sink that appends into a shared buffer,
    /// used as a `tracing_subscriber::fmt` writer to capture log output
    /// scoped to a single test (see
    /// `process_queue_logs_complete_task_error_instead_of_discarding_it`).
    struct CapturingWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl std::io::Write for CapturingWriter {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.buf.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod task_policy_wiring_tests {
    use super::*;
    use crate::config::TaskPolicyOverrides;
    use crate::types::{AgentTask, TaskId, TaskPriority};

    #[test]
    fn unset_task_with_no_overrides_matches_todays_global_default() {
        let task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        let overrides = TaskPolicyOverrides::default();
        let global_default = crate::config::OrchestratorConfig::default().cost_preference;
        let (cost_pref, force_free_pool, risk) =
            resolve_task_cost_policy(&task, &overrides, global_default);
        assert_eq!(
            cost_pref, global_default,
            "no policy anywhere must reproduce today's behavior exactly"
        );
        assert!(!force_free_pool);
        assert_eq!(risk, crate::mode::RiskPosture::Moderate);
    }

    #[test]
    fn source_override_applies_when_no_explicit_hint() {
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        task.trigger_source = Some(crate::mode::TriggerSource::Automated);
        let mut source = std::collections::HashMap::new();
        source.insert(
            "Automated".to_string(),
            crate::config::TaskPolicyEntry {
                clutch: Some("free".to_string()),
                risk: Some("high".to_string()),
            },
        );
        let overrides = TaskPolicyOverrides {
            category: std::collections::HashMap::new(),
            source,
        };
        let (_cost_pref, force_free_pool, risk) = resolve_task_cost_policy(
            &task,
            &overrides,
            crate::config::OrchestratorConfig::default().cost_preference,
        );
        assert!(
            force_free_pool,
            "Automated source override (Free clutch) must force the free-only pool"
        );
        assert_eq!(risk, crate::mode::RiskPosture::High);
    }

    #[test]
    fn source_risk_policy_applies_even_when_no_clutch_policy_is_configured() {
        // Regression test: resolve_task_cost_policy used to discard its
        // resolved risk axis whenever no *clutch* policy applied anywhere,
        // even if a category/source *risk* policy was configured — so the
        // ModelLean::Intelligence -> CostPreference::Performance nudge in
        // AiTaskProcessor::process (which reads this function's third return
        // value, not AgentTask::resolved_risk()) would never see it.
        let mut task = AgentTask::new(TaskId(1), "t", TaskPriority::Normal, vec![]);
        task.trigger_source = Some(crate::mode::TriggerSource::Automated);
        let mut source = std::collections::HashMap::new();
        source.insert(
            "Automated".to_string(),
            crate::config::TaskPolicyEntry {
                clutch: None,
                risk: Some("low".to_string()),
            },
        );
        let overrides = TaskPolicyOverrides {
            category: std::collections::HashMap::new(),
            source,
        };
        let global_default = crate::config::OrchestratorConfig::default().cost_preference;
        let (cost_pref, force_free_pool, risk) =
            resolve_task_cost_policy(&task, &overrides, global_default);
        assert_eq!(
            cost_pref, global_default,
            "no clutch policy anywhere must still preserve the global cost-preference default"
        );
        assert!(!force_free_pool);
        assert_eq!(
            risk,
            crate::mode::RiskPosture::Low,
            "the source's risk policy must be resolved and returned even though no clutch policy applied"
        );
    }
}
