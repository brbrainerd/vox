---
title: "Semantic Coverage — Honest Status (2026-06-15)"
description: "Verified accounting of the semantic test-coverage initiative: what is proven, what is merely reached-but-unproven, what was actually tested vs what remains exigent."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Semantic Coverage — Honest Status (2026-06-15)

## TL;DR

- **We proved 15.9% of production behavior, not "most of it."** Of 19,445 production symbols, only **3,088 (15.9%)** are assertion-backed. **5,793** are reached-but-unproven (executed by a test with **zero asserted behavior**) — the keystone gap.
- **The keystone gap is 5,793 (reproducible), not 3.** An earlier committed artifact reported reached-but-unproven as **"3"** (a near-empty partial run) and was then corrected to a directional **7,950**; the now-**reproducible** figure from a real 1.19M-line-execution run is **5,793**. That is the real size of the remaining work.
- **We tested the easy leaf functions, not the exigent structural gaps.** The ~2,409 merged semcov tests are broad and well-written, but they overwhelmingly target tokenizers, validators, span merges, serde round-trips, and formatter idempotence — exactly the "useless touch" surface the initiative was meant to avoid.
- **Of the 7 named structural pipeline-gap patterns, 6 have NO dedicated coverage and 1 (split-brain) has a single weak intra-crate case.** The named headline regression — `top-level let → Decl::Const → catch-all → value vanishes` — is **completely untested** (zero `Decl::Const` references in the entire suite).
- **Reach is now reproducible locally, but not yet a CI gate.** The 5,793 comes from a real full-workspace run (1.19M lines executed, 60 crates) after fixing the `peft-rs` exit-101 blocker. The remaining gap is wiring this into CI as an enforced ratchet.

## What is actually DONE (verified)

- **~2,409 semcov `#[test]` functions across ~214 files are merged to `main`** (via PR #299; the larger PR #303 was closed but its content had already landed). This is real, merged, running code — not a branch.
- **3,088 production symbols are genuinely proven** with specific behavioral assertions on real code paths. That is a real floor, not an aspiration.
- **Adversarial quality audit of 7 sampled modules found 65–93% "real signal"** — specific behavioral assertions on real paths, with **no purely vacuous tests** in the sample. Crypto AEAD tests check authentication (not just round-trips).
- **The misreported "3" artifact has been corrected to the reproducible 5,793** (via 7,950). The lie is out of the record.
- **Two new `vox-code-audit` detectors landed and DO target the structural patterns**: `catch_all_swallow` (wildcard match arm drops a value) and `cross_crate_dup` (split-brain duplicated logic). These are the right detectors for patterns #1 and #7 — though detectors are not tests, and the tests that would exercise the patterns still do not exist.

## What is PARTIAL / misleading

- **Reach is reproducible locally but not yet gate-enforced in CI.** The 5,793 is from a real full-workspace run (1.19M lines executed); the earlier 3-crate exclusion (`vox-compiler`/`vox-corpus`/`vox-config`) is resolved — all now compile and execute their own tests after the `peft-rs` fix. What remains is porting the profraw-merge/chunked-export path into CI (Linux, no arg limits) and failing the build when reached-but-unproven rises.
- **The quality audit, while encouraging, named recurring weak patterns** that inflate apparent coverage:
  - `assert_ne!` on **derived enum discriminants** (proves the derive macro works, not your logic).
  - **Re-asserting self-constructed literals** (you built the value two lines up; asserting it equals itself proves nothing).
  - **Overstated `// Catches:` comments on no-panic tests** — a test that only proves "doesn't panic" claiming to catch a behavioral regression.
  - **Crypto hash correctness is under-pinned**: only SHA3 has a known-answer vector. The other hashes are round-trip-only, which does not pin correctness against an external reference.
- **"Proven > Reached" anomalies exist** (e.g. `vox-cli`: 650 proven vs 219 reached). Proven is a static `proves`-edge count (lcov-independent) while reached is matched per-file/line, so the two are measured differently and a crate whose tests live elsewhere can show proven > reached. Read the two columns as distinct signals, not a subset relationship.

## What is NOT started (the exigent core)

The whole point of the initiative was to close **structural pipeline-gap patterns** — places where a construct is silently dropped between stages. As of 2026-06-15, **3 of 7 are now covered** (#1, #5, #7) by dedicated tests in `crates/vox-compiler/src/semcov_struct_pipeline_tests.rs` and `crates/vox-cli/tests/effort_pricing_parity.rs`; 4 remain.

| # | Pattern | Coverage | Evidence |
|---|---------|----------|----------|
| 1 | Silent-drop catch-all match arms | ✅ **DONE** | `semcov_struct_pipeline_tests`: top-level `let`→`Decl::Const` must lower into `hir.consts`, not the `legacy_ast_nodes` catch-all; name, value, type-annotation and multi-binding survival all pinned. |
| 2 | Decorator cliff (keyword recognized, arg ignored) | **NONE** (deferred) | Investigation found this is doc-vs-parser **drift**, not a silent HIR drop: `@deprecated("reason")` is documented but does not parse (`Expected fn, found (`). Better fixed as a doc/parser correction than a coverage test. |
| 3 | Context-dependent silent drop | **NONE** (in progress) | No test asserts "same node survives in context A, dropped in context B." Design in flight. |
| 4 | Dead emitters (value produced, never consumed) | **NONE** (in progress) | `hir.lower_warnings` is a prime suspect (is anything consuming the "silently dropped" warnings?). Design in flight. |
| 5 | Half-wired `when {}` blocks | ✅ **DONE** | `semcov_struct_pipeline_tests`: a `when src { fetching… empty… error e… ok x… }` lowers to `HirExpr::AsyncView` with all four arms surviving (`missing_arms()` empty) and bindings intact. |
| 6 | Structural-only goldens (runtime output unasserted) | **NONE** (biggest remaining) | Still no `stdout`/behavioral-output assertions; needs a harness that compiles a `.vox` program and asserts observable output. |
| 7 | Split-brain (duplicated logic diverged across crates) | ✅ **DONE** | `effort_pricing_parity.rs`: asserts the byte-identical `ModelRates::cost_usd` copies in `vox-effort-audit` and `vox-effort-route` agree across a direction-sensitive matrix; fails on any divergence. |

- **Headline bug — `top-level let → Decl::Const → catch-all → value vanishes`: now regression-guarded.** The fix already existed in `hir/lower/mod.rs` (Const is lowered into `hir.consts`); the test pins it so it cannot silently regress.
- **The 5,793 reached-but-unproven set is the largely-untouched core.** Every one of these symbols is executed by a test that asserts nothing about its behavior. Converting them to proven is the actual remaining initiative; patterns #1/#5/#7 have made a first dent in the structural slice.

**Honest 2-sentence verdict:** The semcov suite is broad and well-written but almost entirely targets **leaf utility functions** (tokenizers, identifier validators, span merges, redaction, cost/quota math, serde round-trips, formatter idempotence) — exactly the "useless touch" surface, not the structural wiring gaps. Of the 7 structural patterns, **6 have NO dedicated coverage and 1 (split-brain) has only a single weak intra-crate case**, and the named headline regression (`top-level let → Decl::Const → catch-all`) is **completely untested**.

## True remaining scope (by the numbers)

| Metric | Count | % of symbols |
|---|---:|---:|
| Production symbols (defs) | 19,445 | 100% |
| Proven (assertion-backed) | 3,088 | 15.9% |
| Reached-but-unproven (zero asserted behavior) | **5,793** | 29.8% |
| Crates with reached > 0 | 60 | — |

> **Reproducible figure (2026-06-15).** Supersedes the earlier directional 7,950.
> Produced from a real full-workspace `llvm-cov` run (**1.19M lines executed**),
> reproduced after fixing the `peft-rs` candle-0.10 compile error (the exit-101
> blocker) and merging profraws via `llvm-profdata -f file-list` (Windows
> arg-limit dodge). CI-gating still TODO.

**6 highest-leverage target crates** (largest reached-but-unproven, where proving symbols buys the most):

| Crate | Reached | Proven | Reached-not-proven |
|---|---:|---:|---:|
| vox-compiler | 947 | 392 | **699** |
| vox-orchestrator | 783 | 641 | **666** |
| vox-code-audit | 758 | 343 | **571** |
| vox-publisher | 487 | 112 | **419** |
| vox-codegen | 426 | 80 | **358** |
| vox-populi | 383 | 156 | **330** |

These 6 crates account for **~3,043** of the 5,793 reached-but-unproven symbols (~53%). `vox-codegen` (358 unproven, only 80 proven) is the worst proven-ratio of the group and is where patterns #2/#5/#6 live — the best place to attack structure and volume at once.

## Prioritized next actions

**Progress since first draft (2026-06-15):**
- ✅ **Reach reproducibility unblocked.** Fixed the `peft-rs` candle-0.10 exit-101 compile error and established a repeatable local profile path (profraw batch-merge). Reach is now a real 5,793, not a directional estimate. *Remaining:* wire it into CI as a gate.
- ✅ **Structural patterns #1, #5, #7 now have real tests** (committed): headline `let`→`Decl::Const` catch-all survival, `when{}` four-arm survival, and `ModelRates::cost_usd` cross-crate split-brain parity.

Remaining, in priority order:
1. **Land the reach run as a CI gate** — *lever: turns the reproducible local number into an enforced ratchet.* Port the profraw-merge/chunked-export path (now proven locally) into CI on Linux (no arg limits) and fail when reached-but-unproven rises.
2. **Finish the structural-pattern tests #3, #4, #6** — *lever: closes the actual initiative.* #3 context-dependent drop, #4 dead-emitter (`lower_warnings` consumption), #6 the stdout-golden harness. (#1/#5/#7 done.)
3. **Stand up the behavioral-output (stdout) golden harness** (pattern #6) — *lever: the missing capability behind "structural-only goldens"; compile a `.vox` program and assert observable output.*
4. **Attack `vox-codegen` (358 unproven, 80 proven)** — *lever: worst proven-ratio + home of patterns #2/#5/#6.*
5. **Then grind `vox-compiler` (699) + `vox-orchestrator` (666) + `vox-code-audit` (571)** — *lever: largest raw reduction of the reached-but-unproven set.*
6. **Audit and downgrade the weak-test tail** — *lever: stops apparent coverage from inflating.* Replace `assert_ne!`-on-derived-discriminant and self-literal tests, add known-answer vectors for every hash (not just SHA3), correct overstated `// Catches:` comments.
7. **Wire `catch_all_swallow` / `cross_crate_dup` into the gate with fixtures** — *lever: converts the structural patterns from "detectable" to "regression-guarded."*