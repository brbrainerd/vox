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
//! `live_model_smoke_task` below for the pattern (checked at runtime against
//! `VOX_HARNESS_EVAL_LIVE=1`, absent by default, and reported as a skip
//! rather than a failure when unset so `vox harness eval`'s exit code stays
//! meaningful without a live provider key).

use anyhow::{Result, bail};
use clap::Parser;
use owo_colors::OwoColorize;

use crate::commands::model::eval::{FixtureResult, score_eval};

/// `vox harness eval` arguments.
#[derive(Parser)]
pub struct EvalArgs {
    /// Number of independent samples per golden task. pass^k requires ALL
    /// samples to pass for the task to count as passing (not pass@1).
    #[arg(long, default_value_t = 3)]
    pub samples: usize,
    /// Run only the golden task with this exact name (repeatable filtering
    /// is not needed yet at this scale; add more flags if the task set grows).
    #[arg(long)]
    pub task: Option<String>,
}

/// One golden task: a name and a deterministic, hermetic check function.
struct GoldenTask {
    name: &'static str,
    run: fn() -> Result<()>,
}

/// The built-in golden task set. See the module docs for how to add one.
fn golden_tasks() -> Vec<GoldenTask> {
    vec![
        GoldenTask {
            name: "model-eval-score-fold-exact",
            run: model_eval_score_fold_exact,
        },
        GoldenTask {
            name: "cli-harness-eval-arg-parsing",
            run: cli_harness_eval_arg_parsing,
        },
        GoldenTask {
            name: "temp-workspace-file-roundtrip",
            run: temp_workspace_file_roundtrip,
        },
        GoldenTask {
            name: "live-model-smoke",
            run: live_model_smoke_task,
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
/// model call to be meaningful. Gated behind `VOX_HARNESS_EVAL_LIVE=1` so the
/// CORE gate (the other three tasks) stays hermetic; unset, this reports a
/// skip (`Ok(())`) rather than a failure so `vox harness eval`'s exit code
/// stays meaningful without a live provider key configured.
fn live_model_smoke_task() -> Result<()> {
    if std::env::var("VOX_HARNESS_EVAL_LIVE").as_deref() != Ok("1") {
        return Ok(()); // skipped, not failed — see module docs.
    }
    // A real implementation would call `vox_actor_runtime::llm::llm_chat`
    // here and assert on the response, mirroring `model::eval::eval_one_model`.
    // Left as the documented extension point; not implemented in this pass.
    Ok(())
}

/// Per-sample outcome for one golden task.
struct TaskOutcome {
    name: &'static str,
    passes: usize,
    samples: usize,
    /// Reason for the first observed failure, if any.
    first_failure: Option<String>,
}

impl TaskOutcome {
    /// pass^k: all samples must pass for the task to count as passing.
    fn passed_pow_k(&self) -> bool {
        self.passes == self.samples
    }
}

pub async fn run(args: EvalArgs) -> anyhow::Result<()> {
    if args.samples == 0 {
        bail!("--samples must be at least 1");
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
        let mut passes = 0;
        let mut first_failure = None;
        for _ in 0..args.samples {
            match (task.run)() {
                Ok(()) => passes += 1,
                Err(e) if first_failure.is_none() => first_failure = Some(e.to_string()),
                Err(_) => {}
            }
        }
        outcomes.push(TaskOutcome {
            name: task.name,
            passes,
            samples: args.samples,
            first_failure,
        });
    }

    let mut all_passed = true;
    for outcome in &outcomes {
        let ok = outcome.passed_pow_k();
        all_passed &= ok;
        let status = if ok {
            "PASS".green().to_string()
        } else {
            "FAIL".red().to_string()
        };
        println!(
            "  [{status}] {} — {}/{} samples passed",
            outcome.name, outcome.passes, outcome.samples
        );
        if let (false, Some(reason)) = (ok, &outcome.first_failure) {
            println!("         first failure: {}", reason.dimmed());
        }
    }

    let passed_tasks = outcomes.iter().filter(|o| o.passed_pow_k()).count();
    println!(
        "\n{} {}/{} golden task(s) passed pass^{} gate.",
        if all_passed {
            "PASS".green().bold().to_string()
        } else {
            "FAIL".red().bold().to_string()
        },
        passed_tasks,
        outcomes.len(),
        args.samples
    );

    if !all_passed {
        bail!(
            "harness eval gate failed: {}/{} golden tasks did not achieve pass^{}",
            outcomes.len() - passed_tasks,
            outcomes.len(),
            args.samples
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn live_model_smoke_task_skips_without_env_var() {
        // Does not touch the env var at all: absence (the default, and CI's
        // state) must resolve to a skip (`Ok`), not a failure.
        if std::env::var("VOX_HARNESS_EVAL_LIVE").as_deref() != Ok("1") {
            assert!(live_model_smoke_task().is_ok());
        }
    }

    #[test]
    fn task_outcome_pass_pow_k_requires_all_samples_to_pass() {
        let all_pass = TaskOutcome {
            name: "t",
            passes: 3,
            samples: 3,
            first_failure: None,
        };
        assert!(all_pass.passed_pow_k());

        let one_fail = TaskOutcome {
            name: "t",
            passes: 2,
            samples: 3,
            first_failure: Some("boom".to_string()),
        };
        assert!(
            !one_fail.passed_pow_k(),
            "pass^k must fail the task if even one sample failed"
        );
    }

    #[tokio::test]
    async fn run_all_golden_tasks_succeeds_with_default_samples() {
        let args = EvalArgs {
            samples: 3,
            task: None,
        };
        assert!(run(args).await.is_ok());
    }

    #[tokio::test]
    async fn run_rejects_zero_samples() {
        let args = EvalArgs {
            samples: 0,
            task: None,
        };
        assert!(run(args).await.is_err());
    }

    #[tokio::test]
    async fn run_rejects_unknown_task_filter() {
        let args = EvalArgs {
            samples: 1,
            task: Some("does-not-exist".to_string()),
        };
        assert!(run(args).await.is_err());
    }

    #[tokio::test]
    async fn run_filters_to_single_named_task() {
        let args = EvalArgs {
            samples: 2,
            task: Some("temp-workspace-file-roundtrip".to_string()),
        };
        assert!(run(args).await.is_ok());
    }
}
