---
title: "Vox Efficacy Benchmark — Adversarial Audit and Corrected Design (2026-09-01)"
description: "Seven-track adversarial review of the Vox efficacy benchmark plan, with empirically-confirmed defects including a total-compromise scoring exploit, a non-standard pass@k, a statistically inert significance test, and a corpus that exercises 17% of the language; plus the corrected design."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
training_rationale: "EXCLUDED DELIBERATELY. Documents a working exploit against the benchmark scorer and names held-out fixture IDs. Feeding this to MENS would both teach the exploit and leak held-out detail."
---

# Vox Efficacy Benchmark — Adversarial Audit (2026-09-01)

> **Audits:** [implementation plan](../../superpowers/plans/2026-09-01-vox-efficacy-benchmark-and-leaderboard.md) and [research spec](vox-mens-comparative-efficacy-benchmarking-research-2026-09-01.md), both authored 2026-09-01.
>
> **Method:** seven parallel adversarial review tracks (statistics/literature comparability, LLM-facade codebase audit, harness red-team, corpus/contamination, CI-automation, hostile peer review, language-surface coverage), plus first-party empirical verification against a real `vox` binary (`0.6.0+build.4866`). Every finding below marked **[VERIFIED]** was reproduced directly by this session, not merely reported.
>
> **Verdict:** the plan's *architecture* (symbolic verification, tuple rows, interval reporting) is sound. Its *implementation* contains a total-compromise scoring exploit, and its *headline claim* — ranking frontier models against each other — is not achievable at the current corpus size regardless of implementation quality. Do not run this benchmark or publish any number from it until C1–C7 are fixed.

## Severity index

| # | Defect | Severity | Status |
|---|---|---|---|
| C1 | `let assert` shadowing scores wrong answers as correct | **critical** | [VERIFIED] exploit |
| C2 | `pass@k` is not the literature estimator; degenerate at n=k | **critical** | [VERIFIED] |
| C3 | Early-break makes the estimator uncomputable, biases cost | **critical** | confirmed by review |
| C4 | CI-overlap significance test → effective α ≈ 0.005 | **critical** | confirmed by review |
| C5 | n=31 gives 9% power at a 10-point difference | **critical** | [VERIFIED] |
| C6 | Pipe deadlock; and failure detail reads the wrong stream | **critical** | [VERIFIED] |
| C7 | Held-out split is triple-inconsistent; a held-out twin leaks | **critical** | [VERIFIED] |
| H1–H8 | Metric fabrication, guard false positives, facade defects | high | mixed |
| S1–S3 | Corpus measures 17% of the language; no LLM harness exists | strategic | confirmed |

---

## C1 — Total-compromise scoring exploit [VERIFIED]

**A four-line candidate scores 100% on every fixture, and the integrity guard passes it as clean.**

```vox
let assert = fn(c: bool) to bool { return true }

fn nth_prime(n: int) to int {
    return 0        // wrong for every input
}
```

Composed with fixture `041`'s real assertion block exactly as Task 4 specifies, this yields `vox check` exit 0 and `vox run --mode interp` exit 0 — the harness records `compiled: true, tests_passed: true`.

Empirically established mechanics (each tested):

| Shadowing form | Effect |
|---|---|
| `fn assert(c: bool) to bool { … }` top-level, before `main` | **no effect** — builtin wins, exit 1 |
| `fn assert(…)` after `main` | **no effect** — exit 1 |
| `let assert = fn(c: bool) to bool { return true }` top-level | **EXPLOIT** — exit 0 on a wrong answer |
| `let assert = …` inside `main` | **EXPLOIT** — exit 0 |

Controls confirmed sound: a wrong answer without shadowing exits 1; a correct answer exits 0. So the harness is otherwise discriminating — this is a specific hole, not general breakage.

Note this refines the review that surfaced it: the reported mechanism (a top-level `fn assert`) does **not** work. Only the `let`-bound closure does. Had the fix been written from the report alone, it would have guarded the wrong construct and left the real hole open.

**Why the guard misses it:** Task 5's `MARKERS` scans for filesystem/network/answer-key strings. `let assert` contains none.

**Fix (defense in depth, all three):**
1. **Reject shadowing at ingest** — refuse any candidate matching `^\s*(let|fn)\s+(assert|main|print)\b`. Cheap, exact, no parser needed.
2. **Make `assert` non-shadowable in the eval path** — the real fix; a benchmark whose oracle can be redefined by the code under test is not an oracle. This is a `vox-compiler` change with value beyond the benchmark.
3. **Differential control** — additionally run each candidate against a *mutated* assertion block whose expected values are wrong. A correct solution must FAIL that. Any candidate passing both the real and mutated blocks is neutralizing assertions; score it as cheating. This catches the whole class, including forms nobody has thought of.

Fix 3 is the one that generalizes and should be considered mandatory before any public run.

---

## C2 — `pass@k` is not pass@k [VERIFIED]

Task 1 computes `pass_at_k` as "any attempt passed" with `k = max(attempts.len())`. The literature standard ([Chen et al. 2021, arXiv:2107.03374](https://arxiv.org/abs/2107.03374)) is the unbiased estimator over n samples with c correct:

```
pass@k = E_problems[ 1 − C(n−c, k) / C(n, k) ]
```

Two independent errors. **First, the estimator.** At n=k the plan's indicator is what you get, and it is degenerate — verified across all n≤60 that the product form matches the naive binomial exactly, and that at n=k any fixture with c≥1 scores **1.000**:

| c=1 | n=1 | n=5 | n=10 | n=20 |
|---|---|---|---|---|
| pass@k at k=n | 1.000 | 1.000 | 1.000 | 1.000 |
| pass@1 (correct) | 1.000 | 0.200 | 0.100 | 0.050 |

**Second, `k` is data-dependent.** It is derived from observed attempts, so a strong model reports k=1, a weak one k=5, and `--from-dir` rows are pinned k=1 — then all three are sorted into one "pass@k" column. `k` must be a config input, asserted identical across every compared row.

Use the numerically stable product form (the naive binomial overflows at literature scales):

```rust
/// Unbiased pass@k for one problem: `n` samples drawn, `c` correct.
/// Product form per openai/human-eval; the C(n-c,k)/C(n,k) form loses precision.
pub fn pass_at_k(n: usize, c: usize, k: usize) -> f64 {
    assert!(n >= k, "pass@k requires n >= k; got n={n}, k={k}");
    if n - c < k { return 1.0; }
    let mut prod = 1.0f64;
    for j in (n - c + 1)..=n { prod *= 1.0 - (k as f64) / (j as f64); }
    1.0 - prod
}
```

---

## C3 — Early-break corrupts both the estimator and every cost metric

Task 7 does `if outcome.tests_passed { break; }`. Consequences:

- **The estimator becomes uncomputable.** It needs (n, c) per problem; early stopping yields an n correlated with success — textbook optional stopping.
- **Every cost/latency metric is conditioned on the outcome.** An easy fixture costs 1 call, a hard one 5. A model that fails more therefore *spends more per fixture*, so `cost_per_success_usd` and `tokens_per_pass` are inflated superlinearly for weaker models and are **not comparable across models with different pass rates**. This silently rewards the strongest model twice.

**Fix:** run all n attempts unconditionally; record exactly n outcomes; derive c. If cost is the concern, lower n — never stop early.

---

## C4 — The significance test is statistically inert

`significant_deltas` declares a change significant iff two 95% CIs do not overlap. Non-overlapping CIs do imply significance; overlapping CIs do **not** imply its absence. Two 95% intervals that merely touch correspond to p ≈ 0.01 ([Cumming 2009](https://onlinelibrary.wiley.com/doi/10.1002/sim.3471)), so the gate fires at an effective α ≈ 0.005 — a false-negative rate several-fold above nominal. Relative efficiency versus the correct test is ~0.5 ([Schenker & Gentleman 2001](https://www.tandfonline.com/doi/abs/10.1198/000313001317097960)).

Worse, the design **discards the pairing**: every model attempts the same fixtures, so the data are paired binary outcomes. The correct test is **McNemar's**, exact when discordant pairs b+c < 25 (which, at 31 fixtures, is always).

Task 13's test `significant_deltas_ignores_movement_inside_the_confidence_interval` asserts this bug as intended behavior — the test must be rewritten, not just the code.

Also required: Holm–Bonferroni across the m(m−1)/2 pairwise claims published per run, and correction across the *sequence* of scheduled runs, which is otherwise an uncorrected family that will manufacture SCIENTIA findings over time.

Wilson intervals stay valid **only** for single-attempt pass@1. For pass@k (a mean of per-problem estimates) use a **cluster bootstrap over problems** — which the spec's §F already specified and the plan silently downgraded to Wilson.

---

## C5 — The corpus cannot resolve the differences the leaderboard exists to show [VERIFIED]

Exact McNemar power by enumeration, α=0.05 two-sided:

| True difference | Power at N=31 | Power at N=164 |
|---|---|---|
| 10 points | **0.09 – 0.11** | 0.79 – 0.98 |
| 15 points | 0.25 – 0.31 | 0.97 – 1.00 |
| 20 points | 0.42 – 0.56 | ~1.00 |
| 30 points | 0.73 – 0.89 | ~1.00 |

**At N=31, a real 10-point difference is detected 9% of the time.** Frontier coding models typically sit within 15 points of each other, so the leaderboard would report "tied" for genuinely different systems in the overwhelming majority of comparisons — while looking authoritative.

Problems required for 80% power:

| Difference | Low-noise | High-noise |
|---|---|---|
| 5 points | 338 | 658 |
| 10 points | 122 | 208 |
| 15 points | 72 | 112 |
| 20 points | 50 | 74 |

**Implication:** the 31-problem held-out set supports no comparative ranking. The full 164 supports ~10-point resolution. Publishing model-vs-model rankings requires either the full corpus (and accepting a 10-point floor, stated on the page) or expansion past ~200 problems.

---

## C6 — Pipe deadlock, and the failure column reads the wrong stream [VERIFIED]

`run_with_timeout` pipes stdout and stderr but drains neither until after the process exits. Measured: a cascading-type-error file produces **163,149 bytes** of diagnostics — versus Rust's ~8 KiB Windows anonymous-pipe buffer. The child blocks writing, `try_wait` never completes, the harness kills it at the timeout and records `"timed out"`.

This is the *likely* case, not an exotic one: cascading `Cannot unify Int with Option(Int)` errors are exactly what the most common Vox mistake produces. Cost: a false failure attributed to the model, plus 30 s burned per occurrence.

Compounding: `vox check` writes diagnostics to **stdout**, but `detail` is built from `check.stderr` — so the failure column is always the generic fallback and the real diagnostic is discarded.

**Fix (lazy and correct):** redirect both streams to files in the workdir rather than pipes — no deadlock, no reader threads — then read them after exit and build `detail` from stdout first, stderr second.

---

## CORRECTION (2026-09-01, post-publication) — the contamination window is NOT inert

An earlier revision of this audit stated that `--cutoff` "yields zero fixtures for essentially every model" because frontier models have post-corpus training cutoffs. **That claim was wrong.** It was reasoned from an assumption about frontier cutoffs rather than measured, and the live OpenRouter catalog refutes it.

Measured against the live `/api/v1/models` snapshot (419 models, 2026-09-01):

| Fact | Value |
|---|---|
| Models declaring a `knowledge_cutoff` | 183 / 419 |
| Range of declared cutoffs | 2021-09-30 → **2026-02-16** |
| Models with a cutoff **after** the corpus date (2026-05-27) | **0** |

Every model with a declared cutoff was trained *before* the corpus was published. So `eligible_after(fixtures, model.knowledge_cutoff)` admits **all 164 fixtures for every such model**, and the windowing mechanism works exactly as designed — in the opposite direction from what this audit originally claimed.

What remains true, and is the correctly-stated risk:

- **236 of 419 models (56%) declare no cutoff at all.** Their contamination status is unknown and must be treated as such — not silently assumed clean.
- **The corpus is public, so the window decays.** Models trained after 2026-05-27 will have seen it. The guard is sound *today* and expires without ongoing fixture authorship.
- **Use `knowledge_cutoff` as the window input**, not a hand-typed `--cutoff`. It is per-model, machine-readable, and removes the operator's ability to pick a flattering date.

C7 below (split inconsistency and the duplicate leak) is unaffected by this correction and stands as written.

---

## C7 — The held-out split is triple-inconsistent, and a held-out answer already leaks [VERIFIED]

Three sources disagree about which fixtures are held out:

| Source | Held-out count |
|---|---|
| `manifest.v1.yaml` (`training_eligible: false`) | **31** |
| Per-fixture `spec.toml` | **10** |
| The `.vox` solution files themselves | **0** (no file carries any marker) |

**21 fixtures** are held-out per manifest and training-eligible per their own spec (ids 065–075, 098–100, 128, 148, 160–164). The manifest's own header comment says "44 held-out", a fourth number.

**A held-out answer already leaks by duplication:** fixtures **072 (held-out)** and **141 (training-eligible)** have byte-identical reference bodies modulo the function name. Training on 141 hands over the answer to 072. Three further exact-duplicate pairs exist within the training-eligible set (014/063, 020/134, 026/153).

**On the file-level marker gap:** the corpus extractor decides eligibility from *file content* (`part_helpers.rs:259` checks whether the file contains `training_eligible: false`), and **zero of 328 `.vox` files carry it**. This is currently **latent, not active** — the extractor walks `examples/golden/`, `crates/**/tests/`, and integration fixtures, not `contracts/eval/`. But the manifest's designation is invisible to the extractor, so the only thing preventing ingestion of all 164 answer keys is a path that happens not to be on a list.

**Fixes:** pick one SSOT and add a CI gate asserting the three agree; delete or differentiate the duplicate pairs; add `// training_eligible: false` headers to held-out `.vox` files as defense in depth; derive `added_at` from `git log --diff-filter=A` rather than hand-assignment.

**Also fixed during this audit:** the research spec was marked `training_eligible: true` while naming five held-out fixtures and stating the exact idiom that solves one — a contamination leak inside the document warning about contamination. Now `false`.

---

## High-severity defects (H1–H8)

**H1 — `--from-dir` publishes fabricated zeros.** External-harness rows hardcode `tokens=0, latency=0`, and the leaderboard schema types these non-nullable, so the page renders "0 tokens/pass, 0 ms" for Claude Code — a false published claim. Make them `Option`, widen the schema to `["number","null"]`, render `—`, and add a per-row `measured: {tokens, latency, attempts}` provenance block.

**H2 — Integrity-guard false positives.** Raw substring matching over comments and strings: `"net."` matches "…over the inter**net.**"; `"tests.vox"` matches a comment. A hit scores a *correct* solution as failed **and brands it cheating** — the worst error class in the harness, and unfalsifiable from the leaderboard. Strip comments/strings, require call syntax, drop bare `"net."`.

**H3 — `strip_candidate_main` deletes real code.** It cuts from the first `\nfn main(` to EOF, so helpers written after a demo `main` vanish; and `starts_with("fn main(")` deletes the entire solution. Both are model-output shapes the "verified across 164 fixtures" argument never covered. Brace-match and excise only the `main` block.

**H4 — `seed` does not exist.** `LlmConfig` has no seed field and none reaches the OpenRouter request body. The plan's "temperature 0.0 + seed 42 = reproducible" is unimplementable as written — either plumb it (3 files) or delete the claim. Note that temperature 0 alone is not determinism on OpenRouter (MoE routing, provider fallback).

**H5 — Rate limits are scored as model failures.** Provider errors return as `Ok(Err(msg))`, so activity-level retry never fires, and the plan's error arm records a failed attempt. At thousands of calls this will materially and invisibly depress every published pass rate. Classify on the facade's existing `RATE_LIMITED_PREFIX` / `CONTEXT_EXCEEDED_PREFIX`, retry those, and count infrastructure errors in a separate `n_infra_errors` field so the denominator stays honest. Note `infer_with_retry` is *not* a retry loop despite the name — do not substitute it.

**H6 — No spend ceiling.** Add `--max-spend-usd` and a `run_complete: bool` on the artifact; a budget-truncated sweep must never publish as a full one. `OrchestratorBudgetGate` exists to reuse if the crate edge allows.

**H7 — `config_digest` is unstable and under-specified.** It uses `DefaultHasher`, which Rust does not guarantee stable across releases — a toolchain bump silently re-keys every historical row. It also omits the prompt template, temperature, top_p, max_tokens, model version string, and **the Vox compiler commit**, which directly determines pass/fail. Cross-time deltas will attribute compiler changes to model changes. Use sha256 over a canonical string including all of these.

**H8 — `max_tokens: 1024` truncates reasoning models [VERIFIED].** Reference solutions max at ~320 tokens, so 1024 is ample for the *answer* — but it caps reasoning+answer, and a thinking model (the catalog includes one) gets truncated mid-thought and scored as a model failure that is a harness artifact.

---

## Strategic findings (S1–S3)

**S1 — The corpus exercises 17% of the language and 0% of what distinguishes it.** Measured against the compiler's own `feature_matrix.rs` (133 features):

| Category | Exercised | Total | % |
|---|---|---|---|
| Decorators | **0** | 56 | 0% |
| Declarations | **1** (`fn`) | 41 | 2% |
| Statements | 5 | 8 | 63% |
| Expressions | 17 | 28 | 61% |
| **Total** | **23** | **133** | **17.3%** |

Zero occurrences of `actor`, `workflow`, `activity`, `component`, `state_machine`, `routes`, `table`, `query`, `mutation`, `server`, `tool`, `resource`, `@durable`, `@pure`, `@uses`, `@auth`, `Id[T]`, `Result`, or any user-defined type in a signature. **Every enforcement rule in AGENTS.md §"Vox Language Enforcement Rules" targets syntax this corpus never contains** — a model could score 100% while being unable to write a line any of those lints applies to.

The corpus measures generic imperative algorithms transliterated into a C-family surface. That is a legitimate *floor* test; it is not evidence about Vox syntax, and the leaderboard's title would be wrong. A `contracts/eval/vox-syntax/` tier keyed to the feature matrix — with **negative fixtures** (the un-annotated version must fail `vox check`) — is where the real measurement lives.

**S2 — No model has ever been scored on this corpus.** `crates/vox-audit/src/subcommands/humaneval.rs` states the LLM-panel harness is future work; `--llm-panel` resolution is a TODO. The runner only re-checks that authored references compile. Related: an audit records that five CR-L measurement harnesses were deleted by a bad merge on 2026-05-27 and replaced with unconditional `InfrastructureError` stubs, unnoticed for 96 days.

**S3 — The measurement question is mis-framed, and the fix is cheap.** Frontier models have essentially never seen Vox, so a zero-shot prompt measures *guessability from Rust/Python surface similarity* — a floor near zero that says nothing about the model or the language. The scientifically meaningful and literature-comparable measurement is **in-context acquisition of an unseen grammar** ([MTOB](https://arxiv.org/abs/2309.16575), [MultiPL-E](https://arxiv.org/abs/2208.08227)).

The material already exists: `vox-grammar-export`'s `emit_compact_llm_prompt()` is **~780 tokens** and is already the documented SSOT for the LLM grammar prompt — nothing in the benchmark path injects it. Report conditions as separate columns, never averaged:

| Condition | Context | Measures |
|---|---|---|
| C0 zero-shot | signature + task only | transfer from neighboring languages |
| C1 grammar | + ~780-token compact grammar | applying a formal spec |
| C2 few-shot | + 3 fixed worked examples | idiom transfer |
| C3 full docs | + stdlib reference | practical ceiling |

**The C0→C3 lift is the headline result** — it is falsifiable, it is what a language designer can act on, and it is the only framing under which "Vox is a good LLM target" is a testable claim rather than a slogan. Add `condition_id` and `context_hash` to the run config and schema; without them a later doc edit silently changes every score.

---

## Conflict of interest and what may be claimed

Vox authors the language, the corpus, the compiler-as-judge, the harness, and a contestant (MENS). No COI statement exists in either document. Structural exposures: problem selection is Vox's; reference solutions define "idiomatic"; assertion strength is unaudited (mean **5.9 assertions/fixture** [VERIFIED] versus HumanEval's 7.7, which [EvalPlus](https://arxiv.org/abs/2305.01210) showed inflated pass@1 by ~19% — so every published rate here is an **upper bound**).

**"MENS beats Claude at Vox" is a tautology**, not a finding: MENS is fine-tuned on Vox; the others get a zero-shot prompt with no language reference. The meaningful comparisons are MENS versus **its own base checkpoint** (isolates the intervention), and all models under **ICL-matched conditions** (C1–C3 above).

**Harness rows (Claude Code, Cursor, Warp) are not salvageable as designed** — hand-driven by a single operator who is the project author, no turn/time budget, no transcript, no version pin, while Vox's own lane runs automated. Publish model-only in one automated lane; quarantine harness rows to a clearly-labeled exploratory appendix with full transcripts, or drop them.

**Defensible today:** the C0→C3 context lift; failure-class taxonomies (descriptive, no ranking); MENS versus its base checkpoint; cost/latency at matched quality; "here is our corpus and runner — score your own system." **Not defensible from this project at any sample size:** any row ranking Claude/GPT/Gemini/Kimi/Qwen/Grok against each other, any harness ranking, or "Vox is a better LLM target than Python" (there is no cross-language arm, and the training-data-volume confound is uncontrollable).

---

## What the audit confirmed as sound

Not everything broke, and two suspected defects were **false positives** worth recording so they are not "fixed" into new bugs:

- **`fn assert` shadowing does not work** — only the `let`-bound form does (C1). Fixing the reported mechanism would have guarded the wrong construct.
- **`vox check` / `vox run` never disagree** — tested across all 164 fixtures: 164 both-pass, 0 disagreements. The theoretical parse-path divergence is not real today.
- Failing `assert` exits 1; passing exits 0; `vox check` exits 1 on type errors — the core symbolic signal is sound [VERIFIED].
- All 164 `tests.vox` have exactly one `fn main`, always last, with nothing after it — `extract_test_block`'s suffix cut is correct for the corpus as authored. 13 fixtures define helpers before `main`, but every `main` calls only the pinned signature function.
- No fixture reference contains any integrity marker, so the oracle control will not self-flag.
- Scheduled workflows are exempt from the concurrency guard; self-hosted runners need no exception row — the automation is legal to build as designed.

---

## Corrected priority order

1. **C1** — close the shadowing exploit (all three layers, differential control mandatory). Nothing else matters until scoring cannot be trivially defeated.
2. **C7** — reconcile the split, delete the duplicate leak, add the CI gate.
3. **C6** — files instead of pipes; read stdout for `detail`.
4. **C2 + C3** — unbiased estimator, config-driven k, no early break.
5. **C4** — McNemar + Holm; delete `intervals_overlap` from the significance path; bootstrap for pass@k intervals.
6. **C5** — publish the resolution floor on the page; use the full 164 or expand past 200 before any ranking.
7. **S3** — add condition C0–C3 with the existing 780-token grammar prompt. This converts the project from an indefensible ranking into a publishable measurement.
8. **H1–H8**, then **S1** (the `vox-syntax` tier), then automation.

Automation (workflow, `--json` discovery trigger, deploy path) is correctly scoped by the CI track and is **not** the bottleneck — publishing a broken number on a schedule is worse than publishing nothing. Build it last.
