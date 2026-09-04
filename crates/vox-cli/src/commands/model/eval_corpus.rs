//! `vox model eval-corpus` — score one (model, harness, condition) tuple
//! against the held-out HumanEval-Vox corpus.
//!
//! Two input modes:
//!   * `--from-dir <dir>`: score pre-generated solutions. This is how external
//!     harnesses (Claude Code, Cursor, Warp, Grok) enter the leaderboard — we
//!     cannot drive their UIs, but we can score their output with the identical
//!     verifier, which is what makes the comparison fair. Tokens/latency/cost
//!     are `None` for these rows, never a fabricated `0`.
//!   * live generation (default): generate through the `vox_actor_runtime::llm`
//!     facade for a registry model id, drawing `n` samples per fixture with NO
//!     early stop (the unbiased pass@k estimator needs every outcome).
//!
//! Correctness comes from `vox_corpus::humaneval_runner::verify_program` alone
//! — compiler and test exit codes, canary-checked against oracle-neutralizing
//! candidates. No LLM judge participates in the correctness path.
//!
//! See `docs/superpowers/plans/2026-09-01-vox-efficacy-benchmark-v2.md` and
//! `docs/src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md`.

use anyhow::{Context, Result};
use clap::Parser;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use vox_corpus::humaneval_runner::conditions::{Condition, build_context, build_prompt};
use vox_corpus::humaneval_runner::{
    Fixture, eligible_after, held_out, load_corpus, verify_program,
};
use vox_eval::corpus_score::{AttemptOutcome, CorpusScore, FixtureOutcome, score_corpus};

/// `vox model eval-corpus` arguments.
#[derive(Parser, Debug, Clone)]
pub struct EvalCorpusArgs {
    /// Model id to score (registry id for live mode, or a label for `--from-dir`).
    #[arg(long)]
    pub model: String,
    /// Harness that produced the solutions (e.g. `vox-harness`, `claude-code`, `cursor`, `warp`).
    #[arg(long, default_value = "vox-harness")]
    pub harness: String,
    /// Local model checkpoint label (e.g. a MENS build id). Folded into the row's
    /// model id so successive checkpoints are separate leaderboard rows.
    #[arg(long)]
    pub checkpoint: Option<String>,
    /// Prompt condition: C0 zero-shot, C1 grammar, C2 few-shot, C3 full docs.
    #[arg(long, default_value = "C1")]
    pub condition: String,
    /// Samples per fixture. All are drawn; there is no early stop, so the
    /// unbiased pass@k estimator is computable.
    #[arg(long, default_value_t = 1)]
    pub n: usize,
    /// k for pass@k. Must satisfy k <= n.
    #[arg(long, default_value_t = 1)]
    pub k: usize,
    /// Sampling temperature. Use 0.0 with n=1 (greedy headline) or >=0.6 with n>=10.
    #[arg(long, default_value_t = 0.0)]
    pub temperature: f32,
    #[arg(long, default_value_t = 4096)]
    pub max_tokens: u64,
    /// Score pre-generated `<fixture-id>.vox` solutions from this directory instead of generating.
    #[arg(long)]
    pub from_dir: Option<PathBuf>,
    /// Corpus root.
    #[arg(long, default_value = "contracts/eval/humaneval-vox")]
    pub corpus: PathBuf,
    /// Score only fixtures added strictly after this ISO date (a model's
    /// training cutoff — prefer the OpenRouter catalog's `knowledge_cutoff`
    /// over a hand-typed date).
    #[arg(long)]
    pub cutoff: Option<String>,
    /// Include training-eligible fixtures. Off by default: only held-out
    /// fixtures support an external claim.
    #[arg(long, default_value_t = false)]
    pub include_training_eligible: bool,
    /// Path to the `vox` binary. Defaults to `target/release/vox[.exe]`, then
    /// `target/debug/vox[.exe]`.
    #[arg(long)]
    pub vox_bin: Option<PathBuf>,
    /// Per-subprocess timeout in seconds.
    #[arg(long, default_value_t = 60)]
    pub timeout_secs: u64,
    /// Abort the sweep once spend exceeds this; the run is marked incomplete.
    #[arg(long)]
    pub max_spend_usd: Option<f64>,
    /// Write the run report JSON here.
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Skip the `model_scoreboard` write-back.
    #[arg(long, default_value_t = false)]
    pub no_write_back: bool,
}

/// Resolve the `(model_id, harness_id)` pair a leaderboard row is keyed by.
///
/// A local checkpoint label is folded into the model id as `<model>@<checkpoint>`
/// so successive MENS builds are separate rows rather than silently overwriting
/// one another. External models pass `None` and keep their bare registry id.
/// MENS thereby enters the leaderboard as a registry id like any other model,
/// scored by identical code on identical axes — no special case anywhere here.
#[must_use]
pub fn mens_row_identity(model: &str, harness: &str, checkpoint: Option<&str>) -> (String, String) {
    match checkpoint {
        Some(c) => (format!("{model}@{c}"), harness.to_string()),
        None => (model.to_string(), harness.to_string()),
    }
}

/// sha256-based row identity over every knob that can change a score.
///
/// An earlier design hashed with `DefaultHasher`, which Rust does not
/// guarantee stable across releases — a toolchain bump would silently re-key
/// every historical row.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn config_digest(
    harness: &str,
    n: usize,
    k: usize,
    condition: &str,
    context_hash: &str,
    temperature: f32,
    max_tokens: u64,
) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(format!(
        "{harness}|{n}|{k}|{condition}|{context_hash}|{temperature}|{max_tokens}"
    ));
    // `Sha256::finalize()` returns a `GenericArray`, which does not implement
    // `LowerHex` directly (unlike a `[u8]` slice) — format each byte.
    h.finalize()
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Provider-side failures that must NOT be scored as model failures.
#[must_use]
pub fn is_infra_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("rate limited")
        || m.contains("429")
        || m.contains("context length")
        || m.contains("502")
        || m.contains("503")
        || m.contains("timeout")
}

/// Pull Vox source from a completion, unwrapping a fenced block when present.
#[must_use]
pub fn extract_vox_code(completion: &str) -> String {
    let Some(open) = completion.find("```") else {
        return completion.trim().to_string();
    };
    let after = &completion[open + 3..];
    let body = &after[after.find('\n').map_or(0, |i| i + 1)..];
    match body.find("```") {
        Some(c) => body[..c].trim().to_string(),
        None => body.trim().to_string(),
    }
}

/// Load `<id>[-slug].vox` solutions, erroring on ambiguity rather than
/// silently picking one by directory order.
pub fn load_solution_dir(dir: &Path) -> Result<HashMap<String, String>> {
    let mut out: HashMap<String, String> = HashMap::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("vox") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let id = stem.split('-').next().unwrap_or(&stem).to_string();
        if out.contains_key(&id) {
            anyhow::bail!(
                "ambiguous fixture id `{id}` in {} — two files map to the same fixture",
                dir.display()
            );
        }
        out.insert(id, std::fs::read_to_string(&path)?);
    }
    Ok(out)
}

/// Resolve the `vox` binary to verify with: an explicit path, else the release
/// build, else the dev build.
///
/// The dev fallback is deliberate. A release build of this workspace needs
/// multi-GB of RAM to borrow-check at `opt-level=3` with LTO, and on a
/// contended machine rustc can die with an allocation failure
/// (`STATUS_STACK_BUFFER_OVERRUN` / `0xc0000409`) that looks nothing like an
/// out-of-memory error. Verification only needs `vox check`/`vox run` to be
/// correct, not fast — published latency comes from the generation call, not
/// this subprocess, so the profile cannot bias a metric.
pub fn resolve_vox_binary(explicit: Option<&Path>) -> Result<PathBuf> {
    let name = if cfg!(windows) { "vox.exe" } else { "vox" };
    if let Some(p) = explicit {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        anyhow::bail!(
            "vox binary not found at {} — build it first: cargo build -p vox-cli --release \
             (add `-j 2` if rustc dies with an allocation failure on a contended machine)",
            p.display()
        );
    }
    for profile in ["release", "debug"] {
        let candidate = PathBuf::from("target").join(profile).join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    anyhow::bail!(
        "no vox binary at target/release/{name} or target/debug/{name} — build one first: \
         cargo build -p vox-cli --release (add `-j 2` if rustc dies with an allocation \
         failure on a contended machine)"
    )
}

/// Select the fixtures this run will score, applying the held-out and
/// rolling-window filters.
fn select_fixtures(all: &[Fixture], args: &EvalCorpusArgs) -> Vec<Fixture> {
    let base: Vec<Fixture> = if args.include_training_eligible {
        all.to_vec()
    } else {
        held_out(all).into_iter().cloned().collect()
    };
    match &args.cutoff {
        Some(cutoff) => eligible_after(&base, cutoff).into_iter().cloned().collect(),
        None => base,
    }
}

/// Generate one candidate solution through the model-agnostic LLM facade.
///
/// Returns `(source, total_tokens, latency_ms, cost_usd)`. All LLM traffic
/// goes through `vox_actor_runtime::llm` per the workspace's model-agnostic
/// boundary — never a vendor SDK or hostname.
async fn generate_candidate(
    model_id: &str,
    prompt: &str,
    temperature: f32,
    max_tokens: u64,
) -> Result<(String, u32, i64, Option<f64>), String> {
    use vox_actor_runtime::ActivityOptions;
    use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig, llm_chat};

    let mut config = LlmConfig::openrouter(model_id);
    config.max_tokens = Some(max_tokens);
    config.temperature = Some(temperature);
    config.telemetry_task_category = Some("eval-corpus".to_string());

    let messages = vec![LlmChatMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
        ..Default::default()
    }];

    let started = std::time::Instant::now();
    let outcome = llm_chat(&ActivityOptions::new(), messages, config).await;
    let latency_ms = started.elapsed().as_millis() as i64;

    let response = match outcome {
        vox_actor_runtime::ActivityResult::Ok(inner) => inner,
        vox_actor_runtime::ActivityResult::Failed(e) => return Err(e.to_string()),
        vox_actor_runtime::ActivityResult::Cancelled => return Err("activity cancelled".into()),
    }?;

    Ok((
        extract_vox_code(&response.content),
        response.prompt_tokens + response.completion_tokens,
        latency_ms,
        response.cost_usd,
    ))
}

/// Build the scoreboard row recording a corpus run.
///
/// `quality_score` is the measured pass@1 — replacing the router's
/// `log10(max_tokens)` heuristic with a real number under the `vox-codegen`
/// task category, never overwriting the general-purpose row `vox model eval`
/// maintains (that lives under `task_category = "general"`).
#[must_use]
pub fn scoreboard_row_from_corpus(
    model_id: &str,
    harness_id: &str,
    score: &CorpusScore,
) -> vox_db::store::types::ModelScoreboardRow {
    vox_db::store::types::ModelScoreboardRow {
        model_id: model_id.to_string(),
        task_category: "vox-codegen".to_string(),
        strength_tag: harness_id.to_string(),
        window_days: 7,
        n_calls: score.n_fixtures as i64,
        success_rate: score.pass_at_1,
        p50_latency_ms: score.p50_ms,
        p99_latency_ms: None,
        cost_per_success_usd: score.cost_per_success_usd,
        quality_score: score.pass_at_1,
        updated_at_ms: vox_db::now_unix_ms() as i64,
        success_count: (score.pass_at_1 * score.n_fixtures as f64).round() as i64,
        cumulative_cost_usd: score.cumulative_cost_usd.unwrap_or(0.0),
        // The corpus harness measures pass@1, p50 latency and cost; it never observes
        // streaming timings. `None` is "not measured" — the honest value, and the one the
        // scoreboard renders as an empty cell rather than as a confident `0.0`. (These three
        // are display-only columns; the Pareto axes are reliability, cost and p50 latency.)
        p95_ttft_ms: None,
        p95_tpot_ms: None,
        goodput_tokens_per_sec: None,
    }
}

pub async fn run(args: EvalCorpusArgs) -> Result<()> {
    anyhow::ensure!(
        args.k <= args.n,
        "--k ({}) must not exceed --n ({}) — pass@k needs at least k samples",
        args.k,
        args.n
    );
    if args.temperature == 0.0 && args.n > 1 {
        eprintln!(
            "{} temperature=0.0 with n={} draws near-identical samples; pass@k will \
             collapse toward pass@1. Use --temperature >= 0.6 for a meaningful pass@k, \
             or --n 1 for a cheaper greedy headline.",
            "warning:".yellow(),
            args.n
        );
    }

    let repo_root = std::env::current_dir()?;
    let condition = Condition::parse(&args.condition)?;
    let ctx = build_context(condition, &repo_root)?;

    let all = load_corpus(&args.corpus)?;
    let fixtures = select_fixtures(&all, &args);
    anyhow::ensure!(
        !fixtures.is_empty(),
        "no fixtures selected — a --cutoff after the corpus's newest problem leaves \
         nothing scoreable, which is the honest result, not an error to paper over"
    );

    let (row_model_id, row_harness_id) =
        mens_row_identity(&args.model, &args.harness, args.checkpoint.as_deref());
    let digest = config_digest(
        &args.harness,
        args.n,
        args.k,
        condition.id(),
        &ctx.context_hash,
        args.temperature,
        args.max_tokens,
    );

    let vox_bin = resolve_vox_binary(args.vox_bin.as_deref())?;
    let timeout = std::time::Duration::from_secs(args.timeout_secs);
    let workdir = std::env::temp_dir().join(format!("vox-eval-corpus-{}", std::process::id()));

    let solutions = match &args.from_dir {
        Some(dir) => Some(load_solution_dir(dir)?),
        None => None,
    };
    let measured = solutions.is_none();

    println!(
        "{} {} / {} [{}] — {} fixture(s), {} attempt(s) each",
        " EVAL-CORPUS ".on_blue().white().bold(),
        row_model_id,
        row_harness_id,
        condition.id(),
        fixtures.len(),
        if measured { args.n } else { 1 }
    );

    let mut cumulative_spend = 0.0f64;
    let mut run_complete = true;
    let mut n_infra_errors = 0usize;
    let mut outcomes: Vec<FixtureOutcome> = Vec::with_capacity(fixtures.len());

    'fixtures: for fixture in &fixtures {
        let tests_source = std::fs::read_to_string(&fixture.tests_path)
            .with_context(|| format!("reading {}", fixture.tests_path.display()))?;

        let n_attempts = if measured { args.n } else { 1 };
        let mut attempts = Vec::with_capacity(n_attempts);

        for _ in 0..n_attempts {
            if let Some(cap) = args.max_spend_usd {
                if cumulative_spend >= cap {
                    run_complete = false;
                    break 'fixtures;
                }
            }

            let (candidate, tokens, latency_ms, cost_usd) = match &solutions {
                Some(map) => match map.get(&fixture.id) {
                    Some(src) => (extract_vox_code(src), 0u32, 0i64, None),
                    None => {
                        // A missing solution is a miss, not a skip: skipping
                        // would shrink the denominator and inflate the rate.
                        attempts.push(AttemptOutcome {
                            compiled: false,
                            tests_passed: false,
                            cheated: false,
                            total_tokens: 0,
                            latency_ms: 0,
                            cost_usd: None,
                        });
                        continue;
                    }
                },
                None => {
                    let prompt = build_prompt(&ctx, &fixture.signature, &fixture.prompt);
                    match generate_candidate(
                        &args.model,
                        &prompt,
                        args.temperature,
                        args.max_tokens,
                    )
                    .await
                    {
                        Ok(v) => v,
                        Err(reason) => {
                            if is_infra_error(&reason) {
                                n_infra_errors += 1;
                                tracing::warn!(model = %args.model, fixture = %fixture.id, error = %reason, "infra error, not counted as a model failure");
                                continue;
                            }
                            tracing::warn!(model = %args.model, fixture = %fixture.id, error = %reason, "generation failed");
                            attempts.push(AttemptOutcome {
                                compiled: false,
                                tests_passed: false,
                                cheated: false,
                                total_tokens: 0,
                                latency_ms: 0,
                                cost_usd: None,
                            });
                            continue;
                        }
                    }
                }
            };

            if let Some(c) = cost_usd {
                cumulative_spend += c;
            }

            let outcome = verify_program(&vox_bin, &candidate, &tests_source, &workdir, timeout)?;
            attempts.push(AttemptOutcome {
                compiled: outcome.compiled,
                tests_passed: outcome.tests_passed,
                cheated: outcome.cheated,
                total_tokens: tokens,
                latency_ms,
                cost_usd,
            });
            // Deliberately NO early break on a pass: the unbiased pass@k
            // estimator needs (n, c) — every attempt's outcome, not just the
            // first success.
        }

        let c = attempts.iter().filter(|a| a.tests_passed).count();
        outcomes.push(FixtureOutcome {
            fixture_id: fixture.id.clone(),
            n: attempts.len(),
            c,
            attempts,
        });
    }

    let effective_k = if measured { args.k } else { 1 };
    let score = score_corpus(&outcomes, effective_k.max(1), measured);

    println!(
        "pass@1 {:.1}% | pass@{} {:.1}% | compile {:.1}% | cheated {} | infra-errors {}{}",
        score.pass_at_1 * 100.0,
        score.k,
        score.pass_at_k * 100.0,
        score.compile_rate * 100.0,
        score.n_cheated,
        n_infra_errors,
        if run_complete {
            ""
        } else {
            " | INCOMPLETE (spend cap hit)"
        }
    );

    if !args.no_write_back {
        match vox_db::VoxDb::connect(
            vox_db::DbConfig::resolve_canonical().map_err(anyhow::Error::msg)?,
        )
        .await
        {
            Ok(db) => {
                let row = scoreboard_row_from_corpus(&row_model_id, &row_harness_id, &score);
                match db.upsert_model_scoreboard(row).await {
                    Ok(()) => println!("Wrote measured quality to model_scoreboard (vox-codegen)."),
                    Err(e) => tracing::warn!(error = %e, "scoreboard upsert failed"),
                }
            }
            Err(e) => tracing::warn!(error = %e, "DB unavailable; skipping write-back"),
        }
    }

    if let Some(path) = &args.output {
        let report = serde_json::json!({
            "schema_version": 1,
            "model_id": row_model_id,
            "harness_id": row_harness_id,
            "config_digest": digest,
            "condition": condition.id(),
            "context_hash": ctx.context_hash,
            "cutoff": args.cutoff,
            "held_out_only": !args.include_training_eligible,
            "run_complete": run_complete,
            "n_infra_errors": n_infra_errors,
            "score": score,
            "fixtures": outcomes,
        });
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
        println!("Wrote report to {}", path.display());
    }

    std::fs::remove_dir_all(&workdir).ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mens_checkpoints_are_distinct_leaderboard_rows() {
        let (id_a, _) = mens_row_identity("vox/mens", "vox-harness", Some("2026-09-01-a"));
        let (id_b, _) = mens_row_identity("vox/mens", "vox-harness", Some("2026-09-15-b"));
        assert_ne!(
            id_a, id_b,
            "different checkpoints -> different row identity"
        );
        assert!(
            id_a.contains("2026-09-01-a"),
            "checkpoint is visible in the row id"
        );
    }

    #[test]
    fn external_models_without_a_checkpoint_keep_their_bare_id() {
        let (id, harness) = mens_row_identity("moonshot/kimi-k2.6-thinking", "vox-harness", None);
        assert_eq!(
            id, "moonshot/kimi-k2.6-thinking",
            "no checkpoint -> unchanged id"
        );
        assert_eq!(harness, "vox-harness");
    }

    #[test]
    fn config_digest_is_stable_and_covers_every_knob() {
        let a = config_digest("vox-harness", 10, 1, "C1", "ctxhash", 0.8, 4096);
        assert_eq!(
            a,
            config_digest("vox-harness", 10, 1, "C1", "ctxhash", 0.8, 4096)
        );
        assert_eq!(a.len(), 16, "sha256 prefix");
        assert_ne!(
            a,
            config_digest("claude-code", 10, 1, "C1", "ctxhash", 0.8, 4096)
        );
        assert_ne!(
            a,
            config_digest("vox-harness", 10, 1, "C0", "ctxhash", 0.8, 4096)
        );
        assert_ne!(
            a,
            config_digest("vox-harness", 10, 1, "C1", "OTHER", 0.8, 4096)
        );
        assert_ne!(
            a,
            config_digest("vox-harness", 10, 1, "C1", "ctxhash", 0.2, 4096),
            "temperature must be part of row identity"
        );
    }

    #[test]
    fn provider_errors_are_infra_not_model_failures() {
        assert!(is_infra_error("rate limited: 429 too many requests"));
        assert!(is_infra_error("context length exceeded"));
        assert!(!is_infra_error("model returned malformed code"));
    }

    #[test]
    fn extract_vox_code_unwraps_fences_and_passes_bare_code() {
        assert!(extract_vox_code("```vox\nfn f() to int { return 1 }\n```").starts_with("fn f()"));
        assert!(extract_vox_code("```\nfn f() to int { return 1 }\n```").starts_with("fn f()"));
        assert_eq!(
            extract_vox_code("fn f() to int { return 1 }").trim(),
            "fn f() to int { return 1 }"
        );
    }

    #[test]
    fn load_solution_dir_errors_on_ambiguous_ids() {
        let d = std::env::temp_dir().join(format!("vox-sol-amb-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("041.vox"), "fn a() to int { return 1 }").unwrap();
        std::fs::write(d.join("041-nth-prime.vox"), "fn b() to int { return 2 }").unwrap();
        assert!(
            load_solution_dir(&d).is_err(),
            "ambiguous fixture id must be an error"
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn load_solution_dir_ignores_non_vox_files() {
        let d = std::env::temp_dir().join(format!("vox-sol-nonvox-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("041.vox"), "fn a() to int { return 1 }").unwrap();
        std::fs::write(d.join("notes.md"), "ignored").unwrap();
        let sols = load_solution_dir(&d).unwrap();
        assert_eq!(sols.len(), 1);
        assert!(sols.contains_key("041"));
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn resolve_vox_binary_errors_with_build_instructions_when_absent() {
        let missing = std::env::temp_dir().join("definitely-no-vox-binary-here");
        let err = resolve_vox_binary(Some(&missing)).unwrap_err();
        assert!(
            err.to_string().contains("cargo build -p vox-cli --release"),
            "error must tell the operator how to fix it, got: {err}"
        );
        assert!(
            err.to_string().contains("-j 2"),
            "error must mention the low-memory workaround: a release build of this \
             workspace can OOM rustc on a contended machine, and the crash does not \
             look like an out-of-memory error"
        );
    }

    #[test]
    fn resolve_vox_binary_accepts_an_explicit_existing_path() {
        let existing = std::env::current_exe().expect("test binary path");
        assert_eq!(
            resolve_vox_binary(Some(&existing)).expect("resolves"),
            existing
        );
    }
}
