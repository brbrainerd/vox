---
title: "Semantic Coverage — Honest Status (2026-06-15)"
description: "Verified accounting of the semantic test-coverage initiative: what is proven, what is merely reached-but-unproven, what was actually tested vs what remains exigent."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Semantic Coverage — Honest Status (2026-06-15)

## TL;DR

- **We proved 15.9% of production behavior, not "most of it."** Of 19,445 production symbols, only **3,088 (15.9%)** are assertion-backed. **7,950** are reached-but-unproven (executed by a test with **zero asserted behavior**), and **~8,407** are neither reached nor proven (effectively dead/untested).
- **The keystone gap is 7,950, not 3.** A prior committed artifact reported reached-but-unproven as **"3"** — that number came from a near-empty partial llvm-cov run and was flatly wrong. The true figure is **7,950**, and it is the real size of the work.
- **We tested the easy leaf functions, not the exigent structural gaps.** The ~2,409 merged semcov tests are broad and well-written, but they overwhelmingly target tokenizers, validators, span merges, serde round-trips, and formatter idempotence — exactly the "useless touch" surface the initiative was meant to avoid.
- **Of the 7 named structural pipeline-gap patterns, 6 have NO dedicated coverage and 1 (split-brain) has a single weak intra-crate case.** The named headline regression — `top-level let → Decl::Const → catch-all → value vanishes` — is **completely untested** (zero `Decl::Const` references in the entire suite).
- **The reach data is not yet reproducible in CI.** Every CI run was cancelled; the numbers come from one local llvm-cov run that needed a Windows arg-limit chunking workaround and excluded 3 crates from executing their own tests. Treat the reach column as directionally true, not audit-grade.

## What is actually DONE (verified)

- **~2,409 semcov `#[test]` functions across ~214 files are merged to `main`** (via PR #299; the larger PR #303 was closed but its content had already landed). This is real, merged, running code — not a branch.
- **3,088 production symbols are genuinely proven** with specific behavioral assertions on real code paths. That is a real floor, not an aspiration.
- **Adversarial quality audit of 7 sampled modules found 65–93% "real signal"** — specific behavioral assertions on real paths, with **no purely vacuous tests** in the sample. Crypto AEAD tests check authentication (not just round-trips).
- **The misreported "3" artifact has been corrected to the true 7,950.** The lie is out of the record.
- **Two new `vox-code-audit` detectors landed and DO target the structural patterns**: `catch_all_swallow` (wildcard match arm drops a value) and `cross_crate_dup` (split-brain duplicated logic). These are the right detectors for patterns #1 and #7 — though detectors are not tests, and the tests that would exercise the patterns still do not exist.

## What is PARTIAL / misleading

- **Reach numbers are not CI-reproducible.** Every CI attempt was cancelled. The 7,950 figure is from a single local llvm-cov run requiring a Windows command-line arg-limit chunking workaround. **3 crates — `vox-compiler`, `vox-corpus`, `vox-config` — were excluded from executing their own tests** (their transitive reach was still counted). So their own reached/proven splits are softer than the table implies, and nothing here is yet gate-enforced.
- **The quality audit, while encouraging, named recurring weak patterns** that inflate apparent coverage:
  - `assert_ne!` on **derived enum discriminants** (proves the derive macro works, not your logic).
  - **Re-asserting self-constructed literals** (you built the value two lines up; asserting it equals itself proves nothing).
  - **Overstated `// Catches:` comments on no-panic tests** — a test that only proves "doesn't panic" claiming to catch a behavioral regression.
  - **Crypto hash correctness is under-pinned**: only SHA3 has a known-answer vector. The other hashes are round-trip-only, which does not pin correctness against an external reference.
- **"Proven > Reached" anomalies exist** (e.g. `vox-cli`: 641 proven vs 563 reached). This is an artifact of the reach run's exclusions and counting boundaries, not evidence that proof exceeds execution — another reason the reach column is directional, not authoritative.

## What is NOT started (the exigent core)

The whole point of the initiative was to close **structural pipeline-gap patterns** — places where a construct is silently dropped between stages. On that, we have essentially nothing.

| # | Pattern | Coverage | Evidence |
|---|---------|----------|----------|
| 1 | Silent-drop catch-all match arms | **NONE** | Only "catch-all" mention is `vox-cli-core/semcov_wave22:255` asserting `fallback_source_group` returns the `"core"` default — a leaf string-mapping default, not a test that a wildcard arm *swallowed a value that should have been routed*. No test feeds a construct through a real pipeline match and asserts it doesn't vanish. |
| 2 | Decorator cliff (keyword recognized, arg ignored) | **NONE** | No semcov test exercises decorator argument wiring. The only `@`-decorator test (`vox-compiler/semcov_wave17:109`) asserts the *tombstoned* `@component fn` form is **rejected** — guards a removal, not arg propagation. No `@page`/`@route`/`@island` arg-passthrough assertions exist. |
| 3 | Context-dependent silent drop | **NONE** | Closest is wave29 ("body must not run when a seed exists", "extract selecting FIRST WorkflowCompleted") — workflow-replay leaf semantics, not "same node accepted in context A, silently dropped in context B." |
| 4 | Dead emitters (value produced, never consumed) | **NONE** | Zero `emit.*never` / `produced.*consumed` hits. wave29's journal-bookkeeping tests check duplicate entries, not emitter→consumer wiring across a stage boundary. |
| 5 | Half-wired `when {}` blocks | **NONE** | Every `when` hit is incidental English ("when a seed exists", "alert must fire when spend exceeds 80%"). No test parses/lowers a Vox `when {}` block and asserts all branches reach codegen. |
| 6 | Structural-only goldens (runtime output unasserted) | **NONE** (arguably the suite *is* an instance of the problem) | No `stdout` / behavioral-output / `runtime.*assert` anywhere. semcov tests assert in-process return values of leaf functions; they never execute a compiled program and assert its observable output. |
| 7 | Split-brain (duplicated logic diverged across crates) | **PARTIAL (weak)** | One adjacent test: `vox-cli-core/semcov_wave22` "Catches: `ci_nested_target` and `gate_isolated_target` diverging from canonical_workspace_target" — a real same-crate divergence guard. But it's a single intra-crate path-helper case; there is **no cross-crate parity test**. |

- **Headline bug — `top-level let → Decl::Const → catch-all → value vanishes`: NO test targets this.** Zero `Decl::Const` references, zero top-level-`let` lowering tests, and the catch-all reference is unrelated. The compiler semcov file (wave17) covers parser empty-input panics, effect-capability mapping, `strip_tests`, and formatter idempotence — never the const-lowering catch-all.
- **The 7,950 reached-but-unproven set is the untouched core.** Every one of these symbols is executed by a test that asserts nothing about its behavior. Converting them to proven is the actual remaining initiative.

**Honest 2-sentence verdict:** The semcov suite is broad and well-written but almost entirely targets **leaf utility functions** (tokenizers, identifier validators, span merges, redaction, cost/quota math, serde round-trips, formatter idempotence) — exactly the "useless touch" surface, not the structural wiring gaps. Of the 7 structural patterns, **6 have NO dedicated coverage and 1 (split-brain) has only a single weak intra-crate case**, and the named headline regression (`top-level let → Decl::Const → catch-all`) is **completely untested**.

## True remaining scope (by the numbers)

| Metric | Count | % of symbols |
|---|---:|---:|
| Production symbols (defs) | 19,445 | 100% |
| Proven (assertion-backed) | 3,088 | 15.9% |
| Reached-but-unproven (zero asserted behavior) | 7,950 | 40.9% |
| Neither reached nor proven (~dead/untested) | ~8,407 | ~43.2% |
| Crates with reached-not-proven > 0 | 60 | — |

**6 highest-leverage target crates** (largest reached-but-unproven, where proving symbols buys the most):

| Crate | Reached | Proven | Reached-not-proven |
|---|---:|---:|---:|
| vox-orchestrator | 1530 | 616 | **1278** |
| vox-compiler | 967 | 364 | **728** |
| vox-codegen | 639 | 97 | **554** |
| vox-db | 652 | 227 | **548** |
| vox-code-audit | 702 | 309 | **545** |
| vox-orchestrator-mcp | 522 | 157 | **446** |

These 6 crates alone account for **4,099** of the 7,950 reached-but-unproven symbols (~51.6%). `vox-codegen` (554 unproven, only 97 proven) is the worst proven-ratio of the group and is also where patterns #2, #5, and #6 actually live — making it the single best place to attack structure and volume at once.

## Prioritized next actions

1. **Make the reach run CI-reproducible before trusting any number** — *lever: turns the whole table from "one local run" into an enforceable gate.* Land the Windows arg-limit chunking workaround in CI, stop excluding `vox-compiler`/`vox-corpus`/`vox-config` from executing their own tests, and fix whatever cancels every run. Until this lands, every figure here is directional.
2. **Write the 7 structural-pattern tests first, starting with the headline `Decl::Const` catch-all** — *lever: closes the actual initiative, not the proxy.* These are ~7–10 high-value tests that each feed a real construct through a real pipeline stage and assert it survives lowering/codegen. This is the work that was supposed to be done and wasn't.
3. **Add behavioral-output (stdout) golden assertions for compiled programs** — *lever: kills pattern #6, which the current suite arguably *is* an instance of.* Stand up a harness that compiles a `.vox` program and asserts observable output; this is the missing capability behind "structural-only goldens."
4. **Attack `vox-codegen` next (554 unproven, 97 proven)** — *lever: worst proven-ratio + home of patterns #2/#5/#6, so it buys structure and volume simultaneously.*
5. **Then grind the orchestrator stack (`vox-orchestrator` 1278 + `vox-orchestrator-mcp` 446 = 1724 unproven)** — *lever: largest raw reduction of the reached-but-unproven set in a single coherent area.*
6. **Audit and downgrade the weak-test tail** — *lever: stops apparent coverage from inflating.* Replace `assert_ne!`-on-derived-discriminant and self-literal-equality tests with real assertions, add known-answer vectors for every hash (not just SHA3), and correct overstated `// Catches:` comments on no-panic tests so the labels match what is actually proven.
7. **Wire the two new detectors (`catch_all_swallow`, `cross_crate_dup`) into the gate and pair each with a failing/passing test fixture** — *lever: converts the structural patterns from "detectable" to "regression-guarded," and gives patterns #1 and #7 their first real coverage.*