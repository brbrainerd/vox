//! `vox harness eval` — a multi-sample eval gate for the harness itself.
//!
//! Motivation (harness parity plan, Phase 5): a single run of a task is noisy
//! evidence that a harness change (wire format, agent loop, model routing,
//! tool selection, ...) actually improved or regressed real task-completion
//! behavior. This command runs a small set of **golden tasks** — deterministic,
//! hermetic checks against Vox's own workspace/code, not an LLM-judge and not
//! an imported public benchmark — `--samples` times each, and computes
//! **pass^k**: a task counts as passing only if *all* `--samples` independent
//! runs pass. This is deliberately stricter than pass@1 (which only requires
//! one success out of N attempts): pass^k is the right metric for "is this
//! reliable enough to gate on", not "is this achievable at all".
//!
//! ## Adding a golden task
//!
//! Add a `GoldenTask` to [`golden_tasks`]. Each task is a `fn() -> Result<()>`
//! that must be:
//! - **Deterministic**: same inputs, same result, every time.
//! - **Hermetic**: no live network or live model calls, no dependency on
//!   external state (a live Ollama daemon, a populated DB, etc). Prefer
//!   invoking a pure function or a CLI arg-parser directly, or driving a
//!   fixture/temp workspace and asserting exact output/exit-code/file-state.
//! - **Cheap**: this runs `--samples` times per task, every gate invocation.
//!
//! A golden task that genuinely needs a live model call to be meaningful may
//! still be added, but MUST be gated so the core gate stays hermetic — see
//! `live_model_smoke_task` below for the pattern. A task can declare a
//! `skip_if` predicate (checked once, before sampling); when it returns
//! `Some(reason)` the task is reported as **SKIPPED** — a status distinct
//! from PASS/FAIL that is excluded from the pass^k gate determination
//! entirely, so a disabled/not-yet-implemented task can never masquerade as
//! a real pass. `live_model_smoke_task` is currently a stub gated this way
//! (`VOX_HARNESS_EVAL_LIVE` unset -> always SKIPPED); setting the env var
//! does not fake a PASS, it surfaces "not implemented" as its own failure
//! reason so the stub can never be silently miscounted as green.

use anyhow::{Result, bail};
use clap::Parser;
use owo_colors::OwoColorize;

use crate::commands::model::eval::{FixtureResult, score_eval};

/// Upper bound on `--samples`. A typo'd large value (e.g. an extra zero)
/// would otherwise silently run every golden task that many times with no
/// feedback until it finally finished; reject early instead.
const MAX_SAMPLES: usize = 100;

/// `vox harness eval` arguments.
#[derive(Parser)]
pub struct EvalArgs {
    /// Number of independent samples per golden task (1..=100). pass^k
    /// requires ALL samples to pass for the task to count as passing (not
    /// pass@1).
    #[arg(long, default_value_t = 3)]
    pub samples: usize,
    /// Run only the golden task with this exact name (repeatable filtering
    /// is not needed yet at this scale; add more flags if the task set grows).
    #[arg(long)]
    pub task: Option<String>,
    /// Run the live-model-calling golden task corpus (see `live_eval.rs`) instead of the
    /// hermetic gate. Makes real API calls, costs real money (bounded by
    /// `live_eval::LIVE_EVAL_COST_CEILING_USD`), and is intended for scheduled/manual runs, not
    /// every commit.
    #[arg(long)]
    pub live: bool,
}

/// One golden task: a name, a deterministic check function, and an optional
/// gate that turns the whole task into a SKIP (checked once, before any
/// sampling) rather than running it. `skip_if` returns `Some(reason)` when
/// the task should be skipped.
struct GoldenTask {
    name: &'static str,
    run: fn() -> Result<()>,
    skip_if: Option<fn() -> Option<String>>,
}

/// The built-in golden task set. See the module docs for how to add one.
fn golden_tasks() -> Vec<GoldenTask> {
    vec![
        GoldenTask {
            name: "model-eval-score-fold-exact",
            run: model_eval_score_fold_exact,
            skip_if: None,
        },
        GoldenTask {
            name: "cli-harness-eval-arg-parsing",
            run: cli_harness_eval_arg_parsing,
            skip_if: None,
        },
        GoldenTask {
            name: "temp-workspace-file-roundtrip",
            run: temp_workspace_file_roundtrip,
            skip_if: None,
        },
        GoldenTask {
            name: "live-model-smoke",
            run: live_model_smoke_task,
            skip_if: Some(live_model_smoke_skip_reason),
        },
        GoldenTask {
            name: "tool-cap-never-exceeds-cap",
            run: tool_cap_never_exceeds_cap_task,
            skip_if: None,
        },
        GoldenTask {
            name: "agent-loop-terminates",
            run: agent_loop_terminates_task,
            skip_if: None,
        },
        GoldenTask {
            name: "privacy-filter-blocks-live-routing",
            run: privacy_filter_blocks_live_routing_task,
            skip_if: None,
        },
    ]
}

/// Golden task 1: `score_eval` (the pure fold behind `vox model eval`) must
/// compute exact, known values from a fixed synthetic input every time.
/// Exercises harness-adjacent scoring logic without any live LLM call.
fn model_eval_score_fold_exact() -> Result<()> {
    let results = vec![
        FixtureResult {
            correct: true,
            total_tokens: 10,
            latency_ms: 100,
            cost_usd: None,
        },
        FixtureResult {
            correct: true,
            total_tokens: 20,
            latency_ms: 200,
            cost_usd: None,
        },
        FixtureResult {
            correct: false,
            total_tokens: 30,
            latency_ms: 300,
            cost_usd: None,
        },
    ];
    let out = score_eval(&results);
    if (out.intelligence - (2.0 / 3.0)).abs() > 1e-9 {
        bail!("expected intelligence 2/3, got {}", out.intelligence);
    }
    if out.passed != 2 || out.n_calls != 3 {
        bail!(
            "expected passed=2 n_calls=3, got passed={} n_calls={}",
            out.passed,
            out.n_calls
        );
    }
    if out.p50_ms != 200 {
        bail!("expected p50_ms=200, got {}", out.p50_ms);
    }
    Ok(())
}

/// Golden task 2: `vox harness eval` CLI arg parsing must be exact — default
/// `--samples` is 3, and an explicit value round-trips unchanged. Exercises
/// the same clap-derive dispatch pattern every `vox` subcommand relies on.
fn cli_harness_eval_arg_parsing() -> Result<()> {
    let default_args = EvalArgs::try_parse_from(["eval"])
        .map_err(|e| anyhow::anyhow!("default parse failed: {e}"))?;
    if default_args.samples != 3 {
        bail!("expected default samples=3, got {}", default_args.samples);
    }
    if default_args.task.is_some() {
        bail!("expected default task=None, got {:?}", default_args.task);
    }

    let explicit_args = EvalArgs::try_parse_from(["eval", "--samples", "7", "--task", "foo"])
        .map_err(|e| anyhow::anyhow!("explicit parse failed: {e}"))?;
    if explicit_args.samples != 7 {
        bail!("expected samples=7, got {}", explicit_args.samples);
    }
    if explicit_args.task.as_deref() != Some("foo") {
        bail!("expected task=Some(\"foo\"), got {:?}", explicit_args.task);
    }
    Ok(())
}

/// Golden task 3: writing a fixture file to a temp workspace and reading it
/// back must produce byte-exact content and a clean exit — the "did the file
/// get created with expected content" shape of deterministic outcome check
/// called for in the harness eval design.
fn temp_workspace_file_roundtrip() -> Result<()> {
    let dir = std::env::temp_dir().join(format!(
        "vox-harness-eval-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("fixture.txt");
    let expected = "vox harness golden fixture\n";
    std::fs::write(&path, expected)?;
    let actual = std::fs::read_to_string(&path)?;
    let _ = std::fs::remove_dir_all(&dir);
    if actual != expected {
        bail!("file round-trip mismatch: expected {expected:?}, got {actual:?}");
    }
    Ok(())
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default()
}

/// Golden task 4 (extension-point example): a task that would need a live
/// model call to be meaningful. Gated behind `VOX_HARNESS_EVAL_LIVE=1` via
/// [`live_model_smoke_skip_reason`] so the CORE gate (the other three tasks)
/// stays hermetic: with the env var unset, `run()` never even calls this
/// function — the task is reported as SKIPPED, a status excluded from the
/// pass^k gate. With the env var set it still is NOT implemented; it
/// deliberately returns `Err` (never a silent/free `Ok`) so a stub can never
/// be miscounted as a real PASS. Implementing the live call for real is the
/// documented extension point — mirror `model::eval::eval_one_model`'s use of
/// `vox_actor_runtime::llm::llm_chat` when that work is picked up.
fn live_model_smoke_task() -> Result<()> {
    bail!(
        "live-model-smoke is not implemented yet (VOX_HARNESS_EVAL_LIVE=1 opts in to running \
         it, but the live-model call itself is still a documented extension point, not a stub \
         that reports PASS for free)"
    );
}

/// `skip_if` for `live-model-smoke`: skipped unless the caller explicitly
/// opted in with `VOX_HARNESS_EVAL_LIVE=1`.
fn live_model_smoke_skip_reason() -> Option<String> {
    if std::env::var("VOX_HARNESS_EVAL_LIVE").as_deref() == Ok("1") {
        None
    } else {
        Some("VOX_HARNESS_EVAL_LIVE not set to \"1\"".to_string())
    }
}

/// Golden task 5: the tool-selection cap ([`select_tools_for_turn`]) must
/// never offer more than `DEFAULT_MAX_TOOLS` tools for a plain chat turn, and
/// must never offer a `vox_chat_*` tool (the agent loop's recursion guard —
/// see `agent_loop.rs`'s exclusion of that prefix). Regresses silently if the
/// exclude-before-cap ordering (`tool_selection.rs`) is ever reverted.
///
/// [`select_tools_for_turn`]: vox_orchestrator_mcp::llm_bridge::tool_selection::select_tools_for_turn
fn tool_cap_never_exceeds_cap_task() -> Result<()> {
    use vox_mcp_registry::TOOL_REGISTRY;
    use vox_orchestrator_mcp::llm_bridge::tool_selection::{
        DEFAULT_MAX_TOOLS, TurnContext, new_registry_arc_for_eval, select_tools_for_turn,
    };

    let ctx = TurnContext {
        permission_mode: None,
        lanes: vec!["default"],
        active_skill_id: None,
        max_tools: DEFAULT_MAX_TOOLS,
        exclude_name_prefixes: vec!["vox_chat_"],
    };
    let reg = new_registry_arc_for_eval();
    let selected = select_tools_for_turn(TOOL_REGISTRY, &reg, &ctx);
    if selected.len() > DEFAULT_MAX_TOOLS {
        bail!(
            "selected {} tools, cap is {}",
            selected.len(),
            DEFAULT_MAX_TOOLS
        );
    }
    if selected.iter().any(|t| t.name.starts_with("vox_chat_")) {
        bail!("a vox_chat_* tool was offered to the model — recursion-guard regression");
    }
    Ok(())
}

/// Golden task 6: the agent loop must genuinely terminate at its iteration
/// cap rather than recursing forever against a model that always returns a
/// tool call. Bridges into an async check via a dedicated single-threaded
/// tokio runtime (this crate's task functions are plain `fn() -> Result<()>`,
/// so async golden tasks need their own runtime rather than relying on an
/// ambient one).
fn agent_loop_terminates_task() -> Result<()> {
    // `run()` (the caller, transitively) is itself async and — under
    // `#[tokio::test]`'s default current-thread flavor — may already be
    // running inside a tokio runtime, so a nested `Runtime::block_on` here
    // would panic ("Cannot start a runtime from within a runtime"). Run the
    // async check on its own OS thread with its own runtime instead; this
    // works regardless of the caller's runtime flavor.
    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(
            vox_orchestrator_mcp::chat_tools::chat::agent_loop::eval_gate_agent_loop_terminates_check(
            ),
        )
        .map_err(|e| anyhow::anyhow!(e))
    })
    .join()
    .map_err(|_| anyhow::anyhow!("agent-loop-terminates check thread panicked"))?
}

/// Golden task 7: the privacy hard filter must block cloud-provider routing
/// under `local_only` and allow local-provider routing — the property Task
/// 2.2 established. Runs the same pure decision core `privacy_allows` uses,
/// with an explicit mode rather than mutating `VOX_INFERENCE_PRIVACY` (kept
/// hermetic and safe under parallel test/task execution).
fn privacy_filter_blocks_live_routing_task() -> Result<()> {
    vox_orchestrator_mcp::llm_bridge::local_health::eval_gate_privacy_filter_check()
        .map_err(|e| anyhow::anyhow!(e))
}

/// Result of running (or skipping) one golden task.
enum TaskStatus {
    /// All samples ran and all passed (pass^k).
    Passed { passes: usize, samples: usize },
    /// All samples ran but at least one failed.
    Failed {
        passes: usize,
        samples: usize,
        first_failure: String,
    },
    /// The task's `skip_if` predicate opted it out before any sample ran.
    /// Excluded entirely from the pass^k gate determination — a skipped
    /// task can never count toward, or against, the gate passing.
    Skipped { reason: String },
}

/// One named task's [`TaskStatus`].
struct TaskOutcome {
    name: &'static str,
    status: TaskStatus,
}

impl TaskOutcome {
    /// pass^k: all samples must pass for the task to count as passing.
    /// A skipped task is neither passing nor failing the gate.
    fn passed_pow_k(&self) -> bool {
        matches!(self.status, TaskStatus::Passed { .. })
    }

    fn is_skipped(&self) -> bool {
        matches!(self.status, TaskStatus::Skipped { .. })
    }
}

/// Same `git_sha` validation `ingest_runs` applies at write time — re-checked here as a
/// defense-in-depth boundary, since this function is the one that actually constructs a `git`
/// subprocess call from a stored value. A well-formed value will always pass; this only ever
/// rejects something that slipped through some other write path.
fn is_valid_git_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

pub async fn run(args: EvalArgs) -> anyhow::Result<()> {
    if args.live {
        let (mut run, task_results, selection_events) =
            crate::commands::harness::live_eval::run_live(args.samples, args.task.as_deref())
                .await?;
        println!(
            "{} live run {}: {}/{} tasks passed, {} skipped, ${:.4} spent",
            " HARNESS EVAL (LIVE) ".on_blue().white().bold(),
            run.run_id,
            run.pass_count,
            run.task_count,
            run.skip_count,
            run.total_cost_usd
        );

        let db = vox_db::open_project_db().await?;
        // `changed_files` is left empty by `run_live` (which has no DB handle); compute it here
        // by diffing against the immediately-preceding run's `git_sha`. That `git_sha` may have
        // originated from a `publish`-ingested JSONL file rather than a local run, but
        // `publish::ingest_runs` validates the shape of every `git_sha` before it ever enters
        // vox-db, so it can be trusted here without re-validating.
        if let Some(previous) = db.list_harness_eval_runs(1).await?.into_iter().next() {
            if previous.git_sha != run.git_sha
                && is_valid_git_sha(&previous.git_sha)
                && is_valid_git_sha(&run.git_sha)
            {
                let diff_output = std::process::Command::new("git")
                    .args([
                        "diff",
                        "--name-only",
                        &format!("{}..{}", previous.git_sha, run.git_sha),
                    ])
                    .output();
                if let Ok(out) = diff_output {
                    run.changed_files = String::from_utf8_lossy(&out.stdout)
                        .lines()
                        .map(str::to_string)
                        .collect();
                }
            }
        }
        db.record_harness_eval_run(&run).await?;
        for task_result in &task_results {
            db.record_harness_eval_task_result(task_result).await?;
        }
        for event in &selection_events {
            db.record_model_selection_event(event).await?;
        }

        if run.fail_count > 0 {
            anyhow::bail!(
                "harness eval --live gate failed: {}/{} tasks did not pass",
                run.fail_count,
                run.task_count
            );
        }
        return Ok(());
    }

    if args.samples == 0 {
        bail!("--samples must be at least 1");
    }
    if args.samples > MAX_SAMPLES {
        bail!(
            "--samples {} exceeds the maximum of {MAX_SAMPLES}; this cap exists so a typo'd \
             large value can't trigger a silent long-running hang",
            args.samples
        );
    }

    let tasks: Vec<GoldenTask> = golden_tasks()
        .into_iter()
        .filter(|t| args.task.as_deref().is_none_or(|filter| filter == t.name))
        .collect();

    if tasks.is_empty() {
        bail!(
            "no golden task matched --task {:?}",
            args.task.unwrap_or_default()
        );
    }

    println!(
        "{} Running {} golden task(s) x {} sample(s) each (pass^k gate)...",
        " HARNESS EVAL ".on_blue().white().bold(),
        tasks.len(),
        args.samples
    );

    let mut outcomes = Vec::with_capacity(tasks.len());
    for task in &tasks {
        if let Some(skip_reason) = task.skip_if.and_then(|f| f()) {
            outcomes.push(TaskOutcome {
                name: task.name,
                status: TaskStatus::Skipped {
                    reason: skip_reason,
                },
            });
            continue;
        }

        let mut passes = 0;
        let mut first_failure = None;
        for _ in 0..args.samples {
            match (task.run)() {
                Ok(()) => passes += 1,
                Err(e) if first_failure.is_none() => first_failure = Some(e.to_string()),
                Err(_) => {}
            }
        }
        let status = if passes == args.samples {
            TaskStatus::Passed {
                passes,
                samples: args.samples,
            }
        } else {
            TaskStatus::Failed {
                passes,
                samples: args.samples,
                first_failure: first_failure.unwrap_or_default(),
            }
        };
        outcomes.push(TaskOutcome {
            name: task.name,
            status,
        });
    }

    let mut any_failed = false;
    for outcome in &outcomes {
        match &outcome.status {
            TaskStatus::Passed { passes, samples } => {
                println!(
                    "  [{}] {} — {passes}/{samples} samples passed",
                    "PASS".green(),
                    outcome.name
                );
            }
            TaskStatus::Failed {
                passes,
                samples,
                first_failure,
            } => {
                any_failed = true;
                println!(
                    "  [{}] {} — {passes}/{samples} samples passed",
                    "FAIL".red(),
                    outcome.name
                );
                println!("         first failure: {}", first_failure.dimmed());
            }
            TaskStatus::Skipped { reason } => {
                println!(
                    "  [{}] {} — {}",
                    "SKIP".yellow(),
                    outcome.name,
                    reason.dimmed()
                );
            }
        }
    }

    let passed_tasks = outcomes.iter().filter(|o| o.passed_pow_k()).count();
    let skipped_tasks = outcomes.iter().filter(|o| o.is_skipped()).count();
    let gated_tasks = outcomes.len() - skipped_tasks;
    let gate_passed = !any_failed;

    println!(
        "\n{} {}/{} golden task(s) passed pass^{} gate{}.",
        if gate_passed {
            "PASS".green().bold().to_string()
        } else {
            "FAIL".red().bold().to_string()
        },
        passed_tasks,
        gated_tasks,
        args.samples,
        if skipped_tasks > 0 {
            format!(" ({skipped_tasks} skipped, not counted toward the gate)")
        } else {
            String::new()
        }
    );

    if !gate_passed {
        bail!(
            "harness eval gate failed: {}/{} golden tasks did not achieve pass^{}",
            gated_tasks - passed_tasks,
            gated_tasks,
            args.samples
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes the handful of tests that read/write `VOX_HARNESS_EVAL_LIVE`
    /// (a process-global) so they cannot race each other under cargo's
    /// default parallel test execution.
    static LIVE_ENV_VAR_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn model_eval_score_fold_exact_is_deterministic() {
        for _ in 0..5 {
            assert!(model_eval_score_fold_exact().is_ok());
        }
    }

    #[test]
    fn cli_harness_eval_arg_parsing_checks_defaults_and_overrides() {
        assert!(cli_harness_eval_arg_parsing().is_ok());
    }

    #[test]
    fn temp_workspace_file_roundtrip_is_byte_exact() {
        assert!(temp_workspace_file_roundtrip().is_ok());
    }

    #[test]
    fn tool_cap_never_exceeds_cap_task_passes() {
        let result = tool_cap_never_exceeds_cap_task();
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn agent_loop_terminates_task_passes() {
        let result = agent_loop_terminates_task();
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn privacy_filter_blocks_live_routing_task_passes() {
        let result = privacy_filter_blocks_live_routing_task();
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn live_model_smoke_skip_reason_is_some_without_env_var() {
        let _guard = LIVE_ENV_VAR_LOCK.lock().unwrap();
        // SAFETY: guarded by LIVE_ENV_VAR_LOCK; no other test touches this
        // var without holding the same lock.
        unsafe {
            std::env::remove_var("VOX_HARNESS_EVAL_LIVE");
        }
        // Absence (the default, and CI's state) must resolve to a skip
        // reason, so `run()` never even calls the (currently unimplemented)
        // live-model task body.
        assert!(live_model_smoke_skip_reason().is_some());
    }

    #[test]
    fn live_model_smoke_task_errors_rather_than_silently_passing() {
        // Even if something ever called the task body directly (bypassing
        // skip_if), it must never report a free PASS for an unimplemented
        // check — see the no-stubs rule this task was flagged against.
        assert!(live_model_smoke_task().is_err());
    }

    #[test]
    fn task_outcome_pass_pow_k_requires_all_samples_to_pass() {
        let all_pass = TaskOutcome {
            name: "t",
            status: TaskStatus::Passed {
                passes: 3,
                samples: 3,
            },
        };
        assert!(all_pass.passed_pow_k());
        assert!(!all_pass.is_skipped());

        let one_fail = TaskOutcome {
            name: "t",
            status: TaskStatus::Failed {
                passes: 2,
                samples: 3,
                first_failure: "boom".to_string(),
            },
        };
        assert!(
            !one_fail.passed_pow_k(),
            "pass^k must fail the task if even one sample failed"
        );
        assert!(!one_fail.is_skipped());
    }

    #[test]
    fn task_outcome_skipped_counts_as_neither_pass_nor_gate_failure() {
        let skipped = TaskOutcome {
            name: "t",
            status: TaskStatus::Skipped {
                reason: "disabled".to_string(),
            },
        };
        assert!(!skipped.passed_pow_k());
        assert!(skipped.is_skipped());
    }

    #[tokio::test]
    async fn run_all_golden_tasks_succeeds_with_default_samples() {
        let _guard = LIVE_ENV_VAR_LOCK.lock().unwrap();
        // SAFETY: guarded by LIVE_ENV_VAR_LOCK.
        unsafe {
            std::env::remove_var("VOX_HARNESS_EVAL_LIVE");
        }
        // live-model-smoke is SKIPPED by default (VOX_HARNESS_EVAL_LIVE
        // unset) and therefore excluded from the gate, so the other three
        // hermetic tasks passing is sufficient for the gate to pass.
        let args = EvalArgs {
            samples: 3,
            task: None,
            live: false,
        };
        assert!(run(args).await.is_ok());
    }

    #[tokio::test]
    async fn run_fails_the_gate_when_live_task_is_forced_to_run_unimplemented() {
        let _guard = LIVE_ENV_VAR_LOCK.lock().unwrap();
        // SAFETY: guarded by LIVE_ENV_VAR_LOCK; no other test touches this
        // var without holding the same lock.
        unsafe {
            std::env::set_var("VOX_HARNESS_EVAL_LIVE", "1");
        }
        let args = EvalArgs {
            samples: 1,
            task: Some("live-model-smoke".to_string()),
            live: false,
        };
        let result = run(args).await;
        // SAFETY: see above.
        unsafe {
            std::env::remove_var("VOX_HARNESS_EVAL_LIVE");
        }
        assert!(
            result.is_err(),
            "forcing the unimplemented live task to actually run must fail the gate, not pass it for free"
        );
    }

    #[tokio::test]
    async fn run_rejects_zero_samples() {
        let args = EvalArgs {
            samples: 0,
            task: None,
            live: false,
        };
        assert!(run(args).await.is_err());
    }

    #[tokio::test]
    async fn run_rejects_samples_above_max() {
        let args = EvalArgs {
            samples: MAX_SAMPLES + 1,
            task: None,
            live: false,
        };
        assert!(run(args).await.is_err());
    }

    #[tokio::test]
    async fn run_accepts_samples_at_max() {
        let args = EvalArgs {
            samples: MAX_SAMPLES,
            task: Some("temp-workspace-file-roundtrip".to_string()),
            live: false,
        };
        assert!(run(args).await.is_ok());
    }

    #[tokio::test]
    async fn run_rejects_unknown_task_filter() {
        let args = EvalArgs {
            samples: 1,
            task: Some("does-not-exist".to_string()),
            live: false,
        };
        assert!(run(args).await.is_err());
    }

    #[tokio::test]
    async fn run_filters_to_single_named_task() {
        let args = EvalArgs {
            samples: 2,
            task: Some("temp-workspace-file-roundtrip".to_string()),
            live: false,
        };
        assert!(run(args).await.is_ok());
    }
}
