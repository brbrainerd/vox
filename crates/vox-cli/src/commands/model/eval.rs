//! `vox models eval` — a small, deterministic benchmark that makes the three model
//! axes (intelligence / token-efficiency / responsiveness) measurable and data-backed.
//!
//! The harness runs a handful of exact-match fixtures against each candidate model
//! through the [`vox_actor_runtime::llm`] chat facade, then folds the per-call results
//! into three axis scores via the pure [`score_eval`] function (unit-tested offline).
//! Results are written back into the `model_scoreboard` (`quality_score = pass_rate`,
//! replacing the 1.0 placeholder) and the observed p50 is injected into the live
//! registry capability used by the latency scorer.

use clap::Parser;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use vox_actor_runtime::ActivityOptions;
use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig, llm_chat};
use vox_db::store::types::ModelScoreboardRow;
use vox_db::{DbConfig, VoxDb, now_unix_ms};
use vox_orchestrator::models::ModelRegistry;

/// One deterministic benchmark fixture with an exact/substring checkable answer.
struct Fixture {
    /// The user prompt sent to the model.
    prompt: &'static str,
    /// A lowercased substring that must appear in the completion for it to count as correct.
    expect_substring: &'static str,
}

/// The small built-in benchmark. Intentionally not dependent on the (empty)
/// `contracts/eval/humaneval-vox` corpus; these are self-contained and
/// deterministically checkable.
const FIXTURES: &[Fixture] = &[
    Fixture {
        prompt: "What is 17 + 25? Reply with only the number.",
        expect_substring: "42",
    },
    Fixture {
        prompt: "What is 6 multiplied by 7? Reply with only the number.",
        expect_substring: "42",
    },
    Fixture {
        prompt: "What is 100 minus 1? Reply with only the number.",
        expect_substring: "99",
    },
    Fixture {
        prompt: "What is the capital city of France? Reply with only the city name.",
        expect_substring: "paris",
    },
    Fixture {
        prompt: "What is the chemical symbol for water? Reply with only the formula.",
        expect_substring: "h2o",
    },
    Fixture {
        prompt: "Complete the sequence with the next number only: 2, 4, 6, 8, ...",
        expect_substring: "10",
    },
    Fixture {
        prompt: "In one word, what color do you get by mixing blue and yellow paint?",
        expect_substring: "green",
    },
    Fixture {
        prompt: "Output a JSON object with a single key \"status\" whose value is the string \"ok\". \
             Reply with only the JSON.",
        expect_substring: "\"status\"",
    },
    Fixture {
        prompt: "Write a Python expression (no statement, no print) that returns the length of the \
             string 'hello'. Reply with only the expression.",
        expect_substring: "len('hello')",
    },
    Fixture {
        prompt: "What year did the first human land on the Moon? Reply with only the year.",
        expect_substring: "1969",
    },
];

/// One per-fixture outcome captured from a live (or skipped) model call.
#[derive(Debug, Clone)]
pub struct FixtureResult {
    /// Whether the completion satisfied the fixture's exact/substring check.
    pub correct: bool,
    /// Prompt + completion tokens reported by the provider.
    pub total_tokens: u32,
    /// Wall-clock latency of the call in milliseconds.
    pub latency_ms: i64,
    /// Cost of this call in USD as reported by the LLM facade
    /// (`LlmResponse::cost_usd`): provider-reported `total_cost` when present,
    /// else a `cost_per_1k` estimate, else `None` (unknown / free mock backend).
    pub cost_usd: Option<f64>,
}

/// The three folded axis scores for a single model.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEvalResult {
    /// INTELLIGENCE: fraction of fixtures answered correctly (0.0..=1.0).
    pub intelligence: f64,
    /// TOKENS-EFFICIENCY: total tokens divided by max(passed, 1) (lower is better).
    pub tokens_per_pass: f64,
    /// Raw total tokens across all calls (stored alongside the ratio).
    pub total_tokens: u64,
    /// RESPONSIVENESS: true p50 latency in ms over the calls.
    pub p50_ms: i64,
    /// RESPONSIVENESS: true p99 latency in ms over the calls.
    pub p99_ms: i64,
    /// Number of fixtures that ran (excludes skipped/unreachable).
    pub n_calls: usize,
    /// Number of fixtures answered correctly.
    pub passed: usize,
    /// Sum of per-call `cost_usd` across all fixtures with a known cost (USD).
    /// Calls whose cost was `None` (e.g. the free mock backend) contribute 0.
    pub cumulative_cost_usd: f64,
    /// Cost in USD per *successful* answer (`cumulative_cost_usd / passed`), or
    /// `None` when no fixtures passed or no call reported a cost.
    pub cost_per_success_usd: Option<f64>,
}

/// Pure scoring fold: given per-fixture results, compute the three axis scores.
///
/// This is the unit-tested core; the live LLM calls are a thin shell around it.
#[must_use]
pub fn score_eval(results: &[FixtureResult]) -> ModelEvalResult {
    let n_calls = results.len();
    let passed = results.iter().filter(|r| r.correct).count();
    let total_tokens: u64 = results.iter().map(|r| r.total_tokens as u64).sum();

    let intelligence = if n_calls == 0 {
        0.0
    } else {
        passed as f64 / n_calls as f64
    };

    // Lower is better: total tokens per *successful* answer (avoid div-by-zero).
    let tokens_per_pass = total_tokens as f64 / passed.max(1) as f64;

    let mut latencies: Vec<i64> = results.iter().map(|r| r.latency_ms).collect();
    latencies.sort_unstable();
    let p50_ms = percentile(&latencies, 0.50);
    let p99_ms = percentile(&latencies, 0.99);

    // Cost fold: sum known per-call costs; unknown (None) calls contribute 0.
    // `cost_per_success_usd` is None when nothing passed or no cost was reported,
    // so the scoreboard distinguishes "free/unknown" from a real "$0.00".
    let cumulative_cost_usd: f64 = results.iter().filter_map(|r| r.cost_usd).sum();
    let any_cost_known = results.iter().any(|r| r.cost_usd.is_some());
    let cost_per_success_usd = if passed > 0 && any_cost_known {
        Some(cumulative_cost_usd / passed as f64)
    } else {
        None
    };

    ModelEvalResult {
        intelligence,
        tokens_per_pass,
        total_tokens,
        p50_ms,
        p99_ms,
        n_calls,
        passed,
        cumulative_cost_usd,
        cost_per_success_usd,
    }
}

/// Nearest-rank percentile over a pre-sorted ascending slice. Returns 0 when empty.
fn percentile(sorted: &[i64], q: f64) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    // Nearest-rank: ceil(q * n) clamped into [1, n], then 0-indexed.
    let n = sorted.len();
    let rank = (q * n as f64).ceil() as usize;
    let idx = rank.clamp(1, n) - 1;
    sorted[idx]
}

/// Build the scoreboard row that records this eval run for `model_id`.
///
/// `quality_score` is set to the measured pass-rate, replacing the 1.0 placeholder
/// that the telemetry rollup writes absent human feedback.
#[must_use]
pub fn scoreboard_row_from_eval(
    model_id: &str,
    task_category: &str,
    result: &ModelEvalResult,
) -> ModelScoreboardRow {
    let now = now_unix_ms() as i64;
    ModelScoreboardRow {
        model_id: model_id.to_string(),
        task_category: task_category.to_string(),
        strength_tag: "eval".to_string(),
        window_days: 7,
        n_calls: result.n_calls as i64,
        success_rate: result.intelligence,
        p50_latency_ms: Some(result.p50_ms),
        p99_latency_ms: Some(result.p99_ms),
        cost_per_success_usd: result.cost_per_success_usd,
        quality_score: result.intelligence,
        updated_at_ms: now,
        success_count: result.passed as i64,
        cumulative_cost_usd: result.cumulative_cost_usd,
        p95_ttft_ms: None,
        p95_tpot_ms: None,
        goodput_tokens_per_sec: None,
    }
}

/// Per-model row for the output table (or a skip note).
enum Row {
    Scored {
        model_id: String,
        result: ModelEvalResult,
        wrote_back: bool,
    },
    Skipped {
        model_id: String,
        reason: String,
    },
}

/// `vox models eval` arguments.
#[derive(Parser)]
pub struct EvalArgs {
    /// Evaluate only these model ids (repeatable). Defaults to all registry models.
    #[arg(long = "model")]
    pub models: Vec<String>,
    /// Task category recorded on the scoreboard write-back.
    #[arg(long, default_value = "general")]
    pub category: String,
    /// Skip the scoreboard / registry write-back (dry run; still prints scores).
    #[arg(long, default_value_t = false)]
    pub no_write_back: bool,
    /// Write a per-run JSON artifact to this path.
    #[arg(long)]
    pub output: Option<PathBuf>,
}

pub async fn run(args: EvalArgs) -> anyhow::Result<()> {
    let registry = ModelRegistry::new();

    // Resolve the candidate set: explicit --model list, or all registry models.
    let candidates: Vec<String> = if args.models.is_empty() {
        let mut ids: Vec<String> = registry.list_models().into_iter().map(|m| m.id).collect();
        ids.sort();
        ids.dedup();
        ids
    } else {
        args.models.clone()
    };

    if candidates.is_empty() {
        println!("{}", "No candidate models found in the registry.".yellow());
        return Ok(());
    }

    let db = match VoxDb::connect(DbConfig::resolve_canonical().map_err(anyhow::Error::msg)?).await
    {
        Ok(db) => Some(db),
        Err(e) => {
            tracing::warn!(error = %e, "eval: DB unavailable; running without write-back");
            None
        }
    };

    println!(
        "{} Running {} fixtures against {} model(s)...",
        " EVAL ".on_blue().white().bold(),
        FIXTURES.len(),
        candidates.len()
    );

    let mut rows: Vec<Row> = Vec::new();

    for model_id in &candidates {
        match eval_one_model(model_id).await {
            Ok(results) => {
                let result = score_eval(&results);
                let mut wrote_back = false;
                if !args.no_write_back {
                    if let Some(db) = &db {
                        let sb_row = scoreboard_row_from_eval(model_id, &args.category, &result);
                        match db.upsert_model_scoreboard(sb_row).await {
                            Ok(()) => wrote_back = true,
                            Err(e) => {
                                tracing::warn!(error = %e, model_id, "eval: scoreboard upsert failed")
                            }
                        }
                    }
                }
                rows.push(Row::Scored {
                    model_id: model_id.clone(),
                    result,
                    wrote_back,
                });
            }
            Err(reason) => rows.push(Row::Skipped {
                model_id: model_id.clone(),
                reason,
            }),
        }
    }

    print_table(&rows);
    if let Some(path) = &args.output {
        write_artifact(path, &args.category, &rows)?;
        println!("Wrote eval artifact to {}", path.display().bright_cyan());
    }

    Ok(())
}

/// Run all fixtures for a single model through the LLM facade. Returns `Err(reason)`
/// when the model is unreachable (no key / first call fails) so the caller can record
/// a skipped row instead of crashing.
async fn eval_one_model(model_id: &str) -> Result<Vec<FixtureResult>, String> {
    let mut results = Vec::with_capacity(FIXTURES.len());
    let opts = ActivityOptions::new();

    for (i, fixture) in FIXTURES.iter().enumerate() {
        let mut config = LlmConfig::openrouter(model_id);
        config.max_tokens = Some(64);
        config.temperature = Some(0.0);
        config.telemetry_task_category = Some("eval".to_string());

        let messages = vec![LlmChatMessage {
            role: "user".to_string(),
            content: fixture.prompt.to_string(),
            ..Default::default()
        }];

        let started = std::time::Instant::now();
        let outcome = llm_chat(&opts, messages, config).await;
        let latency_ms = started.elapsed().as_millis() as i64;

        // Flatten the durable-activity outcome into Ok(response) / Err(reason).
        let flattened: Result<vox_actor_runtime::llm::LlmResponse, String> = match outcome {
            vox_actor_runtime::ActivityResult::Ok(inner) => inner,
            vox_actor_runtime::ActivityResult::Failed(e) => Err(e.to_string()),
            vox_actor_runtime::ActivityResult::Cancelled => Err("activity cancelled".to_string()),
        };

        match flattened {
            Ok(resp) => {
                let correct = resp
                    .content
                    .to_ascii_lowercase()
                    .contains(fixture.expect_substring);
                results.push(FixtureResult {
                    correct,
                    total_tokens: resp.prompt_tokens + resp.completion_tokens,
                    latency_ms,
                    cost_usd: resp.cost_usd,
                });
            }
            Err(provider_err) => {
                // On the very first fixture, treat a provider error as "unreachable" and
                // skip the whole model. Mid-run errors are recorded as incorrect calls.
                if i == 0 {
                    return Err(provider_err);
                }
                results.push(FixtureResult {
                    correct: false,
                    total_tokens: 0,
                    latency_ms,
                    cost_usd: None,
                });
            }
        }
    }

    Ok(results)
}

fn print_table(rows: &[Row]) {
    use comfy_table::Table;
    let mut table = Table::new();
    table.set_header(vec![
        "Model ID",
        "Intelligence",
        "Tokens/Pass",
        "p50 ms",
        "p99 ms",
        "Write-back",
    ]);
    for row in rows {
        match row {
            Row::Scored {
                model_id,
                result,
                wrote_back,
            } => {
                table.add_row(vec![
                    model_id.clone(),
                    format!(
                        "{:.0}% ({}/{})",
                        result.intelligence * 100.0,
                        result.passed,
                        result.n_calls
                    ),
                    format!("{:.1}", result.tokens_per_pass),
                    result.p50_ms.to_string(),
                    result.p99_ms.to_string(),
                    if *wrote_back {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    },
                ]);
            }
            Row::Skipped { model_id, reason } => {
                table.add_row(vec![
                    model_id.clone(),
                    "skipped".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    "-".to_string(),
                    reason.chars().take(40).collect::<String>(),
                ]);
            }
        }
    }
    println!("{}", table);
}

fn write_artifact(path: &std::path::Path, category: &str, rows: &[Row]) -> anyhow::Result<()> {
    let models: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| match row {
            Row::Scored {
                model_id,
                result,
                wrote_back,
            } => serde_json::json!({
                "model_id": model_id,
                "status": "scored",
                "intelligence": result.intelligence,
                "tokens_per_pass": result.tokens_per_pass,
                "total_tokens": result.total_tokens,
                "p50_ms": result.p50_ms,
                "p99_ms": result.p99_ms,
                "n_calls": result.n_calls,
                "passed": result.passed,
                "cumulative_cost_usd": result.cumulative_cost_usd,
                "cost_per_success_usd": result.cost_per_success_usd,
                "wrote_back": wrote_back,
            }),
            Row::Skipped { model_id, reason } => serde_json::json!({
                "model_id": model_id,
                "status": "skipped",
                "reason": reason,
            }),
        })
        .collect();

    let report = serde_json::json!({
        "schema_version": 1,
        "suite": "models-eval-builtin",
        "task_category": category,
        "fixture_count": FIXTURES.len(),
        "recorded_at_ms": now_unix_ms(),
        "models": models,
    });

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(correct: bool, tokens: u32, latency: i64) -> FixtureResult {
        FixtureResult {
            correct,
            total_tokens: tokens,
            latency_ms: latency,
            cost_usd: None,
        }
    }

    fn rc(correct: bool, tokens: u32, latency: i64, cost: Option<f64>) -> FixtureResult {
        FixtureResult {
            correct,
            total_tokens: tokens,
            latency_ms: latency,
            cost_usd: cost,
        }
    }

    #[test]
    fn score_eval_folds_cost_summing_known_and_ignoring_unknown() {
        // Two passing calls with cost, one passing free (None), one failing.
        let results = vec![
            rc(true, 10, 100, Some(0.02)),
            rc(true, 20, 200, Some(0.03)),
            rc(true, 30, 300, None), // free/unknown -> contributes 0
            rc(false, 40, 400, Some(0.05)),
        ];
        let out = score_eval(&results);
        // 0.02 + 0.03 + 0.05 = 0.10 cumulative (None ignored).
        assert!((out.cumulative_cost_usd - 0.10).abs() < 1e-9);
        // 3 passed; cost per success = 0.10 / 3.
        let cps = out.cost_per_success_usd.expect("some cost known");
        assert!((cps - (0.10 / 3.0)).abs() < 1e-9);
    }

    #[test]
    fn score_eval_cost_per_success_none_when_all_costs_unknown() {
        let results = vec![rc(true, 10, 100, None), rc(false, 20, 200, None)];
        let out = score_eval(&results);
        assert_eq!(out.cumulative_cost_usd, 0.0);
        assert!(
            out.cost_per_success_usd.is_none(),
            "no known cost -> None, not a misleading $0.00"
        );
    }

    #[test]
    fn score_eval_folds_three_axes_from_synthetic_results() {
        // 3 of 4 correct; tokens 10+20+30+40 = 100 over 3 passes.
        let results = vec![
            r(true, 10, 100),
            r(true, 20, 200),
            r(false, 30, 300),
            r(true, 40, 400),
        ];
        let out = score_eval(&results);

        assert_eq!(out.n_calls, 4);
        assert_eq!(out.passed, 3);
        assert!((out.intelligence - 0.75).abs() < 1e-9, "3/4 correct");
        assert_eq!(out.total_tokens, 100);
        assert!(
            (out.tokens_per_pass - (100.0 / 3.0)).abs() < 1e-9,
            "tokens per passing answer"
        );
        // Nearest-rank p50 over [100,200,300,400]: ceil(0.5*4)=2 -> index 1 -> 200.
        assert_eq!(out.p50_ms, 200);
        // p99: ceil(0.99*4)=4 -> index 3 -> 400.
        assert_eq!(out.p99_ms, 400);
    }

    #[test]
    fn score_eval_handles_zero_passes_without_div_by_zero() {
        let results = vec![r(false, 50, 10), r(false, 50, 20)];
        let out = score_eval(&results);
        assert_eq!(out.intelligence, 0.0);
        // tokens_per_pass uses max(passed,1) = 1 -> 100 / 1.
        assert_eq!(out.tokens_per_pass, 100.0);
        assert_eq!(out.p50_ms, 10);
    }

    #[test]
    fn score_eval_empty_is_zeroed() {
        let out = score_eval(&[]);
        assert_eq!(out.intelligence, 0.0);
        assert_eq!(out.tokens_per_pass, 0.0);
        assert_eq!(out.p50_ms, 0);
        assert_eq!(out.p99_ms, 0);
        assert_eq!(out.n_calls, 0);
    }

    #[test]
    fn scoreboard_write_back_sets_quality_to_pass_rate() {
        let results = vec![r(true, 10, 100), r(true, 20, 200), r(false, 30, 300)];
        let out = score_eval(&results);
        let row = scoreboard_row_from_eval("test/model", "general", &out);

        assert_eq!(row.model_id, "test/model");
        assert_eq!(row.task_category, "general");
        // quality_score replaces the 1.0 placeholder with the measured pass-rate.
        assert!((row.quality_score - (2.0 / 3.0)).abs() < 1e-9);
        assert!((row.success_rate - (2.0 / 3.0)).abs() < 1e-9);
        assert_eq!(row.success_count, 2);
        assert_eq!(row.n_calls, 3);
        assert_eq!(row.p50_latency_ms, Some(out.p50_ms));
        assert_eq!(row.p99_latency_ms, Some(out.p99_ms));
    }
}
