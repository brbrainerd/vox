---
title: "Vox Efficacy Benchmark & Public Leaderboard — Implementation Plan"
description: "Task-by-task TDD plan wiring the held-out humaneval-vox corpus to the model registry, adding rolling-window contamination resistance, external-harness comparison rows, Wilson-interval statistics, a live voxlang.org leaderboard, Hugging Face dataset publishing, SCIENTIA finding-candidate automation, and MENS assessment scaffolding."
category: "Plans"
status: "draft"
training_eligible: false
---

# Vox Efficacy Benchmark & Public Leaderboard Implementation Plan

> ## ⛔ SUPERSEDED IN PART — DO NOT EXECUTE AS WRITTEN (2026-09-01)
>
> A seven-track adversarial audit found defects that make this plan unsafe to
> execute unmodified. Read
> [**the audit**](../../src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md)
> **first**; it supersedes the tasks named below.
>
> **Empirically verified, not theoretical:**
> - **A four-line candidate scores 100% on every fixture** and passes Task 5's
>   integrity guard cleanly (`let assert = fn(c: bool) to bool { return true }`).
>   Task 5 must be rewritten before any run. Note a top-level `fn assert` does
>   *not* work — only the `let`-bound form — so guard the right construct.
> - **Task 1's `pass_at_k` is not pass@k.** At n=k it returns 1.000 for any
>   fixture with one success. Use the unbiased estimator; make `k` a config input.
> - **Task 7's `break` on first success** makes that estimator uncomputable and
>   biases every cost metric against weaker models.
> - **Task 13's CI-overlap test is statistically inert** (effective α ≈ 0.005),
>   and its test asserts the bug as intended behavior. The data are paired — use
>   McNemar + Holm.
> - **Task 6 deadlocks**: measured 163,149 bytes of compile diagnostics versus an
>   ~8 KiB Windows pipe buffer, and `detail` reads stderr while diagnostics go to
>   stdout. Redirect to files.
> - **31 held-out fixtures give 9% power at a 10-point difference.** No
>   model-vs-model ranking is supportable at that size.
> - **The held-out split is triple-inconsistent** (manifest 31 / spec.toml 10 /
>   `.vox` files 0), and held-out fixture 072 has a byte-identical
>   training-eligible twin (141).
>
> Tasks 2, 3, 4, 10, 11, 12, 14, 15 are broadly sound. Tasks 1, 5, 6, 7, 8, 9, 13
> need the audit's corrections applied before execution.
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make "how well does an AI write Vox?" a continuously-measured, contamination-resistant, publicly-published number that compares Vox+MENS against Claude, Kimi K2, Qwen, Grok, and external harnesses (Claude Code, Cursor, Warp) on identical held-out problems — and feed every run into the existing SCIENTIA publication pipeline automatically.

**Architecture:** A pure scoring/statistics core in `vox-eval` (L2, no I/O, fully unit-testable), a corpus + verification layer in `vox-corpus` (L3, manifest loading, rolling-window eligibility, program composition, sandboxed subprocess verification), a CLI runner in `vox-cli` (L4) that both generates solutions live through the `vox_actor_runtime::llm` facade **and** ingests pre-generated solution directories from external harnesses, a JSON leaderboard artifact rendered by a new `docs-astro` route, and publication automation in `vox-publisher`. Correctness is verified **only** by compiler exit code and test-assertion exit code — no LLM judge anywhere in the correctness path.

**Tech Stack:** Rust (workspace edition), `clap` (CLI), `serde`/`serde_json`/`serde_yaml`, `tokio` (async LLM calls), `comfy_table` (terminal output), `reqwest` (HF upload), Astro 6 (`docs-astro`), `vox_actor_runtime::llm` (model-agnostic LLM facade).

**Spec:** [`docs/src/architecture/vox-mens-comparative-efficacy-benchmarking-research-2026-09-01.md`](../../src/architecture/vox-mens-comparative-efficacy-benchmarking-research-2026-09-01.md)

## Global Constraints

Every task's requirements implicitly include this section. Values copied verbatim from `AGENTS.md` and the spec.

- **LLM boundary (normative):** All LLM calls MUST go through `vox_actor_runtime::llm` (`infer_with_retry`, `llm_chat`, `llm_stream`, `llm_embed`). Never hardcode a vendor hostname or instantiate a vendor SDK. Enforced by `vox-code-audit` detector `llm_provider_call` at severity `Error`.
- **Test-first (normative):** Every new `pub fn` in `crates/*/src/**` requires at least one `#[test]` in the same file before the commit lands. Detector: `skeleton/untested-pub-api`. Pre-commit hook `tdd-guard` blocks violations.
- **No new glue scripts:** No new `.ps1`, `.sh`, or `.py` files. Project automation is `.vox` executed via `vox run`.
- **Formatting:** NEVER run `cargo fmt --all` (overflows the Windows `CreateProcess` limit, `os error 206`). Use `vox run scripts/fmt.vox` to write, `VOX_FMT_CHECK=1 vox run scripts/fmt.vox` to check, or `cargo fmt -p <crate>` for one crate.
- **Layers:** `vox-eval` = layer 2, `vox-corpus` = layer 3, `vox-cli` = layer 4, `vox-publisher` = layer 3. Dependencies point same-layer or **down** only. No new crates are created by this plan, so no `contracts/ci/crate-layers.v1.json` edits are needed.
- **Crate edges:** This plan introduces **zero new workspace crate edges** — `vox-cli` already depends on `vox-eval`, `vox-corpus`, `vox-db`, and `vox-publisher` (verified in `crates/vox-cli/Cargo.toml` lines 170/251/185/201). The only new dependency anywhere is the external crate `toml` (already pinned at workspace level, `Cargo.toml:210`). If `vox ci crate-edges` nonetheless rejects something, STOP and report it — **you may NOT author an `exceptions` entry**, those are user-authorized only.
- **Doc frontmatter:** Any new `.md` under `docs/src/` MUST begin with a YAML block containing `title`, `description`, `category`. Verify with `cargo run -p vox-doc-pipeline -- --lint-only --paths <file>.md`.
- **CLI registry:** After adding a CLI subcommand, run `cargo run -p vox-cli -- ci command-sync --write` and refresh the catalog baseline with `UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli command_catalog`.
- **Local CI first:** Run `vox ci pre-push --complete` before pushing. Do NOT use GitHub Actions as the primary feedback loop; remote check-watching is blocked for agent sessions.
- **No LLM judge in the correctness path:** compile exit code and test exit code are the only correctness signals (spec §A lesson 4). This is non-negotiable and is the credibility basis of the whole benchmark.
- **Reproducibility pins:** `temperature: 0.0`, `seed: 42`, `attempts_per_fixture: 5` (from `contracts/eval/README.md`).

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/vox-eval/src/corpus_score.rs` | **Create.** Pure scoring fold: per-fixture outcomes → pass@1/pass@k, compile rate, token/latency/cost aggregates. No I/O. |
| `crates/vox-eval/src/corpus_stats.rs` | **Create.** Wilson score intervals and pairwise-overlap comparison. No I/O. |
| `crates/vox-eval/src/lib.rs` | **Modify.** Add `pub mod corpus_score; pub mod corpus_stats;`. |
| `crates/vox-corpus/src/humaneval_runner/mod.rs` | **Create.** Module root + re-exports. |
| `crates/vox-corpus/src/humaneval_runner/manifest.rs` | **Create.** Load `manifest.v1.yaml` + `spec.toml`; held-out filter; rolling-window eligibility. |
| `crates/vox-corpus/src/humaneval_runner/compose.rs` | **Create.** Compose a runnable program from a candidate solution + the fixture's test assertions. |
| `crates/vox-corpus/src/humaneval_runner/verify.rs` | **Create.** Sandboxed subprocess verification (`vox check` then `vox run`), with timeout. |
| `crates/vox-corpus/src/humaneval_runner/integrity.rs` | **Create.** Reward-hacking guard: reject candidates that reference the reference solution or the test file. |
| `crates/vox-corpus/src/lib.rs` | **Modify.** Add `pub mod humaneval_runner;`. |
| `crates/vox-cli/src/commands/model/eval_corpus.rs` | **Create.** `vox model eval-corpus`: live generation, external-harness ingest, scoreboard write-back, report emission. |
| `crates/vox-cli/src/commands/model/mod.rs` | **Modify.** Register the `EvalCorpus` subcommand. |
| `contracts/reports/vox-efficacy/leaderboard.v1.schema.json` | **Create.** Schema for the published leaderboard artifact. |
| `crates/vox-publisher/src/huggingface_dataset.rs` | **Create.** HF dataset-card generation + Hub upload. |
| `crates/vox-publisher/src/lib.rs` | **Modify.** Add `pub mod huggingface_dataset;`. |
| `docs-astro/src/pages/benchmarks.astro` | **Create.** Public leaderboard page reading the JSON artifact. |
| `contracts/eval/humaneval-vox/manifest.v1.yaml` | **Modify.** Add `added_at` per fixture (rolling-window support). |
| `docs/src/reference/vox-efficacy-benchmark.md` | **Create.** Operator reference for running and publishing the benchmark. |

---

## Phase A — Pure scoring core (`vox-eval`, layer 2)

### Task 1: Corpus scoring fold

**Files:**
- Create: `crates/vox-eval/src/corpus_score.rs`
- Modify: `crates/vox-eval/src/lib.rs`

**Interfaces:**
- Consumes: nothing (this is the base layer).
- Produces: `pub struct AttemptOutcome { pub compiled: bool, pub tests_passed: bool, pub total_tokens: u32, pub latency_ms: i64, pub cost_usd: Option<f64> }`; `pub struct FixtureOutcome { pub fixture_id: String, pub attempts: Vec<AttemptOutcome> }`; `pub struct CorpusScore { pub n_fixtures: usize, pub compile_rate: f64, pub pass_at_1: f64, pub pass_at_k: f64, pub k: usize, pub total_tokens: u64, pub tokens_per_pass: f64, pub p50_ms: i64, pub p99_ms: i64, pub cumulative_cost_usd: f64, pub cost_per_success_usd: Option<f64>, pub n_passed_at_1: usize }`; `pub fn score_corpus(outcomes: &[FixtureOutcome]) -> CorpusScore`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-eval/src/corpus_score.rs` containing ONLY the test module for now:

```rust
//! Pure scoring fold for held-out corpus runs (HumanEval-Vox and siblings).
//!
//! No I/O: takes already-collected per-attempt outcomes and folds them into the
//! comparative axes the efficacy leaderboard publishes. Correctness inputs
//! (`compiled`, `tests_passed`) come from compiler/test-runner exit codes only —
//! never from an LLM judge (see the benchmarking research doc, §A lesson 4).

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(compiled: bool, passed: bool, tokens: u32, latency: i64) -> AttemptOutcome {
        AttemptOutcome {
            compiled,
            tests_passed: passed,
            total_tokens: tokens,
            latency_ms: latency,
            cost_usd: None,
        }
    }

    #[test]
    fn pass_at_1_uses_only_the_first_attempt() {
        // Fixture A: first attempt fails, second passes -> pass@1 miss, pass@k hit.
        // Fixture B: first attempt passes -> both hit.
        let outcomes = vec![
            FixtureOutcome {
                fixture_id: "041".to_string(),
                attempts: vec![attempt(true, false, 100, 10), attempt(true, true, 100, 10)],
            },
            FixtureOutcome {
                fixture_id: "043".to_string(),
                attempts: vec![attempt(true, true, 100, 10)],
            },
        ];
        let score = score_corpus(&outcomes);
        assert_eq!(score.n_fixtures, 2);
        assert!((score.pass_at_1 - 0.5).abs() < 1e-9, "1 of 2 passed first try");
        assert!((score.pass_at_k - 1.0).abs() < 1e-9, "2 of 2 passed within k");
        assert_eq!(score.n_passed_at_1, 1);
        assert_eq!(score.k, 2, "k is the max attempt count observed");
    }

    #[test]
    fn compile_rate_is_separate_from_test_pass_rate() {
        // A compiling-but-wrong answer must NOT score the same as a correct one.
        let outcomes = vec![
            FixtureOutcome {
                fixture_id: "a".to_string(),
                attempts: vec![attempt(true, false, 10, 5)],
            },
            FixtureOutcome {
                fixture_id: "b".to_string(),
                attempts: vec![attempt(false, false, 10, 5)],
            },
        ];
        let score = score_corpus(&outcomes);
        assert!((score.compile_rate - 0.5).abs() < 1e-9, "1 of 2 compiled");
        assert!((score.pass_at_1 - 0.0).abs() < 1e-9, "neither passed tests");
    }

    #[test]
    fn cost_sums_known_and_ignores_unknown() {
        let mut a = attempt(true, true, 10, 5);
        a.cost_usd = Some(0.02);
        let mut b = attempt(true, true, 10, 5);
        b.cost_usd = None; // free / unknown backend contributes 0
        let outcomes = vec![
            FixtureOutcome { fixture_id: "a".to_string(), attempts: vec![a] },
            FixtureOutcome { fixture_id: "b".to_string(), attempts: vec![b] },
        ];
        let score = score_corpus(&outcomes);
        assert!((score.cumulative_cost_usd - 0.02).abs() < 1e-9);
        let cps = score.cost_per_success_usd.expect("one known cost, two passes");
        assert!((cps - 0.01).abs() < 1e-9);
    }

    #[test]
    fn all_costs_unknown_yields_none_not_misleading_zero() {
        let outcomes = vec![FixtureOutcome {
            fixture_id: "a".to_string(),
            attempts: vec![attempt(true, true, 10, 5)],
        }];
        let score = score_corpus(&outcomes);
        assert!(
            score.cost_per_success_usd.is_none(),
            "no known cost -> None, not a misleading $0.00"
        );
    }

    #[test]
    fn empty_corpus_is_zeroed_not_panicking() {
        let score = score_corpus(&[]);
        assert_eq!(score.n_fixtures, 0);
        assert_eq!(score.pass_at_1, 0.0);
        assert_eq!(score.p50_ms, 0);
        assert_eq!(score.tokens_per_pass, 0.0);
    }

    #[test]
    fn latency_percentiles_use_nearest_rank_over_all_attempts() {
        let outcomes = vec![FixtureOutcome {
            fixture_id: "a".to_string(),
            attempts: vec![
                attempt(true, true, 1, 100),
                attempt(true, true, 1, 200),
                attempt(true, true, 1, 300),
                attempt(true, true, 1, 400),
            ],
        }];
        let score = score_corpus(&outcomes);
        // nearest-rank p50 over [100,200,300,400]: ceil(0.5*4)=2 -> idx 1 -> 200
        assert_eq!(score.p50_ms, 200);
        // p99: ceil(0.99*4)=4 -> idx 3 -> 400
        assert_eq!(score.p99_ms, 400);
    }
}
```

Add to `crates/vox-eval/src/lib.rs`, immediately after the existing `pub mod mens;` line:

```rust
pub mod corpus_score;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-eval corpus_score 2>&1 | tail -20`
Expected: FAIL — compile errors, `cannot find type AttemptOutcome`, `cannot find function score_corpus`.

- [ ] **Step 3: Write minimal implementation**

Insert into `crates/vox-eval/src/corpus_score.rs` **above** the `#[cfg(test)] mod tests` block (below the module doc comment):

```rust
use serde::{Deserialize, Serialize};

/// One generation attempt at one fixture, with its verification outcome.
///
/// `compiled` and `tests_passed` are exit-code-derived facts, never LLM judgments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptOutcome {
    /// `vox check` exited 0 on the composed program.
    pub compiled: bool,
    /// `vox run` exited 0 (all assertions in the fixture's test block held).
    pub tests_passed: bool,
    /// Prompt + completion tokens reported by the provider for this attempt.
    pub total_tokens: u32,
    /// Wall-clock latency of the generation call in milliseconds.
    pub latency_ms: i64,
    /// Cost of this attempt in USD, or `None` for free/unknown backends.
    pub cost_usd: Option<f64>,
}

/// All attempts made at a single corpus fixture, in attempt order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureOutcome {
    /// Stable fixture id from the corpus manifest (e.g. `"041"`).
    pub fixture_id: String,
    /// Attempts in order; `attempts[0]` is the pass@1 attempt.
    pub attempts: Vec<AttemptOutcome>,
}

/// Folded comparative axes for one (model, harness, config) tuple over one corpus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusScore {
    /// Number of fixtures attempted.
    pub n_fixtures: usize,
    /// Fraction of fixtures whose first attempt compiled (0.0..=1.0).
    pub compile_rate: f64,
    /// Fraction of fixtures whose FIRST attempt passed its tests.
    pub pass_at_1: f64,
    /// Fraction of fixtures passed within `k` attempts.
    pub pass_at_k: f64,
    /// Max attempts observed across fixtures (the effective `k`).
    pub k: usize,
    /// Total tokens across every attempt.
    pub total_tokens: u64,
    /// Tokens per pass@1 success (lower is better); uses `max(passes, 1)`.
    pub tokens_per_pass: f64,
    /// Nearest-rank p50 latency across every attempt, in ms.
    pub p50_ms: i64,
    /// Nearest-rank p99 latency across every attempt, in ms.
    pub p99_ms: i64,
    /// Sum of known per-attempt costs; unknown (`None`) attempts contribute 0.
    pub cumulative_cost_usd: f64,
    /// Cost per pass@1 success, or `None` when nothing passed or no cost was known.
    pub cost_per_success_usd: Option<f64>,
    /// Count of fixtures passed on the first attempt (numerator of `pass_at_1`).
    pub n_passed_at_1: usize,
}

/// Fold per-fixture outcomes into the published comparative axes.
///
/// `pass_at_1` counts only `attempts[0]`; `pass_at_k` counts a fixture as passed
/// if ANY attempt passed. Keeping them separate is what lets the leaderboard
/// distinguish "gets it right first try" from "gets there with retries" — the
/// same distinction Aider's two-attempt polyglot protocol reports.
#[must_use]
pub fn score_corpus(outcomes: &[FixtureOutcome]) -> CorpusScore {
    let n_fixtures = outcomes.len();

    let n_compiled_at_1 = outcomes
        .iter()
        .filter(|f| f.attempts.first().is_some_and(|a| a.compiled))
        .count();
    let n_passed_at_1 = outcomes
        .iter()
        .filter(|f| f.attempts.first().is_some_and(|a| a.tests_passed))
        .count();
    let n_passed_at_k = outcomes
        .iter()
        .filter(|f| f.attempts.iter().any(|a| a.tests_passed))
        .count();

    let k = outcomes.iter().map(|f| f.attempts.len()).max().unwrap_or(0);

    let all_attempts: Vec<&AttemptOutcome> =
        outcomes.iter().flat_map(|f| f.attempts.iter()).collect();

    let total_tokens: u64 = all_attempts.iter().map(|a| a.total_tokens as u64).sum();

    let frac = |num: usize| {
        if n_fixtures == 0 {
            0.0
        } else {
            num as f64 / n_fixtures as f64
        }
    };

    let tokens_per_pass = if n_fixtures == 0 {
        0.0
    } else {
        total_tokens as f64 / n_passed_at_1.max(1) as f64
    };

    let mut latencies: Vec<i64> = all_attempts.iter().map(|a| a.latency_ms).collect();
    latencies.sort_unstable();

    let cumulative_cost_usd: f64 = all_attempts.iter().filter_map(|a| a.cost_usd).sum();
    let any_cost_known = all_attempts.iter().any(|a| a.cost_usd.is_some());
    let cost_per_success_usd = if n_passed_at_1 > 0 && any_cost_known {
        Some(cumulative_cost_usd / n_passed_at_1 as f64)
    } else {
        None
    };

    CorpusScore {
        n_fixtures,
        compile_rate: frac(n_compiled_at_1),
        pass_at_1: frac(n_passed_at_1),
        pass_at_k: frac(n_passed_at_k),
        k,
        total_tokens,
        tokens_per_pass,
        p50_ms: percentile(&latencies, 0.50),
        p99_ms: percentile(&latencies, 0.99),
        cumulative_cost_usd,
        cost_per_success_usd,
        n_passed_at_1,
    }
}

/// Nearest-rank percentile over a pre-sorted ascending slice. Returns 0 when empty.
fn percentile(sorted: &[i64], q: f64) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    let n = sorted.len();
    let rank = (q * n as f64).ceil() as usize;
    sorted[rank.clamp(1, n) - 1]
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-eval corpus_score 2>&1 | tail -20`
Expected: PASS — 6 tests.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-eval
git add crates/vox-eval/src/corpus_score.rs crates/vox-eval/src/lib.rs
git commit -m "feat(vox-eval): pure corpus scoring fold with pass@1/pass@k and compile rate"
```

---

### Task 2: Wilson confidence intervals and pairwise comparison

Point scores without uncertainty are the single most common way a benchmark claim gets dismissed (spec §A lesson 3: a 2026 audit found 11 of 40 Open LLM Leaderboard pairwise rankings failed statistical-resolution targets). Every published delta must carry an interval.

**Files:**
- Create: `crates/vox-eval/src/corpus_stats.rs`
- Modify: `crates/vox-eval/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct ConfidenceInterval { pub point: f64, pub low: f64, pub high: f64 }`; `pub fn wilson_interval(successes: usize, trials: usize, z: f64) -> ConfidenceInterval`; `pub const Z_95: f64`; `pub fn intervals_overlap(a: &ConfidenceInterval, b: &ConfidenceInterval) -> bool`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-eval/src/corpus_stats.rs` with ONLY the doc comment and test module:

```rust
//! Confidence intervals for published benchmark deltas.
//!
//! A leaderboard that prints bare point scores invites exactly the criticism that
//! sank several 2025-2026 leaderboards: two models 3 points apart whose intervals
//! fully overlap are not distinguishable, and saying otherwise is a measurement
//! claim the data does not support. Wilson score intervals are used rather than
//! the normal approximation because corpus sizes here are small (31 held-out
//! fixtures) and proportions run near 0 or 1, where the normal approximation
//! produces bounds outside [0, 1].

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wilson_interval_brackets_the_point_estimate() {
        let ci = wilson_interval(8, 10, Z_95);
        assert!((ci.point - 0.8).abs() < 1e-9);
        assert!(ci.low < ci.point, "low bound below point");
        assert!(ci.high > ci.point, "high bound above point");
    }

    #[test]
    fn wilson_interval_stays_inside_zero_one_at_the_extremes() {
        // The normal approximation would produce a negative low bound here.
        let all_fail = wilson_interval(0, 10, Z_95);
        assert!(all_fail.low >= 0.0, "low bound must not go negative");
        assert!(all_fail.high <= 1.0);
        assert_eq!(all_fail.point, 0.0);

        let all_pass = wilson_interval(10, 10, Z_95);
        assert!(all_pass.high <= 1.0, "high bound must not exceed 1");
        assert!(all_pass.low >= 0.0);
        assert_eq!(all_pass.point, 1.0);
    }

    #[test]
    fn wilson_interval_narrows_as_sample_size_grows() {
        let small = wilson_interval(8, 10, Z_95);
        let large = wilson_interval(800, 1000, Z_95);
        let small_width = small.high - small.low;
        let large_width = large.high - large.low;
        assert!(
            large_width < small_width,
            "more trials at the same rate must tighten the interval: {large_width} vs {small_width}"
        );
    }

    #[test]
    fn zero_trials_is_the_full_unit_interval_not_a_nan() {
        let ci = wilson_interval(0, 0, Z_95);
        assert_eq!(ci.point, 0.0);
        assert_eq!(ci.low, 0.0);
        assert_eq!(ci.high, 1.0);
    }

    #[test]
    fn overlapping_intervals_are_not_a_distinguishable_ranking() {
        // 24/31 vs 26/31 on a small held-out corpus: visibly different point
        // scores, but the intervals overlap, so the leaderboard must not claim
        // one beats the other.
        let a = wilson_interval(24, 31, Z_95);
        let b = wilson_interval(26, 31, Z_95);
        assert!(
            intervals_overlap(&a, &b),
            "31-fixture corpus cannot resolve a 2-fixture gap"
        );

        // A large, real gap on a large sample IS resolvable.
        let weak = wilson_interval(200, 1000, Z_95);
        let strong = wilson_interval(800, 1000, Z_95);
        assert!(!intervals_overlap(&weak, &strong));
    }
}
```

Add to `crates/vox-eval/src/lib.rs`, immediately after the `pub mod corpus_score;` line added in Task 1:

```rust
pub mod corpus_stats;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-eval corpus_stats 2>&1 | tail -20`
Expected: FAIL — `cannot find function wilson_interval`, `cannot find value Z_95`.

- [ ] **Step 3: Write minimal implementation**

Insert into `crates/vox-eval/src/corpus_stats.rs` above the test module:

```rust
use serde::{Deserialize, Serialize};

/// z-score for a two-sided 95% confidence interval.
pub const Z_95: f64 = 1.959_963_984_540_054;

/// A point estimate with its lower and upper confidence bounds, all in `[0, 1]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceInterval {
    /// The observed proportion (`successes / trials`).
    pub point: f64,
    /// Lower confidence bound, clamped to `>= 0.0`.
    pub low: f64,
    /// Upper confidence bound, clamped to `<= 1.0`.
    pub high: f64,
}

/// Wilson score interval for a binomial proportion.
///
/// Preferred over the normal (Wald) approximation because this benchmark's
/// held-out corpus is small and pass rates cluster near the extremes, where Wald
/// produces bounds outside `[0, 1]`. With `trials == 0` this returns the full
/// unit interval rather than `NaN`, so an unmeasured model renders as "unknown"
/// instead of poisoning the leaderboard with `NaN`.
#[must_use]
pub fn wilson_interval(successes: usize, trials: usize, z: f64) -> ConfidenceInterval {
    if trials == 0 {
        return ConfidenceInterval { point: 0.0, low: 0.0, high: 1.0 };
    }
    let n = trials as f64;
    let p = successes as f64 / n;
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let center = p + z2 / (2.0 * n);
    let margin = z * ((p * (1.0 - p) / n) + (z2 / (4.0 * n * n))).sqrt();
    ConfidenceInterval {
        point: p,
        low: ((center - margin) / denom).max(0.0),
        high: ((center + margin) / denom).min(1.0),
    }
}

/// True when two intervals overlap — i.e. the ranking between them is NOT
/// statistically resolvable at the interval's confidence level.
///
/// The leaderboard uses this to decide whether to render a "beats" claim or a
/// "tied within measurement error" band.
#[must_use]
pub fn intervals_overlap(a: &ConfidenceInterval, b: &ConfidenceInterval) -> bool {
    a.low <= b.high && b.low <= a.high
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-eval corpus_stats 2>&1 | tail -20`
Expected: PASS — 5 tests.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-eval
git add crates/vox-eval/src/corpus_stats.rs crates/vox-eval/src/lib.rs
git commit -m "feat(vox-eval): Wilson confidence intervals for benchmark deltas"
```

---

## Phase B — Corpus loading and verification (`vox-corpus`, layer 3)

### Task 3: Rolling-window corpus manifest loader

The corpus today is a fixed 164-problem set with 120 problems marked `training_eligible: true` — meaning MENS may legitimately train on them, making any claim built on those problems worthless as external evidence. Only the 31 held-out problems are usable, and even those need per-problem addition dates so a model can be scored only on problems that postdate its training cutoff (the LiveCodeBench pattern, spec §A).

**Files:**
- Create: `crates/vox-corpus/src/humaneval_runner/mod.rs`
- Create: `crates/vox-corpus/src/humaneval_runner/manifest.rs`
- Modify: `crates/vox-corpus/src/lib.rs`
- Modify: `contracts/eval/humaneval-vox/manifest.v1.yaml`

**Interfaces:**
- Consumes: nothing from prior tasks.
- Produces: `pub struct Fixture { pub id: String, pub slug: String, pub training_eligible: bool, pub added_at: String, pub prompt: String, pub signature: String, pub tests_path: PathBuf }`; `pub fn load_corpus(corpus_root: &Path) -> anyhow::Result<Vec<Fixture>>`; `pub fn held_out(fixtures: &[Fixture]) -> Vec<&Fixture>`; `pub fn eligible_after(fixtures: &[Fixture], cutoff: &str) -> Vec<&Fixture>`.

- [ ] **Step 1: Add `added_at` to the manifest**

The manifest's own header comments record the real expansion dates. Apply them by fixture id range. Open `contracts/eval/humaneval-vox/manifest.v1.yaml` and add an `added_at` key to every entry in the `fixtures:` list, using these ranges (derived from the file's own `# Landed:` / `# Expanded:` comments at lines 9-13):

- ids `001`–`050` → `added_at: "2026-05-26"`
- ids `051`–`075` → `added_at: "2026-05-26"`
- ids `076`–`100` → `added_at: "2026-05-27"`
- ids `101`–`164` → `added_at: "2026-05-27"`

Each fixture entry becomes:

```yaml
  - id: "001"
    slug: "001-fizzbuzz"
    title: "fizzbuzz"
    training_eligible: true
    added_at: "2026-05-26"
    provenance: "original"
    files:
      spec: "problems/001-fizzbuzz.spec.toml"
      reference: "problems/001-fizzbuzz/reference.vox"
      tests: "problems/001-fizzbuzz/tests.vox"
```

Also add this comment block directly above the `fixtures:` key, so the next person to add problems knows the contract:

```yaml
# `added_at` (ISO date) is the contamination-resistance field: a model is scored
# only on fixtures added AFTER its published training cutoff (the LiveCodeBench
# pattern). New fixtures MUST carry the date they were added and MUST NOT be
# back-dated. Rotating new held-out problems in on a schedule is what keeps this
# corpus honest as MENS's training set grows.
```

- [ ] **Step 2: Write the failing test**

Create `crates/vox-corpus/src/humaneval_runner/mod.rs`:

```rust
//! HumanEval-Vox corpus runner: load fixtures, compose runnable programs from a
//! candidate solution, verify by compiler/test exit code, and guard against
//! reward hacking.
//!
//! Deliberately split from the scoring fold (`vox_eval::corpus_score`) so the
//! arithmetic stays pure and unit-testable while the I/O lives here.

pub mod compose;
pub mod integrity;
pub mod manifest;
pub mod verify;

pub use compose::compose_program;
pub use integrity::{IntegrityViolation, check_candidate_integrity};
pub use manifest::{Fixture, eligible_after, held_out, load_corpus};
pub use verify::{VerifyOutcome, verify_program};
```

Create `crates/vox-corpus/src/humaneval_runner/manifest.rs` with ONLY the doc comment and test module:

```rust
//! Corpus manifest + per-fixture spec loading, with rolling-window eligibility.

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn corpus_root() -> std::path::PathBuf {
        repo_root().join("contracts/eval/humaneval-vox")
    }

    #[test]
    fn loads_the_real_corpus_with_prompts_and_dates() {
        let fixtures = load_corpus(&corpus_root()).expect("corpus loads");
        assert_eq!(fixtures.len(), 164, "manifest declares count_current: 164");

        let first = fixtures.iter().find(|f| f.id == "001").expect("fixture 001");
        assert_eq!(first.slug, "001-fizzbuzz");
        assert!(first.training_eligible);
        assert_eq!(first.added_at, "2026-05-26");
        assert!(
            first.prompt.contains("FizzBuzz"),
            "prompt is read from the spec.toml, got: {}",
            first.prompt
        );
        assert!(first.signature.starts_with("fn fizzbuzz"));
        assert!(first.tests_path.exists(), "tests.vox path resolves on disk");
    }

    #[test]
    fn held_out_selects_only_non_training_eligible_fixtures() {
        let fixtures = load_corpus(&corpus_root()).expect("corpus loads");
        let ho = held_out(&fixtures);
        assert_eq!(ho.len(), 31, "manifest declares held_out_current: 31");
        assert!(
            ho.iter().all(|f| !f.training_eligible),
            "every held-out fixture must be training_eligible: false"
        );
    }

    #[test]
    fn eligible_after_excludes_fixtures_added_on_or_before_the_cutoff() {
        let fixtures = load_corpus(&corpus_root()).expect("corpus loads");
        // A model whose training cutoff postdates the whole corpus can be scored
        // on nothing — the honest answer, not a silently-inflated score.
        assert!(eligible_after(&fixtures, "2026-06-01").is_empty());
        // A cutoff before the first expansion admits everything.
        assert_eq!(eligible_after(&fixtures, "2026-05-01").len(), 164);
        // A cutoff between the two expansion dates admits only the later batch.
        let mid = eligible_after(&fixtures, "2026-05-26");
        assert!(!mid.is_empty(), "the 2026-05-27 batch is still eligible");
        assert!(
            mid.iter().all(|f| f.added_at.as_str() > "2026-05-26"),
            "strictly after the cutoff"
        );
    }
}
```

Add to `crates/vox-corpus/src/lib.rs`, in the `pub mod` block (keep alphabetical — after `pub mod flywheel;`):

```rust
pub mod humaneval_runner;
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-corpus humaneval_runner::manifest 2>&1 | tail -20`
Expected: FAIL — `cannot find function load_corpus`, and unresolved `compose`/`integrity`/`verify` modules.

- [ ] **Step 4: Write minimal implementation**

Insert into `crates/vox-corpus/src/humaneval_runner/manifest.rs` above the test module:

```rust
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// One corpus problem: its prompt, its required signature, and where its tests live.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Stable id, never recycled (e.g. `"041"`).
    pub id: String,
    /// Directory-name slug (e.g. `"041-nth-prime"`).
    pub slug: String,
    /// `false` = held out from MENS training; only these are valid for external claims.
    pub training_eligible: bool,
    /// ISO date this fixture entered the corpus (rolling-window contamination guard).
    pub added_at: String,
    /// The natural-language task given to the model.
    pub prompt: String,
    /// The exact function signature the solution must provide.
    pub signature: String,
    /// Absolute path to the fixture's `tests.vox`.
    pub tests_path: PathBuf,
}

/// Load every fixture declared by `<corpus_root>/manifest.v1.yaml`, reading each
/// one's prompt and signature from its `spec.toml`.
///
/// Fails loudly on a manifest entry whose files are missing — a silently-skipped
/// fixture would quietly shrink the denominator and inflate every pass rate.
pub fn load_corpus(corpus_root: &Path) -> Result<Vec<Fixture>> {
    let manifest_path = corpus_root.join("manifest.v1.yaml");
    let raw = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("reading {}", manifest_path.display()))?;
    let doc: serde_yaml::Value =
        serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", manifest_path.display()))?;

    let entries = doc
        .get("fixtures")
        .and_then(|f| f.as_sequence())
        .context("manifest has no `fixtures:` sequence")?;

    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let s = |key: &str| -> Result<String> {
            entry
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .with_context(|| format!("fixture entry missing `{key}`"))
        };
        let id = s("id")?;
        let slug = s("slug")?;
        let added_at = s("added_at").with_context(|| {
            format!("fixture {id} has no `added_at` — required for rolling-window scoring")
        })?;
        let training_eligible = entry
            .get("training_eligible")
            .and_then(serde_yaml::Value::as_bool)
            .with_context(|| format!("fixture {id} missing `training_eligible`"))?;

        let spec_rel = entry
            .get("files")
            .and_then(|f| f.get("spec"))
            .and_then(|v| v.as_str())
            .with_context(|| format!("fixture {id} missing `files.spec`"))?;
        let tests_rel = entry
            .get("files")
            .and_then(|f| f.get("tests"))
            .and_then(|v| v.as_str())
            .with_context(|| format!("fixture {id} missing `files.tests`"))?;

        let spec_path = corpus_root.join(spec_rel);
        let spec_raw = std::fs::read_to_string(&spec_path)
            .with_context(|| format!("reading {}", spec_path.display()))?;
        let spec: toml::Value = spec_raw
            .parse()
            .with_context(|| format!("parsing {}", spec_path.display()))?;
        let problem = spec.get("problem").context("spec.toml has no [problem]")?;
        let field = |key: &str| -> Result<String> {
            problem
                .get(key)
                .and_then(|v| v.as_str())
                .map(|v| v.trim().to_string())
                .with_context(|| format!("{} missing [problem].{key}", spec_path.display()))
        };

        out.push(Fixture {
            id,
            slug,
            training_eligible,
            added_at,
            prompt: field("prompt")?,
            signature: field("signature")?,
            tests_path: corpus_root.join(tests_rel),
        });
    }
    Ok(out)
}

/// The held-out subset — the ONLY fixtures valid for an external efficacy claim,
/// because training-eligible fixtures may legitimately appear in MENS's corpus.
#[must_use]
pub fn held_out(fixtures: &[Fixture]) -> Vec<&Fixture> {
    fixtures.iter().filter(|f| !f.training_eligible).collect()
}

/// Fixtures added strictly after `cutoff` (an ISO `YYYY-MM-DD` date).
///
/// ISO dates compare correctly as strings, so no date parsing is needed.
/// Scoring a model only on problems that postdate its training cutoff is the
/// LiveCodeBench contamination-resistance mechanism: a model cannot have
/// memorized a problem that did not exist when it was trained.
#[must_use]
pub fn eligible_after<'a>(fixtures: &'a [Fixture], cutoff: &str) -> Vec<&'a Fixture> {
    fixtures
        .iter()
        .filter(|f| f.added_at.as_str() > cutoff)
        .collect()
}
```

Add the `toml` dependency to `crates/vox-corpus/Cargo.toml` under `[dependencies]` (alphabetical, after `syn`):

```toml
toml = { workspace = true }
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-corpus humaneval_runner::manifest 2>&1 | tail -20`
Expected: PASS — 3 tests. If `held_out_selects_only_non_training_eligible_fixtures` reports a count other than 31, the manifest's `held_out_current` is stale — fix the assertion to the real count AND update `held_out_current` in the manifest to match, in the same commit.

- [ ] **Step 6: Verify no crate-edge violation**

Run: `cargo run -q -p vox-cli -- ci crate-edges`
Expected: PASS. `toml` is an external crate, not a workspace edge. If this fails, STOP and report — do not author an `exceptions` entry.

- [ ] **Step 7: Format and commit**

```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/humaneval_runner/ crates/vox-corpus/src/lib.rs crates/vox-corpus/Cargo.toml contracts/eval/humaneval-vox/manifest.v1.yaml
git commit -m "feat(vox-corpus): humaneval-vox loader with rolling-window added_at eligibility"
```

---

### Task 4: Compose a runnable program from a candidate solution

A fixture's `tests.vox` contains BOTH the reference implementation AND the assertion block. To grade a *candidate* solution we must take the candidate's code and the fixture's assertions, and nothing of the reference.

**Files:**
- Create: `crates/vox-corpus/src/humaneval_runner/compose.rs`

**Interfaces:**
- Consumes: `Fixture` from Task 3.
- Produces: `pub fn extract_test_block(tests_source: &str) -> anyhow::Result<String>`; `pub fn strip_candidate_main(candidate: &str) -> String`; `pub fn compose_program(candidate: &str, tests_source: &str) -> anyhow::Result<String>`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-corpus/src/humaneval_runner/compose.rs` with ONLY the doc comment and test module:

```rust
//! Compose a runnable Vox program from a candidate solution plus a fixture's
//! assertion block.
//!
//! A fixture's `tests.vox` embeds the reference implementation followed by
//! `fn main()` holding the assertions. Grading a candidate means taking the
//! candidate's code plus that `main()` — and none of the reference. Getting this
//! wrong in the permissive direction (leaving the reference in) would make every
//! candidate pass, so the extraction is fail-closed: no `fn main()` is an error,
//! never a silently-empty test.

#[cfg(test)]
mod tests {
    use super::*;

    const TESTS_SRC: &str = r#"fn fizzbuzz(n: int) to list[str] {
    return []
}

fn main() to str {
    assert(fizzbuzz(3) == ["1", "2", "Fizz"])
    return "ok"
}
"#;

    #[test]
    fn extract_test_block_takes_main_and_drops_the_reference() {
        let block = extract_test_block(TESTS_SRC).expect("has a main");
        assert!(block.starts_with("fn main() to str {"));
        assert!(block.contains("assert(fizzbuzz(3)"));
        assert!(
            !block.contains("return []"),
            "the reference implementation body must not survive extraction"
        );
    }

    #[test]
    fn extract_test_block_fails_closed_when_there_is_no_main() {
        let err = extract_test_block("fn helper() to int { return 1 }").unwrap_err();
        assert!(
            err.to_string().contains("fn main"),
            "error must name what is missing, got: {err}"
        );
    }

    #[test]
    fn strip_candidate_main_removes_a_model_authored_main() {
        // Models often append their own `fn main()` demo; keeping it would
        // duplicate the symbol and fail compilation for a reason unrelated to
        // whether the model solved the problem.
        let candidate = "fn f() to int {\n    return 1\n}\n\nfn main() to str {\n    return \"demo\"\n}\n";
        let stripped = strip_candidate_main(candidate);
        assert!(stripped.contains("fn f() to int"));
        assert!(!stripped.contains("fn main"));
    }

    #[test]
    fn strip_candidate_main_is_a_noop_without_a_main() {
        let candidate = "fn f() to int {\n    return 1\n}\n";
        assert_eq!(strip_candidate_main(candidate).trim(), candidate.trim());
    }

    #[test]
    fn compose_program_joins_candidate_and_assertions() {
        let candidate = "fn fizzbuzz(n: int) to list[str] {\n    return [\"1\", \"2\", \"Fizz\"]\n}\n";
        let program = compose_program(candidate, TESTS_SRC).expect("composes");
        assert!(program.contains("return [\"1\", \"2\", \"Fizz\"]"), "candidate body present");
        assert!(program.contains("assert(fizzbuzz(3)"), "assertions present");
        assert_eq!(program.matches("fn main").count(), 1, "exactly one main");
        assert!(
            !program.contains("return []"),
            "the reference body must never reach the composed program"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-corpus humaneval_runner::compose 2>&1 | tail -20`
Expected: FAIL — `cannot find function extract_test_block`.

- [ ] **Step 3: Write minimal implementation**

Insert into `crates/vox-corpus/src/humaneval_runner/compose.rs` above the test module:

```rust
use anyhow::{Result, bail};

/// The assertion block of a fixture: everything from its `fn main(` line to EOF.
///
/// Fixtures place `fn main()` last, after the reference implementation, so a
/// suffix cut is exact for the corpus as authored.
///
/// ponytail: suffix-from-first-`fn main(` rather than brace matching — correct
/// for every fixture in the corpus today (verified by the loader test walking
/// all 164). If a fixture ever puts a helper *after* `main`, switch to brace
/// matching; the failure mode is loud (compile error), not silent.
pub fn extract_test_block(tests_source: &str) -> Result<String> {
    match tests_source.find("\nfn main(") {
        Some(idx) => Ok(tests_source[idx + 1..].to_string()),
        None if tests_source.starts_with("fn main(") => Ok(tests_source.to_string()),
        None => bail!(
            "fixture tests.vox has no `fn main(` assertion block — refusing to \
             grade against an empty test (fail-closed)"
        ),
    }
}

/// Drop a candidate-authored `fn main(...)` and everything after it.
///
/// Models routinely append a demo `main`; keeping it collides with the fixture's
/// own `main` and fails compilation for a reason that has nothing to do with
/// whether the model solved the problem.
#[must_use]
pub fn strip_candidate_main(candidate: &str) -> String {
    if candidate.starts_with("fn main(") {
        return String::new();
    }
    match candidate.find("\nfn main(") {
        Some(idx) => candidate[..idx + 1].to_string(),
        None => candidate.to_string(),
    }
}

/// Build the program that will be compiled and run: the candidate's code,
/// followed by the fixture's assertion block.
pub fn compose_program(candidate: &str, tests_source: &str) -> Result<String> {
    let body = strip_candidate_main(candidate);
    let test_block = extract_test_block(tests_source)?;
    Ok(format!("{}\n\n{}", body.trim_end(), test_block.trim_start()))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-corpus humaneval_runner::compose 2>&1 | tail -20`
Expected: PASS — 5 tests.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/humaneval_runner/compose.rs
git commit -m "feat(vox-corpus): compose candidate solution with fixture assertion block"
```

---

### Task 5: Reward-hacking integrity guard

Artificial Analysis's Coding Agent Index runs a review pass over every attempt that passes its deterministic verifier, zeroing any that edited test files or read the reference solution (spec §C metric family 3). Vox needs the same guard, and can implement it deterministically because the attack surface is narrow: a candidate is a single Vox source string, so any reference to the fixture's own reference/test files is definitionally illegitimate.

**Files:**
- Create: `crates/vox-corpus/src/humaneval_runner/integrity.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub enum IntegrityViolation { ReferencesReferenceSolution, ReferencesTestFile, PerformsFileIo, PerformsNetworkIo }`; `pub fn check_candidate_integrity(candidate: &str) -> Vec<IntegrityViolation>`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-corpus/src/humaneval_runner/integrity.rs` with ONLY the doc comment and test module:

```rust
//! Deterministic reward-hacking guard for candidate solutions.
//!
//! A legitimate candidate is a pure function plus helpers. Reading the reference
//! solution, touching the test file, or reaching the filesystem/network are all
//! ways to pass without solving the problem. Detecting them by source inspection
//! is deterministic — no LLM judge — which keeps the whole correctness path
//! judge-free (spec §A lesson 4).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_solution_has_no_violations() {
        let candidate = "fn nth_prime(n: int) to int {\n    return 2\n}\n";
        assert!(check_candidate_integrity(candidate).is_empty());
    }

    #[test]
    fn reading_the_reference_solution_is_a_violation() {
        let candidate = "fn f() to str {\n    return std.fs.read(\"reference.vox\")\n}\n";
        let v = check_candidate_integrity(candidate);
        assert!(v.contains(&IntegrityViolation::ReferencesReferenceSolution));
    }

    #[test]
    fn touching_the_test_file_is_a_violation() {
        let candidate = "fn f() to str {\n    return \"tests.vox\"\n}\n";
        let v = check_candidate_integrity(candidate);
        assert!(v.contains(&IntegrityViolation::ReferencesTestFile));
    }

    #[test]
    fn filesystem_and_network_access_are_violations() {
        let fs_candidate = "fn f() to str {\n    return std.fs.read(\"data.txt\")\n}\n";
        assert!(check_candidate_integrity(fs_candidate).contains(&IntegrityViolation::PerformsFileIo));

        let net_candidate = "fn f() to str {\n    return http.get(\"https://example.com\")\n}\n";
        assert!(
            check_candidate_integrity(net_candidate).contains(&IntegrityViolation::PerformsNetworkIo)
        );
    }

    #[test]
    fn violations_are_deduplicated() {
        let candidate =
            "fn f() to str {\n    let a = std.fs.read(\"x\")\n    let b = std.fs.read(\"y\")\n    return a\n}\n";
        let v = check_candidate_integrity(candidate);
        assert_eq!(
            v.iter().filter(|x| **x == IntegrityViolation::PerformsFileIo).count(),
            1,
            "one violation per kind, not one per occurrence"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-corpus humaneval_runner::integrity 2>&1 | tail -20`
Expected: FAIL — `cannot find function check_candidate_integrity`.

- [ ] **Step 3: Write minimal implementation**

Insert into `crates/vox-corpus/src/humaneval_runner/integrity.rs` above the test module:

```rust
use serde::{Deserialize, Serialize};

/// A way a candidate could pass without having solved the problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityViolation {
    /// Mentions the fixture's reference solution.
    ReferencesReferenceSolution,
    /// Mentions the fixture's test file.
    ReferencesTestFile,
    /// Reaches the filesystem — no corpus problem requires it.
    PerformsFileIo,
    /// Reaches the network — no corpus problem requires it.
    PerformsNetworkIo,
}

/// Source markers that indicate each violation kind. Checked as substrings
/// because a candidate is a single self-contained source string.
const MARKERS: &[(&str, IntegrityViolation)] = &[
    ("reference.vox", IntegrityViolation::ReferencesReferenceSolution),
    ("tests.vox", IntegrityViolation::ReferencesTestFile),
    ("std.fs.", IntegrityViolation::PerformsFileIo),
    ("std.process.", IntegrityViolation::PerformsFileIo),
    ("http.", IntegrityViolation::PerformsNetworkIo),
    ("net.", IntegrityViolation::PerformsNetworkIo),
    ("fetch(", IntegrityViolation::PerformsNetworkIo),
];

/// Inspect a candidate solution for reward-hacking markers.
///
/// Returns each violation kind at most once. An attempt with any violation is
/// scored as a failure regardless of its exit code — passing by cheating is not
/// passing.
#[must_use]
pub fn check_candidate_integrity(candidate: &str) -> Vec<IntegrityViolation> {
    let mut found: Vec<IntegrityViolation> = Vec::new();
    for (marker, violation) in MARKERS {
        if candidate.contains(marker) && !found.contains(violation) {
            found.push(*violation);
        }
    }
    found
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-corpus humaneval_runner::integrity 2>&1 | tail -20`
Expected: PASS — 5 tests.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/humaneval_runner/integrity.rs
git commit -m "feat(vox-corpus): deterministic reward-hacking guard for candidate solutions"
```

---

### Task 6: Sandboxed subprocess verification

**Files:**
- Create: `crates/vox-corpus/src/humaneval_runner/verify.rs`

**Interfaces:**
- Consumes: `compose_program` (Task 4), `check_candidate_integrity` (Task 5).
- Produces: `pub struct VerifyOutcome { pub compiled: bool, pub tests_passed: bool, pub violations: Vec<IntegrityViolation>, pub detail: String }`; `pub fn verify_program(vox_bin: &Path, program: &str, candidate: &str, workdir: &Path, timeout: Duration) -> anyhow::Result<VerifyOutcome>`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-corpus/src/humaneval_runner/verify.rs` with ONLY the doc comment and test module:

```rust
//! Verify a composed program by running the real Vox toolchain in a subprocess.
//!
//! Two calls, so compile success and test success stay separable metrics:
//! `vox check` (does it compile?) then `vox run --mode interp` (do the assertions
//! hold?). Subprocess isolation also means a generated infinite loop is a
//! timeout, not a hung benchmark run.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_violation_short_circuits_before_any_subprocess() {
        // A cheating candidate is scored failed WITHOUT executing anything, so
        // this test needs no vox binary on disk.
        let dir = std::env::temp_dir().join(format!("vox-verify-cheat-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let outcome = verify_program(
            std::path::Path::new("definitely-not-a-real-binary"),
            "fn main() to str { return \"ok\" }",
            "fn f() to str { return std.fs.read(\"reference.vox\") }",
            &dir,
            std::time::Duration::from_secs(5),
        )
        .expect("integrity path must not error");
        assert!(!outcome.compiled);
        assert!(!outcome.tests_passed);
        assert!(outcome.violations.contains(&IntegrityViolation::ReferencesReferenceSolution));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_binary_is_an_error_not_a_silent_failure() {
        // A missing toolchain must NOT look like "the model failed" — that would
        // silently report 0% for every model.
        let dir = std::env::temp_dir().join(format!("vox-verify-nobin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = verify_program(
            std::path::Path::new("definitely-not-a-real-binary"),
            "fn main() to str { return \"ok\" }",
            "fn f() to int { return 1 }",
            &dir,
            std::time::Duration::from_secs(5),
        );
        assert!(result.is_err(), "a missing vox binary must surface as Err");
        std::fs::remove_dir_all(&dir).ok();
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-corpus humaneval_runner::verify 2>&1 | tail -20`
Expected: FAIL — `cannot find function verify_program`.

- [ ] **Step 3: Write minimal implementation**

Insert into `crates/vox-corpus/src/humaneval_runner/verify.rs` above the test module:

```rust
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::integrity::{IntegrityViolation, check_candidate_integrity};

/// Result of verifying one composed program.
#[derive(Debug, Clone)]
pub struct VerifyOutcome {
    /// `vox check` exited 0.
    pub compiled: bool,
    /// `vox run` exited 0 (every assertion held).
    pub tests_passed: bool,
    /// Reward-hacking markers found in the candidate; any entry forces a failure.
    pub violations: Vec<IntegrityViolation>,
    /// Short human-readable reason, for the report's failure column.
    pub detail: String,
}

/// Compile and run `program`, returning exit-code-derived facts.
///
/// `candidate` is the model's raw output (pre-composition) and is what the
/// integrity guard inspects — checking the composed program instead would flag
/// the fixture's own assertions.
///
/// Returns `Err` when the toolchain itself is unusable (missing binary, unwritable
/// workdir). That distinction matters: a broken harness must never be reported as
/// a model scoring 0%.
pub fn verify_program(
    vox_bin: &Path,
    program: &str,
    candidate: &str,
    workdir: &Path,
    timeout: Duration,
) -> Result<VerifyOutcome> {
    let violations = check_candidate_integrity(candidate);
    if !violations.is_empty() {
        return Ok(VerifyOutcome {
            compiled: false,
            tests_passed: false,
            detail: format!("integrity violations: {violations:?}"),
            violations,
        });
    }

    std::fs::create_dir_all(workdir)
        .with_context(|| format!("creating workdir {}", workdir.display()))?;
    let src = workdir.join("candidate.vox");
    std::fs::write(&src, program).with_context(|| format!("writing {}", src.display()))?;

    let check = run_with_timeout(vox_bin, &["check", &src.to_string_lossy()], timeout)?;
    if !check.success {
        return Ok(VerifyOutcome {
            compiled: false,
            tests_passed: false,
            violations,
            detail: first_line(&check.stderr, "compile failed"),
        });
    }

    let run = run_with_timeout(
        vox_bin,
        &["run", "--mode", "interp", &src.to_string_lossy()],
        timeout,
    )?;
    Ok(VerifyOutcome {
        compiled: true,
        tests_passed: run.success,
        violations,
        detail: if run.success {
            String::new()
        } else {
            first_line(&run.stderr, "assertion failed or timed out")
        },
    })
}

struct ProcOutcome {
    success: bool,
    stderr: String,
}

/// Spawn `vox_bin` with `args`, killing it if it outlives `timeout`.
///
/// ponytail: 50 ms poll loop rather than a async/wait-with-timeout dependency —
/// the benchmark makes a few hundred of these calls, so poll granularity is
/// irrelevant next to process startup, and it adds no dependency.
fn run_with_timeout(vox_bin: &Path, args: &[&str], timeout: Duration) -> Result<ProcOutcome> {
    let mut child = Command::new(vox_bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!(
                "spawning vox toolchain at {} — build it with \
                 `cargo build -p vox-cli --release` before running the benchmark",
                vox_bin.display()
            )
        })?;

    let started = Instant::now();
    loop {
        match child.try_wait().context("polling vox subprocess")? {
            Some(_) => break,
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(ProcOutcome {
                    success: false,
                    stderr: format!("timed out after {}s", timeout.as_secs()),
                });
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }

    let out = child.wait_with_output().context("collecting vox output")?;
    if out.status.code().is_none() && !out.status.success() {
        bail!("vox subprocess terminated by signal: {:?}", out.status);
    }
    Ok(ProcOutcome {
        success: out.status.success(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    })
}

/// First non-empty line of `text`, truncated to 200 chars, or `fallback`.
fn first_line(text: &str, fallback: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(200).collect::<String>())
        .unwrap_or_else(|| fallback.to_string())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-corpus humaneval_runner 2>&1 | tail -20`
Expected: PASS — all Phase B tests (manifest 3, compose 5, integrity 5, verify 2).

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/humaneval_runner/verify.rs
git commit -m "feat(vox-corpus): sandboxed subprocess verification with timeout"
```

---

## Phase C — CLI runner (`vox-cli`, layer 4)

### Task 7: `vox model eval-corpus` with offline directory ingest

Offline ingest comes FIRST because it is what makes external harnesses (Claude Code, Cursor, Warp, Grok) comparable at all: we cannot drive their UIs from Rust, but we can score solution files they produced, with the identical scorer and verifier. This is also how the runner stays testable without API credentials.

**Files:**
- Create: `crates/vox-cli/src/commands/model/eval_corpus.rs`
- Modify: `crates/vox-cli/src/commands/model/mod.rs`

**Interfaces:**
- Consumes: `vox_corpus::humaneval_runner::{load_corpus, held_out, eligible_after, compose_program, verify_program}`, `vox_eval::corpus_score::{AttemptOutcome, FixtureOutcome, score_corpus}`, `vox_eval::corpus_stats::{wilson_interval, Z_95}`.
- Produces: `pub struct EvalCorpusArgs`; `pub async fn run(args: EvalCorpusArgs) -> anyhow::Result<()>`; `pub fn resolve_vox_binary(explicit: Option<&Path>) -> anyhow::Result<PathBuf>`; `pub fn load_solution_dir(dir: &Path) -> anyhow::Result<HashMap<String, String>>`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/src/commands/model/eval_corpus.rs` with the args struct declared but the body unimplemented, plus the test module:

```rust
//! `vox model eval-corpus` — score a (model, harness, config) tuple against the
//! held-out HumanEval-Vox corpus, verified by compiler and test exit codes only.
//!
//! Two input modes:
//!   * `--from-dir <dir>`: score pre-generated solutions. This is how external
//!     harnesses (Claude Code, Cursor, Warp, Grok) enter the leaderboard — we
//!     cannot drive their UIs, but we can score their output with the identical
//!     verifier, which is what makes the comparison fair.
//!   * live generation (default): generate through the `vox_actor_runtime::llm`
//!     facade for a registry model id.
//!
//! Rows are keyed by the tuple `(model_id, harness_id, config_digest)`, never by
//! bare model name: the same base model scores differently under different
//! scaffolding, so collapsing them would publish a misleading comparison.

use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// `vox model eval-corpus` arguments.
#[derive(Parser, Debug, Clone)]
pub struct EvalCorpusArgs {
    /// Model id to score (registry id for live mode, or a label for `--from-dir`).
    #[arg(long)]
    pub model: String,
    /// Harness that produced the solutions (e.g. `vox-harness`, `claude-code`, `cursor`, `warp`).
    #[arg(long, default_value = "vox-harness")]
    pub harness: String,
    /// Score pre-generated `<fixture-id>.vox` solutions from this directory instead of generating.
    #[arg(long)]
    pub from_dir: Option<PathBuf>,
    /// Corpus root.
    #[arg(long, default_value = "contracts/eval/humaneval-vox")]
    pub corpus: PathBuf,
    /// Score only fixtures added strictly after this ISO date (the model's training cutoff).
    #[arg(long)]
    pub cutoff: Option<String>,
    /// Include training-eligible fixtures. Off by default: only held-out fixtures
    /// support an external claim.
    #[arg(long, default_value_t = false)]
    pub include_training_eligible: bool,
    /// Attempts per fixture (pass@k). Pinned to 5 by `contracts/eval/README.md`.
    #[arg(long, default_value_t = 5)]
    pub attempts: usize,
    /// Path to the `vox` binary. Defaults to `target/release/vox[.exe]`.
    #[arg(long)]
    pub vox_bin: Option<PathBuf>,
    /// Per-subprocess timeout in seconds.
    #[arg(long, default_value_t = 30)]
    pub timeout_secs: u64,
    /// Write the run report JSON here.
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Skip the `model_scoreboard` write-back.
    #[arg(long, default_value_t = false)]
    pub no_write_back: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_solution_dir_keys_by_fixture_id_prefix() {
        let dir = std::env::temp_dir().join(format!("vox-soldir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("041.vox"), "fn nth_prime(n: int) to int { return 2 }").unwrap();
        std::fs::write(dir.join("043-triangle-valid.vox"), "fn triangle_valid() to bool { return true }").unwrap();
        std::fs::write(dir.join("notes.md"), "ignored").unwrap();

        let sols = load_solution_dir(&dir).expect("loads");
        assert_eq!(sols.len(), 2, "non-.vox files are ignored");
        assert!(sols.contains_key("041"));
        assert!(sols.contains_key("043"), "slug suffix is stripped to the id");
        std::fs::remove_dir_all(&dir).ok();
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
             workspace OOMs rustc on a contended machine, and the crash does not \
             look like an out-of-memory error"
        );
    }

    #[test]
    fn resolve_vox_binary_accepts_an_explicit_existing_path() {
        // Any existing file resolves; the caller chose it deliberately.
        let existing = std::env::current_exe().expect("test binary path");
        assert_eq!(
            resolve_vox_binary(Some(&existing)).expect("resolves"),
            existing
        );
    }

    #[test]
    fn config_digest_is_stable_and_tuple_sensitive() {
        let a = config_digest("vox-harness", 5, Some("2026-05-26"));
        let b = config_digest("vox-harness", 5, Some("2026-05-26"));
        let c = config_digest("claude-code", 5, Some("2026-05-26"));
        assert_eq!(a, b, "same inputs -> same digest");
        assert_ne!(a, c, "different harness -> different digest");
    }
}
```

Register the subcommand. In `crates/vox-cli/src/commands/model/mod.rs`:

Add to the `pub mod` list (after `pub mod eval;`):
```rust
pub mod eval_corpus;
```

Add to the `ModelCmd` enum (after the `Eval(eval::EvalArgs),` arm):
```rust
    /// Score a model or external harness against the held-out HumanEval-Vox corpus.
    EvalCorpus(eval_corpus::EvalCorpusArgs),
```

Add to the `run` match (after the `ModelCmd::Eval(args) => eval::run(args).await,` arm):
```rust
        ModelCmd::EvalCorpus(args) => eval_corpus::run(args).await,
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli eval_corpus 2>&1 | tail -20`
Expected: FAIL — `cannot find function load_solution_dir`, `resolve_vox_binary`, `config_digest`, `run`.

- [ ] **Step 3: Write minimal implementation**

Insert into `crates/vox-cli/src/commands/model/eval_corpus.rs`, between the `EvalCorpusArgs` struct and the test module:

```rust
/// Load `<fixture-id>[-slug].vox` solutions from a directory, keyed by fixture id.
///
/// Non-`.vox` files are ignored so a harness can drop READMEs or logs alongside
/// its output without breaking the ingest.
pub fn load_solution_dir(dir: &Path) -> Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("reading solution dir {}", dir.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("vox") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        // `041.vox` and `041-nth-prime.vox` both key to `041`.
        let id = stem.split('-').next().unwrap_or(&stem).to_string();
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        out.insert(id, source);
    }
    Ok(out)
}

/// Resolve the `vox` binary to verify with: an explicit path, else the release
/// build, else the dev build.
///
/// The dev fallback is deliberate. A release build of this workspace needs
/// multi-GB of RAM to borrow-check `vox-orchestrator-mcp` at `opt-level=3` with
/// LTO, and on a contended machine rustc dies with an allocation failure
/// (`STATUS_STACK_BUFFER_OVERRUN` / `0xc0000409`) that looks nothing like an
/// out-of-memory error. Verification only needs `vox check` and `vox run` to be
/// *correct*, not fast, so a dev binary is a fully valid verifier — and
/// requiring `--release` would strand an operator behind a confusing crash for
/// no measurement benefit. Latency numbers come from the generation call, not
/// from this subprocess, so the profile does not affect any published metric.
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

/// Stable short digest of the scored configuration.
///
/// Published alongside every row so a reader can tell which exact configuration
/// produced a score — the governance requirement that the Leaderboard Illusion
/// findings make non-optional (spec §A lesson 2).
#[must_use]
pub fn config_digest(harness: &str, attempts: usize, cutoff: Option<&str>) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    harness.hash(&mut h);
    attempts.hash(&mut h);
    cutoff.unwrap_or("none").hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Select the fixtures this run will score, applying the held-out and
/// rolling-window filters.
fn select_fixtures(
    all: &[vox_corpus::humaneval_runner::Fixture],
    args: &EvalCorpusArgs,
) -> Vec<vox_corpus::humaneval_runner::Fixture> {
    use vox_corpus::humaneval_runner::{eligible_after, held_out};
    let base: Vec<&vox_corpus::humaneval_runner::Fixture> = if args.include_training_eligible {
        all.iter().collect()
    } else {
        held_out(all)
    };
    match &args.cutoff {
        Some(cutoff) => {
            let owned: Vec<vox_corpus::humaneval_runner::Fixture> =
                base.into_iter().cloned().collect();
            eligible_after(&owned, cutoff).into_iter().cloned().collect()
        }
        None => base.into_iter().cloned().collect(),
    }
}

pub async fn run(args: EvalCorpusArgs) -> Result<()> {
    use vox_corpus::humaneval_runner::{compose_program, load_corpus, verify_program};
    use vox_eval::corpus_score::{AttemptOutcome, FixtureOutcome, score_corpus};
    use vox_eval::corpus_stats::{Z_95, wilson_interval};

    let all = load_corpus(&args.corpus)?;
    let fixtures = select_fixtures(&all, &args);
    anyhow::ensure!(
        !fixtures.is_empty(),
        "no fixtures selected — a --cutoff after the corpus's newest problem \
         leaves nothing scoreable, which is the honest result, not an error to paper over"
    );

    let vox_bin = resolve_vox_binary(args.vox_bin.as_deref())?;
    let timeout = std::time::Duration::from_secs(args.timeout_secs);
    let workdir = std::env::temp_dir().join(format!("vox-eval-corpus-{}", std::process::id()));

    let solutions = match &args.from_dir {
        Some(dir) => Some(load_solution_dir(dir)?),
        None => None,
    };

    println!(
        "Scoring {} / {} ({} fixtures, {} attempts each)",
        args.model,
        args.harness,
        fixtures.len(),
        args.attempts
    );

    let mut outcomes: Vec<FixtureOutcome> = Vec::with_capacity(fixtures.len());
    for fixture in &fixtures {
        let tests_source = std::fs::read_to_string(&fixture.tests_path)
            .with_context(|| format!("reading {}", fixture.tests_path.display()))?;

        let mut attempts = Vec::new();
        let n_attempts = if solutions.is_some() { 1 } else { args.attempts };
        for _ in 0..n_attempts {
            let (candidate, tokens, latency_ms, cost_usd) = match &solutions {
                // Offline ingest: the solution already exists; no generation cost.
                Some(map) => match map.get(&fixture.id) {
                    Some(src) => (src.clone(), 0u32, 0i64, None),
                    None => {
                        // A missing solution is a miss, not a skip: skipping would
                        // shrink the denominator and inflate the pass rate.
                        attempts.push(AttemptOutcome {
                            compiled: false,
                            tests_passed: false,
                            total_tokens: 0,
                            latency_ms: 0,
                            cost_usd: None,
                        });
                        continue;
                    }
                },
                None => anyhow::bail!(
                    "live generation lands in the next task; pass --from-dir for now"
                ),
            };

            let program = compose_program(&candidate, &tests_source)?;
            let outcome = verify_program(&vox_bin, &program, &candidate, &workdir, timeout)?;
            attempts.push(AttemptOutcome {
                compiled: outcome.compiled,
                tests_passed: outcome.tests_passed,
                total_tokens: tokens,
                latency_ms,
                cost_usd,
            });
            if outcome.tests_passed {
                break; // no need to burn further attempts once it passes
            }
        }
        outcomes.push(FixtureOutcome {
            fixture_id: fixture.id.clone(),
            attempts,
        });
    }

    let score = score_corpus(&outcomes);
    let ci = wilson_interval(score.n_passed_at_1, score.n_fixtures, Z_95);
    println!(
        "pass@1 {:.1}% (95% CI {:.1}%–{:.1}%) | pass@{} {:.1}% | compile {:.1}%",
        score.pass_at_1 * 100.0,
        ci.low * 100.0,
        ci.high * 100.0,
        score.k,
        score.pass_at_k * 100.0,
        score.compile_rate * 100.0
    );

    if let Some(path) = &args.output {
        let report = serde_json::json!({
            "schema_version": 1,
            "model_id": args.model,
            "harness_id": args.harness,
            "config_digest": config_digest(&args.harness, args.attempts, args.cutoff.as_deref()),
            "cutoff": args.cutoff,
            "held_out_only": !args.include_training_eligible,
            "score": score,
            "pass_at_1_ci": ci,
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
```

**No `Cargo.toml` change is needed.** Verified: `crates/vox-cli/Cargo.toml` already declares `vox-eval` (line 170), `vox-db` (line 185), `vox-publisher` (line 201), and `vox-corpus` (line 251). This plan introduces **zero new workspace crate edges** — every dependency it relies on already exists, so `vox ci crate-edges` cannot reject it and no ledger entry is ever needed.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli eval_corpus 2>&1 | tail -20`
Expected: PASS — 3 tests.

- [ ] **Step 5: Verify crate edges and sync the CLI registry**

```bash
cargo run -q -p vox-cli -- ci crate-edges
cargo run -q -p vox-cli -- ci command-sync --write
UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli command_catalog
```

Expected: `crate-edges` PASS (vox-cli L4 → vox-corpus L3 and vox-eval L2 are downward edges). If it fails, STOP and report — do not author an `exceptions` entry.

- [ ] **Step 6: Smoke-test the real path end to end**

Build a `vox` binary first. If rustc dies with `STATUS_STACK_BUFFER_OVERRUN` / exit
code `0xc0000409` and a backtrace containing `rust_oom` / `handle_alloc_error`,
that is **memory exhaustion, not a code error** — this workspace's release build
needs multi-GB to borrow-check `vox-orchestrator-mcp` at `opt-level=3` with LTO.
Retry with `-j 2`, or build the dev profile (`cargo build -p vox-cli`), which the
resolver accepts as a fallback.

```bash
cargo build -p vox-cli --release -j 2
mkdir -p /tmp/vox-sol-demo
cp contracts/eval/humaneval-vox/problems/041-nth-prime/reference.vox /tmp/vox-sol-demo/041.vox
cargo run -q -p vox-cli --release -- model eval-corpus --model reference-oracle --harness oracle --from-dir /tmp/vox-sol-demo --cutoff 2026-05-26
```

Expected: the reference solution for 041 passes (`pass@1` counts 1 of the eligible fixtures; every other held-out fixture reports a miss because no solution file was supplied). This is the harness's own sanity check: if a known-correct reference solution does not pass, the composition or verification path is broken, not the model.

- [ ] **Step 7: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/model/eval_corpus.rs crates/vox-cli/src/commands/model/mod.rs crates/vox-cli/Cargo.toml contracts/cli/command-registry.yaml crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt docs/src/reference/cli-command-surface.generated.md
git commit -m "feat(vox-cli): vox model eval-corpus with external-harness solution ingest"
```

---

### Task 8: Live generation through the LLM facade

**Files:**
- Modify: `crates/vox-cli/src/commands/model/eval_corpus.rs`

**Interfaces:**
- Consumes: `EvalCorpusArgs`, `resolve_vox_binary` (Task 7).
- Produces: `pub fn build_generation_prompt(fixture: &Fixture) -> String`; `pub fn extract_vox_code(completion: &str) -> String`; `async fn generate_candidate(model_id: &str, fixture: &Fixture) -> Result<(String, u32, i64, Option<f64>), String>`.

- [ ] **Step 1: Write the failing test**

Add these tests to the existing `#[cfg(test)] mod tests` block in `crates/vox-cli/src/commands/model/eval_corpus.rs`:

```rust
    fn demo_fixture() -> vox_corpus::humaneval_runner::Fixture {
        vox_corpus::humaneval_runner::Fixture {
            id: "041".to_string(),
            slug: "041-nth-prime".to_string(),
            training_eligible: false,
            added_at: "2026-05-26".to_string(),
            prompt: "Return the nth prime number (1-indexed).".to_string(),
            signature: "fn nth_prime(n: int) to int".to_string(),
            tests_path: std::path::PathBuf::from("unused"),
        }
    }

    #[test]
    fn generation_prompt_carries_signature_and_forbids_a_main() {
        let p = build_generation_prompt(&demo_fixture());
        assert!(p.contains("fn nth_prime(n: int) to int"), "exact signature is pinned");
        assert!(p.contains("Return the nth prime number"), "task prompt included");
        assert!(
            p.to_lowercase().contains("do not") && p.to_lowercase().contains("main"),
            "must instruct the model not to emit its own main"
        );
    }

    #[test]
    fn extract_vox_code_unwraps_a_fenced_block() {
        let completion = "Here you go:\n\n```vox\nfn f() to int {\n    return 1\n}\n```\n\nHope that helps!";
        let code = extract_vox_code(completion);
        assert!(code.starts_with("fn f() to int"));
        assert!(!code.contains("```"), "fences stripped");
        assert!(!code.contains("Hope that helps"), "prose after the fence dropped");
    }

    #[test]
    fn extract_vox_code_handles_an_untagged_fence() {
        let completion = "```\nfn f() to int {\n    return 1\n}\n```";
        assert!(extract_vox_code(completion).starts_with("fn f() to int"));
    }

    #[test]
    fn extract_vox_code_passes_through_bare_code() {
        let completion = "fn f() to int {\n    return 1\n}\n";
        assert_eq!(extract_vox_code(completion).trim(), completion.trim());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli eval_corpus 2>&1 | tail -20`
Expected: FAIL — `cannot find function build_generation_prompt`, `extract_vox_code`.

- [ ] **Step 3: Write minimal implementation**

Insert into `crates/vox-cli/src/commands/model/eval_corpus.rs`, above the test module:

```rust
use vox_corpus::humaneval_runner::Fixture;

/// Build the single-turn prompt for one fixture.
///
/// The signature is pinned verbatim so a correct solution under a different name
/// is not scored as a failure of capability, and the model is told not to emit
/// its own `main` (the composer strips one anyway, but not asking for it saves
/// tokens and reduces variance).
#[must_use]
pub fn build_generation_prompt(fixture: &Fixture) -> String {
    format!(
        "Write a Vox function with EXACTLY this signature:\n\n    {}\n\n\
         Task: {}\n\n\
         Rules:\n\
         - Reply with ONLY Vox source code, no prose and no explanation.\n\
         - Do NOT write a `fn main()`; only the function above plus any helpers it needs.\n\
         - Do not read files or access the network.",
        fixture.signature, fixture.prompt
    )
}

/// Pull Vox source out of a completion, unwrapping a fenced block when present.
#[must_use]
pub fn extract_vox_code(completion: &str) -> String {
    let Some(open) = completion.find("```") else {
        return completion.trim().to_string();
    };
    // Skip the fence and any language tag on the same line.
    let after_fence = &completion[open + 3..];
    let body_start = after_fence.find('\n').map_or(0, |i| i + 1);
    let body = &after_fence[body_start..];
    match body.find("```") {
        Some(close) => body[..close].trim().to_string(),
        None => body.trim().to_string(),
    }
}

/// Generate one candidate solution through the model-agnostic LLM facade.
///
/// Returns `(source, total_tokens, latency_ms, cost_usd)`. All LLM traffic goes
/// through `vox_actor_runtime::llm` per the workspace's model-agnostic boundary —
/// never a vendor SDK or hostname.
async fn generate_candidate(
    model_id: &str,
    fixture: &Fixture,
) -> Result<(String, u32, i64, Option<f64>), String> {
    use vox_actor_runtime::ActivityOptions;
    use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig, llm_chat};

    let mut config = LlmConfig::openrouter(model_id);
    config.max_tokens = Some(1024);
    // Pinned by contracts/eval/README.md for reproducibility.
    config.temperature = Some(0.0);
    config.telemetry_task_category = Some("eval-corpus".to_string());

    let messages = vec![LlmChatMessage {
        role: "user".to_string(),
        content: build_generation_prompt(fixture),
        ..Default::default()
    }];

    let started = std::time::Instant::now();
    let outcome = llm_chat(&ActivityOptions::new(), messages, config).await;
    let latency_ms = started.elapsed().as_millis() as i64;

    let response = match outcome {
        vox_actor_runtime::ActivityResult::Ok(inner) => inner,
        vox_actor_runtime::ActivityResult::Failed(e) => return Err(e.to_string()),
        vox_actor_runtime::ActivityResult::Cancelled => return Err("activity cancelled".into()),
    }
    .map_err(|e| e.to_string())?;

    Ok((
        extract_vox_code(&response.content),
        response.prompt_tokens + response.completion_tokens,
        latency_ms,
        response.cost_usd,
    ))
}
```

Replace the `None => anyhow::bail!("live generation lands in the next task; pass --from-dir for now"),` arm in `run` with:

```rust
                None => match generate_candidate(&args.model, fixture).await {
                    Ok(v) => v,
                    Err(reason) => {
                        // A provider error is a failed attempt, recorded with its
                        // latency — not a skip, which would shrink the denominator.
                        tracing::warn!(model = %args.model, fixture = %fixture.id, error = %reason, "generation failed");
                        attempts.push(AttemptOutcome {
                            compiled: false,
                            tests_passed: false,
                            total_tokens: 0,
                            latency_ms: 0,
                            cost_usd: None,
                        });
                        continue;
                    }
                },
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli eval_corpus 2>&1 | tail -20`
Expected: PASS — 7 tests.

- [ ] **Step 5: Verify the LLM-boundary detector is satisfied**

Run: `cargo run -q -p vox-code-audit -- --path crates/vox-cli/src/commands/model/eval_corpus.rs 2>&1 | tail -20`
Expected: no `llm_provider_call` finding. All traffic goes through `vox_actor_runtime::llm`.

- [ ] **Step 6: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/model/eval_corpus.rs
git commit -m "feat(vox-cli): live corpus generation via the model-agnostic LLM facade"
```

---

### Task 9: Scoreboard write-back so the router learns from measured quality

Today `quality_score` in the live scorer is `log10(max_tokens)` blended with a free/paid constant (`crates/vox-orchestrator/src/models/scoring.rs:152-162`) — a heuristic, not a measurement. This task feeds it a real number.

**Files:**
- Modify: `crates/vox-cli/src/commands/model/eval_corpus.rs`

**Interfaces:**
- Consumes: `CorpusScore` (Task 1), `EvalCorpusArgs` (Task 7).
- Produces: `pub fn scoreboard_row_from_corpus(model_id: &str, harness_id: &str, score: &CorpusScore) -> ModelScoreboardRow`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `eval_corpus.rs`:

```rust
    #[test]
    fn scoreboard_row_uses_measured_pass_rate_as_quality() {
        use vox_eval::corpus_score::{AttemptOutcome, FixtureOutcome, score_corpus};
        let pass = AttemptOutcome {
            compiled: true,
            tests_passed: true,
            total_tokens: 100,
            latency_ms: 250,
            cost_usd: Some(0.01),
        };
        let fail = AttemptOutcome { tests_passed: false, ..pass.clone() };
        let outcomes = vec![
            FixtureOutcome { fixture_id: "a".into(), attempts: vec![pass.clone()] },
            FixtureOutcome { fixture_id: "b".into(), attempts: vec![pass.clone()] },
            FixtureOutcome { fixture_id: "c".into(), attempts: vec![fail] },
        ];
        let score = score_corpus(&outcomes);
        let row = scoreboard_row_from_corpus("moonshot/kimi-k2.6-thinking", "vox-harness", &score);

        assert_eq!(row.model_id, "moonshot/kimi-k2.6-thinking");
        assert_eq!(
            row.task_category, "vox-codegen",
            "corpus results must not overwrite the general-purpose category"
        );
        assert_eq!(row.strength_tag, "vox-harness", "harness travels with the row");
        assert!(
            (row.quality_score - (2.0 / 3.0)).abs() < 1e-9,
            "quality is the MEASURED pass@1, not a token-count heuristic"
        );
        assert!((row.success_rate - (2.0 / 3.0)).abs() < 1e-9);
        assert_eq!(row.success_count, 2);
        assert_eq!(row.n_calls, 3);
        assert_eq!(row.p50_latency_ms, Some(score.p50_ms));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli eval_corpus::tests::scoreboard 2>&1 | tail -20`
Expected: FAIL — `cannot find function scoreboard_row_from_corpus`.

- [ ] **Step 3: Write minimal implementation**

Insert into `eval_corpus.rs` above the test module:

```rust
use vox_db::store::types::ModelScoreboardRow;
use vox_eval::corpus_score::CorpusScore;

/// Build the scoreboard row recording a corpus run.
///
/// Written under `task_category = "vox-codegen"` so it never overwrites the
/// general-purpose row `vox model eval` maintains, and with `strength_tag` set to
/// the harness so the same base model under two harnesses stays two rows.
///
/// `quality_score` is the measured pass@1. That is the point of this whole
/// pipeline: the router's quality axis stops being `log10(max_tokens)` and starts
/// being "did this model actually write working Vox".
#[must_use]
pub fn scoreboard_row_from_corpus(
    model_id: &str,
    harness_id: &str,
    score: &CorpusScore,
) -> ModelScoreboardRow {
    ModelScoreboardRow {
        model_id: model_id.to_string(),
        task_category: "vox-codegen".to_string(),
        strength_tag: harness_id.to_string(),
        window_days: 7,
        n_calls: score.n_fixtures as i64,
        success_rate: score.pass_at_1,
        p50_latency_ms: Some(score.p50_ms),
        p99_latency_ms: Some(score.p99_ms),
        cost_per_success_usd: score.cost_per_success_usd,
        quality_score: score.pass_at_1,
        updated_at_ms: vox_db::now_unix_ms() as i64,
        success_count: score.n_passed_at_1 as i64,
        cumulative_cost_usd: score.cumulative_cost_usd,
    }
}
```

Insert this block into `run`, immediately after the `println!` that prints the pass rates and before the `if let Some(path) = &args.output` block:

```rust
    if !args.no_write_back {
        match vox_db::VoxDb::connect(
            vox_db::DbConfig::resolve_canonical().map_err(anyhow::Error::msg)?,
        )
        .await
        {
            Ok(db) => {
                let row = scoreboard_row_from_corpus(&args.model, &args.harness, &score);
                match db.upsert_model_scoreboard(row).await {
                    Ok(()) => println!("Wrote measured quality to model_scoreboard (vox-codegen)."),
                    Err(e) => tracing::warn!(error = %e, "scoreboard upsert failed"),
                }
            }
            Err(e) => tracing::warn!(error = %e, "DB unavailable; skipping write-back"),
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli eval_corpus 2>&1 | tail -20`
Expected: PASS — 8 tests.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/model/eval_corpus.rs
git commit -m "feat(vox-cli): write measured Vox-codegen pass rate to the model scoreboard"
```

---

## Phase D — Public leaderboard

### Task 10: Leaderboard artifact schema and builder

**Files:**
- Create: `contracts/reports/vox-efficacy/leaderboard.v1.schema.json`
- Create: `crates/vox-cli/src/commands/model/eval_corpus_leaderboard.rs`
- Modify: `crates/vox-cli/src/commands/model/mod.rs`

**Interfaces:**
- Consumes: per-run report JSON files written by `--output` (Task 7).
- Produces: `pub struct LeaderboardRow`; `pub fn build_leaderboard(runs: &[serde_json::Value]) -> Result<serde_json::Value>`; `pub struct LeaderboardArgs`; `pub async fn run(args: LeaderboardArgs) -> Result<()>`.

- [ ] **Step 1: Create the schema**

Create `contracts/reports/vox-efficacy/leaderboard.v1.schema.json`:

```json
{
  "x-vox-version": 1,
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://voxlang.org/contracts/reports/vox-efficacy/leaderboard.v1.schema.json",
  "title": "Vox efficacy leaderboard v1",
  "description": "Published comparative scores for (model, harness, config) tuples on the held-out HumanEval-Vox corpus. Every row carries a confidence interval; rows whose intervals overlap are not a resolvable ranking.",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "generated_at_ms", "corpus", "rows"],
  "properties": {
    "schema_version": { "type": "integer", "const": 1 },
    "generated_at_ms": { "type": "integer" },
    "corpus": {
      "type": "object",
      "additionalProperties": false,
      "required": ["id", "held_out_only", "n_fixtures"],
      "properties": {
        "id": { "type": "string" },
        "held_out_only": { "type": "boolean" },
        "n_fixtures": { "type": "integer" },
        "corpus_hash": { "type": "string" }
      }
    },
    "rows": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": [
          "model_id",
          "harness_id",
          "config_digest",
          "pass_at_1",
          "pass_at_1_ci_low",
          "pass_at_1_ci_high",
          "compile_rate",
          "n_fixtures"
        ],
        "properties": {
          "model_id": { "type": "string" },
          "harness_id": { "type": "string" },
          "config_digest": { "type": "string" },
          "cutoff": { "type": ["string", "null"] },
          "pass_at_1": { "type": "number", "minimum": 0, "maximum": 1 },
          "pass_at_1_ci_low": { "type": "number", "minimum": 0, "maximum": 1 },
          "pass_at_1_ci_high": { "type": "number", "minimum": 0, "maximum": 1 },
          "pass_at_k": { "type": "number", "minimum": 0, "maximum": 1 },
          "compile_rate": { "type": "number", "minimum": 0, "maximum": 1 },
          "n_fixtures": { "type": "integer" },
          "tokens_per_pass": { "type": "number" },
          "p50_ms": { "type": "integer" },
          "cost_per_success_usd": { "type": ["number", "null"] }
        }
      }
    }
  }
}
```

- [ ] **Step 2: Write the failing test**

Create `crates/vox-cli/src/commands/model/eval_corpus_leaderboard.rs` with ONLY the doc comment and test module:

```rust
//! `vox model eval-corpus-leaderboard` — merge per-run reports into the published
//! leaderboard artifact that `docs-astro` renders.
//!
//! Rows are sorted by pass@1 descending, but the artifact carries each row's
//! confidence interval so the renderer can show a "tied within measurement error"
//! band rather than implying a resolvable ranking between overlapping rows.

#[cfg(test)]
mod tests {
    use super::*;

    fn run_json(model: &str, harness: &str, passed: usize, n: usize) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "model_id": model,
            "harness_id": harness,
            "config_digest": "abc123",
            "cutoff": "2026-05-26",
            "held_out_only": true,
            "score": {
                "n_fixtures": n,
                "compile_rate": 1.0,
                "pass_at_1": passed as f64 / n as f64,
                "pass_at_k": passed as f64 / n as f64,
                "k": 1,
                "total_tokens": 1000,
                "tokens_per_pass": 100.0,
                "p50_ms": 250,
                "p99_ms": 900,
                "cumulative_cost_usd": 0.5,
                "cost_per_success_usd": 0.05,
                "n_passed_at_1": passed
            }
        })
    }

    #[test]
    fn leaderboard_sorts_by_pass_at_1_descending() {
        let runs = vec![
            run_json("qwen/qwen3-coder-next-32b", "vox-harness", 10, 31),
            run_json("anthropic/claude-opus-4.7", "claude-code", 25, 31),
            run_json("moonshot/kimi-k2.6-thinking", "vox-harness", 18, 31),
        ];
        let board = build_leaderboard(&runs).expect("builds");
        let rows = board["rows"].as_array().expect("rows array");
        assert_eq!(rows[0]["model_id"], "anthropic/claude-opus-4.7");
        assert_eq!(rows[2]["model_id"], "qwen/qwen3-coder-next-32b");
    }

    #[test]
    fn every_row_carries_a_confidence_interval() {
        let board = build_leaderboard(&[run_json("m", "h", 24, 31)]).expect("builds");
        let row = &board["rows"][0];
        let low = row["pass_at_1_ci_low"].as_f64().expect("ci low");
        let high = row["pass_at_1_ci_high"].as_f64().expect("ci high");
        let point = row["pass_at_1"].as_f64().expect("point");
        assert!(low < point && point < high, "interval brackets the point estimate");
        assert!(low >= 0.0 && high <= 1.0);
    }

    #[test]
    fn same_model_under_two_harnesses_stays_two_rows() {
        // Collapsing these would publish a misleading comparison: the same base
        // model scores differently under different scaffolding.
        let runs = vec![
            run_json("anthropic/claude-opus-4.7", "claude-code", 25, 31),
            run_json("anthropic/claude-opus-4.7", "vox-harness", 22, 31),
        ];
        let board = build_leaderboard(&runs).expect("builds");
        assert_eq!(board["rows"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn empty_input_produces_a_valid_empty_board_not_an_error() {
        let board = build_leaderboard(&[]).expect("builds");
        assert_eq!(board["schema_version"], 1);
        assert_eq!(board["rows"].as_array().unwrap().len(), 0);
    }
}
```

Register the subcommand in `crates/vox-cli/src/commands/model/mod.rs`:

Add to the module list:
```rust
pub mod eval_corpus_leaderboard;
```
Add to `ModelCmd` (after the `EvalCorpus` arm):
```rust
    /// Merge per-run corpus reports into the published leaderboard artifact.
    EvalCorpusLeaderboard(eval_corpus_leaderboard::LeaderboardArgs),
```
Add to the `run` match:
```rust
        ModelCmd::EvalCorpusLeaderboard(args) => eval_corpus_leaderboard::run(args).await,
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-cli eval_corpus_leaderboard 2>&1 | tail -20`
Expected: FAIL — `cannot find function build_leaderboard`.

- [ ] **Step 4: Write minimal implementation**

Insert into `eval_corpus_leaderboard.rs` above the test module:

```rust
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use vox_eval::corpus_stats::{Z_95, wilson_interval};

/// `vox model eval-corpus-leaderboard` arguments.
#[derive(Parser, Debug, Clone)]
pub struct LeaderboardArgs {
    /// Directory of per-run report JSON files (from `eval-corpus --output`).
    #[arg(long, default_value = "contracts/reports/vox-efficacy/runs")]
    pub runs_dir: PathBuf,
    /// Where to write the merged leaderboard artifact.
    #[arg(long, default_value = "contracts/reports/vox-efficacy/leaderboard.v1.json")]
    pub output: PathBuf,
}

/// Merge per-run reports into the leaderboard artifact, sorted by pass@1 descending.
pub fn build_leaderboard(runs: &[serde_json::Value]) -> Result<serde_json::Value> {
    let mut rows: Vec<serde_json::Value> = Vec::with_capacity(runs.len());
    let mut n_fixtures_seen = 0usize;

    for run in runs {
        let score = run.get("score").context("run report has no `score` object")?;
        let n = score
            .get("n_fixtures")
            .and_then(serde_json::Value::as_u64)
            .context("score.n_fixtures missing")? as usize;
        let passed = score
            .get("n_passed_at_1")
            .and_then(serde_json::Value::as_u64)
            .context("score.n_passed_at_1 missing")? as usize;
        n_fixtures_seen = n_fixtures_seen.max(n);

        let ci = wilson_interval(passed, n, Z_95);
        rows.push(serde_json::json!({
            "model_id": run.get("model_id").cloned().unwrap_or(serde_json::Value::Null),
            "harness_id": run.get("harness_id").cloned().unwrap_or(serde_json::Value::Null),
            "config_digest": run.get("config_digest").cloned().unwrap_or(serde_json::Value::Null),
            "cutoff": run.get("cutoff").cloned().unwrap_or(serde_json::Value::Null),
            "pass_at_1": ci.point,
            "pass_at_1_ci_low": ci.low,
            "pass_at_1_ci_high": ci.high,
            "pass_at_k": score.get("pass_at_k").cloned().unwrap_or(serde_json::Value::Null),
            "compile_rate": score.get("compile_rate").cloned().unwrap_or(serde_json::Value::Null),
            "n_fixtures": n,
            "tokens_per_pass": score.get("tokens_per_pass").cloned().unwrap_or(serde_json::Value::Null),
            "p50_ms": score.get("p50_ms").cloned().unwrap_or(serde_json::Value::Null),
            "cost_per_success_usd": score.get("cost_per_success_usd").cloned().unwrap_or(serde_json::Value::Null),
        }));
    }

    rows.sort_by(|a, b| {
        let av = a["pass_at_1"].as_f64().unwrap_or(0.0);
        let bv = b["pass_at_1"].as_f64().unwrap_or(0.0);
        bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(serde_json::json!({
        "schema_version": 1,
        "generated_at_ms": vox_db::now_unix_ms(),
        "corpus": {
            "id": "humaneval-vox",
            "held_out_only": true,
            "n_fixtures": n_fixtures_seen,
        },
        "rows": rows,
    }))
}

pub async fn run(args: LeaderboardArgs) -> Result<()> {
    let mut runs = Vec::new();
    if args.runs_dir.exists() {
        for entry in std::fs::read_dir(&args.runs_dir)
            .with_context(|| format!("reading {}", args.runs_dir.display()))?
        {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            runs.push(
                serde_json::from_str(&raw)
                    .with_context(|| format!("parsing {}", path.display()))?,
            );
        }
    }

    let board = build_leaderboard(&runs)?;
    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.output, serde_json::to_string_pretty(&board)?)?;
    println!(
        "Wrote leaderboard with {} row(s) to {}",
        board["rows"].as_array().map_or(0, Vec::len),
        args.output.display()
    );
    Ok(())
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-cli eval_corpus_leaderboard 2>&1 | tail -20`
Expected: PASS — 4 tests.

- [ ] **Step 6: Sync the registry and commit**

```bash
cargo fmt -p vox-cli
cargo run -q -p vox-cli -- ci command-sync --write
UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli command_catalog
git add crates/vox-cli/src/commands/model/eval_corpus_leaderboard.rs crates/vox-cli/src/commands/model/mod.rs contracts/reports/vox-efficacy/ contracts/cli/command-registry.yaml crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt docs/src/reference/cli-command-surface.generated.md
git commit -m "feat(vox-cli): leaderboard artifact builder with per-row confidence intervals"
```

---

### Task 11: Public leaderboard page on the docs site

**Files:**
- Create: `docs-astro/src/pages/benchmarks.astro`

**Interfaces:**
- Consumes: `contracts/reports/vox-efficacy/leaderboard.v1.json` (Task 10).
- Produces: a static `/benchmarks` route.

- [ ] **Step 1: Create the page**

Create `docs-astro/src/pages/benchmarks.astro`:

```astro
---
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..', '..');
const artifact = join(repoRoot, 'contracts', 'reports', 'vox-efficacy', 'leaderboard.v1.json');

type Row = {
  model_id: string;
  harness_id: string;
  config_digest: string;
  cutoff: string | null;
  pass_at_1: number;
  pass_at_1_ci_low: number;
  pass_at_1_ci_high: number;
  compile_rate: number;
  n_fixtures: number;
  tokens_per_pass: number | null;
  p50_ms: number | null;
  cost_per_success_usd: number | null;
};

let board: { generated_at_ms: number; corpus: { id: string; n_fixtures: number }; rows: Row[] } | null = null;
if (existsSync(artifact)) {
  board = JSON.parse(readFileSync(artifact, 'utf8'));
}

const pct = (v: number | null) => (v === null || v === undefined ? '—' : `${(v * 100).toFixed(1)}%`);
const num = (v: number | null, digits = 0) =>
  v === null || v === undefined ? '—' : v.toFixed(digits);

// Two rows are only a resolvable ranking when their intervals do not overlap.
const topRow = board?.rows?.[0];
const tiedWithTop = (r: Row) =>
  topRow !== undefined && r !== topRow && r.pass_at_1_ci_high >= topRow.pass_at_1_ci_low;
---

<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Vox efficacy benchmark</title>
  </head>
  <body>
    <main>
      <h1>Vox efficacy benchmark</h1>

      {!board && (
        <p>
          No leaderboard artifact has been generated yet. Run
          <code>vox model eval-corpus</code> for each candidate, then
          <code>vox model eval-corpus-leaderboard</code>.
        </p>
      )}

      {board && (
        <>
          <p>
            Held-out HumanEval-Vox ({board.corpus.n_fixtures} fixtures).
            Correctness is decided by the Vox compiler and the fixture's own test
            assertions — exit codes only, no LLM judge. Rows are
            (model, harness, config) tuples, because the same base model scores
            differently under different scaffolding.
          </p>
          <p>
            <strong>Reading the intervals:</strong> each pass@1 carries a 95%
            Wilson confidence interval. Rows marked <em>tied</em> overlap the
            leader's interval and are not a resolvable ranking at this corpus size.
          </p>

          <div style="overflow-x: auto;">
            <table>
              <thead>
                <tr>
                  <th>Model</th>
                  <th>Harness</th>
                  <th>pass@1 (95% CI)</th>
                  <th>Compiles</th>
                  <th>Tokens/pass</th>
                  <th>p50 latency</th>
                  <th>Cost/success</th>
                  <th>Config</th>
                </tr>
              </thead>
              <tbody>
                {board.rows.map((r) => (
                  <tr>
                    <td>{r.model_id}</td>
                    <td>{r.harness_id}</td>
                    <td>
                      {pct(r.pass_at_1)}{' '}
                      <small>({pct(r.pass_at_1_ci_low)}–{pct(r.pass_at_1_ci_high)})</small>
                      {tiedWithTop(r) && <> <em>tied</em></>}
                    </td>
                    <td>{pct(r.compile_rate)}</td>
                    <td>{num(r.tokens_per_pass)}</td>
                    <td>{r.p50_ms === null ? '—' : `${r.p50_ms} ms`}</td>
                    <td>
                      {r.cost_per_success_usd === null
                        ? '—'
                        : `$${r.cost_per_success_usd.toFixed(4)}`}
                    </td>
                    <td><code>{r.config_digest}</code></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <p>
            <small>
              Generated {new Date(board.generated_at_ms).toISOString()}. Every
              scored configuration is pinned and published: no privately-tuned
              variant represents any entrant, including Vox's own.
            </small>
          </p>
        </>
      )}
    </main>
  </body>
</html>
```

- [ ] **Step 2: Generate an artifact and verify the page builds**

```bash
cargo run -q -p vox-cli -- model eval-corpus-leaderboard
cd docs-astro && pnpm install --frozen-lockfile && pnpm run build 2>&1 | tail -20
```

Expected: build succeeds and emits a `benchmarks` route. With no run reports present, the artifact has zero rows and the page renders the empty-state paragraph — verify it does NOT crash on the empty case.

- [ ] **Step 3: Commit**

```bash
git add docs-astro/src/pages/benchmarks.astro contracts/reports/vox-efficacy/leaderboard.v1.json
git commit -m "feat(docs): public Vox efficacy leaderboard page with confidence intervals"
```

---

## Phase E — Publication automation

### Task 12: Hugging Face dataset card generation

Confirmed by grep: no Hugging Face publishing adapter exists anywhere in the repo (the only HF reference is `HuggingFaceCatalog` in `crates/vox-orchestrator/src/catalog.rs:424-427`, which consumes HF as an inference provider). The corpus manifest already carries everything a dataset card needs.

**Files:**
- Create: `crates/vox-publisher/src/huggingface_dataset.rs`
- Modify: `crates/vox-publisher/src/lib.rs`

**Interfaces:**
- Consumes: `contracts/eval/humaneval-vox/manifest.v1.yaml`.
- Produces: `pub struct DatasetCardInput { pub dataset_id: String, pub license: String, pub n_fixtures: usize, pub n_held_out: usize, pub corpus_hash: String, pub homepage: String }`; `pub fn render_dataset_card(input: &DatasetCardInput) -> String`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-publisher/src/huggingface_dataset.rs` with ONLY the doc comment and test module:

```rust
//! Hugging Face dataset-card generation for published Vox eval corpora.
//!
//! Card-generation only — pure, no network. Uploading is a separate step so the
//! card can be reviewed before anything is published, and so this function stays
//! unit-testable without credentials.

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> DatasetCardInput {
        DatasetCardInput {
            dataset_id: "voxlang/humaneval-vox".to_string(),
            license: "apache-2.0".to_string(),
            n_fixtures: 164,
            n_held_out: 31,
            corpus_hash: "sha256:8a2f15".to_string(),
            homepage: "https://voxlang.org/benchmarks".to_string(),
        }
    }

    #[test]
    fn card_opens_with_a_yaml_frontmatter_block() {
        let card = render_dataset_card(&input());
        assert!(card.starts_with("---\n"), "HF requires YAML front matter first");
        let end = card[4..].find("\n---\n").expect("front matter closes");
        let fm = &card[4..4 + end];
        for key in ["license:", "language:", "pretty_name:", "task_categories:", "size_categories:"] {
            assert!(fm.contains(key), "front matter must declare {key}, got:\n{fm}");
        }
    }

    #[test]
    fn card_declares_the_held_out_split_and_its_purpose() {
        let card = render_dataset_card(&input());
        assert!(card.contains("31"), "held-out count stated");
        assert!(card.contains("164"), "total count stated");
        assert!(
            card.to_lowercase().contains("contamination"),
            "the card must explain WHY the split exists, or users will train on it"
        );
    }

    #[test]
    fn card_pins_the_corpus_hash_for_reproducibility() {
        let card = render_dataset_card(&input());
        assert!(card.contains("sha256:8a2f15"));
    }

    #[test]
    fn card_declares_the_license_verbatim_in_front_matter() {
        let card = render_dataset_card(&input());
        assert!(card.contains("license: apache-2.0"));
    }
}
```

Add to `crates/vox-publisher/src/lib.rs` in the module list:

```rust
pub mod huggingface_dataset;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-publisher huggingface_dataset 2>&1 | tail -20`
Expected: FAIL — `cannot find function render_dataset_card`.

- [ ] **Step 3: Write minimal implementation**

Insert into `crates/vox-publisher/src/huggingface_dataset.rs` above the test module:

```rust
/// Inputs for a Hugging Face dataset card.
#[derive(Debug, Clone)]
pub struct DatasetCardInput {
    /// Hub id, e.g. `voxlang/humaneval-vox`.
    pub dataset_id: String,
    /// SPDX-style license id as HF expects it, e.g. `apache-2.0`.
    pub license: String,
    /// Total fixtures in the corpus.
    pub n_fixtures: usize,
    /// Fixtures held out of training (the only ones valid for external claims).
    pub n_held_out: usize,
    /// Corpus content hash, pinned so a result can be tied to an exact snapshot.
    pub corpus_hash: String,
    /// Canonical homepage for the benchmark.
    pub homepage: String,
}

/// Render a Hugging Face dataset card: YAML front matter plus the body.
///
/// The contamination warning is load-bearing, not boilerplate — a public corpus
/// whose held-out split is not conspicuously flagged will be scraped into
/// training sets, and the split's value evaporates the moment that happens.
#[must_use]
pub fn render_dataset_card(input: &DatasetCardInput) -> String {
    let n_training = input.n_fixtures.saturating_sub(input.n_held_out);
    format!(
        "---\n\
         license: {license}\n\
         language:\n\
         - en\n\
         pretty_name: HumanEval-Vox\n\
         task_categories:\n\
         - text2text-generation\n\
         size_categories:\n\
         - n<1K\n\
         tags:\n\
         - code-generation\n\
         - benchmark\n\
         - vox\n\
         ---\n\
         \n\
         # HumanEval-Vox\n\
         \n\
         {n_fixtures} program-synthesis problems in the Vox language, anchored to \
         the HumanEval problem set. Each problem ships a natural-language prompt, a \
         pinned function signature, a reference solution, and an executable test \
         block.\n\
         \n\
         ## Splits\n\
         \n\
         | Split | Count | Use |\n\
         |---|---|---|\n\
         | training-eligible | {n_training} | May appear in model training corpora. |\n\
         | held-out | {n_held_out} | **Never** train on these. |\n\
         \n\
         ## Contamination warning\n\
         \n\
         The held-out split exists to make comparative claims meaningful. A model \
         trained on these {n_held_out} problems cannot be honestly evaluated on \
         them, and any score it reports is a memorization measurement, not a \
         capability measurement. Each fixture additionally carries an `added_at` \
         date so a model can be scored only on problems that postdate its training \
         cutoff.\n\
         \n\
         ## Evaluation\n\
         \n\
         Correctness is decided by the Vox compiler's exit code and the fixture's \
         own test assertions — no LLM judge participates in scoring.\n\
         \n\
         ## Reproducibility\n\
         \n\
         - Corpus hash: `{corpus_hash}`\n\
         - Pinned run settings: `temperature=0.0`, `seed=42`, `attempts_per_fixture=5`\n\
         - Live leaderboard: {homepage}\n\
         \n\
         ## Citation\n\
         \n\
         See `CITATION.cff` in the source repository; each tagged release mints a \
         Zenodo DOI.\n",
        license = input.license,
        n_fixtures = input.n_fixtures,
        n_training = n_training,
        n_held_out = input.n_held_out,
        corpus_hash = input.corpus_hash,
        homepage = input.homepage,
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-publisher huggingface_dataset 2>&1 | tail -20`
Expected: PASS — 4 tests.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-publisher
git add crates/vox-publisher/src/huggingface_dataset.rs crates/vox-publisher/src/lib.rs
git commit -m "feat(vox-publisher): Hugging Face dataset card generation for eval corpora"
```

---

### Task 13: SCIENTIA finding-candidate emission

This is what makes future publications automatic rather than remembered: a benchmark run that moves the needle becomes a publication candidate on its own, routed through the existing worthiness gate.

**Files:**
- Modify: `crates/vox-cli/src/commands/model/eval_corpus_leaderboard.rs`

**Interfaces:**
- Consumes: the leaderboard artifact (Task 10), `vox_eval::corpus_stats::intervals_overlap`.
- Produces: `pub fn significant_deltas(current: &serde_json::Value, previous: &serde_json::Value) -> Vec<String>`; `pub fn finding_candidate_from_deltas(deltas: &[String], now_ms: u64) -> serde_json::Value`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `eval_corpus_leaderboard.rs`:

```rust
    #[test]
    fn significant_deltas_ignores_movement_inside_the_confidence_interval() {
        // 24/31 -> 26/31 looks like a 6-point jump but the intervals overlap,
        // so it is not evidence of anything and must not become a candidate.
        let prev = build_leaderboard(&[run_json("m", "vox-harness", 24, 31)]).unwrap();
        let curr = build_leaderboard(&[run_json("m", "vox-harness", 26, 31)]).unwrap();
        assert!(
            significant_deltas(&curr, &prev).is_empty(),
            "overlapping intervals are not a finding"
        );
    }

    #[test]
    fn significant_deltas_reports_a_non_overlapping_improvement() {
        let prev = build_leaderboard(&[run_json("m", "vox-harness", 200, 1000)]).unwrap();
        let curr = build_leaderboard(&[run_json("m", "vox-harness", 800, 1000)]).unwrap();
        let deltas = significant_deltas(&curr, &prev);
        assert_eq!(deltas.len(), 1);
        assert!(deltas[0].contains("m"), "delta names the model: {}", deltas[0]);
    }

    #[test]
    fn finding_candidate_matches_the_scientia_schema_shape() {
        let candidate = finding_candidate_from_deltas(&["m improved".to_string()], 1_700_000_000_000);
        assert_eq!(candidate["schema_version"], 1);
        assert_eq!(candidate["candidate_class"], "algorithmic_improvement");
        assert!(candidate["candidate_id"].as_str().is_some_and(|s| !s.is_empty()));
        assert_eq!(candidate["created_at_ms"], 1_700_000_000_000u64);
        let signals = candidate["internal_signals"].as_array().expect("signals array");
        assert_eq!(signals.len(), 1);
        for key in ["code", "summary", "strength", "family", "provenance"] {
            assert!(signals[0].get(key).is_some(), "signal must carry {key}");
        }
    }

    #[test]
    fn no_deltas_produces_no_candidate_signals() {
        let candidate = finding_candidate_from_deltas(&[], 1);
        assert_eq!(candidate["internal_signals"].as_array().unwrap().len(), 0);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli eval_corpus_leaderboard 2>&1 | tail -20`
Expected: FAIL — `cannot find function significant_deltas`.

- [ ] **Step 3: Write minimal implementation**

Insert into `eval_corpus_leaderboard.rs` above the test module:

```rust
use vox_eval::corpus_stats::{ConfidenceInterval, intervals_overlap};

/// Rows whose pass@1 moved by more than measurement error since the previous board.
///
/// Overlapping intervals are explicitly NOT reported: publishing a "we improved"
/// claim on a delta the corpus cannot resolve is exactly the failure mode that
/// discredits small-project benchmarks.
#[must_use]
pub fn significant_deltas(
    current: &serde_json::Value,
    previous: &serde_json::Value,
) -> Vec<String> {
    let key = |row: &serde_json::Value| {
        format!(
            "{}|{}",
            row["model_id"].as_str().unwrap_or_default(),
            row["harness_id"].as_str().unwrap_or_default()
        )
    };
    let ci = |row: &serde_json::Value| ConfidenceInterval {
        point: row["pass_at_1"].as_f64().unwrap_or(0.0),
        low: row["pass_at_1_ci_low"].as_f64().unwrap_or(0.0),
        high: row["pass_at_1_ci_high"].as_f64().unwrap_or(1.0),
    };

    let empty = Vec::new();
    let prev_rows = previous["rows"].as_array().unwrap_or(&empty);
    let curr_rows = current["rows"].as_array().unwrap_or(&empty);

    let mut out = Vec::new();
    for curr in curr_rows {
        let Some(prev) = prev_rows.iter().find(|p| key(p) == key(curr)) else {
            continue; // first appearance is not a delta
        };
        let (a, b) = (ci(prev), ci(curr));
        if intervals_overlap(&a, &b) {
            continue;
        }
        let direction = if b.point > a.point { "improved" } else { "regressed" };
        out.push(format!(
            "{} {} on held-out HumanEval-Vox: pass@1 {:.1}% -> {:.1}% (non-overlapping 95% CIs)",
            key(curr),
            direction,
            a.point * 100.0,
            b.point * 100.0
        ));
    }
    out
}

/// Wrap significant deltas as a SCIENTIA `finding-candidate.v1` record.
///
/// Emitting this is what makes follow-up publications automatic: the candidate
/// flows into the existing worthiness gate and dual-approval pipeline instead of
/// depending on someone remembering to check the leaderboard.
#[must_use]
pub fn finding_candidate_from_deltas(deltas: &[String], now_ms: u64) -> serde_json::Value {
    let signals: Vec<serde_json::Value> = deltas
        .iter()
        .map(|d| {
            serde_json::json!({
                "code": "vox_efficacy_delta",
                "summary": d,
                "strength": "strong",
                "family": "BenchmarkDelta",
                "provenance": "vox model eval-corpus-leaderboard",
            })
        })
        .collect();

    serde_json::json!({
        "schema_version": 1,
        "candidate_id": format!("vox-efficacy-{now_ms}"),
        "candidate_class": "algorithmic_improvement",
        "title_hint": "Vox codegen efficacy change on the held-out HumanEval-Vox corpus",
        "internal_signals": signals,
        "created_at_ms": now_ms,
    })
}
```

Add these fields to `LeaderboardArgs`:

```rust
    /// Previous leaderboard artifact to diff against for candidate emission.
    #[arg(long)]
    pub previous: Option<PathBuf>,
    /// Write a SCIENTIA finding-candidate record here when deltas are significant.
    #[arg(long)]
    pub emit_candidate: Option<PathBuf>,
```

And append this block to the end of `run`, before `Ok(())`:

```rust
    if let (Some(prev_path), Some(candidate_path)) = (&args.previous, &args.emit_candidate) {
        let prev_raw = std::fs::read_to_string(prev_path)
            .with_context(|| format!("reading {}", prev_path.display()))?;
        let previous: serde_json::Value = serde_json::from_str(&prev_raw)
            .with_context(|| format!("parsing {}", prev_path.display()))?;
        let deltas = significant_deltas(&board, &previous);
        if deltas.is_empty() {
            println!("No statistically resolvable deltas; no candidate emitted.");
        } else {
            let candidate = finding_candidate_from_deltas(&deltas, vox_db::now_unix_ms());
            if let Some(parent) = candidate_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(candidate_path, serde_json::to_string_pretty(&candidate)?)?;
            println!(
                "Emitted SCIENTIA finding candidate with {} signal(s) to {}",
                deltas.len(),
                candidate_path.display()
            );
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli eval_corpus_leaderboard 2>&1 | tail -20`
Expected: PASS — 8 tests.

- [ ] **Step 5: Sync the registry and commit**

```bash
cargo fmt -p vox-cli
cargo run -q -p vox-cli -- ci command-sync --write
UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli command_catalog
git add crates/vox-cli/src/commands/model/eval_corpus_leaderboard.rs contracts/cli/command-registry.yaml crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt docs/src/reference/cli-command-surface.generated.md
git commit -m "feat(vox-cli): emit SCIENTIA finding candidates for statistically-resolvable benchmark deltas"
```

---

## Phase F — MENS scaffolding and operator documentation

### Task 14: MENS checkpoint scaffolding

MENS is not accessible today. The scaffolding requirement is that when it becomes accessible, scoring it requires **no code change** — only a registry entry and a CLI invocation. This task proves that claim with a test rather than asserting it.

**Files:**
- Modify: `crates/vox-cli/src/commands/model/eval_corpus.rs`

**Interfaces:**
- Consumes: `EvalCorpusArgs` (Task 7), `config_digest` (Task 7).
- Produces: adds `pub checkpoint: Option<String>` to `EvalCorpusArgs`; `pub fn mens_row_identity(model: &str, harness: &str, checkpoint: Option<&str>) -> (String, String)`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `eval_corpus.rs`:

```rust
    #[test]
    fn mens_checkpoints_are_distinct_leaderboard_rows() {
        // Two MENS checkpoints must never collapse into one row: the whole point
        // of scoring MENS over time is seeing checkpoint-to-checkpoint movement.
        let (id_a, _) = mens_row_identity("vox/mens", "vox-harness", Some("2026-09-01-a"));
        let (id_b, _) = mens_row_identity("vox/mens", "vox-harness", Some("2026-09-15-b"));
        assert_ne!(id_a, id_b, "different checkpoints -> different row identity");
        assert!(id_a.contains("2026-09-01-a"), "checkpoint is visible in the row id");
    }

    #[test]
    fn external_models_without_a_checkpoint_keep_their_bare_id() {
        let (id, harness) = mens_row_identity("moonshot/kimi-k2.6-thinking", "vox-harness", None);
        assert_eq!(id, "moonshot/kimi-k2.6-thinking", "no checkpoint -> unchanged id");
        assert_eq!(harness, "vox-harness");
    }

    #[test]
    fn mens_scores_on_the_same_axes_as_any_external_model() {
        // The scoreboard row for MENS must be structurally identical to a
        // competitor's — same category, same fields — so the router's selection
        // logic needs no MENS special case.
        use vox_eval::corpus_score::{AttemptOutcome, FixtureOutcome, score_corpus};
        let a = AttemptOutcome {
            compiled: true,
            tests_passed: true,
            total_tokens: 50,
            latency_ms: 40,
            cost_usd: None,
        };
        let score = score_corpus(&[FixtureOutcome {
            fixture_id: "041".into(),
            attempts: vec![a],
        }]);
        let mens = scoreboard_row_from_corpus("vox/mens@2026-09-01-a", "vox-harness", &score);
        let rival = scoreboard_row_from_corpus("moonshot/kimi-k2.6-thinking", "vox-harness", &score);
        assert_eq!(mens.task_category, rival.task_category);
        assert_eq!(mens.strength_tag, rival.strength_tag);
        assert_eq!(mens.quality_score, rival.quality_score);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli eval_corpus 2>&1 | tail -20`
Expected: FAIL — `cannot find function mens_row_identity`.

- [ ] **Step 3: Write minimal implementation**

Insert into `eval_corpus.rs` above the test module:

```rust
/// Resolve the `(model_id, harness_id)` pair a leaderboard row is keyed by.
///
/// A local checkpoint label is folded into the model id as `<model>@<checkpoint>`
/// so successive MENS builds are separate rows rather than silently overwriting
/// one another. External models pass `None` and keep their bare registry id.
///
/// This is the whole MENS scaffolding contract: MENS enters the leaderboard as a
/// registry id like any other model, is scored on identical axes by identical
/// code, and needs no special case anywhere in the runner or the router.
#[must_use]
pub fn mens_row_identity(
    model: &str,
    harness: &str,
    checkpoint: Option<&str>,
) -> (String, String) {
    match checkpoint {
        Some(c) => (format!("{model}@{c}"), harness.to_string()),
        None => (model.to_string(), harness.to_string()),
    }
}
```

Add to `EvalCorpusArgs`:

```rust
    /// Local model checkpoint label (e.g. a MENS build id). Folded into the row's
    /// model id so successive checkpoints are separate leaderboard rows.
    #[arg(long)]
    pub checkpoint: Option<String>,
```

In `run`, replace the two uses of `args.model` / `args.harness` in the report and write-back with the resolved identity. Insert immediately after the `let all = load_corpus(&args.corpus)?;` line:

```rust
    let (row_model_id, row_harness_id) =
        mens_row_identity(&args.model, &args.harness, args.checkpoint.as_deref());
```

Then in the write-back block change `scoreboard_row_from_corpus(&args.model, &args.harness, &score)` to `scoreboard_row_from_corpus(&row_model_id, &row_harness_id, &score)`, and in the `serde_json::json!` report change `"model_id": args.model` to `"model_id": row_model_id` and `"harness_id": args.harness` to `"harness_id": row_harness_id`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli eval_corpus 2>&1 | tail -20`
Expected: PASS — 11 tests.

- [ ] **Step 5: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/model/eval_corpus.rs
git commit -m "feat(vox-cli): MENS checkpoint row identity for longitudinal scoring"
```

---

### Task 15: Operator reference documentation

**Files:**
- Create: `docs/src/reference/vox-efficacy-benchmark.md`

- [ ] **Step 1: Write the reference doc**

Create `docs/src/reference/vox-efficacy-benchmark.md`:

```markdown
---
title: "Vox efficacy benchmark — operator reference"
description: "How to score a model or external harness against the held-out HumanEval-Vox corpus, publish the leaderboard, and emit SCIENTIA publication candidates."
category: "Language Reference"
---

# Vox efficacy benchmark

Measures how well an AI system writes Vox, verified by compiler and test exit
codes only. No LLM judge participates in scoring.

Design rationale, external research, and the gap analysis this implements:
[Vox & MENS Comparative Efficacy Benchmarking](../architecture/vox-mens-comparative-efficacy-benchmarking-research-2026-09-01.md).

## Prerequisites

The runner shells out to a real `vox` binary:

    cargo build -p vox-cli --release

## Scoring a registry model (live generation)

    vox model eval-corpus --model moonshot/kimi-k2.6-thinking --harness vox-harness --cutoff 2026-05-26 --output contracts/reports/vox-efficacy/runs/kimi.json

Held-out fixtures only, by default. `--include-training-eligible` widens the set
but the result is then **not** valid as an external claim: training-eligible
fixtures may appear in a model's training corpus.

`--cutoff <ISO date>` restricts scoring to fixtures added strictly after a
model's published training cutoff. This is the contamination guard — a model
cannot have memorized a problem that did not exist when it was trained.

## Scoring an external harness (Claude Code, Cursor, Warp, Grok)

These cannot be driven from Rust, so their output is scored instead. Have the
harness solve each held-out fixture, save each solution as
`<fixture-id>.vox` (e.g. `041.vox`), then:

    vox model eval-corpus --model anthropic/claude-opus-4.7 --harness claude-code --from-dir ./claude-code-solutions --output contracts/reports/vox-efficacy/runs/claude-code.json

Identical composition, verification, and scoring code runs in both modes, which
is what makes the comparison fair.

## Scoring a MENS checkpoint

    vox model eval-corpus --model vox/mens --checkpoint 2026-09-01-a --harness vox-harness --output contracts/reports/vox-efficacy/runs/mens-a.json

The checkpoint label becomes part of the row identity (`vox/mens@2026-09-01-a`),
so successive builds are separate rows and their movement is visible over time.
MENS is scored by the same code on the same axes as every external model.

## Publishing the leaderboard

    vox model eval-corpus-leaderboard

Merges every run report into
`contracts/reports/vox-efficacy/leaderboard.v1.json`, which
`docs-astro/src/pages/benchmarks.astro` renders at `/benchmarks`. Every row
carries a 95% Wilson confidence interval; rows overlapping the leader render as
"tied" because that ranking is not resolvable at this corpus size.

## Emitting a publication candidate

    vox model eval-corpus-leaderboard --previous <prior-board>.json --emit-candidate contracts/reports/vox-efficacy/candidate.json

Emits a SCIENTIA `finding-candidate.v1` record **only** for deltas whose
confidence intervals do not overlap. Movement inside the interval is not
reported, because publishing it would be a claim the corpus cannot support.

## Governance rules (non-optional)

- Correctness comes from exit codes. Never introduce an LLM judge into the
  scoring path.
- Every scored configuration is pinned and published, including Vox's own. No
  privately-tuned variant may represent any entrant — the failure mode the
  LMArena "Leaderboard Illusion" findings documented.
- Never back-date a fixture's `added_at`.
- Rotate new held-out problems in on a schedule; a frozen held-out set decays as
  training corpora grow.

## Adding fixtures

Add the problem directory under `contracts/eval/humaneval-vox/problems/`, then
add a manifest entry with today's date as `added_at` and update
`count_current` / `held_out_current`.
```

- [ ] **Step 2: Verify the doc lints**

Run: `cargo run -p vox-doc-pipeline -- --lint-only --paths docs/src/reference/vox-efficacy-benchmark.md`
Expected: no output (clean).

- [ ] **Step 3: Index the doc**

Set valid frontmatter on the new page (`title`, `description`, `category`, `status`).
Starlight lists it. Do **not** create or edit `docs/src/architecture/research-index.md` (retired 2026-09).

- [ ] **Step 4: Run the full local gate**

```bash
vox run scripts/fmt.vox
vox ci pre-push --complete
```

Expected: green. Investigate and fix any failure locally — do not push to see whether CI agrees.

- [ ] **Step 5: Commit**

```bash
git add docs/src/reference/vox-efficacy-benchmark.md
git commit -m "docs: operator reference for the Vox efficacy benchmark"
```

---

## Self-Review Notes

**Spec coverage.** Spec §C's four metric families: correctness (Tasks 1, 6), efficiency (Task 1), robustness/reward-hacking (Task 5), comparative-with-uncertainty (Tasks 2, 10). Spec §D Gap I → Task 12; Gap J wiring → Tasks 7-9, Gap J contamination → Task 3; Gap K → Tasks 10-11. Spec §F's seven architecture points: rolling corpus (3), runner with tuple rows (7-9, 14), anti-gaming (5), statistics (2, 10), publication surface (10-11), governance (10-11, 15), SCIENTIA feed (13). Spec §G automation → Tasks 13, 15.

**Deliberately out of scope**, tracked in the spec's action list and needing their own plans: §E router items 1-5 (reading the write-only `arm_stats` and `scientia_model_profile_learning` loops, unifying the free-tier router onto `vox-orchestrator::models::scoring`, the RouteLLM-style learned classifier, cascade-on-actual-confidence). Task 9 delivers the input those changes consume — a measured `quality_score` in `model_scoreboard` under `task_category = "vox-codegen"` — so they are unblocked but not attempted here. Hugging Face **upload** (as opposed to card generation) is also deferred: Task 12 ships the reviewable card, and the Hub client should mirror `crates/vox-publisher/src/scholarly/zenodo.rs`'s existing `reqwest` setup when it lands.

**Known risk.** Task 7 Step 6's oracle smoke test is the harness's own correctness gate: if a known-correct reference solution fails to pass, the fault is in composition or verification, not the model. Run it before trusting any score. This is not hypothetical rigor — when the spec's §H worked example was actually executed, the oracle control (5/5 on reference solutions) is the only thing that licensed reading the one candidate failure as the model's rather than the harness's.

**Follow-on identified after this plan was drafted.** Executing the spec's §H benchmark surfaced a finding the plan does not yet capture: candidate failures sort into two very different classes — *cheap-and-loud* surface divergences (`&&` vs `and`, `len(xs)` vs `xs.len()`, `==` vs `is`, the last of which is only a warning and does not even fail the run) versus *structural* ones (`xs[i]` returns `Option[int]`, which no surface-token correction reaches and which forces a control-flow rewrite). Aggregate pass rate collapses that distinction, yet the class split is derivable from diagnostics the compiler already emits and is far more actionable for improving Vox as an LLM target. Worth adding as a `failure_class` histogram on each run report — deliberately NOT bolted onto this plan, since it wants its own RED test and would widen Task 1's already-locked `CorpusScore` shape.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-09-01-vox-efficacy-benchmark-and-leaderboard.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
