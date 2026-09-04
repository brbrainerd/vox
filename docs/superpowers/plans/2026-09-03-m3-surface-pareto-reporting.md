# M3 Surface Completion: Pareto Reporting + Two Shipped-Defect Fixes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish Task M3 ("Surface") as a *reporting* change — fix two defects already shipped under M3, then present `vox model scoreboard` / `vox model explain` as a Pareto set with an honest budget-aware recommendation instead of a single composite rank. **No model-selection routing behavior changes.**

**Architecture:** One new pure module (`vox-orchestrator/src/models/pareto.rs`) computing a non-dominated set over three axes already stored in `model_scoreboard`, deliberately placed beside `wilson_score_interval` in `models/` rather than in `routing/` — the module location encodes that this is a metrics helper, not a router. It owns the *only* definition of how a scoreboard row maps to objective space, so the two CLI surfaces cannot drift apart. Both surfaces render through extracted pure `-> String` functions, because `run()` in each is untestable (DB + env) and the render is exactly where the shipped defects live.

**Tech Stack:** Rust, `vox-orchestrator` (`models`), `vox-cli` (`commands/model`, `commands/harness`), existing `wilson_score_interval` and `ModelScore` (shipped in `e497a82fb` / `652f50282`).

**Spec:** No standalone spec. Scoped from `docs/superpowers/plans/2026-08-28-chat-harness-unification.md` §"Task M3: Surface", audited against the codebase on 2026-09-03 across two six-track review rounds. **A prior revision proposed a live-routing algorithm; it was withdrawn — see §Withdrawn Scope.**

## Global Constraints

- **Reporting only. No selection-path code is modified.** No task touches `routing/engine.rs`, `registry_model_resolve.rs`, `models/select.rs`, `models/scoring.rs`, or `contracts/orchestration/model-routing.v1.yaml`. If a task appears to need one of those, stop and re-read §Withdrawn Scope.
- **The quality axis is observed success *rate*, and `success` means "the provider returned a non-error response".** `record_telemetry_outcome` passes `success: true` on any `Ok(resp)` with no inspection of content (`crates/vox-actor-runtime/src/llm/chat.rs:200` and `:433`; `false` only on the error arms at `:252` and `:469`). The MCP chat surface is worse: `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs:1357` hardcodes `success: true` unconditionally into the same column, via `LlmSurfaceTelemetry` → `chat_socrates_meta.rs:194`. It is a **reliability** signal, not an answer-quality signal. No column or legend this plan touches may be headed "Quality" — M0/M2 already retired `quality_score` for being a constant 1.0 (`crates/vox-orchestrator/src/models/scoring.rs:72`; comment at `crates/vox-cli/src/commands/model/explain.rs:157-159`), and substituting a second unlabeled proxy would repeat that mistake.
- **One quality source, one definition.** All quality readings come from `ModelScore.success_count` / `n_calls` via the single `pareto_point_for` in `pareto.rs` (Task 3). Do **not** use `arm_stats` — `list_model_arm_stats` is a *derived view* that calls `get_model_scoreboard` and re-derives successes as `round(success_rate * n_calls)` (`crates/vox-db/src/store/ops_scientia.rs:65,72-75`), discarding the exact `success_count` in the same row. Do **not** define a second copy of `pareto_point_for` per CLI surface; a prior draft did, and nothing could then detect the two drifting apart.
- **A scoreboard row is a `(model_id, task_category, strength_tag)` triple, not a model.** `get_model_scoreboard` filters on `window_days` only (`ops_scientia.rs:27-28`) against a table keyed by all four (`schema/domains/scientia.rs:148`), so one model can appear several times. Every rendered claim must name the triple, never just the model id.
- **Never `cargo fmt --all`** (Windows `CreateProcess` overflow, `AGENTS.md` §Formatting Rust). Use `cargo fmt -p <crate>`.
- **Every commit ends with** `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`, matching `652f50282`, `e0569e7a0`, `e497a82fb`.
- **The gates will not catch a weak test.** `skeleton/untested-pub-api` is *file-level* — it fires only when a file has `pub fn`s and zero test blocks (`crates/vox-code-audit/src/detectors/untested_pub_api.rs:188-190`) and ignores `pub(crate)` (`:157`,`:173`). Every file here already has a test module, so the detector emits nothing regardless. For every test ask: **"what is the simplest wrong implementation that still passes this?"** Two review rounds found that question unasked; the tests below exist because of specific wrong implementations named in the comments.

- [ ] **Step 0 (once, before Task 1): baseline the repo-wide gates.**

`lefthook.yml:45`'s `tdd-guard` runs `toestub .` over the **whole repo** at `enforce-strict` on every commit touching `crates/**/*.rs`, so pre-existing debt anywhere blocks this plan's first commit.

```bash
cargo run -p vox-code-audit --bin toestub -- . --rules skeleton --min-severity warning --mode enforce-strict --format terminal
cargo run -p vox-drift-check -- . --severity warning --fail-on warning
git rev-parse HEAD > /tmp/m3-plan-base.sha   # Task 6's scope check diffs against this, not main
```

Expected: both clean. If not, fix or suppress those first — they are not this plan's work, but they will block it. The recorded SHA matters: this branch already carries Phase G and M0–M3 commits that touched guarded paths, so a `main...HEAD` diff would report a violation from *prior* work.

---

### Task 1: `pass^3` — fix a shipped defect

`b2137a5f6` shipped `pass@3` (Chen et al. — *at least one* of 3 passes). The requirement asks for **`pass^3`** (*all* 3 pass). Opposites; `pass^k ≤ pass@k` always. The requirement wants both "side by side."

**Files:**
- Modify: `crates/vox-cli/src/commands/harness/report.rs` (add beside `pass_at_k`/`mean_pass_at_k`, lines 47-75; replace the print block at `:303-313`)

**Interfaces:**
- Consumes: `vox_db::HarnessEvalTaskResultRecord` (`.total_samples`, `.pass_samples`, both `i64`).
- Produces: `fn pass_hat_k(n: i64, c: i64, k: i64) -> Option<f64>` = `C(c,k)/C(n,k)`; `fn mean_pass_hat_k(task_results: &[vox_db::HarnessEvalTaskResultRecord], k: i64) -> Option<f64>`. Same contract as the `pass_at_k` pair: `None` when `k > n`; undersampled tasks excluded from the mean rather than scored 0.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/vox-cli/src/commands/harness/report.rs, in the existing `mod tests` (`:338`,
// which is `use super::*;` so private fns resolve).

#[test]
fn pass_hat_k_is_probability_that_all_k_drawn_samples_pass() {
    // C(4,3)/C(5,3) = 4/10. Kills: a step function (c>=k -> 1.0) returns 1.0; a swapped
    // numerator (n-i)/(c-i) returns 2.5; a copy-paste of pass_at_k returns 1.0 (its
    // n-c<k early return fires here).
    let got = pass_hat_k(5, 4, 3).expect("computable");
    assert!((got - 0.4).abs() < 1e-12, "expected 0.4, got {got}");
}

#[test]
fn pass_hat_k_reduces_to_the_plain_pass_ratio_at_k_equals_1() {
    // C(c,1)/C(n,1) = c/n. Pins k as a real parameter: an implementation that ignores `k`
    // and hardcodes a 3-term product passes every other test in this file.
    assert_eq!(pass_hat_k(4, 2, 1), Some(0.5));
    assert_eq!(pass_hat_k(5, 4, 1), Some(0.8));
}

#[test]
fn pass_hat_k_falls_as_k_rises_for_the_same_record() {
    // 5 samples, 4 passes: k=1 -> 4/5 = .8; k=2 -> .6; k=3 -> .4; k=4 -> .2.
    for (k, want) in [(1_i64, 0.8_f64), (2, 0.6), (3, 0.4), (4, 0.2)] {
        let got = pass_hat_k(5, 4, k).expect("k <= n");
        assert!((got - want).abs() < 1e-12, "k={k}: expected {want}, got {got}");
    }
    assert_eq!(pass_hat_k(5, 4, 5), Some(0.0), "c=4 < k=5: no all-pass draw exists");
}

#[test]
fn pass_hat_k_is_one_only_when_every_sample_passed() {
    assert_eq!(pass_hat_k(3, 3, 3), Some(1.0));
    assert_eq!(pass_hat_k(5, 5, 3), Some(1.0));
}

#[test]
fn pass_hat_k_is_zero_when_fewer_than_k_samples_passed() {
    // Also kills the with-replacement form (c/n)^k, which would give (2/5)^3 = 0.064.
    assert_eq!(pass_hat_k(5, 2, 3), Some(0.0));
    assert_eq!(pass_hat_k(5, 0, 3), Some(0.0));
}

#[test]
fn pass_hat_k_returns_none_when_k_exceeds_samples_drawn() {
    assert_eq!(pass_hat_k(2, 2, 3), None);
    assert_eq!(pass_hat_k(0, 0, 3), None);
}

#[test]
fn pass_hat_k_rejects_more_passes_than_samples() {
    // `pass_samples > total_samples` is DB data, not a proven invariant. Without a guard the
    // product runs anyway: (7/5)(6/4)(5/3) = 3.5, outside [0,1]. `pass_at_k` is accidentally
    // safe here (its n-c<k branch catches it); this one is not.
    assert_eq!(pass_hat_k(5, 7, 3), None);
}

#[test]
fn pass_hat_k_never_exceeds_pass_at_k_and_both_stay_in_unit_range() {
    // All four pairs yield Some from BOTH functions, so no .expect() panics:
    //   (5,4): at=1.000000 hat=0.400000   (10,7): at=0.991667 hat=0.291667
    //   (20,13): at=0.969298 hat=0.250877 (4,2):  at=1.000000 hat=0.000000
    for (n, c) in [(5_i64, 4_i64), (10, 7), (20, 13), (4, 2)] {
        let at = pass_at_k(n, c, 3).expect("pass@3");
        let hat = pass_hat_k(n, c, 3).expect("pass^3");
        assert!(hat <= at, "pass^3 ({hat}) must not exceed pass@3 ({at}) for n={n}, c={c}");
        // Both bounds on both values -- the earlier draft checked only hat's floor and at's
        // ceiling, i.e. exactly the two halves the swapped-numerator bug cannot violate.
        assert!((0.0..=1.0).contains(&hat), "pass^3 out of [0,1]: {hat} for n={n}, c={c}");
        assert!((0.0..=1.0).contains(&at), "pass@3 out of [0,1]: {at} for n={n}, c={c}");
    }
}

#[test]
fn mean_pass_hat_k_averages_over_only_the_qualifying_tasks() {
    // t1: 1 sample  -> excluded (not scored 0, not in the denominator)
    // t2: 5 samples, 4 passes -> 0.4
    // t3: 3 samples, 3 passes -> 1.0
    // Correct mean = 0.7. Kills: scoring t1 as 0 -> 0.4667; max() -> 1.0; first() -> 0.4;
    // last() -> 1.0. The earlier draft's only mean test was {excluded, 1.0} -> 1.0, which
    // every one of those survives.
    let mut t1 = task_result("t1", "pass");
    t1.total_samples = 1;
    t1.pass_samples = 1;
    let mut t2 = task_result("t2", "pass");
    t2.total_samples = 5;
    t2.pass_samples = 4;
    let mut t3 = task_result("t3", "pass");
    t3.total_samples = 3;
    t3.pass_samples = 3;
    let got = mean_pass_hat_k(&[t1, t2, t3], 3).expect("two tasks qualify");
    assert!((got - 0.7).abs() < 1e-12, "expected 0.7, got {got}");
}

#[test]
fn mean_pass_hat_k_is_none_when_no_task_has_enough_samples() {
    let mut t1 = task_result("t1", "pass");
    t1.total_samples = 1;
    t1.pass_samples = 1;
    assert_eq!(mean_pass_hat_k(&[t1], 3), None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p vox-cli --lib commands::harness::report 2>&1 | head -30
```

Expected: the crate's test binary **fails to compile**, emitting **two** `error[E0425]`s — one naming `pass_hat_k`, one naming `mean_pass_hat_k`. No per-test PASS/FAIL output: in Rust a same-file test against unwritten code fails the whole crate build.

- [ ] **Step 3: Write the minimal implementation**

```rust
// crates/vox-cli/src/commands/harness/report.rs, directly after `mean_pass_at_k` (`:75`).

/// Task M3: `pass^k` — the probability that **all** `k` drawn samples pass, i.e.
/// `C(c,k)/C(n,k)`. The reliability sibling of [`pass_at_k`] ("at least one of `k` passes");
/// the two answer opposite questions and `pass^k <= pass@k` always.
///
/// `None` when `k > n` (not enough samples drawn) or when `c > n` (more passes than samples —
/// DB data, not an invariant; without the guard the product exceeds 1.0).
fn pass_hat_k(n: i64, c: i64, k: i64) -> Option<f64> {
    if k > n || n <= 0 || k <= 0 || c > n || c < 0 {
        return None;
    }
    if c < k {
        // Fewer than k passes exist, so no size-k draw can be all-passes.
        return Some(0.0);
    }
    let mut prob_all_pass = 1.0;
    for i in 0..k {
        prob_all_pass *= (c - i) as f64 / (n - i) as f64;
    }
    Some(prob_all_pass)
}

/// Task M3: mean `pass^k` over every task with at least `k` samples. Same exclusion rule as
/// [`mean_pass_at_k`] — an undersampled task is unknown, not a failure.
fn mean_pass_hat_k(task_results: &[vox_db::HarnessEvalTaskResultRecord], k: i64) -> Option<f64> {
    let scores: Vec<f64> = task_results
        .iter()
        .filter_map(|t| pass_hat_k(t.total_samples, t.pass_samples, k))
        .collect();
    if scores.is_empty() {
        return None;
    }
    Some(scores.iter().sum::<f64>() / scores.len() as f64)
}
```

Then replace the print block at `report.rs:303-313` (currently a 2-tuple match on `mean_pass_at_k`) with:

```rust
    match (
        mean_pass_at_k(&current_task_results, 1),
        mean_pass_at_k(&current_task_results, 3),
        mean_pass_hat_k(&current_task_results, 3),
    ) {
        (Some(p1), Some(at3), Some(hat3)) => println!(
            "pass@1: {:.1}%  pass@3: {:.1}%  pass^3: {:.1}%",
            p1 * 100.0,
            at3 * 100.0,
            hat3 * 100.0
        ),
        (Some(p1), _, _) => println!(
            "pass@1: {:.1}%  pass@3/pass^3: insufficient data (no task sampled >= 3x)",
            p1 * 100.0
        ),
        _ => {}
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p vox-cli --lib commands::harness::report
```

Expected: PASS — 10 new tests plus every pre-existing `report::tests` (`pass_at_k_*`, `mean_pass_at_k_*`, the `detect_regressions` suite). No existing test asserts on printed output, so the print change is regression-free.

- [ ] **Step 4b: Format** — `cargo fmt -p vox-cli`

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/harness/report.rs
git commit -m "$(cat <<'EOF'
fix(vox-cli): report pass^3 alongside pass@3 (Task M3 defect)

Task M3 asks for "pass@1 and pass^3 side by side". b2137a5f6 shipped
pass@3 (at least one of k passes) where the requirement asked for pass^3
(all k pass) -- opposite questions, and pass^k <= pass@k always. Adds
pass_hat_k/mean_pass_hat_k and prints all three. pass@3 stays because it
is independently useful, not because it was what was asked for.

pass_hat_k also guards c > n, which pass_at_k survives by accident:
pass_samples > total_samples is DB data, not an invariant, and the
unguarded product returns 3.5 for (n=5, c=7, k=3).

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: `pareto.rs` — the pure frontier module

Built before its consumers so Tasks 3-4 have one definition to share.

**Files:**
- Create: `crates/vox-orchestrator/src/models/pareto.rs`
- Modify: `crates/vox-orchestrator/src/models/mod.rs` — insert `mod pareto;` between `:6` (`pub mod key_guard;`) and `:7` (`pub mod policy;`), and `pub use pareto::{ParetoPoint, pareto_frontier, pareto_point_for};` between `:24` and `:25` (`pub use policy::{`). Both lists are alphabetical; the brace order shown is what rustfmt produces (type-like before value-like).

**Interfaces:**
- Produces:
  - `pub struct ParetoPoint { pub quality: f64, pub cost_usd: Option<f64>, pub latency_ms: Option<i64> }` — `quality` higher-is-better, the others lower-is-better. **No `index` field**: slice position is the identity, so nothing can disagree with it.
  - `pub fn pareto_frontier(points: &[ParetoPoint]) -> Vec<usize>` — positions of every non-dominated point, in input order.
  - `pub fn pareto_point_for(score: Option<&ModelScore>) -> ParetoPoint` — **the single mapping** from a scoreboard row to objective space, shared by both CLI surfaces.

**Design note — an unknown axis is `incomparable`, not neutral and not worst-case.** Three semantics were considered for `None`:

| Semantics | Transitive? | Cold-start visible? |
|---|---|---|
| worst-case (`None` = +∞) | yes | **no** — every rowless model is dominated by every incumbent |
| neutral (`_ => true`) | **no** | no (see below) |
| **incomparable** (chosen) | **yes** | yes |

Neutral is not transitive, and the failure is reachable: `Some(5.0) ≼ None` and `None ≼ Some(1.0)` while `Some(5.0) ⋠ Some(1.0)`. A verified 4-cycle at equal quality — costs `Some(1.0)/Some(2.0)/None/Some(0.5)` with latencies `None/Some(1)/Some(2)/Some(3)` — makes **every** point dominated and returns an **empty frontier for non-empty input**. Task 4's `budget_recommendation` would then report "no row qualifies" for a table with an obvious answer. Incomparability restores transitivity, which is what makes "the frontier is never empty" true rather than merely hoped for.

- [ ] **Step 1: Create the module, wire it into the tree, and write the failing tests**

**Wire it in first** — a `.rs` file with no `mod` declaration is not compiled at all, so writing tests without this produces `running 0 tests ... ok` and exit 0: a false green, the most dangerous TDD failure mode.

```rust
// crates/vox-orchestrator/src/models/mod.rs — add both lines now, not in Step 3.
mod pareto;
pub use pareto::{ParetoPoint, pareto_frontier, pareto_point_for};
```

```rust
// crates/vox-orchestrator/src/models/pareto.rs — file header plus tests only for now.
use super::ModelScore;

#[cfg(test)]
mod tests {
    use super::*;

    fn p(quality: f64, cost: f64, latency: i64) -> ParetoPoint {
        ParetoPoint { quality, cost_usd: Some(cost), latency_ms: Some(latency) }
    }

    fn score(success_count: i64, n_calls: i64, cost: Option<f64>, p50: Option<i64>) -> ModelScore {
        ModelScore {
            success_count,
            n_calls,
            cost_per_success_usd: cost,
            p50_latency_ms: p50,
            ..ModelScore::default()
        }
    }

    #[test]
    fn empty_input_has_empty_frontier() {
        assert_eq!(pareto_frontier(&[]), Vec::<usize>::new());
    }

    #[test]
    fn single_point_is_its_own_frontier() {
        assert_eq!(pareto_frontier(&[p(0.9, 0.01, 200)]), vec![0]);
    }

    #[test]
    fn strictly_dominated_point_is_excluded() {
        assert_eq!(pareto_frontier(&[p(0.95, 0.01, 200), p(0.80, 0.01, 200)]), vec![0]);
    }

    #[test]
    fn tradeoff_points_are_both_on_the_frontier() {
        assert_eq!(pareto_frontier(&[p(0.95, 0.05, 400), p(0.80, 0.01, 400)]), vec![0, 1]);
    }

    #[test]
    fn identical_points_do_not_dominate_each_other() {
        // Irreflexivity. Without the strict-inequality clause each "dominates" the other and
        // the frontier is EMPTY for non-empty input.
        assert_eq!(pareto_frontier(&[p(0.9, 0.01, 200), p(0.9, 0.01, 200)]), vec![0, 1]);
    }

    #[test]
    fn frontier_preserves_input_order() {
        let a = p(0.95, 0.05, 400);
        let b = p(0.60, 0.09, 900); // dominated by a
        let c = p(0.70, 0.01, 150);
        assert_eq!(pareto_frontier(&[a, b, c]), vec![0, 2], "input order, not sorted");
    }

    #[test]
    fn latency_alone_can_keep_a_point_on_the_frontier() {
        // b is worse on quality and TIED on cost; only latency saves it. A two-axis
        // (quality, cost) implementation returns [0] and fails -- and would otherwise pass
        // every other test in this file, because no other test's outcome depends on latency.
        assert_eq!(pareto_frontier(&[p(0.90, 0.02, 900), p(0.70, 0.02, 100)]), vec![0, 1]);
    }

    #[test]
    fn latency_alone_can_remove_a_point_from_the_frontier() {
        // Mirror: tied on quality AND cost, strictly slower. A latency-blind `dominates`
        // returns [0,1]; correct is [0]. Also kills `a.quality > b.quality` (instead of
        // `>=`), which never dominates on a quality tie.
        assert_eq!(pareto_frontier(&[p(0.80, 0.02, 100), p(0.80, 0.02, 900)]), vec![0]);
    }

    #[test]
    fn a_tie_on_quality_still_lets_cost_decide_domination() {
        // Two models 100%-reliable over the same n produce identical Wilson bounds, so
        // quality ties are routine here, not exotic.
        assert_eq!(pareto_frontier(&[p(0.90, 0.01, 200), p(0.90, 0.50, 200)]), vec![0]);
    }

    #[test]
    fn an_unknown_axis_is_incomparable_not_conceded() {
        // Equal quality, one row missing both measurements: neither dominates.
        let known = p(0.5, 0.02, 300);
        let unknown = ParetoPoint { quality: 0.5, cost_usd: None, latency_ms: None };
        assert_eq!(pareto_frontier(&[known, unknown]), vec![0, 1]);
    }

    #[test]
    fn an_unknown_axis_does_not_rescue_a_worse_quality_point() {
        let good = ParetoPoint { quality: 0.9, cost_usd: None, latency_ms: None };
        let bad = ParetoPoint { quality: 0.2, cost_usd: None, latency_ms: None };
        assert_eq!(pareto_frontier(&[good, bad]), vec![0]);
    }

    #[test]
    fn unknown_on_one_axis_only_still_lets_the_known_axes_decide() {
        // Worse on quality AND cost; an unknown latency must not launder that.
        let good = ParetoPoint { quality: 0.9, cost_usd: Some(0.01), latency_ms: Some(100) };
        let worse = ParetoPoint { quality: 0.5, cost_usd: Some(0.20), latency_ms: None };
        assert_eq!(pareto_frontier(&[good, worse]), vec![0]);
    }

    #[test]
    fn a_mix_of_known_and_unknown_axes_cannot_form_a_domination_cycle() {
        // Under a *neutral* unknown these four form the cycle 0>1>2>3>0 and the frontier is
        // EMPTY for non-empty input. This is the test that pins incomparability.
        let pts = vec![
            ParetoPoint { quality: 0.5, cost_usd: Some(1.0), latency_ms: None },
            ParetoPoint { quality: 0.5, cost_usd: Some(2.0), latency_ms: Some(1) },
            ParetoPoint { quality: 0.5, cost_usd: None, latency_ms: Some(2) },
            ParetoPoint { quality: 0.5, cost_usd: Some(0.5), latency_ms: Some(3) },
        ];
        assert_eq!(pareto_frontier(&pts), vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_non_empty_input_never_produces_an_empty_frontier() {
        // The invariant the doc comment claims. Hostile mix: NaN, all-unknown, a duplicate,
        // and a strictly dominated row.
        let pts = vec![
            p(0.9, 0.01, 200),
            p(0.9, 0.01, 200),
            ParetoPoint { quality: f64::NAN, cost_usd: None, latency_ms: Some(5) },
            ParetoPoint { quality: 0.5, cost_usd: None, latency_ms: None },
            p(0.1, 0.90, 9000),
        ];
        let frontier = pareto_frontier(&pts);
        assert!(!frontier.is_empty());
        assert!(!frontier.contains(&4), "the strictly dominated row must be excluded");
    }

    #[test]
    fn a_non_finite_quality_neither_dominates_nor_is_dominated() {
        // NaN compares false both ways. Pin the exact answer -- asserting only
        // `contains(&0)` would pass for `vec![0]`, `vec![0,1]`, or return-everything.
        let sane = p(0.9, 0.01, 200);
        let nan = ParetoPoint { quality: f64::NAN, cost_usd: Some(0.01), latency_ms: Some(200) };
        assert_eq!(pareto_frontier(&[sane, nan]), vec![0, 1]);
    }

    #[test]
    fn a_nan_quality_point_cannot_dominate_out_a_real_one_even_when_cheapest() {
        // The dangerous direction: if quality comparison is ever "fixed" with
        // total_cmp/unwrap_or(Less), NaN sorts highest and deletes the real row.
        let real = p(0.95, 0.50, 900);
        let nan = ParetoPoint { quality: f64::NAN, cost_usd: Some(0.001), latency_ms: Some(10) };
        assert_eq!(pareto_frontier(&[real, nan]), vec![0, 1]);
    }

    #[test]
    fn appending_a_dominated_point_does_not_change_the_original_frontier() {
        let base = [p(0.95, 0.05, 400), p(0.70, 0.01, 150)];
        let with_extra = [base[0], base[1], p(0.60, 0.09, 900)];
        assert_eq!(pareto_frontier(&base), vec![0, 1]);
        assert_eq!(pareto_frontier(&with_extra), vec![0, 1]);
    }

    #[test]
    fn pareto_point_for_uses_the_wilson_lower_bound_not_the_raw_rate() {
        // 18/20: raw .900000, Wilson lo .698962, center .835548, upper .972134. The band
        // excludes all three wrong readings with ~.05 of margin on the right one. The
        // fixture leaves success_rate at its 0.0 default, so reading that field is excluded
        // too.
        let pt = pareto_point_for(Some(&score(18, 20, Some(0.03), Some(250))));
        assert!(pt.quality > 0.65 && pt.quality < 0.75, "expected ~0.699, got {}", pt.quality);
        assert_eq!(pt.cost_usd, Some(0.03));
        assert_eq!(pt.latency_ms, Some(250));
    }

    #[test]
    fn pareto_point_for_treats_an_absent_or_zero_call_row_as_unobserved() {
        // `wilson_score_interval` is None at n=0, so quality falls back to 0.0. Callers must
        // NOT put these on a frontier -- see `is_observed`.
        for row in [None, Some(&score(0, 0, None, None))] {
            let pt = pareto_point_for(row);
            assert_eq!(pt.quality, 0.0);
            assert_eq!(pt.cost_usd, None);
            assert_eq!(pt.latency_ms, None);
        }
    }

    #[test]
    fn pareto_point_for_discards_a_non_finite_or_negative_cost() {
        // NaN/inf from a divide-by-zero upstream, or a negative from bad data, must read as
        // "unknown" -- never as an unbeatable minimum on the cost axis.
        for bad in [f64::NAN, f64::INFINITY, -1.0] {
            let pt = pareto_point_for(Some(&score(9, 10, Some(bad), Some(100))));
            assert_eq!(pt.cost_usd, None, "cost {bad} must not sort as best-possible");
        }
    }

    #[test]
    fn is_observed_is_false_for_rows_that_cannot_be_ranked() {
        assert!(!is_observed(None));
        assert!(!is_observed(Some(&score(0, 0, None, None))));
        assert!(is_observed(Some(&score(1, 1, None, None))));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p vox-orchestrator --lib models::pareto 2>&1 | tee /tmp/red.txt | head -30
grep -q "running 0 tests" /tmp/red.txt && { echo "FALSE GREEN: pareto.rs is not in the module tree — add 'mod pareto;' to models/mod.rs"; exit 1; }
grep -qE "E0412|E0425" /tmp/red.txt || { echo "unexpected red state"; exit 1; }
```

Expected: compile failure with **both** `error[E0412]` (cannot find type `ParetoPoint`, from the `p()` helper's return type) and `error[E0425]` (cannot find function `pareto_frontier` / `pareto_point_for` / `is_observed`). The `running 0 tests` guard exists because Step 1 wiring is easy to skip, and skipping it yields a green with zero tests.

- [ ] **Step 3: Write the minimal implementation**

```rust
//! Task M3 ("Surface"): the non-dominated subset of a candidate set, for *reporting*.
//!
//! Deliberately beside [`crate::models::wilson_score_interval`] in `models/` rather than in
//! `routing/`: nothing here feeds model selection. `vox model scoreboard` and
//! `vox model explain` use it to stop presenting a single composite rank over axes that trade
//! against each other. See `docs/src/adr/046-pareto-frontier-reporting.md`.

use super::{ModelScore, wilson_score_interval};

/// One candidate's position in objective space. `quality` is higher-is-better; `cost_usd` and
/// `latency_ms` are lower-is-better.
///
/// `None` on a lower-is-better axis means **no measurement**, and is treated as
/// *incomparable*: it neither establishes superiority nor concedes it. Not "worst possible"
/// (that permanently dominates out every model without a `model_scoreboard` row) and not
/// "neutral" (that is non-transitive — see [`pareto_frontier`]).
///
/// Identity is the point's **position** in the slice passed to [`pareto_frontier`]; there is
/// deliberately no `index` field to drift out of sync with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParetoPoint {
    pub quality: f64,
    pub cost_usd: Option<f64>,
    pub latency_ms: Option<i64>,
}

/// `true` when `a` dominates `b`: at least as good on every comparable axis, strictly better
/// on at least one. Non-finite values compare false both ways, so a `NaN` point neither
/// dominates nor is dominated.
fn dominates(a: &ParetoPoint, b: &ParetoPoint) -> bool {
    /// An unknown reading is incomparable, not neutral. Neutral (`_ => true`) is not
    /// transitive — `Some(5) ≼ None ≼ Some(1)` while `Some(5) ⋠ Some(1)` — which admits
    /// domination cycles and an empty frontier for non-empty input.
    fn no_worse<T: PartialOrd>(a: Option<T>, b: Option<T>) -> bool {
        match (a, b) {
            (Some(a), Some(b)) => a <= b,
            (None, None) => true,
            _ => false,
        }
    }
    fn strictly_better<T: PartialOrd>(a: Option<T>, b: Option<T>) -> bool {
        matches!((a, b), (Some(a), Some(b)) if a < b)
    }

    if !(a.quality >= b.quality
        && no_worse(a.cost_usd, b.cost_usd)
        && no_worse(a.latency_ms, b.latency_ms))
    {
        return false;
    }
    a.quality > b.quality
        || strictly_better(a.cost_usd, b.cost_usd)
        || strictly_better(a.latency_ms, b.latency_ms)
}

/// Positions of every point that no other point dominates, in input order.
///
/// Never returns empty for non-empty input: [`dominates`] is irreflexive and transitive —
/// transitive *because* an unknown axis is incomparable rather than neutral, so no chain of
/// comparisons can cycle — and a strict partial order on a finite set always has a maximal
/// element. A `NaN` quality is incomparable in both directions, so such a point is always
/// maximal and never empties the frontier.
#[must_use]
pub fn pareto_frontier(points: &[ParetoPoint]) -> Vec<usize> {
    (0..points.len())
        .filter(|&i| !points.iter().any(|other| dominates(other, &points[i])))
        .collect()
}

/// The single mapping from a scoreboard row to objective space, shared by every surface so
/// two of them cannot mark different frontiers.
///
/// Quality is the 95% Wilson **lower** bound of the observed success rate. `success` means
/// "the provider returned a non-error response" (see this plan's Global Constraints), so this
/// axis is *reliability*, not answer quality. A row with no observations yields `0.0` — which
/// is why callers must gate on [`is_observed`] rather than putting such rows on a frontier.
///
/// A non-finite or negative cost is discarded to `None`: unknown, never an unbeatable minimum.
#[must_use]
pub fn pareto_point_for(score: Option<&ModelScore>) -> ParetoPoint {
    ParetoPoint {
        quality: score
            .and_then(|s| wilson_score_interval(s.success_count, s.n_calls))
            .map_or(0.0, |(lo, _hi)| lo),
        cost_usd: score
            .and_then(|s| s.cost_per_success_usd)
            .filter(|c| c.is_finite() && *c >= 0.0),
        latency_ms: score.and_then(|s| s.p50_latency_ms),
    }
}

/// Whether a row has any observations at all. A row with `n_calls == 0` has quality `0.0` by
/// construction, so including it in a frontier would either delete it (if compared) or hand it
/// an unearned mark (if all its axes are unknown and thus incomparable). Neither is honest:
/// "we measured this and it lost" and "we never tried it" are different facts.
#[must_use]
pub fn is_observed(score: Option<&ModelScore>) -> bool {
    score.is_some_and(|s| s.n_calls > 0)
}
```

Add `is_observed` to the `mod.rs` re-export from Step 1: `pub use pareto::{ParetoPoint, is_observed, pareto_frontier, pareto_point_for};`

- [ ] **Step 4: Run the tests to verify they pass** — `cargo test -p vox-orchestrator --lib models::pareto` → PASS, 21 tests.

- [ ] **Step 4b: Format** — `cargo fmt -p vox-orchestrator`

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/models/pareto.rs crates/vox-orchestrator/src/models/mod.rs
git commit -m "$(cat <<'EOF'
feat(vox-orchestrator): pareto frontier + shared scoreboard-row mapping (Task M3)

A pure reporting helper for `vox model scoreboard`/`explain`, in models/
beside wilson_score_interval rather than routing/ because it does not
feed selection. Owns the only ModelScore -> objective-space mapping so
the two CLI surfaces cannot mark different frontiers.

An unmeasured axis is treated as INCOMPARABLE rather than neutral or
worst-case. Worst-case permanently dominates out every model without a
scoreboard row. Neutral is not transitive -- Some(5) <= None <= Some(1)
while Some(5) > Some(1) -- and a four-point cycle then makes every point
dominated, returning an empty frontier for non-empty input.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: `vox model explain` — suppress unrankable models, mark the frontier

The requirement says *"suppress ranks below minimum-N."* `e497a82fb` shipped a `(low-N)` **marker**; nothing is suppressed, and a 2-call model still wears 🥇.

**Files:**
- Modify: `crates/vox-cli/src/commands/model/explain.rs`
- Modify: `crates/vox-orchestrator/src/models/registry.rs` (doc comment on `MIN_CALLS_FOR_CONFIDENT_RANK`, `:73`)

**Interfaces:**
- Consumes: `MIN_CALLS_FOR_CONFIDENT_RANK` (`registry.rs:73`), `pareto_point_for` / `pareto_frontier` / `is_observed` (Task 2), `registry.get_score` (`registry.rs:336`).
- Produces: `fn partition_by_rank_confidence<'a>(candidates: &'a [ModelSpec], n_calls_of: impl Fn(&str) -> Option<i64>) -> (Vec<&'a ModelSpec>, Vec<&'a ModelSpec>)`; `fn selection_note(n_calls: i64) -> String`; `fn render_candidate_sections(...) -> String`.

**Why a `-> String` renderer:** `run()` is `async` and calls `DbConfig::resolve_canonical()`, `VoxDb::connect()`, and `RouteCapabilityPolicySnapshot::from_env()`, so it can never be unit-tested. Without extraction, `partition_by_rank_confidence` can be perfect while `run()` still prints the old unpartitioned loop — the exact defect this task exists to fix, passing green.

- [ ] **Step 1: Extend both import lists, then write the failing tests**

The test module at `explain.rs:207` is an **explicit list**, not `use super::*` — new tests will not resolve without this edit:

```rust
// crates/vox-cli/src/commands/model/explain.rs:5 — top-of-file imports
use vox_orchestrator::models::{
    MIN_CALLS_FOR_CONFIDENT_RANK, ModelRegistry, ModelScore, ModelSpec, is_observed,
    pareto_frontier, pareto_point_for,
};

// crates/vox-cli/src/commands/model/explain.rs:207 — test-module imports
use super::{
    partition_by_rank_confidence, render_candidate_sections, render_free_tier, selection_note,
    success_rate_display,
};
```

```rust
// in `mod tests`

#[test]
fn partition_by_rank_confidence_demotes_low_call_models_out_of_the_ranked_list() {
    let models = vec![
        free_spec("a/low-n", ProviderType::OpenRouter, ModelTier::Pro),
        free_spec("b/confident", ProviderType::OpenRouter, ModelTier::Pro),
    ];
    let (ranked, unranked) = partition_by_rank_confidence(&models, |id| match id {
        "a/low-n" => Some(2),
        "b/confident" => Some(40),
        _ => None,
    });
    assert_eq!(ranked.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["b/confident"]);
    assert_eq!(unranked.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["a/low-n"]);
}

#[test]
fn partition_by_rank_confidence_treats_a_missing_scoreboard_row_as_unranked() {
    let models = vec![free_spec("novel/model", ProviderType::OpenRouter, ModelTier::Pro)];
    let (ranked, unranked) = partition_by_rank_confidence(&models, |_| None);
    assert!(ranked.is_empty(), "no data is not evidence of rank-worthiness");
    assert_eq!(unranked.len(), 1);
}

#[test]
fn partition_by_rank_confidence_admits_exactly_at_the_threshold() {
    // Uses the symbolic constant: a hardcoded `n >= 5` that drifts when the constant changes
    // would still pass a literal-valued test.
    let models = vec![free_spec("edge/model", ProviderType::OpenRouter, ModelTier::Pro)];
    let (ranked, _) = partition_by_rank_confidence(&models, |_| Some(MIN_CALLS_FOR_CONFIDENT_RANK));
    assert_eq!(ranked.len(), 1, "the threshold is inclusive");
    let (ranked, _) =
        partition_by_rank_confidence(&models, |_| Some(MIN_CALLS_FOR_CONFIDENT_RANK - 1));
    assert!(ranked.is_empty(), "one call below the threshold is not rankable");
}

#[test]
fn partition_by_rank_confidence_preserves_input_order_within_each_half() {
    let models = vec![
        free_spec("a/hi", ProviderType::OpenRouter, ModelTier::Pro),
        free_spec("b/lo", ProviderType::OpenRouter, ModelTier::Pro),
        free_spec("c/hi", ProviderType::OpenRouter, ModelTier::Pro),
        free_spec("d/lo", ProviderType::OpenRouter, ModelTier::Pro),
    ];
    let (ranked, unranked) = partition_by_rank_confidence(&models, |id| {
        if id.ends_with("/hi") { Some(50) } else { Some(1) }
    });
    assert_eq!(ranked.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["a/hi", "c/hi"]);
    assert_eq!(unranked.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["b/lo", "d/lo"]);
}

#[test]
fn partition_by_rank_confidence_handles_an_empty_candidate_list() {
    let (ranked, unranked) = partition_by_rank_confidence(&[], |_| Some(99));
    assert!(ranked.is_empty() && unranked.is_empty());
}

#[test]
fn render_candidate_sections_never_puts_a_medal_on_an_unranked_model() {
    // The defect e497a82fb shipped: present-but-marked is not suppression.
    let models = vec![
        free_spec("a/low-n", ProviderType::OpenRouter, ModelTier::Pro),
        free_spec("b/confident", ProviderType::OpenRouter, ModelTier::Pro),
    ];
    let (ranked, unranked) = partition_by_rank_confidence(&models, |id| {
        if id == "b/confident" { Some(40) } else { Some(2) }
    });
    let out = render_candidate_sections(&ranked, &unranked, |_| None);
    let medal_line = out.lines().find(|l| l.contains('🥇')).expect("a medal is rendered");
    assert!(medal_line.contains("b/confident"), "medal must go to the ranked model: {medal_line}");
    assert!(out.contains("a/low-n"), "the demoted model must still be listed");
    let low_n_line = out.lines().find(|l| l.contains("a/low-n")).expect("listed");
    assert!(
        !low_n_line.contains('🥇') && !low_n_line.contains('🥈') && !low_n_line.contains('🥉'),
        "a 2-call model must hold no rank position: {low_n_line}"
    );
}

#[test]
fn render_candidate_sections_says_so_when_nothing_is_rankable() {
    let models = vec![free_spec("x/new", ProviderType::OpenRouter, ModelTier::Pro)];
    let (ranked, unranked) = partition_by_rank_confidence(&models, |_| None);
    let out = render_candidate_sections(&ranked, &unranked, |_| None);
    assert!(out.contains("no model has enough observations"), "{out}");
    assert!(!out.contains('🥇'), "an empty ranked half renders no medals: {out}");
}

#[test]
fn selection_note_flags_a_selection_that_is_not_rankable() {
    // The router's pick is reported as-is (changing it would fabricate a routing claim), but
    // it must not read as endorsed when it sits in the UNRANKED section.
    assert!(selection_note(2).contains("unranked"), "{}", selection_note(2));
    assert_eq!(selection_note(MIN_CALLS_FOR_CONFIDENT_RANK), "");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p vox-cli --lib commands::model::explain 2>&1 | head -30
```

Expected: compile failure — `error[E0432]: unresolved import` naming `partition_by_rank_confidence`, `render_candidate_sections`, and `selection_note` (the test module now imports symbols that do not exist yet).

- [ ] **Step 3: Write the minimal implementation**

```rust
// crates/vox-cli/src/commands/model/explain.rs, above `run()`.

/// Task M3 ("suppress ranks below minimum-N"): splits candidates into those with enough
/// observations to hold a rank position and those without. `e497a82fb` only *marked* low-N
/// rows, so a 2-call model still printed as rank #1 — the false confidence the requirement
/// targets. `None` (no scoreboard row) is unranked: no data is not evidence of rank-worthiness.
fn partition_by_rank_confidence<'a>(
    candidates: &'a [ModelSpec],
    n_calls_of: impl Fn(&str) -> Option<i64>,
) -> (Vec<&'a ModelSpec>, Vec<&'a ModelSpec>) {
    candidates
        .iter()
        .partition(|m| n_calls_of(&m.id).is_some_and(|n| n >= MIN_CALLS_FOR_CONFIDENT_RANK))
}

/// Annotation for the `Selection:` line when the router's pick is not rankable. The pick
/// itself is never changed — the ranked list is ordered by observed performance while the
/// selection is what the priority scorer chose, and they legitimately differ. Printing the
/// ranked leader as "Selection" would fabricate a routing claim.
fn selection_note(n_calls: i64) -> String {
    if n_calls >= MIN_CALLS_FOR_CONFIDENT_RANK {
        String::new()
    } else {
        format!(
            " (unranked: {n_calls} observed call(s) < {MIN_CALLS_FOR_CONFIDENT_RANK}; the list \
             above is ordered by observed performance, the selection is not)"
        )
    }
}

/// Pure render of both candidate sections, extracted so the partition *and* its presentation
/// are testable — `run()` is async and touches the DB and env, so it never can be.
fn render_candidate_sections(
    ranked: &[&ModelSpec],
    unranked: &[&ModelSpec],
    score_of: impl Fn(&str) -> Option<ModelScore>,
) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();

    // Frontier over the ranked half only: `partition_by_rank_confidence` already excluded
    // unobserved rows, so this is confidence-gated by construction.
    let points: Vec<_> = ranked
        .iter()
        .map(|m| pareto_point_for(score_of(&m.id).as_ref()))
        .collect();
    let frontier = pareto_frontier(&points);

    let _ = writeln!(
        out,
        "Top Candidates (ranked; models with < {MIN_CALLS_FOR_CONFIDENT_RANK} observed calls are \
         listed separately):"
    );
    for (i, entry) in ranked.iter().take(5).enumerate() {
        let prefix = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "  ",
        };
        let mut details = vec![format!("Tier: {:?}", entry.capabilities.tier)];
        if let Some(score) = score_of(&entry.id) {
            details.push(success_rate_display(score.success_count, score.n_calls));
        }
        let mark = if frontier.contains(&i) { " [pareto-optimal]" } else { "" };
        let _ = writeln!(out, "{prefix} {}: {}{mark}", entry.id, details.join(", "));
    }
    if ranked.is_empty() {
        let _ = writeln!(out, "  (no model has enough observations to rank yet)");
    }

    if !unranked.is_empty() {
        let _ = writeln!(out, "\nInsufficient data to rank ({} model(s)):", unranked.len());
        for entry in unranked.iter().take(5) {
            let n = score_of(&entry.id).map_or(0, |s| s.n_calls);
            let _ = writeln!(out, "  - {}: {n} observed call(s)", entry.id);
        }
    }
    out
}
```

In `run()`, replace the "Top Candidates" block (`explain.rs:138-164`) with:

```rust
    let (ranked, unranked) =
        partition_by_rank_confidence(&candidates, |id| registry.get_score(id).map(|s| s.n_calls));
    println!(
        "{} {}",
        " RANK ".on_green().black().bold(),
        render_candidate_sections(&ranked, &unranked, |id| registry.get_score(id).cloned())
    );
```

and replace the `Selection:` line (`explain.rs:166`) with:

```rust
    let selected_calls = registry.get_score(&candidates[0].id).map_or(0, |s| s.n_calls);
    println!(
        "\nSelection: {}{}",
        candidates[0].id.green().bold(),
        selection_note(selected_calls).yellow()
    );
```

Extend the constant's doc comment at `registry.rs:73`:

```rust
/// Task M3: below this many observed *calls*, a model holds no rank position — see
/// `vox model explain`'s `partition_by_rank_confidence`.
///
/// Deliberately distinct from `vox model scoreboard`'s `COST_PER_SUCCESS_MIN_SUCCESSES` (10):
/// these gate different statistics on different denominators. Ranking needs enough *calls* to
/// place a model relative to others; a cost-per-success *ratio* needs enough *successes* in
/// its denominator before one expensive fallback swings it by an order of magnitude.
/// Equalizing them would make one of the two wrong.
///
/// The two surfaces also read it differently, deliberately: `explain` **suppresses**
/// sub-threshold models from its ranked list (a rank position is a claim), while `scoreboard`
/// only **marks** them — a scoreboard is a full inventory keyed (model, category, strength),
/// and hiding thin rows would hide what a reader queries it for. Neither surface will mark a
/// sub-threshold row Pareto-optimal.
pub const MIN_CALLS_FOR_CONFIDENT_RANK: i64 = 5;
```

- [ ] **Step 4: Run the tests to verify they pass** — `cargo test -p vox-cli --lib commands::model::explain` and `cargo test -p vox-orchestrator --lib models::registry`. PASS, including the pre-existing `success_rate_display_*` and `render_free_tier_*`.

- [ ] **Step 4b: Format** — `cargo fmt -p vox-cli && cargo fmt -p vox-orchestrator`

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/model/explain.rs crates/vox-orchestrator/src/models/registry.rs
git commit -m "$(cat <<'EOF'
fix(vox-cli): actually suppress ranks below minimum-N in model explain (Task M3 defect)

Task M3 asks to "suppress ranks below minimum-N". e497a82fb shipped a
"(low-N)" marker instead, so a 2-call model still printed as rank #1.
`vox model explain` now partitions into a ranked list and an explicit
"insufficient data to rank" section, and marks Pareto-optimal rows
within the ranked half.

The `Selection:` line still reports the router's actual pick -- naming
the ranked leader instead would fabricate a routing claim -- but is
annotated when that pick is itself unranked, which otherwise printed an
endorsement one line below the section saying it lacks data.

Rendering is extracted to a pure `-> String` fn: run() is async and
touches the DB and env, so without that the partition could be correct
while the output stayed wrong, with tests green.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: `vox model scoreboard` — mark the frontier, recommend within budget

**Files:**
- Modify: `crates/vox-cli/src/commands/model/scoreboard.rs`

**Interfaces:**
- Consumes: `ParetoPoint`, `pareto_frontier`, `pareto_point_for`, `is_observed`, `MIN_CALLS_FOR_CONFIDENT_RANK` (Tasks 2-3); `ModelScoreboardRow`; the existing `From<ModelScoreboardRow> for ModelScore` (`registry.rs:31-45`).
- Produces: `fn frontier_marker(frontier: &[usize], position: usize, n_calls: i64) -> &'static str`; `fn budget_recommendation(points: &[ParetoPoint], frontier: &[usize], budget_usd: f64) -> Option<usize>`; `fn render_budget_line(labels: &[String], points: &[ParetoPoint], frontier: &[usize], budget_usd: f64) -> String`; `fn pareto_legend() -> &'static str`; a `--budget <usd>` flag.
- **The budget comes from the flag, not `safety.max_cost_usd_per_request`** — that config is a per-*request* ceiling defaulting to 5.0 (`routing/policy.rs:130-132`) while these rows carry an amortized *cost per success*. Comparing them is a unit mismatch that never binds; having the reader supply the number fixes the unit at the call site.

- [ ] **Step 1: Write the failing tests**

```rust
// crates/vox-cli/src/commands/model/scoreboard.rs, in the existing `mod tests` (`:116`,
// which is `use super::*;`).

fn score(success_count: i64, n_calls: i64, cost: Option<f64>, p50: Option<i64>) -> ModelScore {
    ModelScore {
        success_count,
        n_calls,
        cost_per_success_usd: cost,
        p50_latency_ms: p50,
        ..ModelScore::default()
    }
}

#[test]
fn frontier_marker_annotates_only_observed_frontier_members() {
    // Frontier mixes parity (1, 2) so a `position % 2 == 0` implementation cannot pass by
    // accident, which an earlier [0, 2] fixture allowed.
    assert_eq!(frontier_marker(&[1, 2], 0, 40), "");
    assert_eq!(frontier_marker(&[1, 2], 1, 40), " *");
    assert_eq!(frontier_marker(&[1, 2], 2, 40), " *");
    assert_eq!(frontier_marker(&[1, 2], 3, 40), "");
    assert_eq!(frontier_marker(&[], 0, 40), "", "an empty frontier marks nothing");
    assert_eq!(frontier_marker(&[0], 0, 40), " *", "a singleton frontier marks its member");
}

#[test]
fn frontier_marker_never_marks_a_low_n_row() {
    // `success_rate_cell` already prints "(low-N)" on these. A row reading
    // "100.0% (low-N) *" claims both "untrustworthy" and "unbeaten" at once.
    assert_eq!(frontier_marker(&[0], 0, MIN_CALLS_FOR_CONFIDENT_RANK - 1), "");
    assert_eq!(frontier_marker(&[0], 0, MIN_CALLS_FOR_CONFIDENT_RANK), " *");
}

#[test]
fn budget_recommendation_picks_the_best_row_the_reader_can_afford() {
    // wilson_lo(95,100)=0.888248 > wilson_lo(85,100)=0.767163, and neither dominates
    // (a wins quality, b wins cost), so frontier == [0,1].
    // Kills "return the cheapest affordable" (fails the 1.00 case) and "ignore the budget,
    // return highest quality" (fails the 0.05 case).
    let points = vec![
        pareto_point_for(Some(&score(95, 100, Some(0.10), Some(400)))),
        pareto_point_for(Some(&score(85, 100, Some(0.02), Some(400)))),
    ];
    let frontier = pareto_frontier(&points);
    assert_eq!(frontier, vec![0, 1], "precondition: a quality/cost tradeoff");
    assert_eq!(budget_recommendation(&points, &frontier, 0.05), Some(1));
    assert_eq!(budget_recommendation(&points, &frontier, 1.00), Some(0));
}

#[test]
fn budget_recommendation_prefers_the_cheaper_row_on_a_quality_tie() {
    // Identical (success_count, n_calls) gives an exactly equal Wilson bound, so ties are
    // routine. `max_by` returns the LAST maximum, and `get_model_scoreboard` has no ORDER BY
    // (`ops_scientia.rs:21-29`), so leaving the tie to row order is both cost-blind and
    // irreproducible across runs.
    let points = vec![
        pareto_point_for(Some(&score(9, 10, Some(0.01), Some(900)))),
        pareto_point_for(Some(&score(9, 10, Some(0.90), Some(100)))),
    ];
    let frontier = pareto_frontier(&points);
    assert_eq!(budget_recommendation(&points, &frontier, 10.0), Some(0));
}

#[test]
fn budget_recommendation_never_recommends_an_off_frontier_row() {
    let points = vec![
        pareto_point_for(Some(&score(95, 100, Some(0.10), Some(100)))),
        pareto_point_for(Some(&score(85, 100, Some(0.02), Some(100)))),
        pareto_point_for(Some(&score(80, 100, Some(0.02), Some(900)))), // dominated by [1]
    ];
    let frontier = pareto_frontier(&points);
    assert_eq!(frontier, vec![0, 1], "precondition: row 2 is dominated");
    assert_eq!(budget_recommendation(&points, &frontier, 0.05), Some(1));
    // Kills an implementation that ignores `frontier` and scans all points.
    assert_eq!(
        budget_recommendation(&points, &[0], 0.05),
        None,
        "only frontier positions are candidates"
    );
}

#[test]
fn budget_recommendation_is_none_when_nothing_is_affordable() {
    let points = vec![pareto_point_for(Some(&score(95, 100, Some(0.10), Some(400))))];
    let frontier = pareto_frontier(&points);
    assert_eq!(budget_recommendation(&points, &frontier, 0.001), None);
}

#[test]
fn budget_recommendation_excludes_unknown_cost_rows() {
    // Unknown cost stays on the frontier (no evidence against it) but cannot be recommended:
    // a recommendation commits, and "we don't know what this costs" is not affordability.
    let points = vec![pareto_point_for(Some(&score(99, 100, None, Some(100))))];
    let frontier = pareto_frontier(&points);
    assert_eq!(frontier, vec![0]);
    assert_eq!(budget_recommendation(&points, &frontier, 100.0), None);
}

#[test]
fn budget_recommendation_rejects_a_nonsense_budget() {
    let points = vec![pareto_point_for(Some(&score(95, 100, Some(0.10), Some(400))))];
    let frontier = pareto_frontier(&points);
    for bad in [f64::NAN, -1.0, 0.0] {
        assert_eq!(budget_recommendation(&points, &frontier, bad), None, "budget {bad}");
    }
}

#[test]
fn render_budget_line_names_a_row_that_is_both_marked_and_affordable() {
    let labels = vec!["a/pricey (codegen/pro)".to_string(), "b/cheap (codegen/pro)".to_string()];
    let points = vec![
        pareto_point_for(Some(&score(95, 100, Some(0.10), Some(400)))),
        pareto_point_for(Some(&score(85, 100, Some(0.02), Some(400)))),
    ];
    let frontier = pareto_frontier(&points);
    let line = render_budget_line(&labels, &points, &frontier, 0.05);
    assert!(line.contains("b/cheap"), "{line}");
    assert!(!line.contains("a/pricey"), "must not name the over-budget row: {line}");
}

#[test]
fn render_budget_line_says_nothing_qualifies_rather_than_naming_an_overbudget_row() {
    let labels = vec!["a/pricey (codegen/pro)".to_string()];
    let points = vec![pareto_point_for(Some(&score(95, 100, Some(0.10), Some(400))))];
    let frontier = pareto_frontier(&points);
    let line = render_budget_line(&labels, &points, &frontier, 0.001);
    assert!(!line.contains("a/pricey"), "must not name an unaffordable row: {line}");
    assert!(line.contains("no Pareto-optimal row qualifies"), "{line}");
}

#[test]
fn render_budget_line_disambiguates_rows_of_the_same_model() {
    // A scoreboard row is a (model, category, strength) triple; the same model appears more
    // than once. The recommendation must say which row it means.
    let labels = vec!["m/x (codegen/pro)".to_string(), "m/x (research/pro)".to_string()];
    let points = vec![
        pareto_point_for(Some(&score(50, 100, Some(0.90), Some(400)))),
        pareto_point_for(Some(&score(95, 100, Some(0.02), Some(100)))),
    ];
    let frontier = pareto_frontier(&points);
    let line = render_budget_line(&labels, &points, &frontier, 1.0);
    assert!(line.contains("research/pro"), "must name the winning row's category: {line}");
}

#[test]
fn pareto_legend_states_the_strictly_better_clause_and_avoids_the_word_quality() {
    // "at least as good on all axes" alone is wrong: identical rows are each at least as good
    // as the other, yet neither is dominated, so both are marked.
    let legend = pareto_legend();
    assert!(legend.contains("strictly better"), "{legend}");
    // Global Constraint, enforced mechanically rather than by prose: the axis is reliability.
    assert!(!legend.to_lowercase().contains("quality"), "{legend}");
}

#[test]
fn budget_flag_parses_and_defaults_to_none() {
    use clap::Parser as _;
    let with = ScoreboardArgs::try_parse_from(["scoreboard", "--budget", "0.05"]).expect("parses");
    assert_eq!(with.budget, Some(0.05));
    let without = ScoreboardArgs::try_parse_from(["scoreboard"]).expect("parses");
    assert_eq!(without.budget, None);
    assert!(ScoreboardArgs::try_parse_from(["scoreboard", "--budget", "abc"]).is_err());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p vox-cli --lib commands::model::scoreboard 2>&1 | head -40
```

Expected: a cascade of `error[E0412]`/`error[E0425]`. The **first** is `E0412: cannot find type \`ModelScore\`` in the `score()` helper (the `use` line arrives in Step 3), not the functions under test.

- [ ] **Step 3: Write the minimal implementation**

```rust
// crates/vox-cli/src/commands/model/scoreboard.rs — extend the top-of-file imports.
use vox_orchestrator::models::{
    MIN_CALLS_FOR_CONFIDENT_RANK, ModelScore, ParetoPoint, is_observed, pareto_frontier,
    pareto_point_for,
};
```

Add the `--budget` flag to `ScoreboardArgs`:

```rust
    /// Recommend the best Pareto-optimal row costing no more than this many USD per success.
    /// Omit to skip the recommendation line.
    #[arg(long)]
    pub budget: Option<f64>,
```

```rust
/// Suffix marking a row Pareto-optimal. Gated on the same observation threshold as
/// `success_rate_cell`'s `(low-N)`: a row reading "100.0% (low-N) *" would claim both
/// "untrustworthy" and "unbeaten". The gate lives here rather than inside `pareto_frontier`,
/// which is a pure geometric filter with no opinion about sample size.
fn frontier_marker(frontier: &[usize], position: usize, n_calls: i64) -> &'static str {
    if n_calls >= MIN_CALLS_FOR_CONFIDENT_RANK && frontier.contains(&position) {
        " *"
    } else {
        ""
    }
}

/// Task M3's "budget-constrained argmax" as a recommendation: the best frontier row costing no
/// more than `budget_usd`. `None` when nothing qualifies — saying "nothing fits" is more useful
/// than recommending something that doesn't.
///
/// Rows with unknown cost are excluded: they stay on the frontier (absence of evidence is not
/// evidence of inferiority) but a recommendation has to commit. A non-finite or non-positive
/// budget also yields `None`; `run()` rejects those before calling, so this is defence in depth.
///
/// Searching only the frontier is lossless *because* [`pareto_frontier`]'s order is transitive
/// and never lets an unknown-cost row dominate a known-cost one: any affordable dominated row
/// has an affordable, at-least-as-good dominator on the frontier.
///
/// The tie-break is explicit and total — highest quality, then cheapest, then fastest, then
/// lowest row index. `Iterator::max_by` returns the *last* maximum, and `get_model_scoreboard`
/// has no `ORDER BY` (`vox-db/src/store/ops_scientia.rs:21-29`), so leaving a tie to row order
/// makes the recommendation both cost-blind and irreproducible across runs.
fn budget_recommendation(
    points: &[ParetoPoint],
    frontier: &[usize],
    budget_usd: f64,
) -> Option<usize> {
    if !budget_usd.is_finite() || budget_usd <= 0.0 {
        return None;
    }
    frontier
        .iter()
        .copied()
        .filter(|&i| points[i].cost_usd.is_some_and(|c| c <= budget_usd))
        .min_by(|&a, &b| {
            let key = |i: usize| {
                let p = &points[i];
                (
                    -p.quality,
                    p.cost_usd.unwrap_or(f64::INFINITY),
                    p.latency_ms.unwrap_or(i64::MAX),
                    i,
                )
            };
            let (qa, ca, la, ia) = key(a);
            let (qb, cb, lb, ib) = key(b);
            qa.total_cmp(&qb).then(ca.total_cmp(&cb)).then(la.cmp(&lb)).then(ia.cmp(&ib))
        })
}

/// The `--budget` recommendation line. `labels[i]` must identify the row's full
/// (model, category, strength) triple — the same model appears under several categories.
fn render_budget_line(
    labels: &[String],
    points: &[ParetoPoint],
    frontier: &[usize],
    budget_usd: f64,
) -> String {
    match budget_recommendation(points, frontier, budget_usd) {
        Some(i) => format!(
            "Within ${budget_usd:.4}/success: {} (best Pareto-optimal row you can afford)",
            labels[i]
        ),
        None => format!(
            "Within ${budget_usd:.4}/success: no Pareto-optimal row qualifies \
             (rows with unknown cost are not recommended)."
        ),
    }
}

/// The `*` legend. Says "strictly better" because domination requires it — two identical rows
/// are each "at least as good" as the other yet neither is dominated, so both are marked.
fn pareto_legend() -> &'static str {
    "* = not beaten by any other row on every measured axis (success rate as a Wilson lower \
     bound, cost/success, p50 latency) — i.e. no row is at least as good on all three and \
     strictly better on one. A row missing a measurement cannot be compared on it. Success \
     counts non-error provider responses, not answer correctness. Rows below the observation \
     threshold are never marked."
}
```

In `run()`, after the `--format json` early return (`:59-62`) and **before** the table loop:

```rust
    let labels: Vec<String> = rows
        .iter()
        .map(|r| format!("{} ({}/{})", r.model_id, r.task_category, r.strength_tag))
        .collect();
    let scores: Vec<ModelScore> = rows.iter().map(|r| ModelScore::from(r.clone())).collect();
    let points: Vec<ParetoPoint> = scores
        .iter()
        .map(|s| pareto_point_for(is_observed(Some(s)).then_some(s)))
        .collect();
    let frontier = pareto_frontier(&points);
```

Change the loop header at `:79` from `for row in rows {` to `for (i, row) in rows.into_iter().enumerate() {`, and the first cell at `:83` from `row.model_id,` to:

```rust
            format!("{}{}", row.model_id, frontier_marker(&frontier, i, row.n_calls)),
```

After `println!("{}", table);`:

```rust
    println!("\n{}", pareto_legend());
    if let Some(budget) = args.budget {
        anyhow::ensure!(
            budget.is_finite() && budget > 0.0,
            "--budget must be a finite, positive USD-per-success amount, got {budget}"
        );
        println!("\n{}", render_budget_line(&labels, &points, &frontier, budget));
    }
```

And in the JSON branch (`:59`), note the ignored flag rather than silently dropping it:

```rust
    if args.format == "json" {
        if args.budget.is_some() {
            eprintln!("note: --budget applies to the table output only; ignored with --format json");
        }
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
```

- [ ] **Step 4: Run the tests to verify they pass** — `cargo test -p vox-cli --lib commands::model::scoreboard`. PASS, 13 new plus the pre-existing `cost_per_success_display_*` and `success_rate_cell_*`.

- [ ] **Step 4b: Format** — `cargo fmt -p vox-cli`

- [ ] **Step 4c: Eyeball the real output.** No assertion can tell you a table is unreadable.

```bash
cargo run -p vox-cli -- model scoreboard
cargo run -p vox-cli -- model scoreboard --budget 0.05
cargo run -p vox-cli -- model scoreboard --format json --budget 0.05   # expect the stderr note
cargo run -p vox-cli -- model explain "write a unit test"
```

**If `model_scoreboard` is empty this step verifies nothing while appearing to succeed** — an empty table renders no `*`, and `render_budget_line` is never reached. Confirm with `cargo run -p vox-cli -- model scoreboard | head` that rows exist first; if not, say so rather than ticking the box.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/model/scoreboard.rs
git commit -m "$(cat <<'EOF'
feat(vox-cli): mark Pareto-optimal rows and recommend within a budget (Task M3)

Task M3 asks for a "Pareto set + budget-constrained argmax, not a
weighted sum". `vox model scoreboard` now marks rows no other row beats
on all of success rate (Wilson lower bound), cost/success, and p50
latency, and `--budget <usd>` names the best affordable one.

The budget is a flag rather than safety.max_cost_usd_per_request: that
config is a per-request ceiling (default 5.0) while these rows carry
amortized cost per success, so comparing them never binds.

The marker is gated on the same observation threshold as the "(low-N)"
cell -- a row reading "100.0% (low-N) *" would claim untrustworthy and
unbeaten at once -- and the recommendation's tie-break is an explicit
total order, since max_by returns the last maximum and
get_model_scoreboard has no ORDER BY.

Reporting only: no selection path is touched.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: ADR 046 + index row + SSOT corrections

**Files:**
- Create: `docs/src/adr/046-pareto-frontier-reporting.md`
- Modify: `docs/src/adr/index.md`
- Modify: `docs/src/architecture/model-catalog-ssot-2026.md`

- [ ] **Step 1: Write the ADR.** Frontmatter exactly as below — `category` is the lint's hard requirement (`crates/vox-doc-pipeline/src/pipeline/lint.rs:450-456`), `"current"` is in `VALID_STATUS` (`:37-45`), `date` is not inspected, and **do not** add `last_updated` (hard error, `:435-447`). Matches `045-tauri-gui-replaces-axum-dashboard.md`.

```md
---
title: "ADR 046 — Pareto-Frontier Reporting for Model Surfaces"
description: "Present model scoreboards as a Pareto set over reliability, cost, and latency instead of one composite rank — and why this is reporting-only, not a routing change."
date: "2026-09-03"
status: "current"
category: "Architecture Decisions (ADRs)"
---
```

Body must cover:

- **Context.** Task M3 asks for "Pareto set + budget-constrained argmax, not a weighted sum" under a heading titled *Surface*, beside five unambiguous reporting items.
- **Decision.** Implement it as reporting. No selection path changes.
- **Why not a routing change** — four independent reasons, each verified against source on 2026-09-03:
  1. `success` means "non-error provider response", not answer correctness (`crates/vox-actor-runtime/src/llm/chat.rs:200`, `:433`; `false` only at `:252`, `:469`), and the MCP chat surface hardcodes `success: true` outright (`crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs:1357`). A router optimizing it optimizes provider uptime. `quality_score` was already retired for being a constant 1.0.
  2. The Wilson lower bound rises **monotonically with sample count at a fixed observed rate, at a decelerating rate** — 0.596 → 0.826 → 0.880 for 9/10, 90/100, 900/1000. An incumbent's bound improves purely by being sampled, so a frontier built on it plus frontier-restricted exploration is a rich-get-richer loop, not an explorer.
  3. `ModelSelectionEngine` is constructed at exactly one non-test site — inside `resolve_model_with_registry_fallbacks`, which has **zero production callers** (two test files, two re-exports). Live selection runs through `models::select::decide` (`models/select.rs:87`), `best_for_task_with_filter` (`registry.rs:718`), `FreeTierRouter`, and pins. A knob on that engine would change no production traffic while reading like a global routing switch.
  4. `RoutingPolicy` is `include_str!`-baked at compile time (`routing/policy.rs:233-239`) with a Clavis override for `epsilon_ceiling` but none for `algorithm`, so such a knob's rollback would be rebuild-and-redeploy, not a config push.
- **Consequences.** The reported frontier is over reliability/cost/latency and labelled as such. An unmeasured axis is *incomparable*, which keeps the order transitive and the frontier non-empty. Rows with no observations are excluded from the frontier and listed separately. Nothing feeds selection, so rollback is reverting the commits.
- **Supersedes.** `docs/src/architecture/model-autonomic-system-2026.md:19,177` describes `resolve_model_with_registry_fallbacks` as a live selection path and plans to keep it "as a thin wrapper … so older callers don't break". As of 2026-09-03 it has no production callers; this ADR supersedes that characterization.
- **Follow-ups.** Link §Blocked Follow-Ups.

**Do not** carry §Withdrawn Scope rows 2 and 6 into the ADR — they cite audit reasoning about code that no longer exists, not verifiable source. They belong in the plan appendix only.

- [ ] **Step 2: Add the index row.** Append after the `045` row, before the trailing `See also:` paragraph:

```md
| [046](046-pareto-frontier-reporting.md) | **Pareto-frontier reporting for model surfaces** — scoreboard/explain mark non-dominated rows over reliability, cost, and latency; explicitly reporting-only, with the four reasons a routing change was rejected |
```

- [ ] **Step 3: Correct four stale rows in the model-catalog SSOT.** `docs/src/architecture/model-catalog-ssot-2026.md:95-98` marks these "never read" or "Rust const only". All four are read today; nothing mechanically checks this table, so it rotted:

| Row | Actually read at |
|---|---|
| `LATENCY_EXCELLENT_MS` (`:95`) | `crates/vox-orchestrator/src/models/scoring.rs:76-78` (from `latency_bands.excellent_ms`) |
| `LATENCY_POOR_MS` (`:96`) | `crates/vox-orchestrator/src/models/scoring.rs:76-78` (from `latency_bands.poor_ms`) |
| `exploration.budget_usd_per_day` (`:97`) | `crates/vox-orchestrator/src/runtime.rs:708-710` |
| `safety.max_cost_usd_per_request` (`:98`) | `crates/vox-orchestrator/src/models/registry.rs:818-820`, enforced `:828-830` |

Change each to ✅ with the citation. Correcting only two of the four would leave the table half-rotten.

- [ ] **Step 4: Verify the docs gates.** `--paths` resolves relative to `docs/src/` (`crates/vox-doc-pipeline/src/pipeline/mod.rs`) — passing `docs/src/adr/...` resolves to `docs/src/docs/src/adr/...`, finds nothing, and reports clean while linting nothing.

```bash
cargo run -p vox-doc-pipeline -- --lint-only --paths adr/046-pareto-frontier-reporting.md
cargo run -p vox-cli -- ci doc-inventory generate --output docs/agents/doc-inventory.json
cargo run -p vox-cli -- ci doc-inventory verify
```

**The `generate` line is not optional.** `docs/agents/doc-inventory.json` is a per-file inventory (~5800 entries, each with `path` and `lines_total`), so creating `046-…md` and editing two existing docs makes it stale. `vox ci doc-inventory verify` only *checks* — it never writes — and `pre_push.rs:1111-1129` hard-errors on drift inside the `--complete` tier. `ssot-drift` does **not** catch this (`check_docs_ssot` only asserts the file exists with `schema_version >= 3`), so without the regeneration the failure first appears at Task 6.

Also verify the ADR's cross-tree link resolves — `vox ci check-links` runs in `--complete`. From `docs/src/adr/`, the plan is at `../../superpowers/plans/2026-09-03-m3-surface-pareto-reporting.md`.

- [ ] **Step 5: Commit** (stages the plan itself, which no earlier task touches)

```bash
git add docs/src/adr/046-pareto-frontier-reporting.md docs/src/adr/index.md \
        docs/src/architecture/model-catalog-ssot-2026.md docs/agents/doc-inventory.json \
        docs/superpowers/plans/2026-09-03-m3-surface-pareto-reporting.md
git commit -m "$(cat <<'EOF'
docs: ADR 046 — Pareto-frontier reporting, and why not routing (Task M3)

Records the four verified findings behind scoping M3's Pareto item as
reporting: success means non-error provider response rather than answer
correctness (and the MCP chat surface hardcodes it true); the Wilson
lower bound rises monotonically with sample count, so a frontier built
on it is rich-get-richer; ModelSelectionEngine's only non-test call site
has zero production callers; and RoutingPolicy.algorithm is compile-time
baked with no Clavis override.

Also corrects four model-catalog SSOT rows listing LATENCY_EXCELLENT_MS,
LATENCY_POOR_MS, exploration.budget_usd_per_day and
safety.max_cost_usd_per_request as never-read; all four are read today.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: Full-gate verification

- [ ] **Step 1: Run the tier that covers this diff**

```bash
cargo run -q -p vox-cli -- ci pre-push --complete
```

Expected PASS. Per gate: `check_fmt` (each task ran `cargo fmt -p`); full-tree doc lint (Task 5's literal frontmatter); `ssot-drift` — no contract or generated artifact is touched, and `--budget` adds no drift because `cli-command-surface.generated.md` records commands at path granularity with no flag column (`command_sync.rs:52`, generated line 226) and the capability registry has `parameters: null` for this command; workspace clippy `-D warnings` (`too-many-arguments-threshold = 12` in `clippy.toml`; nothing here approaches it, and no `#[allow]` is needed anywhere); scoped TOESTUB given Step 0's baseline.

- [ ] **Step 2: Confirm no selection-path file was touched**

```bash
if git diff --name-only "$(cat /tmp/m3-plan-base.sha)"..HEAD \
   | grep -E 'routing/(engine|policy)\.rs|registry_model_resolve\.rs|models/(select|scoring)\.rs|model-routing\.v1\.yaml'; then
  echo "SCOPE VIOLATION"
  exit 1
fi
echo "scope clean"
```

Two things matter here and an earlier draft got both wrong. It diffs against **Step 0's recorded SHA**, not `main` — this branch already carries Phase G and M0–M3 commits that touched guarded paths, so `main...HEAD` would report a violation from prior work. And it uses `if/then/exit 1` rather than `grep … && echo … || echo …`, which exits 0 in *both* branches and is therefore a print statement, not a gate.

---

## Withdrawn Scope

The first revision (7 tasks) added an opt-in `pareto_argmax` **selection algorithm** gated on `RoutingPolicy.exploration.algorithm`, with a 5% forced-exploration stream. Two six-track audits withdrew it. Rows marked *(audit-derived)* reason about code that no longer exists and are **not** carried into ADR 046.

| # | Finding | Evidence |
|---|---|---|
| 1 | The quality axis was provider uptime, not correctness | `crates/vox-actor-runtime/src/llm/chat.rs:200` — `success: true` on any `Ok(resp)`, no content inspection; `message.rs:1357` hardcodes it |
| 2 | Wilson-lower-bound quality + frontier-only exploration = permanent cold-start lockout *(audit-derived)* | traced independently by two tracks against the withdrawn code |
| 3 | The chosen dispatch site has no production callers | `resolve_model_with_registry_fallbacks` — two test files, two re-exports, zero production callers |
| 4 | No runtime kill switch | `routing/policy.rs:233-239` `include_str!`; Clavis override for `epsilon_ceiling`, none for `algorithm` |
| 5 | The budget clause compared unlike units and never bound | `safety.max_cost_usd_per_request` default 5.0 (`policy.rs:130-132`), per-request, vs an amortized `cost_per_success_usd` |
| 6 | With exploration off the frontier barely changed the pick — its only real effect was constraining exploration alternatives, i.e. the untested half was load-bearing *(audit-derived)* | audit reasoning, no source artifact |

Mechanical defects in that revision are not repeated here: a test asserting an unreachable index; `s + f` `u32` overflow that panics in debug (with `record_bandit_outcome_saturates_at_u32_max` proving the input reachable); an unused import failing `-D warnings`; and Task 1 committing a `mod.rs` referencing a Task 2 symbol.

## Blocked Follow-Ups

1. **Give `success` a real definition.** Wire M2's `completeness_ok` (`bd5c14e05`) into `ModelOutcome.success` so `success_rate` stops meaning "HTTP 200" — and fix `message.rs:1357`'s unconditional `true`. **Prerequisite for everything else here**, and it would retroactively give this plan's axis real meaning at no cost to these surfaces.
2. **Decide whether `ModelSelectionEngine` should exist.** Dead code with live-looking config (`exploration.*`, `routing_objective.kind`). Wire it to a real entry point or delete it and its contract block; a plausible-looking dead router is its own hazard.
3. **`novel_explores_so_far` is hardcoded `0`** (`registry_model_resolve.rs:205`), so `max_concurrent_explorations: 2` never binds and every untried model gets the novelty multiplier on every request. Dead ceiling in dead code.
4. **`vox-config`'s `ExplorationConfig` is a split-brain second parser** of `model-routing.v1.yaml` (`crates/vox-config/src/model_routing.rs:76-78`) carrying only `budget_usd_per_day`. Harmless today; a trap for the next routing config field.
5. **`ModelScore.n_calls` is an arbitrary task-category slice.** `refresh_model_scoreboard` builds its map with `scoreboard.insert(row.model_id.clone(), …)` — last row wins across `(task_category, strength_tag)` rows (`orchestrator/core/telemetry.rs:114-119`) — while `list_model_arm_stats` sums them. Any surface reading both mixes a total with a slice. `vox model scoreboard` sidesteps it by rendering per-row and labelling the triple; `vox model explain` does **not**. `explain` reads scores through `ModelRegistry::inject_scoreboard`, which is keyed by `model_id` in the registry API itself, so its per-model figures — including the rank-confidence suppression and the `[pareto-optimal]` mark — hang on one arbitrary slice. Re-keying to the triple would ripple into selection, outside this plan's reporting-only scope, so `explain` discloses the limitation in its rendered output instead. A per-model view, or `explain` showing per-model totals, would have to resolve it.
